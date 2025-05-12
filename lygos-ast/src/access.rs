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

/*#[derive(Debug, Visitable)]
pub struct MemberExpr {
    pub obj: Box<AST>,
    pub member: Option<Box<AST>>,
    pub deref: bool,
    pub use_index: bool,
    pub index: usize,
}*/

/*impl MemberExpr {
    pub fn new(obj: Box<AST>, member: Box<AST>, deref: bool) -> Self {
        Self {
            obj,
            member: Some(member),
            deref,
            use_index: false,
            index: 0,
        }
    }

    pub fn new_index(obj: Box<AST>, index: usize) -> Self {
        Self {
            obj,
            member: None,
            deref: false,
            use_index: true,
            index,
        }
    }
}*/
