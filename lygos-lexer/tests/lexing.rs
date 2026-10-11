use lygos_lexer::Lexer;
use lygos_token::TokenType;

fn tokens(source: &str) -> Vec<TokenType> {
    Lexer::from(source.chars().collect())
        .get_tokens()
        .into_iter()
        .map(|t| t.typ)
        .collect()
}

#[test]
fn eof_without_sentinel_or_newline() {
    assert_eq!(tokens(""), vec![TokenType::Eof]);
    assert_eq!(tokens("// no newline"), vec![TokenType::Eof]);
    assert_eq!(
        tokens("_value"),
        vec![TokenType::Id("_value".into()), TokenType::Eof]
    );
}

#[test]
fn comments_do_not_consume_following_tokens() {
    assert_eq!(
        tokens("/*a*//*b*/1"),
        vec![TokenType::Integer("1".into()), TokenType::Eof]
    );
    assert_eq!(
        tokens("/* outer /* inner */ tail */2"),
        vec![TokenType::Integer("2".into()), TokenType::Eof]
    );
}

#[test]
fn escaped_quotes_and_member_access_after_numbers() {
    assert_eq!(
        tokens(r#""a\"b" '\'' 12.field 1.25"#),
        vec![
            TokenType::String(r#"a\"b"#.into()),
            TokenType::Char(r#"\'"#.into()),
            TokenType::Integer("12".into()),
            TokenType::Dot,
            TokenType::Id("field".into()),
            TokenType::Float("1.25".into()),
            TokenType::Eof,
        ]
    );
}

#[test]
fn multicharacter_operator_spans() {
    let tokens = Lexer::from("a == b && c".chars().collect()).get_tokens();
    assert_eq!(tokens[1].loc.range, 2..4);
    assert_eq!(tokens[3].loc.range, 7..9);
}

#[test]
#[should_panic(expected = "unterminated literal")]
fn unterminated_literal() {
    tokens("\"unfinished");
}

#[test]
#[should_panic(expected = "unterminated block comment")]
fn unterminated_comment() {
    tokens("/* unfinished");
}

#[test]
fn fallible_lexing_reports_errors_without_panicking() {
    for (source, message) in [
        ("/*", "unterminated block comment"),
        ("\"", "unterminated literal"),
        ("@", "unknown token"),
    ] {
        let error = Lexer::from(source.chars().collect())
            .try_get_tokens()
            .unwrap_err();
        assert!(error.to_string().contains(message));
    }
}
