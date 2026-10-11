use lygos::code_gen::CodeGenerator;
use lygos_ast::type_check::TypeChecker;
use lygos_lexer::Lexer;
use lygos_llvm::{builder::IRBuilder, context::Context, jit::Jit, module::Module};
use lygos_parser::Parser;

extern "C" fn double(value: i32) -> i32 {
    value * 2
}

#[test]
fn jit_owns_its_context_and_resolves_explicit_host_symbols() {
    Context::init_all();
    let source = "fn host_double(value:i32)->i32; fn run()->i32 { return host_double(21); }";
    let ast = Parser::from(Lexer::from(source.chars().collect()).get_tokens()).build_raw_ast();
    let typed = TypeChecker::analyze(&ast).unwrap();
    let mut jit = Jit::try_new().unwrap();
    unsafe {
        jit.define_symbol("host_double", double as *const () as *mut ())
            .unwrap();
    }
    {
        let context = Context::new();
        let module = Module::new("owned-context", &context);
        let builder = IRBuilder::new(&context);
        CodeGenerator::new(&context, &module, &builder)
            .generate(&typed)
            .unwrap();
        jit.add_module(module).unwrap();
    } // Both the source context and builder are gone before materialization.
    drop(typed);
    drop(ast);
    let entry: unsafe extern "C" fn() -> i32 =
        unsafe { std::mem::transmute(jit.lookup("run").unwrap()) };
    assert_eq!(unsafe { entry() }, 42);
    assert!(jit.lookup("does_not_exist").is_none());
    assert!(jit.last_error().contains("does_not_exist"));
}
