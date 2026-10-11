use lygos::{code_gen::CodeGenerator, modules};
use lygos_ast::type_check::TypeChecker;
use lygos_llvm::{builder::IRBuilder, context::Context, jit::Jit, module::Module};
use std::{
    cell::RefCell,
    collections::HashMap,
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "lygos-modules-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&dir).unwrap();
        Self(dir)
    }
    fn source(&self, name: &str, source: &str) -> PathBuf {
        let path = self.0.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, source).unwrap();
        path
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let path = self.0.canonicalize().unwrap();
        assert!(path.starts_with(std::env::temp_dir().canonicalize().unwrap()));
        fs::remove_dir_all(path).unwrap();
    }
}
thread_local! {
    static BUFFERS:RefCell<HashMap<usize,Vec<u8>>>=RefCell::new(HashMap::new());
    static OUTPUT:RefCell<Vec<u8>>=const{RefCell::new(Vec::new())};
}
extern "C" fn allocate(size: u64) -> *mut i8 {
    let mut data = vec![0u8; size.max(1) as usize];
    let ptr = data.as_mut_ptr();
    BUFFERS.with(|buffers| buffers.borrow_mut().insert(ptr as usize, data));
    ptr.cast()
}
extern "C" fn reallocate(ptr: *mut i8, size: u64) -> *mut i8 {
    BUFFERS.with(|buffers| {
        let mut buffers = buffers.borrow_mut();
        let mut data = buffers.remove(&(ptr as usize)).unwrap();
        data.resize(size.max(1) as usize, 0);
        let ptr = data.as_mut_ptr();
        buffers.insert(ptr as usize, data);
        ptr.cast()
    })
}
extern "C" fn release(ptr: *mut i8) {
    BUFFERS.with(|buffers| buffers.borrow_mut().remove(&(ptr as usize)));
}
extern "C" fn stream(index: u32) -> *mut u8 {
    (index as usize + 1) as *mut u8
}
extern "C" fn fwrite(data: *const i8, size: u64, count: u64, _stream: *mut u8) -> u64 {
    let bytes = unsafe { std::slice::from_raw_parts(data.cast::<u8>(), (size * count) as usize) };
    OUTPUT.with(|output| output.borrow_mut().extend_from_slice(bytes));
    count
}
extern "C" fn write(_fd: u32, data: *const i8, length: u64) -> i64 {
    fwrite(data, 1, length, std::ptr::null_mut()) as i64
}
fn execute(path: &std::path::Path) -> i32 {
    let ast = modules::load(path).unwrap();
    let typed = TypeChecker::analyze(&ast).unwrap();
    drop(ast);
    Context::init_all();
    let context = Context::new();
    let mut module = Module::new("module-regression", &context);
    let mut jit = Jit::new();
    jit.configure_module(&mut module);
    let builder = IRBuilder::new(&context);
    CodeGenerator::new(&context, &module, &builder)
        .generate(&typed)
        .unwrap();
    unsafe {
        for (name, address) in [
            ("malloc", allocate as *const () as *mut ()),
            ("realloc", reallocate as *const () as *mut ()),
            ("free", release as *const () as *mut ()),
            ("__acrt_iob_func", stream as *const () as *mut ()),
            ("fwrite", fwrite as *const () as *mut ()),
            ("write", write as *const () as *mut ()),
        ] {
            jit.define_symbol(name, address).unwrap();
        }
    }
    jit.add_module(module).unwrap();
    let run: unsafe extern "C" fn() -> i32 =
        unsafe { std::mem::transmute(jit.lookup("run").unwrap()) };
    unsafe { run() }
}

#[test]
fn namespaces_types_methods_aliases_and_shared_dependencies_execute() {
    let files = Files::new();
    files.source("common.ly","struct Number { value:i32; } impl Number { fn new(value:i32)->Self { return Self${value}; } fn read(&self)->i32 { return self.value; } } fn number()->i32 { return 3; }");
    files.source(
        "left.ly",
        "import common; fn value()->i32 { return common::number(); }",
    );
    files.source(
        "right/mod.ly",
        "import \"../common.ly\" as shared; fn value()->i32 { return shared::number()+1; }",
    );
    let entry=files.source("main.ly",r#"import left; import right; import "common.ly" as numbers; fn value()->i32 { return 10; } fn run()->i32 { let a:numbers::Number=numbers::Number${2}; let b=numbers::Number::new(5); let f=left::value; return f()+right::value()+a.read()+b.read()+value(); }"#);
    assert_eq!(execute(&entry), 24);
}
#[test]
fn existing_print_macros_format_and_release_buffers() {
    OUTPUT.with(|out| out.borrow_mut().clear());
    let files = Files::new();
    let entry = files.source(
        "main.ly",
        r#"
        import print;
        fn run()->i32 {
            print::println$();
            print::print$("plain\0text");
            print::println$(" {} {} {}",42,-7,(:bool)1);
            let high:u64=(:u64)0-(:u64)1;
            let low:i64=(:i64)(high/(:u64)2)+(:i64)1;
            print::println$("{} {}",high,low);
            let args=print::format_args$("hello {}", "world");
            print::print_fmt_(args);
            return 0;
        }
    "#,
    );
    assert_eq!(execute(&entry), 0);
    OUTPUT.with(|output| {
        assert_eq!(
            output.borrow().as_slice(),
            b"\nplain\0text 42 -7 true\n18446744073709551615 -9223372036854775808\nhello world"
        )
    });
    BUFFERS.with(|buffers| {
        assert!(
            buffers.borrow().is_empty(),
            "formatted buffers must be freed"
        )
    });
}
#[test]
fn module_errors_include_import_locations() {
    for (source, expected) in [
        ("import absent; fn run(){}", "cannot find module"),
        (
            "import helper; import helper; fn run(){}",
            "duplicate module alias",
        ),
        (
            "import helper; fn helper(){}",
            "conflicts with module alias",
        ),
        (
            "import helper; fn run(){helper::absent();}",
            "has no declaration",
        ),
        (
            "import helper; fn run(){let module=helper;}",
            "namespace cannot be used",
        ),
    ] {
        let files = Files::new();
        files.source("helper.ly", "fn value()->i32{return 1;}");
        let entry = files.source("main.ly", source);
        let error = modules::load(&entry).unwrap_err();
        assert!(error.message.contains(expected), "{}", error.message);
        assert_eq!(
            error.loc.file.canonicalize().unwrap(),
            entry.canonicalize().unwrap()
        );
    }
    let files = Files::new();
    let entry = files.source("a.ly", "import b;");
    files.source("b.ly", "import a;");
    let error = modules::load(entry).unwrap_err();
    assert!(error.message.contains("import cycle"));
    assert!(!error.related.is_empty());
}
#[test]
fn local_print_module_overrides_the_bundle_and_macro_arguments_use_caller_scope() {
    let files = Files::new();
    files.source(
        "print.ly",
        r#"fn helper(value:i32)->i32{return value+1;} macro wrap {(value:$)->{ helper($value); }}"#,
    );
    let entry = files.source(
        "main.ly",
        r#"import print; fn run()->i32 { let helper=4; return print::wrap$(helper); }"#,
    );
    assert_eq!(execute(&entry), 5);
}

#[test]
fn imported_token_macros_can_expand_helpers_and_bound_recursion() {
    let files = Files::new();
    files.source(
        "helper.ly",
        "macro inner {(n:$)->{ $n + 1; }} macro outer {(n:$)->{ inner$($n); }}",
    );
    let entry = files.source(
        "main.ly",
        "import helper; fn run()->i32 { return helper::outer$(4); }",
    );
    assert_eq!(execute(&entry), 5);
    files.source("helper.ly", "macro recurse {()->{ recurse$(); }}");
    let entry = files.source("main.ly", "import helper; fn run(){ helper::recurse$(); }");
    let error = modules::load(entry).unwrap_err();
    assert!(
        error.message.contains("expansion limit"),
        "{}",
        error.message
    );
}
