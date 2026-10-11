use lygos_token::{Location, Tagged};

lazy_static::lazy_static! {
    pub static ref BaseTypes: Vec<&'static str> = {
        vec![
            "i8",
            "i16",
            "i32",
            "i64",

            "u8",
            "u16",
            "u32",
            "u64",

            "f32",
            "f64",

            "bool",

            "str",

            "void",
        ]
    };
}

#[derive(Debug, Clone)]
pub struct Path {
    pub loc: Location,
    pub path: String,
}

impl Path {
    pub fn new(loc: Location, path: impl ToString) -> Path {
        Self {
            loc,
            path: path.to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Pointer {
    pub loc: Location,
    pub typ: Box<Type>,
    pub is_ref: bool,
    pub is_mut: bool,
}

impl Pointer {
    pub fn new(loc: Location, typ: Box<Type>, is_ref: bool, is_mut: bool) -> Pointer {
        Self {
            loc,
            typ,
            is_ref,
            is_mut,
        }
    }

    pub fn set_type(&mut self, typ: Type) {
        *self.typ = typ;
    }

    pub fn inner(&self) -> Type {
        *self.typ.clone()
    }
}

#[derive(Debug, Clone)]
pub struct Array {
    pub loc: Location,
    pub typ: Box<Type>,
    pub elems: usize,
}

impl Array {
    pub fn new(loc: Location, typ: Type, elems: usize) -> Array {
        Self {
            loc,
            typ: Box::new(typ),
            elems,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FuncPtr {
    pub loc: Location,
    pub params: Vec<Type>,
    pub ret_type: Box<Type>,
}

impl FuncPtr {
    pub fn new(loc: Location, params: Vec<Type>, ret_type: Type) -> FuncPtr {
        Self {
            loc,
            params,
            ret_type: Box::new(ret_type),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ArraySlice {
    pub loc: Location,
    pub typ: Box<Type>,
}

impl ArraySlice {
    pub fn new(loc: Location, typ: Type) -> ArraySlice {
        Self {
            loc,
            typ: Box::new(typ),
        }
    }
}

/*
 *  NOTE(S): variables that are allocated on the stack should probably be treated as pointers in the type
 *  system unlike literal values as they are right. This change would probably make the decicion on
 *  weather a value should be (auto)dereffernced much easier while at the same time invalidation a
 *  lot of assumptions currently made in the type system that stack allocated variables are allways
 *  literal values instead of pointers
 *
 */

/*
 *  FIXME: give every Path & FunctionPtr a unique id to compare them
 */

#[derive(Debug, Clone)]
pub enum Type {
    Path(Path),
    Pointer(Pointer),
    Array(Array),
    FuncPtr(FuncPtr),
    Slice(ArraySlice),
}

impl Type {
    pub fn get_loc(&self) -> Location {
        match self {
            Type::Path(p) => p.loc.clone(),
            Type::Pointer(ptr) => ptr.loc.clone(),
            Type::Array(arr) => arr.loc.clone(),
            Type::Slice(slice) => slice.loc.clone(),
            Type::FuncPtr(fn_ptr) => fn_ptr.loc.clone(),
        }
    }

    pub fn get_name(&self) -> String {
        match self {
            Type::Path(p) => p.path.clone(),
            Type::Pointer(ptr) => ptr.typ.get_name(),
            Type::Array(arr) => arr.typ.get_name(),
            Type::Slice(slice) => slice.typ.get_name(),
            Type::FuncPtr(_) => self.get_full_name(),
        }
    }

    pub fn get_full_name(&self) -> String {
        match self {
            Type::Path(p) => p.path.clone(),
            Type::Pointer(ptr) => {
                let prefix = if ptr.is_ref {
                    if ptr.is_mut { "&mut " } else { "&" }
                } else if ptr.is_mut {
                    "*mut "
                } else {
                    "*const "
                };
                prefix.to_owned() + &ptr.typ.get_full_name()
            }
            Type::Array(arr) => {
                format!("[{};{}]", arr.typ.get_full_name(), arr.elems)
            }
            Type::Slice(slice) => "[".to_owned() + &slice.typ.get_full_name() + "]",
            Type::FuncPtr(func) => {
                format!(
                    "fn ({}) -> {}",
                    func.params
                        .iter()
                        .map(Type::get_full_name)
                        .collect::<Vec<_>>()
                        .join(", "),
                    func.ret_type.get_full_name()
                )
            }
        }
    }

    /// Structural equality. Source locations are not part of a type's identity.
    pub fn matches(&self, rhs: &Type) -> bool {
        match (self, rhs) {
            (Type::Path(lhs), Type::Path(rhs)) => lhs.path == rhs.path,
            (Type::Pointer(lhs), Type::Pointer(rhs)) => {
                lhs.is_ref == rhs.is_ref && lhs.is_mut == rhs.is_mut && lhs.typ.matches(&rhs.typ)
            }
            (Type::Array(lhs), Type::Array(rhs)) => {
                lhs.elems == rhs.elems && lhs.typ.matches(&rhs.typ)
            }
            (Type::Slice(lhs), Type::Slice(rhs)) => lhs.typ.matches(&rhs.typ),
            (Type::FuncPtr(lhs), Type::FuncPtr(rhs)) => {
                lhs.params.len() == rhs.params.len()
                    && lhs
                        .params
                        .iter()
                        .zip(&rhs.params)
                        .all(|(lhs, rhs)| lhs.matches(rhs))
                    && lhs.ret_type.matches(&rhs.ret_type)
            }
            _ => false,
        }
    }

    /// Parse a path with optional pointer/reference prefixes. Compound types use
    /// the language parser; unsupported or incomplete spellings return None.
    pub fn from_string(src: Tagged<String>) -> Option<Type> {
        let s = src.inner().trim();
        let loc = src.loc().clone();
        if s.starts_with('*') || s.starts_with('&') {
            let is_ref = s.starts_with('&');
            let mut rest = s[1..].trim_start();
            if rest == "mut" {
                return None;
            }
            let is_mut = rest
                .strip_prefix("mut")
                .is_some_and(|suffix| suffix.starts_with(char::is_whitespace));
            let is_const = rest
                .strip_prefix("const")
                .is_some_and(|suffix| suffix.starts_with(char::is_whitespace));
            if !is_ref && !is_mut && !is_const {
                return None;
            }
            if is_ref && is_const {
                return None;
            }
            if is_mut {
                rest = rest[3..].trim_start();
            }
            if is_const {
                rest = rest[5..].trim_start();
            }
            let typ = Self::from_string(Tagged::new(loc.clone(), rest.to_owned()))?;
            return Some(Type::Pointer(Pointer::new(
                loc,
                Box::new(typ),
                is_ref,
                is_mut,
            )));
        }
        let mut chars = s.chars();
        if !chars
            .next()
            .is_some_and(|ch| ch.is_alphabetic() || ch == '_')
            || !chars.all(|ch| ch.is_alphanumeric() || ch == '_')
        {
            return None;
        }
        Some(Type::Path(Path::new(loc, s)))
    }

    pub fn get_base(&self) -> Option<&Type> {
        match self {
            Type::Pointer(ptr) => Some(&ptr.typ),
            Type::Array(arr) => Some(&arr.typ),
            Type::Slice(slice) => Some(&slice.typ),
            _ => None,
        }
    }

    pub fn is_pointer_like(&self) -> bool {
        matches!(self, Self::Pointer(_))
    }
}
