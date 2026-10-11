use lygos_ast::type_check::TypeChecker;
use lygos_lexer::Lexer;
use lygos_parser::Parser;

fn check(source: &str) -> Result<(), String> {
    let ast = Parser::from(Lexer::from(source.chars().collect()).get_tokens()).build_ast();
    TypeChecker::analyze(&ast)
        .map(|_| ())
        .map_err(|errors| errors.into_iter().next().unwrap().message)
}

#[test]
fn strings_can_only_be_held_through_references() {
    for source in [
        "fn f() { let text = \"hello\"; let copy: &str = text; }",
        "fn f(text: &str) -> &str { return text; }",
        "fn f() -> &str { return \"hello\"; }",
        "struct S { text: &str; } fn f() { let s = S${\"hello\"}; }",
        "type Text = str; fn f(x: &Text) -> &Text { return x; }",
        "fn f() { let texts = [\"a\", \"b\"]; }",
        "static text: &str = \"hello\";",
    ] {
        assert!(check(source).is_ok(), "{source}");
    }
    for source in [
        "fn f() { let text: str = \"hello\"; }",
        "fn f() { let mut text: str; }",
        "fn f(text: str) {}",
        "fn f() -> str { return \"hello\"; }",
        "struct S { text: str; }",
        "type Text = str; struct S { text: Text; }",
        "struct S { texts: [str; 2]; }",
        "fn f(callback: fn(str)) {}",
        "static text: str = \"hello\";",
        "fn f(text: &str) { let copy = *text; }",
        "fn f(text: *const str) { text[0]; }",
        "fn f() { sizeof$(str); }",
    ] {
        let message = check(source).unwrap_err();
        assert!(message.contains("reference-only"), "{source}: {message}");
    }
}

#[test]
fn repeat_arrays_have_fixed_lengths_and_element_types() {
    assert!(check("fn f() { let values: [i32; 4] = [0; 4]; let empty: [i32; 0] = [1; 0]; let texts: [&str; 3] = [\"a\"; 3]; }").is_ok());
    assert!(
        check("fn f() { let values: [i32; 3] = [0; 4]; }")
            .unwrap_err()
            .contains("expected")
    );
    assert!(
        check("fn f() { let values = [1; 2]; values[2]; }")
            .unwrap_err()
            .contains("bounds")
    );
    assert!(
        check("fn f() { let values: [i8; 2] = [1; 2]; }")
            .unwrap_err()
            .contains("expected")
    );
}

#[test]
fn aliases_forward_declarations_and_function_values() {
    for source in [
        "type Size = Count; type Count = i32; fn f(n: Size) -> Count { return n; }",
        "fn f() -> i32 { return g(1); } fn g(x: i32) -> i32 { return x; }",
        "fn f(n: Alias) -> i32; type Alias = i32; fn f(n: i32) -> Alias { return n; }",
        "type Callback = fn(i32) -> i32; fn f(callback: Callback) -> i32 { return callback(1); }",
        "fn id(x: i32) -> i32 { return x; } fn get() -> fn(i32) -> i32 { return id; } fn use() -> i32 { return get()(3); }",
        "struct Node { next: *mut Node; }",
        "type Nodes = *mut Node; struct Node { next: Nodes; }",
    ] {
        assert_eq!(check(source), Ok(()), "{source}");
    }
}

#[test]
fn calls_and_return_types_are_checked() {
    for (source, expected) in [
        ("fn f() -> i32 { return; }", "expected `i32`, found `void`"),
        ("fn f() { return 1; }", "expected `void`, found `i32`"),
        ("fn f() -> i32 {}", "not all paths return"),
        (
            "fn f(x: bool) -> i32 { if x { return 1; } }",
            "not all paths return",
        ),
        (
            "fn f(x: i8); fn g() { f(1); }",
            "expected `i8`, found `i32`",
        ),
        (
            "fn f(cb: fn(i32)); fn g(cb: fn(i32)) { cb(); }",
            "expected 1 arguments",
        ),
        ("fn f() { let x = 1; x(); }", "not callable"),
        (
            "fn f() -> i32; fn f() -> i8 { return 'a'; }",
            "expected `i32`, found `i8`",
        ),
        ("fn f(x: i32, x: i32);", "duplicate parameter"),
    ] {
        assert!(check(source).unwrap_err().contains(expected), "{source}");
    }
    assert!(check("fn f(x: bool) -> i32 { if x { return 1; } else { return 2; } }").is_ok());
}

#[test]
fn invalid_types_and_recursive_layouts_are_diagnosed() {
    for (source, expected) in [
        ("type A = B; type B = A;", "cyclic type alias"),
        ("type A = *mut A;", "cyclic type alias"),
        ("struct A { b: B; } struct B { a: A; }", "infinitely sized"),
        ("struct A { children: [A; 1]; }", "infinitely sized"),
        ("fn f(x: Unknown);", "unknown type"),
        ("fn f(x: void);", "void is not a value type"),
        ("enum E: f32 { A }", "integer type"),
        ("enum E { A, A }", "duplicate enum variant"),
        ("struct A { x: i32; x: i8; }", "duplicate field"),
    ] {
        assert!(check(source).unwrap_err().contains(expected), "{source}");
    }
}

#[test]
fn scopes_mutability_conditions_and_indexing() {
    for (source, expected) in [
        ("fn f() { let x = 1; x = 2; }", "immutable variable"),
        ("fn f(x: &i32) { *x = 2; }", "cannot write through"),
        (
            "fn f() { let x = 1; let y = &mut x; }",
            "immutable variable",
        ),
        ("fn f() { if (:bool)1 { let x = 1; } x; }", "unknown value"),
        (
            "fn f() { for let mut i = 0 in i < 2 {} i; }",
            "unknown value",
        ),
        ("fn f() { if 1 {} }", "expected `bool`"),
        ("fn f() { break; }", "break outside"),
        ("fn f() { let x = 1; x[0]; }", "indexing requires"),
        ("fn f() { let x = [1, 2]; x[(:f32)0]; }", "integer type"),
        ("fn f() { let x = [1, 2]; x[2]; }", "out of bounds"),
        ("fn f() { let x = [1, 'a']; }", "expected `i32`, found `i8`"),
        ("fn f() { let x = []; }", "empty array"),
        ("fn f() { let x: *mut i32 = 1; }", "expected `*mut i32`"),
        ("fn f() { let x: &i32 = (:*mut i32)0; }", "expected `&i32`"),
    ] {
        assert!(check(source).unwrap_err().contains(expected), "{source}");
    }
    for source in [
        "fn f() { let mut x = 1; let p = &mut x; *p = 2; }",
        "fn f() { let x = 1; if (:bool)1 { let x = 'a'; x; } x; }",
        "fn f() { for let mut i = 0 in i < 2 { i = i + 1; } }",
        "fn f(p: *mut i8) { p[0] = 'a'; *p = 'b'; }",
        "struct Buffer { data: *mut i8; len: u32; } impl Buffer { fn push(&mut self, char: i8) { let len = self.len; self.data[len] = char; } }",
    ] {
        assert!(check(source).is_ok(), "{source}");
    }
}

#[test]
fn raw_pointer_writes_are_independent_of_binding_mutability() {
    assert!(
        check("struct S { data: *mut i8; } impl S { fn write(&self) { self.data[0] = 'a'; } }")
            .is_ok()
    );
    assert!(check("struct S { data: *mut i8; } impl S { fn replace(&self, data: *mut i8) { self.data = data; } }").unwrap_err().contains("cannot write through"));
    assert!(
        check("fn f(p: &i8) { p[0] = 'a'; }")
            .unwrap_err()
            .contains("cannot write through")
    );
    assert!(
        check("fn f(p: *const i8) { p[0] = 'a'; }")
            .unwrap_err()
            .contains("cannot write through `*const i8`")
    );
    assert!(
        check("fn f(p: *const i8) { *p = 'a'; }")
            .unwrap_err()
            .contains("cannot write through `*const i8`")
    );
    assert!(
        check("fn f(p: *const i8) { let mut q = p; q[0] = 'a'; }")
            .unwrap_err()
            .contains("cannot write through `*const i8`")
    );
    assert!(
        check("fn f(p: *mut i8); fn g(p: *const i8) { f(p); }")
            .unwrap_err()
            .contains("expected `*mut i8`, found `*const i8`")
    );
}

#[test]
fn pointer_write_diagnostic_points_to_target_and_declaration() {
    let source = "struct Buffer { data: *const i8; } impl Buffer { fn push(&mut self, c: i8) { self.data[0] = c; } }";
    let ast = Parser::from(Lexer::from(source.chars().collect()).get_tokens()).build_ast();
    let error = TypeChecker::check(&ast).unwrap_err();
    assert!(error.message.contains("cannot write through `*const i8`"));
    assert_eq!(&source[error.loc.range.clone()], "self.data[0]");
    assert_eq!(&source[error.related[0].0.range.clone()], "*const i8");
    assert!(error.help.as_deref().unwrap().contains("*mut i8"));
    assert!(error.help.as_deref().unwrap().contains("let mut"));
}

#[test]
fn structs_methods_and_trait_contracts() {
    let prelude = "struct Pair { x: i32; } impl Pair { fn new() -> Self { return Self${ 1 }; } fn read(&self, add: i32) -> i32 { return self.x + add; } fn change(&mut self) { self.x = 2; } }";
    assert!(
        check(&format!(
            "{prelude} fn f() -> i32 {{ let mut p = Pair::new(); p.change(); return p.read(3); }}"
        ))
        .is_ok()
    );
    for (body, expected) in [
        ("let p = Pair${};", "every field"),
        ("let p = Pair${ .x = 'a' };", "expected `i32`"),
        ("let p = Pair${ .y = 1 };", "unknown initializer"),
        ("let p = Pair::new(); p.change();", "immutable variable"),
        ("let p = Pair::new(); p.read('a');", "expected `i32`"),
        ("let p = Pair::new(); p.new();", "associated function"),
        ("let p = Pair::new(); p.missing;", "unknown field"),
    ] {
        assert!(
            check(&format!("{prelude} fn f() {{ {body} }}"))
                .unwrap_err()
                .contains(expected),
            "{body}"
        );
    }
    for (source, expected) in [
        (
            "trait T { fn get(&self) -> i32; } struct S {} impl T for S {}",
            "missing trait method",
        ),
        (
            "trait T { fn get(&self) -> i32; } struct S {} impl T for S { fn get(&self) -> i8 { return 'a'; } }",
            "expected `i32`",
        ),
        ("struct S {} impl Missing for S {}", "unknown trait"),
    ] {
        assert!(check(source).unwrap_err().contains(expected), "{source}");
    }
    assert!(check("trait Clone { fn clone(&self) -> Self; } struct S {} impl Clone for S { fn clone(&self) -> Self { return Self${}; } }").is_ok());
}

#[test]
fn definite_initialization_merges_control_flow() {
    for source in [
        "fn f() -> i32 { let mut x: i32; return x; }",
        "fn f(c: bool) -> i32 { let mut x: i32; if c { x = 1; } return x; }",
        "fn f(c: bool) -> i32 { let mut x: i32; for let mut i = 0 in c { x = 1; break; } return x; }",
        "fn f() { let mut x: i32; x = x + 1; }",
        "fn f() -> i32 { let mut x: i32; (:bool)0 && ((x = 1) == 1); return x; }",
    ] {
        assert!(
            check(source).unwrap_err().contains("uninitialized"),
            "{source}"
        );
    }
    for source in [
        "fn f() -> i32 { let mut x: i32; x = 2; return x; }",
        "fn f(c: bool) -> i32 { let mut x: i32; if c { x = 1; } else { x = 2; } return x; }",
        "fn f(c: bool) -> i32 { let mut x: i32; if c { return 1; } else { x = 2; } return x; }",
    ] {
        assert_eq!(check(source), Ok(()), "{source}");
    }
}
