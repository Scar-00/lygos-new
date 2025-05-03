use lygos_util::Pointer;
use crate::symbols::{self, Symbol, Variable, Function};
use lygos_token::Tagged;
use lygos_logging::{Logger, ErrorLabel};
use std::collections::HashMap;
use lygos_llvm::value::ValueRef;

#[derive(Debug)]
pub struct Scope {
    pub parent: Pointer<Scope>,
    symbol_table: HashMap<String, Symbol>,
    //known_symbols: HashSet<String>,
    return_value: Option<ValueRef>,
}

impl Scope {
    pub fn new() -> Self {
        Self { parent: Pointer::new(), symbol_table: HashMap::new(), /*known_symbols: HashSet::new(),*/ return_value: None }
    }

    pub fn with_parent(parent: &Scope) -> Self {
        Self{ parent: Pointer::from(parent), symbol_table: HashMap::new(), /*known_symbols: HashSet::new(),*/ return_value: None }
    }

    pub fn set_parent(&mut self, parent: &Scope) {
        self.parent = Pointer::from(parent);
    }

    pub fn get_return_alloc(&self) -> Option<&ValueRef> {
        if self.return_value.is_some() {
            return self.return_value.as_ref();
        }

        if self.parent.is_null() {
            return None;
        }

        self.parent.as_ref().get_return_alloc()
    }

    pub fn set_return_alloc(&mut self, alloc: ValueRef) {
        self.return_value = Some(alloc);
    }

    pub fn add_symbol<S: AsRef<str>>(&mut self, id: S, sym: Symbol) {
        let scope = self.resolve(id.as_ref());
        if let Some(symbol) = scope.symbol_table.get_mut(id.as_ref()) {
            if let Symbol::Function(func) = &symbol {
                if func.is_definition {
                    Logger::error_msg_labels(
                        format!("cannot redefine symbol `{}`", id.as_ref()), &[
                        ErrorLabel::from(symbol.loc(), "first defined here"),
                        ErrorLabel::from(sym.loc(), "later redefined here"),
                    ]);
                }
            }
        }
        self.symbol_table.insert(id.as_ref().to_string(), sym);
    }

    pub fn resolve_symbol<S: AsRef<str>>(&self, name: &Tagged<S>) -> &mut Symbol {
        let scope = self.resolve(name.inner().as_ref());
        assert!(scope.symbol_table.contains_key(name.inner().as_ref()), "{:#?}\n{:#?}", &*self.parent, name.inner().as_ref());
        scope.symbol_table.get_mut(name.inner().as_ref()).unwrap()
    }

    pub fn try_resolve_symbol<S: AsRef<str>>(&self, name: &Tagged<S>) -> Option<&mut Symbol> {
        if self.has_symbol(name.inner()) {
            return Some(self.resolve_symbol(name));
        }
        None
    }

    /*pub fn get_struct<S: AsRef<str>>(&self, name: &Tagged<S>) -> &mut symbol::Struct {
        if !self.has_symbol(name.inner()) {
            error_msg_label(
                format!("could not resolve struct type `{}`", name.inner().as_ref()).as_str(),
                ErrorLabel::from(&name.loc(), "unknwon type")
            );
        }
        /*if let Symbol::Struct(strct) = self.resolve_symbol(name) {
            return strct;
        }else {
            error_msg_label(
                format!("could not resolve struct type `{}`", name.inner().as_ref()).as_str(),
                ErrorLabel::from(&name.loc(), "unknwon type")
            );
        }*/
        match self.resolve_symbol(name) {
            Symbol::Struct(strct) => return strct,
            Symbol::TypeAlias(alias) => return self.get_struct(&Tagged::new(alias.dest_type.get_loc(), alias.dest_type.get_full_name())),
            _ => error_msg_label(
                format!("could not resolve struct type `{}`", name.inner().as_ref()).as_str(),
                ErrorLabel::from(&name.loc(), "unknwon type")
            ),
        }
    }*/

    pub fn get_function<S: AsRef<str>>(&self, name: &Tagged<S>) -> &mut Function {
        if !self.has_symbol(name.inner()) {
            Logger::error_msg_label(
                format!("could not resolve function `{}`", name.inner().as_ref()).as_str(),
                ErrorLabel::from(name.loc(), "unknwon function")
            );
        }
        if let Symbol::Function(func) = self.resolve_symbol(name) {
            func
        }else {
            Logger::error_msg_label(
                format!("could not resolve function `{}`", name.inner().as_ref()).as_str(),
                ErrorLabel::from(name.loc(), "unknwon function")
            );
        }
    }

    pub fn get_variable<S: AsRef<str>>(&self, name: &Tagged<S>) -> &mut Variable {
        if !self.has_symbol(name.inner()) {
            Logger::error_msg_label(
                format!("could not resolve variable `{}`", name.inner().as_ref()).as_str(),
                ErrorLabel::from(name.loc(), "unknwon variable")
            );
        }
        if let Symbol::Variable(var) = self.resolve_symbol(name) {
            var
        }else {
            Logger::error_msg_label(
                format!("could not resolve variable `{}`", name.inner().as_ref()).as_str(),
                ErrorLabel::from(name.loc(), "unknwon variable")
            );
        }
    }

    /*pub fn get_enum<S: AsRef<str>>(&self, name: &Tagged<S>) -> &mut symbol::Enum {
        if !self.has_symbol(name.inner()) {
            error_msg_label(
                format!("could not resolve enum `{}`", name.inner().as_ref()).as_str(),
                ErrorLabel::from(&name.loc(), "unknwon enum")
            );
        }
        if let Symbol::Enum(e) = self.resolve_symbol(name) {
            return e;
        }else {
            error_msg_label(
                format!("could not resolve enum `{}`", name.inner().as_ref()).as_str(),
                ErrorLabel::from(&name.loc(), "unknwon enum")
            );
        }
    }

    pub fn is_enum<S: AsRef<str>>(&self, name: &Tagged<S>) -> bool {
        if !self.has_symbol(name.inner()) {
            return false;
        }
        if let Symbol::Enum(_) = self.resolve_symbol(name) {
            return true;
        }
        return false;
    }

    pub fn is_struct<S: AsRef<str>>(&self, name: &Tagged<S>) -> bool {
        if !self.has_symbol(name.inner()) {
            return false;
        }
        if let Symbol::Struct(_) = self.resolve_symbol(name) {
            return true;
        }
        return false;
    }

    pub fn resolve_type(&self, typ: &Type, ctx: &crate::GenerationContext) -> llvm::TypeRef {
        return match typ {
            Type::Path(path) => {
                if let Some(ty) = crate::types::base_to_type(&path.path, ctx) {
                    return ty;
                }

                let scope = self.resolve(&path.path);
                if let Some(entry) = scope.symbol_table.get_mut(&path.path) {
                    match entry {
                        Symbol::Struct(s) => {
                            if let Some(ty) = &s.generated {
                                return ty.clone();
                            }else {
                                let mut ty = llvm::StructTypeRef::create_opaque(ctx.ctx, s.name.inner());
                                s.generated = Some(Into::<llvm::TypeRef>::into(ty.clone()));
                                let fields: Vec<llvm::TypeRef> = s.fields.iter().map(|f| self.resolve_type(&f.typ, ctx)).collect();
                                ty.set_body(&fields, false);
                                let ty = Into::<llvm::TypeRef>::into(ty);
                                s.generated = Some(ty.clone());
                                return ty;
                            }
                        }
                        Symbol::Enum(e) => {
                            return self.resolve_type(&e.typ, ctx);
                        }
                        Symbol::TypeAlias(a) => {
                            return self.resolve_type(&a.dest_type, ctx);
                        }
                        _ => Logger::error_msg_label(format!("unknown type `{:#?}`", path).as_str(), crate::log::ErrorLabel::from(&path.loc, "unknown type")),
                    }
                }
                Logger::error_msg_label(format!("unknown type `{}`", path.path).as_str(), crate::log::ErrorLabel::from(&path.loc, "unknown type"));
            },
            Type::Pointer(ptr) => {
                llvm::TypeRef::get_ptr(self.resolve_type(&ptr.typ, ctx), 0)
            }
            Type::Array(arr) => {
                return llvm::ArrayTypeRef::get(&self.resolve_type(&arr.typ, ctx), arr.elems).into();
            }
            Type::Slice(slice) => {
                return llvm::StructTypeRef::get(&ctx.ctx,
                        &[
                            self.resolve_type(&slice.typ, ctx),
                            llvm::TypeRef::get_int(&ctx.ctx, 64)
                        ],
                false)
                .into();
            }
            Type::FuncPtr(func) => {
                let params: Vec<llvm::TypeRef> = func.params.iter().map(|param| self.resolve_type(param, ctx)).collect();
                let func_ty = llvm::FunctionTypeRef::get(
                    self.resolve_type(&*func.ret_type, ctx),
                    &params,
                    false
                );
                return llvm::TypeRef::get_ptr(func_ty.into(), 0);
            }
        }
    }*/

    pub fn has_symbol<S: AsRef<str>>(&self, name: S) -> bool {
        let scope = self.resolve(name.as_ref());
        scope.symbol_table.contains_key(name.as_ref())
    }

    #[allow(clippy::mut_from_ref)]
    fn resolve(&self, id: &str) -> &mut Self {
        if self.symbol_table.contains_key(id) {
            return unsafe{lygos_util::to_mut(self)};
        }

        if self.parent.is_null() {
            return unsafe{lygos_util::to_mut(self)};
        }

        unsafe{lygos_util::to_mut(self)}.parent.as_mut().resolve(id)
    }

    /*pub fn check_structs(&self) {
        for entry in &self.symbol_table {
            if let Symbol::Struct(strct) = entry.1 {
                if !strct.resolved {
                    error_msg_label(
                        &format!("cannot resolve type `{}` in this scope", strct.name.inner()),
                        ErrorLabel::from(strct.name.loc(), "unknown type")
                    );
                }
            }
        }
    }*/
}
