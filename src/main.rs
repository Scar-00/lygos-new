mod symbol_visitor;
use symbol_visitor::SymbolCollector;

use std::path::PathBuf;

use lygos_lexer::Lexer;
use clap::Parser as _;
use lygos_parser::Parser;

#[derive(Debug, clap::Parser)]
struct CliArgs {
    input: PathBuf,
}

const CLI: &[&str] = &["./simple", concat!(concat!(std::env!("CARGO_MANIFEST_DIR"), "/tests"), "/simple_program.ly")];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = CliArgs::parse_from(CLI);

    let mut lexer = Lexer::new(&args.input)?;
    let mut parser = Parser::from(lexer.get_tokens());
    let mut root = parser.build_ast();

    let mut symbol_collector = SymbolCollector::new();
    root.accept(&mut symbol_collector);

    println!("{root:#?}");
    Ok(())
}
