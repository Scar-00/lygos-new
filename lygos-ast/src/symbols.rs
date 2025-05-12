use std::collections::{HashMap, HashSet};

use lygos_llvm::{value::ValueRef, types::TypeRef};
use lygos_logging::{ErrorLabel, Logger};
use lygos_token::{Location, Tagged};

use crate::{types::Type, FunctionArg, StructField};

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum Symbol {
    Variable(Variable),
    Function(Function),
    Struct(Struct),
    //Enum(Enum),
    //Macro(Macro),
    //TypeAlias(TypeAlias),
    //Trait(Trait),
}

impl Symbol {
    pub fn loc(&self) -> &Location {
        match self {
            Symbol::Variable(var) => &var.loc,
            Symbol::Function(func) => func.name.loc(),
            Symbol::Struct(s) => s.name.loc(),
            //Symbol::Enum(e) => e.name.loc(),
            //Symbol::Macro(m) => m.inner.id.loc(),
            //Symbol::TypeAlias(ty) => ty.id.loc(),
            //Symbol::Trait(t) => t.inner.id.loc(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Variable {
    pub loc: Location,
    pub typ: Type,
    pub alloca: ValueRef,
    pub is_const: bool,
}

impl Variable {
    pub fn new(loc: Location, typ: Type, alloca: ValueRef, is_const: bool) -> Self {
        Self{ loc, typ, alloca, is_const }
    }
}

#[derive(Debug, Clone)]
pub struct Function {
    pub name: Tagged<String>,
    pub name_mangeled: String,
    pub args: Vec<FunctionArg>,
    pub ret_type: Type,
    pub is_definition: bool,
}

impl Function {
    pub fn new(name: Tagged<String>, name_mangeled: String, args: Vec<FunctionArg>, ret_type: Type, is_definition: bool) -> Self {
        Self{ name, name_mangeled, args, ret_type, is_definition }
    }
}

#[derive(Debug, Clone)]
pub struct Struct {
    pub name: Tagged<String>,
    pub fields: Vec<StructField>,
    pub functions: HashMap<String, Function>,
    pub traits: HashSet<String>,
    pub generated: Option<TypeRef>,
    pub resolved: bool,
}

impl Struct {
    pub fn new(name: Tagged<String>, fields: Vec<StructField>) -> Self {
        Self{ name, fields, functions: HashMap::new(), traits: HashSet::new(), generated: None, resolved: true }
    }

    pub fn new_dummy(name: Tagged<String>) -> Self {
        Self{ name, fields: Vec::new(), functions: HashMap::new(), traits: HashSet::new(), generated: None, resolved: true }
    }

    pub fn register_function(&mut self, func: Function) {
        self.functions.insert(func.name.inner().clone(), func);
    }

    pub fn get_function<S: AsRef<str>>(&mut self, loc: &Location, id: S) -> &mut Function {
        if !self.functions.contains_key(id.as_ref()) {
            Logger::error_msg_label(
                "unknown function",
                ErrorLabel::from(loc, format!("unknown function `{}` in struct `{}`", id.as_ref(), self.name.inner()))
            );
        }
        self.functions.get_mut(id.as_ref()).unwrap()
    }

    pub fn register_trait_impl<S: AsRef<str>>(&mut self, name: &Tagged<S>) {
        if self.traits.contains(name.inner().as_ref()) {
            Logger::error_msg_label(format!("trait `{}` is already implemented for type `{}`", name.inner().as_ref(), self.name.inner()).as_str(),
                ErrorLabel::from(name.loc(), "cannot reimplement trait")
            );
        }
        self.traits.insert(name.inner().as_ref().to_string());
    }

    pub fn implements_trait<S: AsRef<str>>(&self, name: S) -> bool {
        self.traits.contains(name.as_ref())
    }

    pub fn resolve(&mut self, name: Tagged<String>, fields: Vec<StructField>) {
        self.resolved = true;
        self.name = name;
        self.fields = fields;
    }
}
/*
#[derive(Debug, Clone)]
pub struct Enum {
    name: Tagged<String>,
    pub variants: Vec<String>,
    pub typ: Type,
}

impl Enum {
    pub fn new(name: Tagged<String>, variants: Vec<String>, typ: Type) -> Self {
        Self{ name, variants, typ }
    }
}

#[derive(Debug, Clone)]
pub struct Macro {
    inner: Pointer<crate::ast::Macro>
}

impl Macro {
    pub fn new(inner: Pointer<crate::ast::Macro>) -> Self {
        Self{ inner }
    }
}

#[derive(Debug, Clone)]
pub struct TypeAlias {
    id: Tagged<String>,
    pub dest_type: Type,
}

impl TypeAlias {
    pub fn new(id: Tagged<String>, dest_type: Type) -> Self {
        Self{ id, dest_type }
    }
}

#[derive(Debug, Clone)]
pub struct Trait {
    inner: Pointer<crate::ast::Trait>
}

impl Trait {
    pub fn new(inner: Pointer<crate::ast::Trait>) -> Self {
        Self{ inner }
    }
}*/
