use lygos_token::{Location, Tagged};

use crate::{AST, types::Type};

pub fn unescape_literal(raw: &str) -> Result<String, String> {
    let mut result = String::new();
    let mut chars = raw.chars();
    while let Some(ch) = chars.next() {
        result.push(if ch == '\\' {
            match chars.next().ok_or("unterminated escape")? {
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                '0' => '\0',
                '\\' => '\\',
                '"' => '"',
                '\'' => '\'',
                ch => return Err(format!("unknown escape `\\{ch}`")),
            }
        } else {
            ch
        });
    }
    Ok(result)
}

#[derive(Debug, Clone)]
pub struct Identifier {
    pub id: Tagged<String>,
    pub deref: bool,
}

impl Identifier {
    pub fn new(id: Tagged<String>) -> Self {
        Self { id, deref: true }
    }
}

#[derive(Debug, Clone)]
pub struct StringLiteral {
    pub value: Tagged<String>,
}

impl StringLiteral {
    pub fn new(value: Tagged<String>) -> Self {
        Self { value }
    }
}

#[derive(Debug, Clone)]
pub enum NumberType {
    Int,
    Char,
    Float,
    Double,
}

#[derive(Debug, Clone)]
pub struct NumberLiteral {
    pub value: Tagged<String>,
    pub typ: NumberType,
}

impl NumberLiteral {
    pub fn new(value: Tagged<String>, typ: NumberType) -> Self {
        Self { value, typ }
    }
}

#[derive(Debug, Clone)]
pub struct StaticLiteral {
    pub id: Tagged<String>,
    pub typ: Type,
    pub value: Option<Box<AST>>,
}

impl StaticLiteral {
    pub fn new(id: Tagged<String>, typ: Type, value: Option<Box<AST>>) -> Self {
        Self { id, typ, value }
    }
}

#[derive(Debug, Clone)]
pub struct TypeAlias {
    pub id: Tagged<String>,
    pub typ: Type,
}

impl TypeAlias {
    pub fn new(id: Tagged<String>, typ: Type) -> Self {
        Self { id, typ }
    }
}

#[derive(Debug, Clone)]
pub struct ArrayLiteral {
    pub loc: Location,
    pub values: Vec<AST>,
    pub repeat: Option<usize>,
}

impl ArrayLiteral {
    pub fn new(loc: Location, values: Vec<AST>) -> Self {
        Self {
            loc,
            values,
            repeat: None,
        }
    }
}
