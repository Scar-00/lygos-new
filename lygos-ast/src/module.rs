use crate::Block;
use lygos_token::{Location, Tagged};

#[derive(Debug, Clone)]
pub struct Import {
    pub loc: Location,
    pub source: String,
    pub alias: Tagged<String>,
    pub is_path: bool,
}

#[derive(Debug, Clone)]
pub struct Module {
    pub body: Block,
}

impl Module {
    pub fn new(body: Block) -> Self {
        Self { body }
    }
}
