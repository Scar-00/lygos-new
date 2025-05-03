use std::path::PathBuf;

use lygos_lexer::Lexer;
use clap::Parser as _;
use lygos_parser::Parser;

#[derive(Debug, clap::Parser)]
struct CliArgs {
    input: PathBuf,
}

const CLI: &'static [&str] = &["./simple", concat!(concat!(std::env!("CARGO_MANIFEST_DIR"), "/tests"), "/simple_program.ly")];

#[test]
fn simple() -> Result<(), Box<dyn std::error::Error>> {
    let args = CliArgs::parse_from(CLI);

    let mut lexer = Lexer::new(&args.input)?;
    let mut parser = Parser::from(lexer.get_tokens());
    let root = parser.build_ast();
    println!("{root:#?}");
    Ok(())
}
