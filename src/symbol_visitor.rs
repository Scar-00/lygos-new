use lygos_ast::{symbols::{Function as FunctionSymbol, Symbol, Variable}, *};
use lygos_util as util;

pub struct SymbolCollector<> {
    current_scope: util::Pointer<Scope>,
}

impl SymbolCollector {
    pub fn new() -> Self {
        Self{
            current_scope: util::Pointer::new(),
        }
    }
}

impl ASTVisitor for SymbolCollector {
    type Output = ();

    fn visit_module(&mut self,node: &mut Module) -> Self::Output {
        node.body.scope.set_parent(&self.current_scope);
        self.current_scope.set_value(&node.body.scope);
        node.body.body.iter_mut().for_each(|child| {
            child.accept(self)
        });
    }

    fn visit_function(&mut self,node: &mut Function) -> Self::Output {
        node.body.scope.set_parent(&self.current_scope);
        let func_sym = FunctionSymbol::new(
            node.id.clone(),
            node.name_mangeled.clone(),
            node.args.clone(),
            node.ret_type.clone(),
            node.is_def
        );
        self.current_scope.add_symbol(&node.name_mangeled, Symbol::Function(func_sym));
        self.current_scope.set_value(&node.body.scope);
        node.body.body.iter_mut().for_each(|child| {
            child.accept(self)
        });
    }

    fn visit_vardecl(&mut self,node: &mut VarDecl) -> Self::Output {
        /*let ty = match (node.typ, node.value) {
            (Some(ty), None) => {
                ty
            },
            (None, Some(ty)) => {
                ty
            }
            (Some(var_typ), Some(value_typ)) => {
                todo!("check at given and received type match at variable delclaration");
            }
            (None, None) => {
                unreachable!("variable has no type");
            }
        };


        let var = Variable::new(node.id.loc().clone(), ty, alloca, is_const)
        self.current_scope.add_symbol(node.id.inner(), Symbol::Variable(Var);*/
    }

    fn visit_assignmentexpr(&mut self,node: &mut AssignmentExpr) -> Self::Output {
    }

    fn visit_callexpr(&mut self,node: &mut CallExpr) -> Self::Output {
    }

    fn visit_returnexpr(&mut self,node: &mut ReturnExpr) -> Self::Output {
    }

    fn visit_binaryexpr(&mut self,node: &mut BinaryExpr) -> Self::Output {
    }

    fn visit_typealias(&mut self,node: &mut TypeAlias) -> Self::Output {
    }

    fn visit_numberliteral(&mut self,node: &mut NumberLiteral) -> Self::Output {
    }

    fn visit_stringliteral(&mut self,node: &mut StringLiteral) -> Self::Output {
    }

    fn visit_staticliteral(&mut self,node: &mut StaticLiteral) -> Self::Output {
    }

    fn visit_identifier(&mut self,node: &mut Identifier) -> Self::Output {
    }
}
