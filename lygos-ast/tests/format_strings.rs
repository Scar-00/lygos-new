use lygos_ast::parse_format_string;

#[test]
fn parses_empty_pieces_debug_and_escaped_braces() {
    let (pieces, debug) = parse_format_string("{}{{{:?}}}").unwrap();
    assert_eq!(pieces, ["", "{", "}"]);
    assert_eq!(debug, [false, true]);
    assert_eq!(
        parse_format_string("").unwrap(),
        (vec![String::new()], vec![])
    );
    for format in ["{", "}", "{x}", "{:}"] {
        assert!(parse_format_string(format).is_err());
    }
}
