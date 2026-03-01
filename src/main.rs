mod code_gen;
mod locator;
mod preprocessor;
mod symbol_visitor;
mod type_getter;
use libffi::middle::Cif;
use lygos_ast::Scope;
use lygos_llvm::jit::Jit;
use symbol_visitor::SymbolCollector;

use lygos_lexer::Lexer;
use lygos_llvm::{builder::IRBuilder, context::Context, module::Module};
use lygos_parser::Parser;
use lygos_util::cli::CliArgs;
use lygos_util::cli::CommandExt;

use std::path::PathBuf;
use std::process::Command;

const CLI: &[&str] = &[
    "./simple",
    lygos_util::tests::concat!(lygos_util::tests::TEST_DIR, "/simple_program.ly"),
    "-e",
    "llvm-ir",
];

fn main2() -> Result<(), Box<dyn std::error::Error>> {
    let args = CliArgs::parse_from(CLI);
    //let args = CliArgs::parse();
    color_eyre::install()?;

    let Some(input) = args.input else {
        return Err("No input file".to_string().into());
    };
    let mut lexer = Lexer::new(&input)?;
    let mut parser = Parser::from(lexer.get_tokens());
    let mut root = parser.build_ast();

    let mut symbol_collector = SymbolCollector::new();
    let mut scope_root = Scope::new();
    root.accept(&mut symbol_collector, &mut scope_root);

    Context::init_all();
    let tt = Context::get_default_target_triple();
    let target = Context::lookup_target(&tt);

    let target_machine = Context::create_target_machine(target, tt.as_str(), "generic", "");

    let ctx = Context::new();
    let mut m = Module::new(input.display().to_string(), &ctx);
    m.set_data_layout(&target_machine);
    m.set_target_triple(&tt);
    let builder = IRBuilder::new(&ctx);

    let mut code_generator = code_gen::CodeGenerator::new(&ctx, &m, &builder);
    root.accept(&mut code_generator, &mut scope_root);

    println!("{}", m.print());

    let obj_path = input.with_extension("o");
    if !Context::emit_obj_file(obj_path.display().to_string(), &m, &target_machine) {
        return Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::Other,
            "fail",
        )));
    }
    let exe_path = input.with_extension("exe");
    let mut cmd = Command::from_array([
        "clang",
        "-o",
        &exe_path.display().to_string(),
        &obj_path.display().to_string(),
        //"-lc",
    ]);
    println!("{}", cmd.display());
    cmd.spawn()?.wait()?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    use libffi::middle::{arg, CodePtr, Type as FFIType};
    color_eyre::install()?;

    let input = PathBuf::from("./tests/simple_program.ly");
    let mut lexer = Lexer::new(&input)?;
    let mut parser = Parser::from(lexer.get_tokens());
    let mut root = parser.build_ast();

    let mut symbol_collector = SymbolCollector::new();
    let mut scope_root = Scope::new();
    root.accept(&mut symbol_collector, &mut scope_root);

    Context::init_all();
    let tt = Context::get_default_target_triple();
    let target = Context::lookup_target(&tt);

    let target_machine = Context::create_target_machine(target, tt.as_str(), "generic", "");

    let ctx = Context::new();
    let mut m = Module::new(input.display().to_string(), &ctx);
    m.set_data_layout(&target_machine);
    m.set_target_triple(&tt);
    let builder = IRBuilder::new(&ctx);

    let mut code_generator = code_gen::CodeGenerator::new(&ctx, &m, &builder);
    root.accept(&mut code_generator, &mut scope_root);

    let mut jit = Jit::new();
    jit.add_module(m);

    let add_ptr = jit.lookup("add").unwrap();

    let func_type = Cif::new([FFIType::i32(), FFIType::i32()], FFIType::i32());
    let ret = unsafe { func_type.call::<i32>(CodePtr(add_ptr as *mut _), &[arg(&10), arg(&20)]) };
    println!("ret = {ret}");
    Ok(())
}
