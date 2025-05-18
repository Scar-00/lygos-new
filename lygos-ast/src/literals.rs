use lygos_macros::Visitable;
use lygos_token::{Location, Tagged};

use crate::{types::Type, AST};

#[derive(Debug, Clone, Visitable)]
pub struct Identifier {
    pub id: Tagged<String>,
    pub deref: bool,
}

impl Identifier {
    pub fn new(id: Tagged<String>) -> Self {
        Self{ id, deref: true }
    }
}

#[derive(Debug, Visitable)]
pub struct StringLiteral {
    pub value: Tagged<String>,
}

impl StringLiteral {
    pub fn new(value: Tagged<String>) -> Self {
        Self{ value }
    }
}

#[derive(Debug)]
pub enum NumberType {
    Int,
    Char,
    Float,
    Double,
}

#[derive(Debug, Visitable)]
pub struct NumberLiteral {
    pub value: Tagged<String>,
    pub typ: NumberType,
}

impl NumberLiteral {
    pub fn new(value: Tagged<String>, typ: NumberType) -> Self {
        Self{ value, typ }
    }
}

#[derive(Debug, Visitable)]
pub struct StaticLiteral {
    pub id: Tagged<String>,
    pub typ: Type,
    pub value: Option<Box<AST>>
}

impl StaticLiteral {
    pub fn new(id: Tagged<String>, typ: Type, value: Option<Box<AST>>) -> Self {
        Self{ id, typ, value }
    }
}

#[derive(Debug, Visitable)]
pub struct TypeAlias {
    pub id: Tagged<String>,
    pub typ: Type,
}

impl TypeAlias {
    pub fn new(id: Tagged<String>, typ: Type) -> Self {
        Self{ id, typ }
    }
}

#[derive(Debug, Visitable)]
pub struct ArrayLiteral {
    pub loc: Location,
    pub values: Vec<AST>,
}

impl ArrayLiteral {
    pub fn new(loc: Location, values: Vec<AST>) -> Self {
        Self{ loc, values }
    }
}
