use crate::AST;

#[derive(Debug, Clone)]
pub struct CallExpr {
    pub caller: Box<AST>,
    pub args: Vec<AST>,
}

impl CallExpr {
    pub fn new(caller: Box<AST>, args: Vec<AST>) -> Self {
        Self { caller, args }
    }
}

#[derive(Debug, Clone)]
pub struct MemberCallExpr {
    pub obj: Box<AST>,
    pub r#fn: Box<AST>,
    pub deref: bool,
}

impl MemberCallExpr {
    pub fn new(obj: Box<AST>, r#fn: Box<AST>, deref: bool) -> Self {
        Self { obj, r#fn, deref }
    }
}
