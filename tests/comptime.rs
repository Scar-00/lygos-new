use lygos_ast::{AST, type_check::TypeChecker};
use lygos_lexer::Lexer;
use lygos_parser::{Parser, comptime};

fn expand(source: &str) -> Result<AST, String> {
    let raw = Parser::from(Lexer::from(source.chars().collect()).get_tokens()).build_raw_ast();
    comptime::expand(raw).map_err(|error| error.message)
}
fn check(source: &str) -> AST {
    let ast = expand(source).unwrap();
    TypeChecker::analyze(&ast).unwrap();
    ast
}

#[test]
fn compile_time_values_loops_helpers_and_nested_macros() {
    let ast = check(
        r#"
        macro fn multiply(a: i32, b: i32) -> i32 { return a * b; }
        macro fn factorial(n: i32) -> i32 {
            let mut value = 1;
            for let mut i = 1 in i <= n { value = multiply(value, i); i = i + 1; }
            return value;
        }
        macro fn greeting(name: &str) -> &str { return "hello " + name; }
        fn run() -> i32 { let text: &str = greeting$("Lygos"); return factorial$(multiply$(2, 3)); }
    "#,
    );
    let AST::Module(module) = ast else { panic!() };
    assert_eq!(
        module.body.body.len(),
        1,
        "compile-time definitions must be erased"
    );
}

#[test]
fn inline_syntax_is_not_evaluated_and_is_checked_after_expansion() {
    check(
        r#"
        macro fn twice(value: Ast) -> Ast { return ast::binary("+", value, ast::clone(value)); }
        fn run(n: i32) -> i32 { return twice$(n); }
    "#,
    );
    let ast = expand(r#"
        macro fn bad(value: Ast) -> Ast { return ast::binary("+", value, ast::parse_expr("\"bad\"")); }
        fn run() -> i32 { return bad$(3); }
    "#).unwrap();
    assert!(TypeChecker::analyze(&ast).is_err());
}

#[test]
fn attributes_modify_declarations_and_derives_emit_methods() {
    check(
        r#"
        macro fn rename(item: Ast, name: &str) { ast::rename(item, name); }
        macro fn add_tag(item: Ast) { ast::add_field(item, "tag", "i32"); }
        macro fn FieldCount(item: Ast) {
            let count = ast::field_count(item);
            let name = ast::name(item);
            ast::emit(ast::parse_item("impl " + name + " { fn field_count(&self) -> i32 { return " + ast::to_string(count) + "; } }"));
        }
        #[rename("Renamed")]
        #[add_tag]
        #[derive(FieldCount)]
        struct Original { value:i32; }
        fn run() -> i32 { let s = Renamed${7, 9}; return s.field_count(); }
    "#,
    );
    check(
        r#"
        macro fn replacement(item: Ast) -> Ast { return ast::parse_item("struct New { value:i32; }"); }
        #[replacement] struct Old {}
        fn run() -> i32 { let s = New${3}; return s.value; }
    "#,
    );
}

#[test]
fn node_methods_share_handles_and_support_chained_receivers() {
    check(
        r#"
        macro fn twice(value:Ast) -> Ast { return value.binary("+", value.clone()); }
        macro fn Rename(item:Ast) {
            let alias = item;
            alias.rename("Renamed");
            let copy = item.clone();
            copy.rename("Copy");
            copy.add_field("tag", "i32");
            copy.emit();
            if item.name() != "Renamed" { ast::error("shared handle lost mutation"); }
            if item.field_count() != 1 { ast::error("clone was not independent"); }
            if item.field_name(0) != "x" { ast::error("wrong field name"); }
            if item.field_type(0) != "i32" { ast::error("wrong field type"); }
            if item.kind() != "struct" { ast::error("wrong kind"); }
            ast::parse_item("struct Chained {}").clone().emit();
        }
        #[Rename] struct Original { x:i32; }
        fn run() -> i32 {
            let a = Renamed${1}; let b = Copy${2, 3}; let c = Chained${};
            return twice$(a.x) + b.tag;
        }
    "#,
    );
    check(
        r#"
        macro fn Replace(item:Ast) {
            item.replace(ast::parse_item("struct New {}"));
        }
        #[Replace] struct Old {}
        fn run() { let value = New${}; }
    "#,
    );
    for (source, expected) in [
        (
            "macro fn f() -> i32 { return 1.name(); } fn run() { f$(); }",
            "Ast handle",
        ),
        (
            "macro fn f(value:Ast) -> Ast { return value.missing(); } fn run() { f$(1); }",
            "unknown Ast method",
        ),
        (
            "macro fn f(value:Ast) -> Ast { return value.clone(1); } fn run() { f$(1); }",
            "expects 0 arguments",
        ),
    ] {
        assert!(expand(source).unwrap_err().contains(expected), "{source}");
    }
}

#[test]
fn meta_typedefs_are_validated_erased_and_support_typed_reflection() {
    let mut source = comptime::META_SOURCE.to_owned();
    source.push_str(r#"
        macro fn sum(left:AstExpr, right:AstExpr) -> AstExpr { return left.binary("+",right); }
        macro fn first(item:AstDecl) -> AstField { return item.field(0); }
        macro fn inspect(typ:AstType) -> &str { return typ.element().name(); }
        macro fn Change(item:AstDecl) {
            let fields: [AstField] = item.fields();
            if fields.len() != 2 { ast::error("wrong field count"); }
            let field:AstField = first(item);
            field.rename("renamed");
            let old:AstType = field.field_type();
            if old.name() != "i8" { ast::error("wrong type"); }
            field.set_type(ast::parse_type("i32"));
            if fields[0].field_type().name() != "i32" { ast::error("field mutation was not visible"); }
            let array = ast::parse_type("[i32; 3]");
            if array.kind() != "array" || array.array_len() != 3 { ast::error("wrong array type"); }
            if array.clone().element().name() != "i32" { ast::error("wrong element type"); }
            let ref_type = ast::parse_type("&mut i32");
            if !ref_type.is_reference() || !ref_type.is_mutable() { ast::error("wrong reference qualifiers"); }
            if inspect(ref_type) != "i32" { ast::error("typed helper failed"); }
            item.as_decl();
            ast::int(1).as_expr();
        }
        #[Change] struct S { old:i8; next:i32; }
        fn run() -> i32 { let s = S${7,2}; return sum$(s.renamed,s.next); }
    "#);
    let ast = check(&source);
    let AST::Module(module) = ast else { panic!() };
    assert_eq!(
        module.body.body.len(),
        2,
        "meta declarations must not reach runtime analysis"
    );
}

#[test]
fn specialized_meta_types_reject_wrong_kinds_and_stale_fields() {
    for (source, expected) in [
        (
            "macro fn Expr(item:AstExpr) {} #[Expr] struct S {}",
            "requires `AstExpr`",
        ),
        (
            "macro fn f(item:Ast) -> AstExpr { return ast::parse_item(\"struct S {}\"); } fn run() { f$(1); }",
            "must return `AstExpr`",
        ),
        (
            "macro fn D(item:AstDecl) { item.as_expr(); } #[D] struct S {}",
            "wrong node kind",
        ),
        (
            "macro fn D(item:AstDecl) { let field=item.field(0); item.replace(ast::parse_item(\"struct New {}\")); field.name(); } #[D] struct S { x:i32; }",
            "no longer refers",
        ),
        (
            "macro fn D(item:AstDecl) { item.field(-1); } #[D] struct S {}",
            "index out of bounds",
        ),
        (
            "macro fn D(item:AstDecl) { item.field(0).set_type(\"i32\"); } #[D] struct S { x:i8; }",
            "AstType handle",
        ),
        (
            "#[meta] struct NotBuiltin {}",
            "invalid compiler meta declaration",
        ),
        (
            "#[meta] struct Ast { fake:i32; }",
            "invalid compiler meta declaration",
        ),
        ("struct AstExpr {}", "cannot be redefined"),
        (
            "macro fn D(item:AstDecl) { let field=item.field(0); item.replace(ast::parse_item(\"struct New { y:i32; }\")); field.name(); } #[D] struct S { x:i32; }",
            "no longer refers",
        ),
        (
            "macro fn f() -> AstType { return ast::parse_type(\"i32\"); } fn run() { f$(); }",
            "cannot become runtime expressions",
        ),
    ] {
        let message = expand(source).unwrap_err();
        assert!(message.contains(expected), "{source}: {message}");
    }
}

#[test]
fn compile_time_failures_are_bounded_and_diagnosed() {
    for (source, expected) in [
        (
            "macro fn f(x:i32) -> i32 { return x / 0; } fn run() { f$(1); }",
            "division by zero",
        ),
        (
            "macro fn f(x:i32) -> i32 { return x; } fn run() { f$(\"bad\"); }",
            "requires `i32`",
        ),
        (
            "macro fn f() -> i32 { let x = 1; x = 2; return x; } fn run() { f$(); }",
            "immutable",
        ),
        (
            "macro fn f() -> i32 { return f(); } fn run() { f$(); }",
            "call depth",
        ),
        (
            "macro fn f() -> i32 { for let mut i=0 in i == 0 {} return 0; } fn run() { f$(); }",
            "execution limit",
        ),
        (
            "macro fn f() -> i32 { return missing(); } fn run() { f$(); }",
            "unknown compile-time function",
        ),
        ("#[unknown] struct S {}", "unknown compile-time function"),
        (
            "macro fn D(item:Ast) { ast::error(\"cannot derive this\"); } #[derive(D)] struct S {}",
            "cannot derive this",
        ),
        (
            "macro fn f() -> i32 { return 1; } fn run() { f$(2); }",
            "expects 0 arguments",
        ),
        (
            "macro fn f() -> Ast { let mut node=ast::int(1); for let mut i=0 in i<20 { node=ast::binary(\"+\",node,node); i=i+1; } return node; } fn run() { f$(); }",
            "syntax allocation limit",
        ),
        (
            "macro fn f() -> i32 { let mut x:i32=2147483647; x=x+1; return 0; } fn run() { f$(); }",
            "declared type",
        ),
    ] {
        let message = expand(source).unwrap_err();
        assert!(message.contains(expected), "{source}: {message}");
    }
}

#[test]
fn native_macros_preserve_operand_order_and_short_circuiting() {
    let ast = check(
        r#"
        macro fn combine(a:i32,b:i32,c:i32,d:i32)->i32 { return a*1000+b*100+c*10+d; }
        macro fn calculate()->i32 {
            let mut x=1;
            let a=x+(x=2);
            let b=combine(x,x=4,x,x=5);
            if (:bool)0 && (:bool)(1/0) { ast::error("short circuit failed"); }
            if (:bool)1 || (:bool)(1/0) {} else { ast::error("short circuit failed"); }
            return a+b;
        }
        fn run()->i32 { return calculate$(); }
    "#,
    );
    let AST::Module(module) = ast else { panic!() };
    let AST::Function(function) = &module.body.body[0] else {
        panic!()
    };
    let AST::ReturnExpr(ret) = &function.body.body[0] else {
        panic!()
    };
    let Some(value) = &ret.value else { panic!() };
    let AST::NumberLiteral(value) = &**value else {
        panic!()
    };
    assert_eq!(value.value.inner(), "2448");
}

#[test]
fn cached_native_macros_keep_invocations_and_lexical_scopes_separate() {
    let ast = check(
        r#"
        macro fn odd(n:i32)->bool { if n==0 { return (:bool)0; } return even(n-1); }
        macro fn even(n:i32)->bool { if n==0 { return (:bool)1; } return odd(n-1); }
        macro fn answer(n:i32)->i32 {
            let mut total=0;
            for let mut i=0 in i<n {
                let mut j=0;
                for let mut k=0 in k<3 {
                    if k==2 { break; }
                    let i=10;
                    j=j+i; k=k+1;
                }
                total=total+j; i=i+1;
            }
            if even(n) { return total; } else { return total+1; }
        }
        fn run()->i32 { return answer$(2)+answer$(3); }
    "#,
    );
    let AST::Module(module) = ast else { panic!() };
    let AST::Function(function) = &module.body.body[0] else {
        panic!()
    };
    let AST::ReturnExpr(ret) = &function.body.body[0] else {
        panic!()
    };
    let AST::BinaryExpr(sum) = &**ret.value.as_ref().unwrap() else {
        panic!()
    };
    let (AST::NumberLiteral(a), AST::NumberLiteral(b)) = (&*sum.lhs, &*sum.rhs) else {
        panic!()
    };
    assert_eq!(a.value.inner(), "40");
    assert_eq!(b.value.inner(), "61");
}

#[test]
fn native_host_failures_report_the_call_chain_and_bound_value_storage() {
    let raw = Parser::from(
        Lexer::from(
            r#"
        macro fn inner()->i32 { return 1/0; }
        macro fn outer()->i32 { return inner(); }
        fn run()->i32 { return outer$(); }
    "#
            .chars()
            .collect(),
        )
        .get_tokens(),
    )
    .build_raw_ast();
    let error = comptime::expand(raw).unwrap_err();
    assert!(error.message.contains("division by zero"));
    assert!(
        error
            .related
            .iter()
            .any(|(_, message)| message.contains("`inner`"))
    );
    assert!(
        error
            .related
            .iter()
            .any(|(_, message)| message.contains("`outer`"))
    );
    let error = expand(
        r#"
        macro fn allocate()->i32 {
            let mut values=[0;10000];
            for let mut i=0 in i<100 { values=[0;10000]; i=i+1; }
            return 0;
        }
        fn run()->i32 { return allocate$(); }
    "#,
    )
    .unwrap_err();
    assert!(error.contains("value storage limit"), "{error}");
}
