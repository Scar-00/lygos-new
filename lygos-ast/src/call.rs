use lygos_macros::Visitable;

use crate::AST;

#[derive(Debug, Visitable)]
pub struct CallExpr {
    pub caller: Box<AST>,
    pub args: Vec<AST>,
}

impl CallExpr {
    pub fn new(caller: Box<AST>, args: Vec<AST>) -> Self {
        Self{ caller, args }
    }
}
