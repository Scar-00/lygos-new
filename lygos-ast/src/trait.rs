use lygos_token::Tagged;

use crate::Block;

#[derive(Debug, Clone)]
pub struct Trait {
    pub id: Tagged<String>,
    pub funcs: Block,
}

impl Trait {
    pub fn new(id: Tagged<String>, funcs: Block) -> Self {
        Self { id, funcs }
    }
}
