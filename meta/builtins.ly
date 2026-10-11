// Compiler-provided compile-time handles. Empty declarations are opaque,
// not zero-sized runtime structs. #[meta] declarations are validated and erased.
#[meta] struct Ast {}
#[meta] struct AstExpr {}
#[meta] struct AstDecl {}
#[meta] struct AstField {}
#[meta] struct AstType {}
// Namespace for constructors and compile-time utilities.
#[meta] struct ast {}

#[meta]
impl Ast {
    fn name(self) -> &str;
    fn kind(self) -> &str;
    fn clone(self) -> Ast;
    fn rename(self, name: &str);
    fn replace(self, replacement: Ast);
    fn emit(self);
    fn field_count(self) -> i64;
    fn field_name(self, index: i64) -> &str;
    fn field_type(self, index: i64) -> &str;
    fn add_field(self, name: &str, type_name: &str);
    fn binary(self, operator: &str, right: AstExpr) -> AstExpr;
    fn as_expr(self) -> AstExpr;
    fn as_decl(self) -> AstDecl;
    fn field(self, index: i64) -> AstField;
    fn fields(self) -> [AstField];
}

// Specialized syntax handles also accept the general Ast methods above.
#[meta]
impl AstExpr {
    fn clone(self) -> AstExpr;
    fn binary(self, operator: &str, right: AstExpr) -> AstExpr;
}
#[meta]
impl AstDecl {
    fn clone(self) -> AstDecl;
    fn name(self) -> &str;
    fn rename(self, name: &str);
    fn emit(self);
    fn field(self, index: i64) -> AstField;
    fn fields(self) -> [AstField];
}

#[meta]
impl AstField {
    fn name(self) -> &str;
    fn rename(self, name: &str);
    fn field_type(self) -> AstType;
    fn set_type(self, typ: AstType);
}

// Type handles describe source annotations, not inferred expression types.
#[meta]
impl AstType {
    fn name(self) -> &str;
    fn kind(self) -> &str;
    fn clone(self) -> AstType;
    fn is_reference(self) -> bool;
    fn is_mutable(self) -> bool;
    fn element(self) -> AstType;
    fn array_len(self) -> i64;
}

#[meta]
impl ast {
    fn parse_expr(source: &str) -> AstExpr;
    fn parse_item(source: &str) -> AstDecl;
    fn parse_type(source: &str) -> AstType;
    fn int(value: i64) -> AstExpr;
    fn ident(name: &str) -> AstExpr;
    fn binary(operator: &str, left: AstExpr, right: AstExpr) -> AstExpr;
    fn error(message: &str);
    fn to_string(value: i64) -> &str;
    fn to_string(value: bool) -> &str;
    fn to_string(value: &str) -> &str;
    // Legacy free-function forms of the node operations.
    fn name(node: Ast) -> &str;
    fn kind(node: Ast) -> &str;
    fn clone(node: Ast) -> Ast;
    fn rename(node: Ast, name: &str);
    fn replace(node: Ast, replacement: Ast);
    fn emit(node: Ast);
    fn field_count(node: Ast) -> i64;
    fn field_name(node: Ast, index: i64) -> &str;
    fn field_type(node: Ast, index: i64) -> &str;
    fn add_field(node: Ast, name: &str, type_name: &str);
    fn as_expr(node: Ast) -> AstExpr;
    fn as_decl(node: Ast) -> AstDecl;
    fn field(node: Ast, index: i64) -> AstField;
    fn fields(node: Ast) -> [AstField];
}
