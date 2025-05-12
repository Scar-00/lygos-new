use lygos_macros::Visitable;
use lygos_token::Location;

use crate::AST;

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
