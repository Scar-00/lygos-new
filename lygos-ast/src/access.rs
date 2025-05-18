use lygos_macros::Visitable;

use crate::AST;

#[derive(Debug, Visitable)]
pub enum MemberExpr {
    Member {
        obj: Box<AST>,
        member: Box<AST>,
        deref: bool,
    },
    Index {
        obj: Box<AST>,
        use_index: bool,
        index: usize,
    }
}

#[derive(Debug, Visitable)]
pub struct ResolutionExpr {
    pub obj: Box<AST>,
    pub member: Box<AST>,
}

impl ResolutionExpr {
    pub fn new(obj: Box<AST>, member: Box<AST>) -> Self {
        Self{ obj, member }
    }
}

#[derive(Debug, Visitable)]
pub struct AccessExpr {
    pub obj: Box<AST>,
    pub index: Box<AST>,
}

impl AccessExpr {
    pub fn new(obj: Box<AST>, index: Box<AST>) -> Self {
        Self{ obj, index }
    }
}
