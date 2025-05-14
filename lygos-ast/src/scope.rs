use lygos_llvm::types::{ArrayTypeRef, DoubleTypeRef, FloatTypeRef, FunctionTypeRef, StructTypeRef};
use lygos_util::Pointer as CPointer;
use crate::symbols::{self, Function, Struct, Symbol, Variable};
use crate::types::*;
use lygos_token::Tagged;
use lygos_logging::{Logger, ErrorLabel};
use std::collections::HashMap;
use lygos_llvm::{context::Context, module::Module as LLVMModule, builder::IRBuilder, value::{BasicBlock, ValueRef}, types::TypeRef};

pub struct GenerationContext<'a> {
    pub ctx: &'a Context,
    pub module: &'a LLVMModule<'a>,
    pub builder: &'a IRBuilder<'a>,
    pub current_function: CPointer<crate::Function>,
    pub current_break_point: Vec<CPointer<BasicBlock>>,
}

impl<'a> GenerationContext<'a> {
    pub fn new(ctx: &'a Context, module: &'a LLVMModule<'a>, builder: &'a IRBuilder<'a>) -> GenerationContext<'a> {
        Self{ ctx, module, builder, current_function: CPointer::new(), current_break_point: Vec::new() }
    }
}

#[derive(Debug)]
pub struct Scope {
    pub parent: CPointer<Scope>,
    symbol_table: HashMap<String, Symbol>,
    //known_symbols: HashSet<String>,
    return_value: Option<ValueRef>,
}

impl Scope {
    pub fn new() -> Self {
        Self { parent: CPointer::new(), symbol_table: HashMap::new(), /*known_symbols: HashSet::new(),*/ return_value: None }
    }

    pub fn with_parent(parent: &Scope) -> Self {
        Self{ parent: CPointer::from(parent), symbol_table: HashMap::new(), /*known_symbols: HashSet::new(),*/ return_value: None }
    }

    pub fn base_to_type(ty: impl AsRef<str>, ctx: &crate::GenerationContext) -> Option<TypeRef> {
        let ty = ty.as_ref();
        if !BaseTypes.contains(&ty) {
            return None;
        }

        match ty {
            "i8" => Some(TypeRef::get_int(ctx.ctx, 8)),
            "i16" => Some(TypeRef::get_int(ctx.ctx, 16)),
            "i32" => Some(TypeRef::get_int(ctx.ctx, 32)),
            "i64" => Some(TypeRef::get_int(ctx.ctx, 64)),

            "u8" => Some(TypeRef::get_int(ctx.ctx, 8)),
            "u16" => Some(TypeRef::get_int(ctx.ctx, 16)),
            "u32" => Some(TypeRef::get_int(ctx.ctx, 32)),
            "u64" => Some(TypeRef::get_int(ctx.ctx, 64)),

            "f32" => Some(FloatTypeRef::get(ctx.ctx).into()),
            "f64" => Some(DoubleTypeRef::get(ctx.ctx).into()),

            "bool" => Some(TypeRef::get_int(ctx.ctx, 1)),
            "str" => {
                Some(StructTypeRef::get(ctx.ctx,
                            &[
                                TypeRef::get_ptr(TypeRef::get_int(ctx.ctx, 8), 0),
                                TypeRef::get_int(ctx.ctx, 64)
                            ],
                            false).into())
            },

            "void" => Some(TypeRef::get_void(ctx.ctx)),

            _ => None,
        }
    }

    pub fn set_parent(&mut self, parent: &Scope) {
        self.parent = CPointer::from(parent);
    }

    pub fn get_return_alloc(&self) -> Option<ValueRef> {
        if self.return_value.is_some() {
            return self.return_value;
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

    pub fn get_struct<S: AsRef<str>>(&self, name: &Tagged<S>) -> &mut Struct {
        if !self.has_symbol(name.inner()) {
            Logger::error_msg_label(
                format!("could not resolve struct type `{}`", name.inner().as_ref()).as_str(),
                ErrorLabel::from(name.loc(), "unknwon type")
            );
        }
        match self.resolve_symbol(name) {
            Symbol::Struct(strct) => strct,
            Symbol::TypeAlias(alias) => self.get_struct(&Tagged::new(alias.dest_type.get_loc(), alias.dest_type.get_full_name())),
            _ => Logger::error_msg_label(
                format!("could not resolve struct type `{}`", name.inner().as_ref()).as_str(),
                ErrorLabel::from(name.loc(), "unknwon type")
            ),
        }
    }

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
    }*/

    pub fn is_struct<S: AsRef<str>>(&self, name: &Tagged<S>) -> bool {
        if !self.has_symbol(name.inner()) {
            return false;
        }
        if let Symbol::Struct(_) = self.resolve_symbol(name) {
            return true;
        }
        false
    }

    #[allow(clippy::only_used_in_recursion)]
    pub fn resolve_type(&self, typ: &Type, ctx: &crate::GenerationContext) -> TypeRef {
        match typ {
            Type::Path(path) => {
                if let Some(ty) = Self::base_to_type(&path.path, ctx) {
                    return ty;
                }
                let scope = self.resolve(&path.path);
                if let Some(entry) = scope.symbol_table.get_mut(&path.path) {
                    match entry {
                        Symbol::Struct(s) => {
                            if let Some(ty) = &s.generated {
                                return *ty;
                            }else {
                                let mut ty = StructTypeRef::create_opaque(ctx.ctx, s.name.inner());
                                s.generated = Some(Into::<TypeRef>::into(ty));
                                let fields: Vec<TypeRef> = s.fields.iter().map(|f| self.resolve_type(&f.typ, ctx)).collect();
                                ty.set_body(&fields, false);
                                let ty = Into::<TypeRef>::into(ty);
                                s.generated = Some(ty);
                                return ty;
                            }
                        }
                        /*Symbol::Enum(e) => {
                            return self.resolve_type(&e.typ, ctx);
                        }*/
                        Symbol::TypeAlias(a) => {
                            return self.resolve_type(&a.dest_type, ctx);
                        }
                        _ => Logger::error_msg_label(format!("unknown type `{:#?}`", path), ErrorLabel::from(&path.loc, "unknown type")),
                    }
                }
                Logger::error_msg_label(format!("unknown type `{}`", path.path), ErrorLabel::from(&path.loc, "unknown type"));
            },
            Type::Pointer(ptr) => {
                TypeRef::get_ptr(self.resolve_type(&ptr.typ, ctx), 0)
            }
            Type::Array(arr) => {
                ArrayTypeRef::get(&self.resolve_type(&arr.typ, ctx), arr.elems).into()
            }
            Type::Slice(slice) => {
                StructTypeRef::get(ctx.ctx,
                        &[
                            self.resolve_type(&slice.typ, ctx),
                            TypeRef::get_int(ctx.ctx, 64)
                        ],
                false)
                .into()
            }
            Type::FuncPtr(func) => {
                let params: Vec<TypeRef> = func.params.iter().map(|param| self.resolve_type(param, ctx)).collect();
                let func_ty = FunctionTypeRef::get(
                    self.resolve_type(&func.ret_type, ctx),
                    &params,
                    false
                );
                TypeRef::get_ptr(func_ty.into(), 0)
            }
        }
    }

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
