use lygos_ast::{types::Type, *};
use lygos_lexer::Lexer;
use lygos_parser::Parser;

fn parse(source: &str) -> Module {
    let tokens = Lexer::from(source.chars().collect()).get_tokens();
    let AST::Module(module) = Parser::from(tokens).build_ast() else {
        panic!()
    };
    module
}

fn shape(ast: &AST) -> String {
    match ast {
        AST::Identifier(id) => id.id.inner().clone(),
        AST::NumberLiteral(n) => n.value.inner().clone(),
        AST::BinaryExpr(b) => format!("({} {} {})", shape(&b.lhs), b.op.inner(), shape(&b.rhs)),
        AST::AssignmentExpr(a) => format!("({} = {})", shape(&a.lhs), shape(&a.rhs)),
        AST::UnaryExpr(u) => format!("({} {})", u.op.inner(), shape(&u.obj)),
        AST::CastExpr(c) => format!("(cast {} {})", c.target_type.get_full_name(), shape(&c.obj)),
        AST::CallExpr(c) => format!("{}()", shape(&c.caller)),
        AST::MemberCallExpr(c) => format!("{}.{}", shape(&c.obj), shape(&c.r#fn)),
        AST::MemberExpr(MemberExpr::Member { obj, member, .. }) => {
            format!("{}.{}", shape(obj), shape(member))
        }
        AST::AccessExpr(a) => format!("{}[{}]", shape(&a.obj), shape(&a.index)),
        AST::ResolutionExpr(r) => format!("{}::{}", shape(&r.obj), shape(&r.member)),
        _ => panic!("unexpected node: {ast:?}"),
    }
}

#[test]
fn precedence_and_associativity() {
    for (source, expected) in [
        ("a = b + c * d;", "(a = (b + (c * d)))"),
        ("a = b = c;", "(a = (b = c))"),
        ("a - b - c;", "((a - b) - c)"),
        ("a || b && c;", "(a || (b && c))"),
        ("a == b < c;", "(a == (b < c))"),
        ("!a && b;", "((! a) && b)"),
        ("*p + q;", "((* p) + q)"),
        ("-a * b;", "((- a) * b)"),
        ("(:i64)a + b;", "((cast i64 a) + b)"),
        ("Type::value + 1;", "(Type::value + 1)"),
    ] {
        assert_eq!(shape(&parse(source).body.body[0]), expected, "{source}");
    }
}

#[test]
fn mixed_postfix_chains() {
    for source in [
        "a[0].b()[1].c;",
        "factory()().field;",
        "Type::new()[0].method();",
    ] {
        assert_eq!(
            shape(&parse(source).body.body[0]),
            source.trim_end_matches(';')
        );
    }
}

#[test]
fn macro_arguments_preserve_nested_delimiters() {
    let module = parse("format_args$(foo(1, 2), [3, 4], Pair${ 5, 6 },);");
    let AST::MacroCall(call) = &module.body.body[0] else {
        panic!()
    };
    assert_eq!(call.args.len(), 3);
    assert_eq!(call.args[0].len(), 6);
    assert_eq!(call.args[1].len(), 5);
    assert_eq!(call.args[2].len(), 7);
}

#[test]
fn declarations_and_self_types() {
    let module = parse(
        "let mut a: i32; let mut b: [i8; 4]; impl Pair { fn take(self) -> Self { return Self${ 1 }; } } fn f() { if a { return; } return; }",
    );
    assert_eq!(module.body.body.len(), 4);
    let AST::Impl(item) = &module.body.body[2] else {
        panic!()
    };
    let AST::Function(f) = &item.body.body[0] else {
        panic!()
    };
    assert_eq!(f.args[0].typ.get_full_name(), "Pair");
    assert_eq!(f.ret_type.get_full_name(), "Pair");
    let AST::ReturnExpr(r) = &f.body.body[0] else {
        panic!()
    };
    let AST::StructLiteral(s) = r.value.as_deref().unwrap() else {
        panic!()
    };
    assert_eq!(s.id.inner(), "Pair");
}

#[test]
fn literals_are_expressions_and_types_keep_qualifiers() {
    let module =
        parse("let x = Pair${ 1 }.field + 2; type Callback = fn(&mut i32, [i8; 3]) -> *mut i8;");
    let AST::VarDecl(v) = &module.body.body[0] else {
        panic!()
    };
    assert!(matches!(v.value.as_deref(), Some(AST::BinaryExpr(_))));
    let AST::TypeAlias(alias) = &module.body.body[1] else {
        panic!()
    };
    let Type::FuncPtr(f) = &alias.typ else {
        panic!()
    };
    assert_eq!(f.params[0].get_full_name(), "&mut i32");
    assert_eq!(f.params[1].get_full_name(), "[i8;3]");
}

#[test]
fn empty_input_and_stream_without_eof() {
    assert!(parse("").body.body.is_empty());
    assert!(matches!(
        Parser::from(Vec::new()).build_ast(),
        AST::Module(_)
    ));
    let mut tokens = Lexer::from("1;".chars().collect()).get_tokens();
    tokens.pop();
    assert!(matches!(Parser::from(tokens).build_ast(), AST::Module(_)));
}
