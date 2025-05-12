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
        Self{ loc, path: path.to_string() }
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
        Self{ loc, typ, is_ref, is_mut }
    }

    pub fn set_type(&mut self, typ: Type) {
        *self.typ = typ;
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
        Self{ loc, typ: Box::new(typ), elems }
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
        Self{ loc, params, ret_type: Box::new(ret_type) }
    }
}

#[derive(Debug, Clone)]
pub struct ArraySlice {
    pub loc: Location,
    pub typ: Box<Type>,
}

impl ArraySlice {
    pub fn new(loc: Location, typ: Type) -> ArraySlice {
        Self{ loc, typ: Box::new(typ) }
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
            Type::FuncPtr(_) => todo!(),
        }
    }

    pub fn get_full_name(&self) -> String {
        match self {
            Type::Path(p) => p.path.clone(),
            Type::Pointer(ptr) => {
                let prefix = if ptr.is_ref { "&" } else { "*" }.to_owned();
                prefix + &ptr.typ.get_full_name()
            },
            Type::Array(arr) => {
                format!("[{};{}]", arr.typ.get_full_name(), arr.elems)
            },
            Type::Slice(slice) => {
                "[".to_owned() + &slice.typ.get_full_name() + "]"
            },
            Type::FuncPtr(func) => {
                let mut string = "fn (".to_string();
                func.params.iter().for_each(|param| {
                    string += &(param.get_full_name() + (", "));
                });
                string += &format!(") -> {}", func.ret_type.get_full_name());
                string
            }
        }
    }

    pub fn matches(&self, rhs: &Type) -> bool {
        match (self, rhs) {
            (Type::Pointer(lhs), Type::Pointer(rhs)) => {
                lhs.typ.matches(&rhs.typ)
            }
            _ => self.get_full_name() == rhs.get_full_name(),
        }
    }

    pub fn from_string(src: Tagged<String>) -> Option<Type> {
        let mut s = src.inner().as_str();
        let loc = src.loc().clone();
        if s.starts_with('*') || s.starts_with('&') {
            let is_ref = s.starts_with('&');
            s = &s[..1];
            let is_mut = if s.starts_with("mut") {
                s = &s[..3];
                true
            }else {
                false
            };

            if let Some(ty) = Self::from_string(Tagged::new(loc.clone(), s.to_string())) {
                return Some(Type::Pointer(Pointer::new(loc.clone(), Box::new(ty), is_ref, is_mut)));
            }
            return None;
        }
        Some(Type::Path(Path::new(loc.clone(), s.to_owned())))
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
