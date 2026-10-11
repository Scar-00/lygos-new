use lygos::code_gen::CodeGenerator;
use lygos_ast::type_check::TypeChecker;
use lygos_lexer::Lexer;
use lygos_llvm::{builder::IRBuilder, context::Context, jit::Jit, module::Module};
use lygos_parser::Parser;
use std::sync::Once;

fn execute(source: &str) -> i32 {
    static LLVM: Once = Once::new();
    LLVM.call_once(Context::init_all);
    let ast = Parser::from(Lexer::from(source.chars().collect()).get_tokens()).build_ast();
    let typed = TypeChecker::analyze(&ast).unwrap_or_else(|errors| panic!("{errors:?}\n{source}"));
    drop(ast); // The semantic program owns everything required by the backend.
    let context = Context::new();
    let triple = Context::get_default_target_triple();
    let machine = Context::create_target_machine(
        Context::lookup_target(&triple),
        triple.as_str(),
        "generic",
        "",
    );
    let mut module = Module::new("regression", &context);
    module.set_target_triple(&triple);
    module.set_data_layout(&machine);
    let builder = IRBuilder::new(&context);
    CodeGenerator::new(&context, &module, &builder)
        .generate(&typed)
        .unwrap();
    let mut jit = Jit::new();
    jit.add_module(module).unwrap();
    let pointer = jit.lookup("run").expect("generated run function");
    let function: unsafe extern "C" fn() -> i32 = unsafe { std::mem::transmute(pointer) };
    unsafe { function() }
}

#[test]
fn arithmetic_casts_and_assignment_values() {
    for (body, expected) in [
        (
            "let mut a = 0; let mut b = 0; a = b = 2 + 3 * 4; return a + b;",
            28,
        ),
        ("let mut x = 5; let p = &mut x; *p = *p + 3; return *p;", 8),
        ("return -2147483648;", i32::MIN),
        ("let x: u8 = 255; return (:i32)x;", 255),
        ("let x: u32 = -1; return (:i32)(x / (:u32)2);", i32::MAX),
        ("let x: u32 = -1; return (:i32)(x > (:u32)0);", 1),
        ("let x: f64 = 1.5; return (:i32)(x + (:f64)2.5);", 4),
        ("return (:i32)(-1.5 < 0.0);", 1),
        ("return (:i32)(1.0 != 2.0);", 1),
        ("let x: f64 = 3; return (:i32)(:f32)x;", 3),
        ("let x: u32 = -1; return (:i32)((:f64)x > (:f64)0.0);", 1),
        ("return (:i32)'\\n';", 10),
    ] {
        assert_eq!(
            execute(&format!("fn run() -> i32 {{ {body} }}")),
            expected,
            "{body}"
        );
    }
}

#[test]
fn loop_locals_use_bounded_stack_storage() {
    assert_eq!(
        execute(
            r#"
        fn run()->i32 {
            let mut total=0;
            for let mut i=0 in i<30000 {
                let scratch=[i;128];
                total=total+scratch[0];
                i=i+1;
            }
            return total;
        }
    "#
        ),
        449985000
    );
}

#[test]
fn control_flow_and_short_circuit_side_effects() {
    for (source, expected) in [
        ("fn run() -> i32 { if (:bool)1 { return 3; } return 4; }", 3),
        (
            "fn run() -> i32 { if (:bool)0 { return 3; } else { if (:bool)1 { return 5; } else { return 6; } } }",
            5,
        ),
        (
            "fn run() -> i32 { let mut n = 0; for let mut i = 0 in i < 5 { if i == 3 { break; } n = n + 1; i = i + 1; } return n; }",
            3,
        ),
        (
            "fn run() -> i32 { for let mut i = 0 in i < 5 { return 7; } return 8; }",
            7,
        ),
        (
            "fn touch(x: &mut i32) -> bool { *x = *x + 1; return (:bool)1; } fn run() -> i32 { let mut x = 0; (:bool)0 && touch(&mut x); (:bool)1 || touch(&mut x); return x; }",
            0,
        ),
        (
            "fn touch(x: &mut i32) -> bool { *x = *x + 1; return (:bool)1; } fn run() -> i32 { let mut x = 0; (:bool)1 && touch(&mut x); (:bool)0 || touch(&mut x); return x; }",
            2,
        ),
        (
            "fn run() -> i32 { let x = 8; if (:bool)1 { let x = 3; x; } return x; }",
            8,
        ),
        (
            "fn run() -> i32 { if (:bool)0 { return 1; } else if (:bool)1 { return 2; } else { return 3; } }",
            2,
        ),
        (
            "fn run() -> i32 { let mut x: i32; if (:bool)1 { x = 2; } else { x = 3; } return x; }",
            2,
        ),
    ] {
        assert_eq!(execute(source), expected, "{source}");
    }
}

#[test]
fn direct_indirect_and_nested_calls() {
    for (source, expected) in [
        (
            "fn run() -> i32 { return add(2, 3); } fn add(a: i32, b: i32) -> i32 { return a + b; }",
            5,
        ),
        (
            "fn add(a: i32, b: i32) -> i32 { return a + b; } fn apply(f: fn(i32, i32) -> i32) -> i32 { return f(2, 3); } fn run() -> i32 { return apply(add); }",
            5,
        ),
        (
            "fn id(x: i32) -> i32 { return x; } fn get() -> fn(i32) -> i32 { return id; } fn run() -> i32 { return get()(9); }",
            9,
        ),
        (
            "type Size = i32; fn id(x: Size) -> Size { return x; } fn run() -> i32 { let x: Size = 12; return id(x); }",
            12,
        ),
        (
            "fn run() -> i32 { return result; } static result: i32 = 14;",
            14,
        ),
        (
            "struct S { callback: fn(i32) -> i32; } fn id(x: i32) -> i32 { return x; } fn run() -> i32 { let s = S${ id }; return (s.callback)(11); }",
            11,
        ),
    ] {
        assert_eq!(execute(source), expected, "{source}");
    }
}

#[test]
fn structs_arrays_and_computed_receivers() {
    for (source, expected) in [
        (
            "struct S { x: i32; } impl S { fn new() -> Self { return Self${ 4 }; } fn read(&self) -> i32 { return self.x; } fn change(&mut self, x: i32) { self.x = x; } } fn run() -> i32 { let mut s = S::new(); s.change(7); return s.read(); }",
            7,
        ),
        (
            "struct S { x: i32; } impl S { fn new() -> Self { return Self${ 4 }; } fn read(self) -> i32 { return self.x; } } fn run() -> i32 { return S::new().read(); }",
            4,
        ),
        (
            "struct S { x: i32; } struct Outer { inner: S; } impl S { fn read(&self) -> i32 { return self.x; } } fn run() -> i32 { let o = Outer${ S${ 6 } }; return o.inner.read(); }",
            6,
        ),
        (
            "struct S { x: i32; } fn run() -> i32 { let mut a = [S${ 2 }, S${ 3 }]; a[1].x = 10; return a[1].x; }",
            10,
        ),
        (
            "struct S { x: i32; } fn run() -> i32 { let mut s = S${ 4 }; let p = (:*mut S)&mut s; p->x = 5; return p->x; }",
            5,
        ),
        (
            "struct S { x: i32; } impl S { fn new() -> Self { return Self${ 9 }; } } fn run() -> i32 { return S::new().x; }",
            9,
        ),
        (
            "fn run() -> i32 { let mut a = [1, 2]; let p = &mut a[0]; p[1] = 7; return a[1]; }",
            7,
        ),
        (
            "struct S { x: i32; } type Alias = S; impl Alias { fn new() -> Self { return Self${ 13 }; } } fn run() -> i32 { return Alias::new().x; }",
            13,
        ),
        (
            "struct Buffer { data: *mut i8; len: u32; } impl Buffer { fn push(&mut self, char: i8) { let len = self.len; self.data[len] = char; } } fn run() -> i32 { let mut bytes = ['a', 'b']; let mut buffer = Buffer${ (:*mut i8)&mut bytes[0], (:u32)1 }; buffer.push('c'); return (:i32)bytes[1]; }",
            99,
        ),
        (
            "enum E { A, B } type Alias = E; fn run() -> i32 { return (:i32)Alias::B; }",
            1,
        ),
        (
            "impl Alias { fn new() -> Self { return Self${ 15 }; } } type Alias = S; struct S { x: i32; } fn run() -> i32 { return Alias::new().x; }",
            15,
        ),
        (
            "struct Outer { inner: Inner; } struct Inner { x: i32; } fn run() -> i32 { let outer = Outer${ Inner${ 18 } }; return outer.inner.x; }",
            18,
        ),
        (
            "struct S { x: i32; } impl S { fn read(self) -> i32 { return self.x; } } fn S_read() -> i32 { return 2; } fn run() -> i32 { let s = S${ 5 }; let p = (:*const S)&s; return p->read() + S_read(); }",
            7,
        ),
        (
            "struct Raw { data: *const i8; len: u64; } static text: &str = \"a\\0b\"; fn run() -> i32 { let p = (:*const Raw)text; return (:i32)p->data[2] + (:i32)p->len; }",
            101,
        ),
    ] {
        assert_eq!(execute(source), expected, "{source}");
    }
}

#[test]
fn typed_intrinsics_validate_and_execute_the_runtime_contract() {
    let runtime = r#"
        struct Formatter {}
        struct Argument { value: &i8; formatter: fn(&i8, &Formatter) -> i32; }
        struct Arguments { args: *mut Argument; count: u32; pieces: *mut str; piece_count: u32; }
        impl Arguments { fn new(args: *mut Argument, count: u32, pieces: *mut str, piece_count: u32) -> Self {
            return Self${ args, count, pieces, piece_count };
        } }
        fn i32_fmt(value: &i32, fmt: &Formatter) -> i32 { return *value; }
        fn str_fmt(value: &str, fmt: &Formatter) -> i32 {
            let raw = (:*const RawString)value;
            return (:i32)raw->len;
        }
        struct RawString { data: *const i8; len: u64; }
    "#;
    assert_eq!(
        execute(&format!(
            "{runtime} fn run() -> i32 {{ let x = 7; let args = format_args$(\"{{}}\", x); let fmt = Formatter${{}}; return (args.args[0].formatter)(args.args[0].value, &fmt) + (:i32)args.piece_count; }}"
        )),
        9
    );
    assert_eq!(
        execute(&format!(
            "{runtime} fn run() -> i32 {{ let args = format_args$(\"literal\"); return (:i32)args.count + (:i32)args.piece_count; }}"
        )),
        1
    );
    assert_eq!(
        execute(&format!(
            "{runtime} fn run() -> i32 {{ let args = format_args$(\"{{}}\", \"hello\"); let fmt = Formatter${{}}; return (args.args[0].formatter)(args.args[0].value, &fmt); }}"
        )),
        5
    );
    assert_eq!(
        execute("struct S { a: i32; b: i8; } fn run() -> i32 { return (:i32)sizeof$(S); }"),
        8
    );
}

#[test]
fn string_references_survive_returns_and_aggregate_storage() {
    assert_eq!(
        execute(
            r#"
        struct Raw { data: *const i8; len: u64; }
        struct Holder { text: &str; }
        fn text() -> &str { return "a\0b"; }
        fn identity(value: &str) -> &str { return value; }
        fn run() -> i32 {
            let values = [identity(text()), "other"];
            let holder = Holder${values[0]};
            let raw = (:*const Raw)holder.text;
            return (:i32)raw->data[2] + (:i32)raw->len;
        }
    "#
        ),
        101
    );
}

#[test]
fn repeat_arrays_evaluate_once_and_copy_values() {
    for (source, expected) in [
        (
            "fn run() -> i32 { let mut values = [0; 16]; values[15] = 7; return values[0] + values[15]; }",
            7,
        ),
        (
            "fn next(n: &mut i32) -> i32 { *n = *n + 1; return *n; } fn run() -> i32 { let mut n = 0; let values = [next(&mut n); 4]; return n + values[0] + values[3]; }",
            3,
        ),
        (
            "fn next(n: &mut i32) -> i32 { *n = *n + 1; return *n; } fn run() -> i32 { let mut n = 0; let values = [next(&mut n); 0]; return n; }",
            1,
        ),
        (
            "struct S { x:i32; } fn run() -> i32 { let mut values = [S${3}; 2]; values[0].x = 8; return values[0].x + values[1].x; }",
            11,
        ),
        (
            "fn run() -> i32 { let values: [[i32; 3]; 2] = [[5; 3]; 2]; return values[1][2]; }",
            5,
        ),
    ] {
        assert_eq!(execute(source), expected, "{source}");
    }
}

#[test]
fn compile_time_macros_and_derives_generate_executable_code() {
    assert_eq!(
        execute(
            r#"
        macro fn square(value:i32) -> i32 { return value * value; }
        macro fn twice(value:Ast) -> Ast { return value.binary("+", value.clone()); }
        macro fn AddField(item:Ast) { item.add_field("tag", "i32"); }
        macro fn FieldCount(item:Ast) {
            let name = ast::name(item);
            ast::emit(ast::parse_item("impl " + name + " { fn count(&self) -> i32 { return " + ast::to_string(ast::field_count(item)) + "; } }"));
        }
        #[AddField]
        #[derive(FieldCount)]
        struct S { value:i32; }
        fn run() -> i32 {
            let s = S${square$(3), 4};
            return twice$(s.value) + s.count();
        }
    "#
        ),
        20
    );
}

#[test]
fn typed_meta_fields_modify_the_generated_runtime_layout() {
    assert_eq!(
        execute(
            r#"
        macro fn Add(left:AstExpr,right:AstExpr) -> AstExpr { return left.binary("+",right); }
        macro fn Change(item:AstDecl) {
            let field:AstField = item.field(0);
            field.rename("value");
            field.set_type(ast::parse_type("i32"));
        }
        #[Change] struct S { original:i8; }
        fn run() -> i32 { let s = S${40}; return Add$(s.value,2); }
    "#
        ),
        42
    );
}
