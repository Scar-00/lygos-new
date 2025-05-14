use lygos_ast::{symbols::{Function as FunctionSymbol, Symbol}, *};
use lygos_logging::{ErrorLabel, Logger};

pub struct SymbolCollector;

impl SymbolCollector {
    pub fn new() -> Self {
        Self{}
    }
}

impl ASTVisitor for SymbolCollector {
    type Output = ();

    fn visit_module(&mut self,node: &mut Module, scope: &mut Scope) -> Self::Output {
        node.body.scope.set_parent(scope);
        node.body.body.iter_mut().for_each(|child| {
            child.accept(self, &mut node.body.scope)
        });
    }

    fn visit_function(&mut self,node: &mut Function, scope: &mut Scope) -> Self::Output {
        node.body.scope.set_parent(scope);
        let func_sym = FunctionSymbol::new(
            node.id.clone(),
            node.name_mangeled.clone(),
            node.args.clone(),
            node.ret_type.clone(),
            node.is_def
        );
        scope.add_symbol(&node.name_mangeled, Symbol::Function(func_sym));
        node.body.body.iter_mut().for_each(|child| {
            child.accept(self, &mut node.body.scope)
        });
    }

    fn visit_vardecl(&mut self, _: &mut VarDecl, _: &mut Scope) -> Self::Output {}

    fn visit_assignmentexpr(&mut self, _: &mut AssignmentExpr, _: &mut Scope) -> Self::Output {}

    fn visit_memberexpr(&mut self, _: &mut MemberExpr, _: &mut Scope) -> Self::Output {}

    fn visit_callexpr(&mut self, _: &mut CallExpr, _: &mut Scope) -> Self::Output {}

    fn visit_returnexpr(&mut self, _: &mut ReturnExpr, _: &mut Scope) -> Self::Output {}

    fn visit_binaryexpr(&mut self, _: &mut BinaryExpr, _: &mut Scope) -> Self::Output {}

    fn visit_structdef(&mut self, node: &mut StructDef, scope: &mut Scope) -> Self::Output {
        if let Some(field) = node.is_self_referential() {
            Logger::error_msg_label_info(
                "cannot self referece",
                ErrorLabel::from(&field.typ.get_loc(), "referece to `Self`"),
                "try adding a `*` or `&`"
            );
        }

        if let Some(strct) = scope.try_resolve_symbol(&node.id) {
            if let Symbol::Struct(strct) = strct {
                strct.resolve(node.id.clone(), node.fields.clone());
            }
        }else {
            let r#struct = symbols::Struct::new(node.id.clone(), node.fields.clone());
            scope.add_symbol(node.id.inner(), Symbol::Struct(r#struct));
        }
    }

    fn visit_typealias(&mut self, node: &mut TypeAlias, scope: &mut Scope) -> Self::Output {
        let symbol = symbols::TypeAlias::new(node.id.clone(), node.typ.clone());
        scope.add_symbol(node.id.inner().clone(), Symbol::TypeAlias(symbol));
    }

    fn visit_numberliteral(&mut self, _: &mut NumberLiteral, _: &mut Scope) -> Self::Output {}

    fn visit_stringliteral(&mut self, _: &mut StringLiteral, _: &mut Scope) -> Self::Output {}

    fn visit_structliteral(&mut self, _: &mut StructLiteral, _: &mut Scope) -> Self::Output {}

    fn visit_staticliteral(&mut self, _: &mut StaticLiteral, _: &mut Scope) -> Self::Output {}

    fn visit_identifier(&mut self, _: &mut Identifier, _: &mut Scope) -> Self::Output {}

    fn visit_ifstmt(&mut self, _: &mut IfStmt, _: &mut Scope) -> Self::Output {}
}
