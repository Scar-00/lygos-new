use lygos_lexer::Lexer;
use lygos_parser::Parser;
use lygos_util::cli::CliArgs;

const CLI: &'static [&str] = &[
    "./simple",
    lygos_util::tests::concat!(lygos_util::tests::TEST_DIR, "/simple_program.ly"),
];

#[test]
fn simple() -> Result<(), Box<dyn std::error::Error>> {
    let args = CliArgs::parse_from(CLI);

    let input = args.input.as_deref().ok_or("missing test input")?;
    let mut lexer = Lexer::new(input)?;
    let mut parser = Parser::from(lexer.get_tokens());
    let root = parser.build_ast();
    println!("{root:#?}");
    Ok(())
}
