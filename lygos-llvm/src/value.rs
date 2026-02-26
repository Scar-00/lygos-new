use std::usize;

use crate::{context::Context, ffi::{self, to_cstr}, module::Module, types::{FunctionTypeRef, TypeRef}};

pub use ffi::CastOps;

#[derive(Debug, Clone, Copy)]
pub struct ValueRef(pub(crate) *mut ());

impl ValueRef {
    pub(crate) fn new(v: *mut ()) -> Self {
        Self(v)
    }

    pub fn print(&self) -> String {
        unsafe{ ffi::PrintValue(self.0) }.into()
    }

    pub fn get_type(&self) -> TypeRef {
        TypeRef::new(unsafe{ ffi::ValueType(self.0) })
    }

    pub fn is_constant(&self) -> bool {
        unsafe{ ffi::ValueIsConstatnt(self.0) }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Function(pub(crate) *mut ());

impl Function {
    pub fn create(typ: FunctionTypeRef, name: &str, module: &Module) -> Self {
        Self(unsafe{ ffi::FunctionCreate(typ.0, to_cstr!(name), module.ptr) })
    }

    pub fn args(&self) -> Vec<ValueRef> {
        let ret = unsafe{ffi::FunctionGetArgs(self.0)};
        let vec = unsafe{Vec::from_raw_parts(ret.0, ret.1, ret.1)};
        vec.iter().map(|i| ValueRef::new(*i)).collect()
    }

    pub fn verify(&self) -> bool {
        unsafe{ ffi::FunctionVerify(self.0) }
    }

    pub fn is_var_arg(&self) -> bool {
        unsafe{ ffi::FunctionIsVarArg(self.0) }
    }

    pub fn get_type(&self) -> TypeRef {
        TypeRef::new(unsafe{ ffi::FunctionGetType(self.0) })
    }
}

impl From<Function> for ValueRef {
    fn from(value: Function) -> Self {
        ValueRef(value.0)
    }
}

impl From<ValueRef> for Function {
    fn from(value: ValueRef) -> Self {
        //TODO(S): check if this is actually a function ty
        //debug_assert(value.get_type())!
        Self(value.0)
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct GlobalVariable(*mut ());

impl GlobalVariable {
    pub fn new(m: &Module, typ: &TypeRef, value: Option<&ValueRef>, is_constant: bool) -> Self {
        let val = if let Some(v) = value { v.0 } else { std::ptr::null_mut() };
        Self(unsafe{ ffi::GlobalVariableCreate(m.ptr, typ.0, is_constant, val) })
    }
}

impl From<GlobalVariable> for ValueRef {
    fn from(value: GlobalVariable) -> Self {
        ValueRef(value.0)
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct BasicBlock(pub(crate) *mut ());

impl BasicBlock {
    pub fn new(ctx: &Context, name: &str, parent: Option<&Function>, insert_before: Option<&BasicBlock>) -> Self {
        let parent = if let Some(func) = parent { func.0 } else { std::ptr::null_mut() };
        let insert_before = if let Some(bb) = insert_before { bb.0 } else { std::ptr::null_mut() };
        Self(unsafe{ ffi::CreateBasicBlock(ctx.ptr, to_cstr!(name), parent, insert_before) })
    }

    pub fn get_parent(&self) -> Function {
        Function(unsafe{ ffi::BasicBlockGetParent(self.0) })
    }

    pub fn get_terminator(&self) -> Option<ValueRef> {
        let ret = unsafe{ ffi::BasicBlockGetTerminator(self.0) };
        if ret.is_null() {
            None
        }else {
            Some(ValueRef::new(ret))
        }
    }

    pub fn remove_from_parent(&self) {
        unsafe{ ffi::BasicBlockRemoveFromParent(self.0) };
    }

    pub fn has_n_uses(&self, uses: usize) -> bool {
        unsafe{ ffi::BasicBlockHasNUses(self.0, uses) }
    }
}

pub struct ConstantInt;
impl ConstantInt {
    pub fn get(typ: &TypeRef, value: i32) -> ValueRef {
        ValueRef(unsafe{ ffi::GetConstantInt(typ.0, value) })
    }
}

pub struct ConstantFP;
impl ConstantFP {
    pub fn get(typ: &TypeRef, value: f64) -> ValueRef {
        ValueRef(unsafe{ ffi::GetConstantFP(typ.0, value) })
    }
}

pub struct ConstantStruct;
impl ConstantStruct {
    pub fn get(typ: TypeRef, constants: &[ValueRef]) -> ValueRef {
        let mut constants: Vec<*mut ()> = constants.iter().map(|arg| arg.0).collect();
        ValueRef(unsafe{ ffi::GetConstantStruct(typ.0, constants.as_mut_ptr(), constants.len()) })
    }
}

pub struct ConstantArray;
impl ConstantArray {
    pub fn get(typ: TypeRef, values: &[ValueRef]) -> ValueRef {
        let mut constants: Vec<*mut ()> = values.iter().map(|arg| arg.0).collect();
        ValueRef(unsafe{ ffi::GetConstantArr(typ.0, constants.as_mut_ptr(), constants.len()) })
    }
}
