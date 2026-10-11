use lygos_token::Tagged;

use crate::{AST, types::Type};

#[derive(Debug, Clone)]
pub struct VarDecl {
    pub id: Tagged<String>,
    pub cnst: bool,
    pub typ: Option<Type>,
    pub value: Option<Box<AST>>,
}

impl VarDecl {
    pub fn new(id: Tagged<String>, cnst: bool, typ: Option<Type>, value: Option<Box<AST>>) -> Self {
        Self {
            id,
            cnst,
            typ,
            value,
        }
    }
}
