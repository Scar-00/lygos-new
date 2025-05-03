use lygos_macros::Visitable;

use crate::Block;

#[derive(Debug, Visitable)]
pub struct Module {
    pub body: Block,
}

impl Module {
    pub fn new(body: Block) -> Self {
        Self{ body }
    }
}
