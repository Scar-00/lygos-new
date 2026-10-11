use lygos_token::Tagged;

use crate::{AST, types::Type};

#[derive(Debug, Clone)]
pub struct AssignmentExpr {
    pub lhs: Box<AST>,
    pub rhs: Box<AST>,
}

impl AssignmentExpr {
    pub fn new(lhs: Box<AST>, rhs: Box<AST>) -> Self {
        Self { lhs, rhs }
    }
}

#[derive(Debug, Clone)]
pub struct UnaryExpr {
    pub op: Tagged<String>,
    pub obj: Box<AST>,
}

impl UnaryExpr {
    pub fn new(op: Tagged<String>, obj: Box<AST>) -> Self {
        Self { op, obj }
    }
}

#[derive(Debug, Clone)]
pub struct CastExpr {
    pub obj: Box<AST>,
    pub target_type: Type,
}

impl CastExpr {
    pub fn new(obj: Box<AST>, target_type: Type) -> Self {
        Self { obj, target_type }
    }
}
