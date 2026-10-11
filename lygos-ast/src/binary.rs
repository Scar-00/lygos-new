use lygos_token::Tagged;

use crate::AST;

#[derive(Debug, Clone)]
pub struct BinaryExpr {
    pub lhs: Box<AST>,
    pub rhs: Box<AST>,
    pub op: Tagged<String>,
}

impl BinaryExpr {
    pub fn new(lhs: Box<AST>, rhs: Box<AST>, op: Tagged<String>) -> Self {
        Self { lhs, rhs, op }
    }
}
