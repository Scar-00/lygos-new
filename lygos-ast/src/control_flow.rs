use lygos_macros::Visitable;
use lygos_token::Location;

use crate::{Block, AST};

#[derive(Debug, Visitable)]
pub struct ReturnExpr {
    pub loc: Location,
    pub value: Option<Box<AST>>,
}

impl ReturnExpr {
    pub fn new(loc: Location, value: Option<Box<AST>>) -> Self {
        Self{ loc, value }
    }
}

#[derive(Debug, Visitable)]
pub struct IfStmt {
    pub cond: Box<AST>,
    pub then_body: Block,
    pub else_body: Option<Block>,
}

impl IfStmt {
    pub fn new(cond: Box<AST>, then_body: Block, else_body: Option<Block>) -> Self {
        Self{ cond, then_body, else_body }
    }
}
