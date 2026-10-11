use lygos_ast::{
    arena::{Arena, ArenaId, ExprId},
    type_check::TypeChecker,
    typed::*,
};
use lygos_lexer::Lexer;
use lygos_parser::Parser;

fn analyze(source: &str) -> TypedModule {
    let ast = Parser::from(Lexer::from(source.chars().collect()).get_tokens()).build_ast();
    TypeChecker::analyze(&ast).unwrap_or_else(|errors| panic!("{errors:?}\n{source}"))
}

#[test]
fn arena_ids_survive_growth_and_mutation() {
    let mut arena = Arena::<String, ExprId>::default();
    let first = arena.insert("first".into());
    for i in 0..10_000 {
        arena.insert(i.to_string());
    }
    assert_eq!(arena[first], "first");
    *arena.get_mut(first).unwrap() = "updated".into();
    assert_eq!(arena.get(first).unwrap(), "updated");
    assert_eq!(arena.len(), 10_001);
    assert!(arena.get(ExprId::from_index(20_000)).is_none());
}

#[test]
fn canonical_types_are_interned_without_source_locations() {
    let program = analyze(
        "type A = i32; type B = A; fn first(x: A) -> B { return x; } fn second(y: i32) -> i32 { return y; } struct Node { next: *mut Node; } fn p(x: *const i8, y: *mut i8) {}",
    );
    let ctx = program.context();
    let first = ctx.lookup(program.scope(), "first").unwrap();
    let second = ctx.lookup(program.scope(), "second").unwrap();
    assert_eq!(ctx.symbol(first).typ, ctx.symbol(second).typ);
    assert_eq!(
        ctx.symbol(ctx.lookup(program.scope(), "A").unwrap()).typ,
        ctx.symbol(ctx.lookup(program.scope(), "B").unwrap()).typ
    );
    let pointer = ctx.lookup(program.scope(), "p").unwrap();
    let SemanticType::Function { parameters, .. } = ctx.typ(ctx.symbol(pointer).typ) else {
        panic!()
    };
    assert_ne!(parameters[0], parameters[1]);
    let node = ctx.lookup(program.scope(), "Node").unwrap();
    let SymbolKind::Struct { fields } = &ctx.symbol(node).kind else {
        panic!()
    };
    let SemanticType::Pointer { pointee, .. } = ctx.typ(ctx.symbol(fields[0]).typ) else {
        panic!()
    };
    assert_eq!(*pointee, ctx.symbol(node).typ);
}

#[test]
fn symbol_ids_preserve_shadowing_and_explicit_loads() {
    let program = analyze("fn f() -> i32 { let x = 1; if (:bool)1 { let x = 2; x; } return x; }");
    let ctx = program.context();
    let function = ctx.function(program.functions()[0]);
    let block = ctx.block(function.body.unwrap());
    let StmtKind::Let { symbol: outer, .. } = ctx.statement(block.statements[0]).kind else {
        panic!()
    };
    let StmtKind::If { then_block, .. } = ctx.statement(block.statements[1]).kind else {
        panic!()
    };
    let inner = ctx.block(then_block);
    let StmtKind::Let { symbol: nested, .. } = ctx.statement(inner.statements[0]).kind else {
        panic!()
    };
    assert_ne!(outer, nested);
    assert_eq!(ctx.scope(inner.scope).parent, Some(function.scope));
    assert_eq!(ctx.scope(function.scope).parent, Some(program.scope()));
    let StmtKind::Return(Some(value)) = ctx.statement(block.statements[2]).kind else {
        panic!()
    };
    let ExprKind::Load(place) = ctx.expression(value).kind else {
        panic!()
    };
    assert_eq!(ctx.expression(place).category, ValueCategory::Place);
    let ExprKind::Symbol(symbol) = ctx.expression(place).kind else {
        panic!()
    };
    assert_eq!(symbol, outer);
}

#[test]
fn calls_and_receiver_adjustments_have_resolved_signatures() {
    let program = analyze(
        "struct S { x: i32; } impl S { fn read(&self) -> i32 { return self.x; } } fn id(x: i32) -> i32 { return x; } fn get() -> fn(i32) -> i32 { return id; } fn f() -> i32 { let mut s = S${ 4 }; let p = &mut s; return p.read() + get()(2); }",
    );
    let ctx = program.context();
    let mut direct = 0;
    let mut indirect = 0;
    for (_, expr) in ctx.expressions() {
        match &expr.kind {
            ExprKind::Call {
                callee,
                signature,
                arguments,
            } => {
                let SemanticType::Function { parameters, result } = ctx.typ(*signature) else {
                    panic!()
                };
                assert_eq!(expr.typ, *result);
                assert_eq!(parameters.len(), arguments.len());
                for (param, arg) in parameters.iter().zip(arguments) {
                    assert_eq!(*param, ctx.expression(*arg).typ);
                    assert_eq!(ctx.expression(*arg).category, ValueCategory::Value);
                }
                match callee {
                    Callee::Direct(symbol) => {
                        direct += 1;
                        assert_eq!(ctx.symbol(*symbol).typ, *signature);
                    }
                    Callee::Indirect(value) => {
                        indirect += 1;
                        assert_eq!(ctx.expression(*value).typ, *signature);
                    }
                }
            }
            ExprKind::Field { field, .. } => {
                assert!(matches!(ctx.symbol(*field).kind, SymbolKind::Field { .. }))
            }
            ExprKind::Load(place) => {
                assert_eq!(ctx.expression(*place).category, ValueCategory::Place)
            }
            _ => {}
        }
    }
    assert_eq!(direct, 2);
    assert_eq!(indirect, 1);
}

#[test]
fn numeric_declaration_conversion_is_an_explicit_typed_node() {
    let program = analyze("fn f() -> u8 { let x: u8 = 255; return x; }");
    let ctx = program.context();
    let cast = ctx
        .expressions()
        .find_map(|(_, expr)| {
            if let ExprKind::Cast(value) = expr.kind {
                Some((expr, value))
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(ctx.typ(cast.0.typ), &SemanticType::Primitive(Primitive::U8));
    assert_eq!(
        ctx.typ(ctx.expression(cast.1).typ),
        &SemanticType::Primitive(Primitive::I32)
    );
}

#[test]
fn malformed_intrinsic_runtime_is_rejected_before_llvm() {
    let source = "struct Argument {} struct Arguments {} impl Arguments { fn new() -> Self { return Self${}; } } fn f() { format_args$(\"literal\"); }";
    let ast = Parser::from(Lexer::from(source.chars().collect()).get_tokens()).build_ast();
    let errors = TypeChecker::analyze(&ast).unwrap_err();
    assert!(
        errors[0]
            .message
            .contains("invalid format_args runtime layout")
    );
    assert!(
        errors[0]
            .help
            .as_deref()
            .unwrap()
            .contains("Arguments::new")
    );
}
