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

#[derive(Debug, Visitable)]
pub struct ForStmt {
    pub cond: Box<AST>,
    pub var: Box<AST>,
    pub body: Block,
}

impl ForStmt {
    pub fn new(cond: Box<AST>, var: Box<AST>, body: Block) -> Self {
        Self{ cond, var, body }
    }
}

#[derive(Debug, Visitable)]
pub struct BreakExpr {
    pub loc: Location,
}

impl BreakExpr {
    pub fn new(loc: Location) -> Self {
        Self{ loc }
    }
}

#[derive(Debug, Visitable)]
pub struct BlockExpr {
    body: Block,
}

impl BlockExpr {
    pub fn new(body: Block) -> Self {
        Self{ body }
    }
}
