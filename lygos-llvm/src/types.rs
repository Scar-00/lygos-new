use std::env::VarError;

use libc::VMADDR_CID_ANY;

use crate::{context::Context, ffi::{self, to_cstr}};

#[derive(Debug, Clone, Copy)]
pub struct TypeRef(pub(crate) *mut ());

impl TypeRef {
    pub(crate) fn new(v: *mut ()) -> Self {
        Self(v)
    }

    pub fn get_int(ctx: &Context, width: usize) -> Self {
        Self(unsafe{ ffi::GetIntType(ctx.ptr, width) })
    }

    pub fn get_void(ctx: &Context) -> Self {
        Self(unsafe{ ffi::GetVoidType(ctx.ptr) })
    }

    pub fn get_ptr(base: Self, addr_space: u32) -> Self {
        Self(unsafe{ ffi::GetPointerType(base.0, addr_space) })
    }

    pub fn is_pointer_ty(&self) -> bool {
        unsafe{ ffi::TypeIsPointerType(self.0) }
    }

    pub fn is_int_ty(&self) -> bool {
        unsafe{ ffi::TypeIsIntType(self.0) }
    }

    pub fn is_float_ty(&self) -> bool {
        unsafe{ ffi::TypeIsFloatType(self.0) }
    }

    pub fn is_double_ty(&self) -> bool {
        unsafe{ ffi::TypeIsDoubleType(self.0) }
    }

    pub fn is_floating_point_ty(&self) -> bool {
        unsafe{ ffi::TypeIsFloatingPointType(self.0) }
    }

    pub fn is_struct_ty(&self) -> bool {
        unsafe{ ffi::TypeIsStructType(self.0) }
    }

    pub fn is_array_ty(&self) -> bool {
        unsafe{ ffi::TypeIsArrayType(self.0) }
    }

    pub fn can_losslessly_bit_cast_to(&self, dest: &TypeRef) -> bool {
        unsafe{ ffi::TypeCanLossLesslyBitCast(self.0, dest.0) }
    }

    pub fn get_int_bit_width(&self) -> Option<u32> {
        if !self.is_int_ty() {
            return None;
        }
        Some(unsafe{ ffi::TypeGetIntBitWidth(self.0) })
    }

    pub fn get_num_contained_types(&self) -> usize {
        unsafe{ ffi::TypeGetNumContainedTypes(self.0) }
    }

    pub fn get_contained_type(&self, index: usize) -> Self {
        Self(unsafe{ ffi::TypeGetContainedType(self.0, index) })
    }

    pub fn matches(&self, other: &TypeRef) -> bool {
        unsafe{ ffi::TypeMatches(self.0, other.0) }
    }

    pub fn print(&self) -> String {
        unsafe{ ffi::PrintType(self.0) }.into()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FunctionTypeRef(pub(crate) *mut ());

impl FunctionTypeRef {
    pub fn get(ret_type: TypeRef, params: &[TypeRef], is_var_arg: bool) -> Self {
        let mut params_ptr: Vec<*mut ()> = params.iter().map(|p| p.0).collect();
        FunctionTypeRef(unsafe{ ffi::FunctionTypeGet(ret_type.0, params_ptr.as_mut_ptr(), params_ptr.len(), is_var_arg) })
    }
}

impl From<FunctionTypeRef> for TypeRef {
    fn from(value: FunctionTypeRef) -> Self {
        Self(value.0)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct StructTypeRef(*mut ());

impl StructTypeRef {
    pub fn create(name: &str, elems: &[TypeRef], packed: bool) -> Self {
        let mut elems: Vec<*mut ()> = elems.iter().map(|p| p.0).collect();
        Self(unsafe{ ffi::StructTypeCreate(elems.as_mut_ptr(), elems.len(), to_cstr!(name), packed) })
    }

    pub fn create_opaque(ctx: &Context, name: &str) -> Self {
        Self(unsafe{ ffi::StructTypeCreateOpaque(ctx.ptr, to_cstr!(name)) })
    }

    pub fn set_body(&mut self, elems: &[TypeRef], packed: bool) {
        let mut elems: Vec<*mut ()> = elems.iter().map(|p| p.0).collect();
        unsafe{ ffi::StructTypeSetBody(self.0,elems.as_mut_ptr(), elems.len(), packed ) };
    }

    pub fn get(ctx: &Context, elems: &[TypeRef], packed: bool) -> Self {
        let mut elems: Vec<*mut ()> = elems.iter().map(|p| p.0).collect();
        Self(unsafe{ ffi::StructTypeGet(ctx.ptr, elems.as_mut_ptr(), elems.len(), packed) })
    }
}

impl From<StructTypeRef> for TypeRef {
    fn from(value: StructTypeRef) -> Self {
        Self(value.0)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ArrayTypeRef(*mut ());

impl ArrayTypeRef {
    pub fn get(ty: &TypeRef, elems_count: usize) -> Self {
        Self(unsafe{ ffi::ArrayTypeGet(ty.0, elems_count) })
    }
}

impl From<ArrayTypeRef> for TypeRef {
    fn from(value: ArrayTypeRef) -> Self {
        Self(value.0)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FloatTypeRef(*mut ());

impl FloatTypeRef {
    pub fn get(ctx: &Context) -> Self {
        Self(unsafe{ ffi::FloatTypeGet(ctx.ptr) })
    }
}

impl From<FloatTypeRef> for TypeRef {
    fn from(value: FloatTypeRef) -> Self {
        Self(value.0)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DoubleTypeRef(*mut ());

impl DoubleTypeRef {
    pub fn get(ctx: &Context) -> Self {
        Self(unsafe{ ffi::DoubleTypeGet(ctx.ptr) })
    }
}

impl From<DoubleTypeRef> for TypeRef {
    fn from(value: DoubleTypeRef) -> Self {
        Self(value.0)
    }
}
