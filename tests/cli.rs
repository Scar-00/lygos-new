use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

struct Source(PathBuf);
impl Source {
    fn new(source: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "lygos-test-{}-{}.ly",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::write(&path, source).unwrap();
        Self(path)
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_lygos"))
            .arg(&self.0)
            .args(args)
            .output()
            .unwrap()
    }
}
impl Drop for Source {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
        let _ = fs::remove_file(self.0.with_extension("o"));
        let _ = fs::remove_file(
            self.0
                .with_extension(if cfg!(windows) { "exe" } else { "out" }),
        );
    }
}

#[test]
fn cli_links_and_runs_the_print_module() {
    let source = Source::new(
        "import print; fn main() -> i32 { print::println$(\"value = {}\", 42); return 0; }",
    );
    let linked = Command::new(env!("CARGO_BIN_EXE_lygos"))
        .arg(&source.0)
        .env_remove("LIB")
        .env_remove("LIBPATH")
        .output()
        .unwrap();
    assert!(
        linked.status.success(),
        "{}",
        String::from_utf8_lossy(&linked.stderr)
    );
    let output = Command::new(
        source
            .0
            .with_extension(if cfg!(windows) { "exe" } else { "out" }),
    )
    .output()
    .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n"),
        "value = 42\n"
    );
}

#[test]
fn malformed_programs_produce_diagnostics() {
    for (source, expected) in [
        ("fn f() {", "unclosed delimiter"),
        (
            "macro fn f() -> i32 { return f(); } fn run() { f$(); }",
            "compile-time call depth exceeded",
        ),
        (
            "macro fn f() -> i32 { return 1 / 0; } fn run() { f$(); }",
            "division by zero",
        ),
        ("#[Unknown] struct S {}", "unknown compile-time function"),
        (
            "fn f() { let a = [0; -1]; }",
            "non-negative integer literal",
        ),
        (
            "fn f(n:i32) { let a = [0; n]; }",
            "non-negative integer literal",
        ),
        (
            "fn f() { let a = [0; 999999999999999999999999]; }",
            "does not fit in usize",
        ),
        ("fn f() { let a = [0, 1; 4]; }", "exactly one value"),
        ("fn f() { let a = [0; 4, 5]; }", "after repeat length"),
        ("fn f(x: );", "invalid type"),
        ("fn f() { a + b = 1; }", "invalid assignment target"),
        ("fn f() { call$([1, 2)); }", "mismatched delimiter"),
        ("fn f() { call$(foo(1, 2)", "unclosed delimiter"),
        (
            "type A = [i32; 999999999999999999999999];",
            "invalid array size",
        ),
        ("fn f() { let x = \"unfinished", "unterminated literal"),
        ("/* unfinished", "unterminated block comment"),
        ("fn f() { @; }", "unknown token"),
        (
            "fn f() -> i32 { return 'a'; }",
            "expected `i32`, found `i8`",
        ),
        ("fn f() { let x = 1; x = 2; }", "immutable variable"),
        ("fn f(p: *i8);", "raw pointer type requires a qualifier"),
        (
            "fn f(p: *mut *i8);",
            "raw pointer type requires a qualifier",
        ),
    ] {
        let output = Source::new(source).run(&["--check"]);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success(), "{source}");
        assert!(stderr.contains(expected), "{source}\n{stderr}");
        assert!(!stderr.contains("panicked"), "{source}\n{stderr}");
    }
}

#[test]
fn diagnostics_explain_operation_origin_and_fix() {
    let output = Source::new(
        "struct S { data: *const i8; } impl S { fn write(&mut self) { self.data[0] = 'a'; } }",
    )
    .run(&["--check"]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    for text in [
        "cannot write through `*const i8`",
        "read-only type `*const i8` declared here",
        "requires `*mut i8`",
        "Help:",
        "let mut",
    ] {
        assert!(stderr.contains(text), "{stderr}");
    }
    assert!(!stderr.contains("mutable method"), "{stderr}");
    let method = Source::new(
        "struct S {} impl S { fn change(&mut self) {} } fn f() { let s = S${}; s.change(); }",
    )
    .run(&["--check"]);
    let method_stderr = String::from_utf8_lossy(&method.stderr);
    assert!(
        method_stderr.contains("cannot call a mutable method on an immutable variable `s`"),
        "{method_stderr}"
    );
    assert!(
        method_stderr.contains("this method requires a mutable receiver"),
        "{method_stderr}"
    );
    assert!(
        !method_stderr.contains("assignment requires"),
        "{method_stderr}"
    );
    assert!(
        !stderr.contains('\u{1b}'),
        "captured diagnostics should not contain ANSI escapes"
    );
    let output = Source::new("fn f(x: i8); fn g() { f(1); }").run(&["--check"]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("argument 1 of `f`"), "{stderr}");
    assert!(
        stderr.contains("type `i8` required by this declaration"),
        "{stderr}"
    );
    let output = Source::new("fn f(p: *i8);").run(&["--check"]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("*const T") && stderr.contains("*mut T") && stderr.contains("Help:"),
        "{stderr}"
    );
}

#[test]
fn reports_independent_body_errors_together() {
    let output =
        Source::new("fn first() { let x = 1; x = 2; } fn second(p: *const i8) { p[0] = 'a'; }")
            .run(&["--check"]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    for text in [
        "in function `first`",
        "in function `second`",
        "compilation stopped after 2 errors",
    ] {
        assert!(stderr.contains(text), "{stderr}");
    }
}

#[test]
fn cli_checks_and_emits_the_supplied_file() {
    let source = Source::new("fn main() -> i32 { return 0; }");
    assert!(source.run(&["--check"]).status.success());
    let ir = source.run(&["--emit-type", "llvm-ir"]);
    assert!(
        ir.status.success(),
        "{}",
        String::from_utf8_lossy(&ir.stderr)
    );
    assert!(String::from_utf8_lossy(&ir.stdout).contains("define i32 @main()"));
    let ast = source.run(&["--emit-type", "ast"]);
    assert!(ast.status.success());
    assert!(String::from_utf8_lossy(&ast.stdout).contains("ReturnExpr"));
    let typed = source.run(&["--emit-type", "typed-ast"]);
    assert!(
        typed.status.success(),
        "{}",
        String::from_utf8_lossy(&typed.stderr)
    );
    let dump = String::from_utf8_lossy(&typed.stdout);
    assert!(
        dump.contains("TypedModule") && dump.contains("TypeId") && dump.contains("StmtId"),
        "{dump}"
    );
}
