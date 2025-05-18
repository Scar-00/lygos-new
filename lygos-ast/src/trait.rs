use lygos_macros::Visitable;
use lygos_token::Tagged;

use crate::Block;

#[derive(Debug, Visitable)]
pub struct Trait {
    pub id: Tagged<String>,
    pub funcs: Block,
}

impl Trait {
    pub fn new(id: Tagged<String>, funcs: Block) -> Self {
        Self{ id, funcs }
    }
}
