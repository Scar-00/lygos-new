use lygos_lexer::Lexer;
use lygos_macros::Visitable;
use lygos_token::{Tagged, Token};

use crate::{types::Type, Block, AST};

#[derive(Debug, Clone)]
pub struct StructField {
    pub id: Tagged<String>,
    pub typ: Type,
}

#[derive(Debug, Visitable)]
pub struct StructDef {
    pub id: Tagged<String>,
    pub fields: Vec<StructField>,
}

impl StructDef {
    pub fn new(id: Tagged<String>, fields: Vec<StructField>) -> Self {
        Self{ id, fields }
    }

    pub fn generate_formatter(&self) -> Vec<Token> {
        let mut formatting_function = format!("
            impl Debug for {} {{
                fn fmt_debug(&self, fmt: &mut Formatter) -> FormattingError {{
                    let mut debug = fmt.debug_struct(\"{}\");", self.id.inner(), self.id.inner());
        for field in &self.fields {
            formatting_function += match &field.typ {
                Type::Path(_) => format!("debug.field(\"{}\", format_args$(\"{{:?}}\", self.{}));", field.id.inner(), field.id.inner()),
                Type::Pointer(ptr) => format!("
                    if self.{} != (:*{})0 {{
                        debug.field(\"{}\", format_args$(\"{{:?}}\", self.{}));
                    }}
                ", field.id.inner(), ptr.typ.get_full_name(), field.id.inner(), field.id.inner()),
                Type::Array(_) => "".into(),
                _ => "".into(),
            }.as_str();
        }
        formatting_function += "debug.finish();}}";
        let mut lexer = Lexer::from(formatting_function.chars().collect());
        lexer.get_tokens()
    }

    pub fn is_self_referential(&self) -> Option<&StructField> {
        for field in &self.fields {
            if let Type::Path(ty) = &field.typ {
                if ty.path == *self.id.inner() {
                    return Some(field);
                }
            }
        }
        None
    }
}

#[derive(Debug, Visitable)]
pub struct StructLiteral {
    pub id: Tagged<String>,
    pub inits: Vec<(Option<Tagged<String>>, AST)>,
}

impl StructLiteral {
    pub fn new(id: Tagged<String>, inits: Vec<(Option<Tagged<String>>, AST)>) -> Self {
        Self{ id, inits }
    }
}

#[derive(Debug, Visitable)]
pub struct Impl {
    pub typ: Tagged<String>,
    pub body: Block,
    pub trat: Option<Tagged<String>>,
}

impl Impl {
    pub fn new(typ: Tagged<String>, body: Block, trat: Option<Tagged<String>>) -> Self {
        Self{ typ, body, trat }
    }
}

#[derive(Debug, Visitable)]
pub struct EnumDef {
    pub id: Tagged<String>,
    pub variants: Vec<String>,
    pub typ: Type,
}

impl EnumDef {
    pub fn new(id: Tagged<String>, variants: Vec<String>, typ: Type) -> Self {
        Self{ id, variants, typ }
    }
}
