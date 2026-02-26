use lygos_ast::*;
use lygos_token::Location;

pub struct Locator;

impl ASTVisitor for Locator {
    type Output = Location;

    fn visit_module(&mut self, _: &mut Module, _: &mut Scope) -> Self::Output {
        todo!()
    }

    fn visit_function(&mut self, node: &mut Function, _: &mut Scope) -> Self::Output {
        node.id.loc().clone()
    }

    fn visit_vardecl(&mut self, node: &mut VarDecl, _: &mut Scope) -> Self::Output {
        node.id.loc().clone()
    }

    fn visit_assignmentexpr(&mut self, node: &mut AssignmentExpr, scope: &mut Scope) -> Self::Output {
        node.lhs.accept(self, scope)
    }

    fn visit_memberexpr(&mut self, node: &mut MemberExpr, scope: &mut Scope) -> Self::Output {
        match node {
            MemberExpr::Member { obj, .. } => {
                obj.accept(self, scope)
            }
            MemberExpr::Index { obj, .. } => {
                obj.accept(self, scope)
            }
        }
    }

    fn visit_callexpr(&mut self, node: &mut CallExpr, scope: &mut Scope) -> Self::Output {
        node.caller.accept(self, scope)
    }

    fn visit_membercallexpr(&mut self, node: &mut MemberCallExpr, scope: &mut Scope) -> Self::Output {
        node.obj.accept(self, scope)
    }

    fn visit_accessexpr(&mut self, node: &mut AccessExpr, scope: &mut Scope) -> Self::Output {
        node.obj.accept(self, scope)
    }

    fn visit_unaryexpr(&mut self, node: &mut UnaryExpr, scope: &mut Scope) -> Self::Output {
        node.obj.accept(self, scope)
    }

    fn visit_resolutionexpr(&mut self, node: &mut ResolutionExpr, scope: &mut Scope) -> Self::Output {
        node.obj.accept(self, scope)
    }

    fn visit_castexpr(&mut self, node: &mut CastExpr,scope: &mut Scope) -> Self::Output {
        node.obj.accept(self, scope)
    }

    fn visit_returnexpr(&mut self, node: &mut ReturnExpr, _: &mut Scope) -> Self::Output {
        node.loc.clone()
    }

    fn visit_binaryexpr(&mut self, node: &mut BinaryExpr, scope: &mut Scope) -> Self::Output {
        node.lhs.accept(self, scope) + node.rhs.accept(self, scope)
    }

    fn visit_ifstmt(&mut self, node: &mut IfStmt, scope: &mut Scope) -> Self::Output {
        node.cond.accept(self, scope)
    }

    fn visit_forstmt(&mut self, _: &mut ForStmt, _: &mut Scope) -> Self::Output {
        todo!()
    }

    fn visit_breakexpr(&mut self, node: &mut BreakExpr, _: &mut Scope) -> Self::Output {
        node.loc.clone()
    }

    fn visit_structdef(&mut self, node: &mut StructDef, _: &mut Scope) -> Self::Output {
        node.id.loc().clone()
    }

    fn visit_enumdef(&mut self, node: &mut EnumDef, _: &mut Scope) -> Self::Output {
        node.id.loc().clone()
    }

    fn visit_impl(&mut self, node: &mut Impl, _: &mut Scope) -> Self::Output {
        node.typ.loc().clone()
    }

    fn visit_trait(&mut self, _: &mut Trait, _: &mut Scope) -> Self::Output {
        todo!()
    }

    fn visit_macro(&mut self, node: &mut Macro, _: &mut Scope) -> Self::Output {
        node.id.loc().clone()
    }

    fn visit_macrocall(&mut self, node: &mut MacroCall, _: &mut Scope) -> Self::Output {
        node.id.loc().clone()
    }

    fn visit_typealias(&mut self, node: &mut TypeAlias, _: &mut Scope) -> Self::Output {
        node.id.loc().clone()
    }

    fn visit_numberliteral(&mut self, node: &mut NumberLiteral, _: &mut Scope) -> Self::Output {
        node.value.loc().clone()
    }

    fn visit_stringliteral(&mut self, node: &mut StringLiteral, _: &mut Scope) -> Self::Output {
        node.value.loc().clone()
    }

    fn visit_structliteral(&mut self, node: &mut StructLiteral, _: &mut Scope) -> Self::Output {
        node.id.loc().clone()
    }

    fn visit_arrayliteral(&mut self, node: &mut ArrayLiteral, _: &mut Scope) -> Self::Output {
        node.loc.clone()
    }

    fn visit_staticliteral(&mut self, node: &mut StaticLiteral, _: &mut Scope) -> Self::Output {
        node.id.loc().clone()
    }

    fn visit_identifier(&mut self, node: &mut Identifier, _: &mut Scope) -> Self::Output {
        node.id.loc().clone()
    }

    fn visit_blockexpr(&mut self, _: &mut BlockExpr, _: &mut Scope) -> Self::Output {
        todo!()
    }
}
