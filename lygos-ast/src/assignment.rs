use lygos_macros::Visitable;

use crate::AST;

#[derive(Debug, Visitable)]
pub struct AssignmentExpr {
    pub lhs: Box<AST>,
    pub rhs: Box<AST>,
}

impl AssignmentExpr {
    pub fn new(lhs: Box<AST>, rhs: Box<AST>) -> Self {
        Self{ lhs, rhs }
    }
}
