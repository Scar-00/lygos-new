mod symbol_visitor;
mod code_gen;
mod type_getter;
use lygos_ast::Scope;
use symbol_visitor::SymbolCollector;

use lygos_lexer::Lexer;
use lygos_parser::Parser;
use lygos_util::cli::CliArgs;
use lygos_llvm::{context::Context, module::Module, builder::IRBuilder};

use std::{os::unix::process::CommandExt, process::Command};

const CLI: &[&str] = &["./simple", lygos_util::tests::concat!(lygos_util::tests::TEST_DIR, "/simple_program.ly")];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = CliArgs::deligate_parse_from(CLI);

    let mut lexer = Lexer::new(&args.input)?;
    let mut parser = Parser::from(lexer.get_tokens());
    let mut root = parser.build_ast();

    let mut symbol_collector = SymbolCollector::new();
    let mut scope_root = Scope::new();
    root.accept(&mut symbol_collector, &mut scope_root);

    Context::init_all();
    let tt = Context::get_default_target_triple();
    let target = Context::lookup_target(&tt);

    let cpu = "generic";
    let features = "";
    let target_machine = Context::create_target_machine(target, tt.as_str(), cpu, features);

    let ctx = Context::new();
    let mut m = Module::new(args.input.display().to_string(), &ctx);
    m.set_data_layout(&target_machine);
    m.set_target_triple(&tt);
    let builder = IRBuilder::new(&ctx);

    let mut code_generator = code_gen::CodeGenerator::new(&ctx, &m, &builder);
    root.accept(&mut code_generator, &mut scope_root);

    println!("{}", m.print());

    let obj_path = args.input.with_extension("o");
    if !Context::emit_obj_file(obj_path.display().to_string(), &m, &target_machine) {
        return Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, "fail")));
    }
    let exe_path = args.input.with_extension("");
    let mut cmd = Command::new("clang");
    let cmd = cmd.arg("-o").arg(exe_path).arg(obj_path).arg("-lc");
    println!("{:?}", cmd.exec());

    Ok(())
}
