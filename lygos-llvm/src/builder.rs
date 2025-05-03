use std::marker::PhantomData;

use crate::{context::Context, ffi::{self, to_cstr, CastOps}, types::TypeRef, value::{BasicBlock, ValueRef}};

pub struct IRBuilder<'ctx> {
    phantom: PhantomData<&'ctx Context>,
    ptr: *mut (),
}

impl<'a> IRBuilder<'a> {
    pub fn new(ctx: &'a Context) -> Self {
        Self{ ptr: unsafe { ffi::CreateIRBuilder(ctx.ptr) }, phantom: PhantomData{} }
    }

    pub fn set_insert_point(&self, bb: &BasicBlock) {
        unsafe{ ffi::BuilderSetInsertPoint(self.ptr, bb.0) };
    }

    pub fn get_insert_block(&self) -> BasicBlock {
        BasicBlock(unsafe{ ffi::BuilderGetInsertBlock(self.ptr) })
    }

    /*pub fn save_ip(&self) -> InsertPoint {
        return InsertPoint{ 0: unsafe{ ffi::BuilderSaveInsertPoint(self.ptr) } };
    }

    pub fn restore_ip(&self, ip: &InsertPoint) {
        unsafe{ BuilderRestoreInsertPoint(self.ptr, ip.0) };
    }*/

    pub fn create_store(&self, value: &ValueRef, ptr: &ValueRef) -> ValueRef {
        ValueRef::new(unsafe{ ffi::BuilderCreateStore(self.ptr, value.0, ptr.0) })
    }

    pub fn create_load(&self, ty: &TypeRef, value: &ValueRef) -> ValueRef {
        ValueRef::new(unsafe{ ffi::BuilderCreateLoad(self.ptr, ty.0, value.0) })
    }

    pub fn create_alloca(&self, ty: &TypeRef, array_size: Option<&ValueRef>) -> ValueRef {
        let parent = if let Some(v) = array_size { v.0 } else { std::ptr::null_mut() };
        ValueRef::new(unsafe{ ffi::BuilderCreateAlloca(self.ptr, ty.0, parent) })
    }

    pub fn create_ret(&self, value: Option<&ValueRef>) -> ValueRef {
        ValueRef::new(if let Some(value) = value {
            unsafe{ ffi::BuilderCreateRet(self.ptr, value.0) }
        } else {
            unsafe{ ffi::BuilderCreateRetVoid(self.ptr) }
        })
    }

    pub fn create_call(&self, func: &ValueRef, args: &[ValueRef]) -> ValueRef {
        let mut args: Vec<*mut ()> = args.iter().map(|arg| arg.0).collect();
        ValueRef::new(unsafe{ ffi::BuilderCreateCall(self.ptr, func.0, args.as_mut_ptr(), args.len()) })
    }

    pub fn create_ptr_call(&self, ty: &TypeRef, func: &ValueRef, args: &[ValueRef]) -> ValueRef {
        let mut args: Vec<*mut ()> = args.iter().map(|arg| arg.0).collect();
        ValueRef::new(unsafe{ ffi::BuilderCreatePtrCall(self.ptr, ty.0, func.0, args.as_mut_ptr(), args.len()) })
    }

    pub fn create_gep(&self, base: &TypeRef, val: &ValueRef, idx_list: &[ValueRef], in_bounds: bool) -> ValueRef {
        let mut idx_list: Vec<*mut ()> = idx_list.iter().map(|arg| arg.0).collect();
        ValueRef::new(unsafe{ ffi::BuilderCreateGEP(self.ptr, base.0, val.0, idx_list.as_mut_ptr(), idx_list.len(), in_bounds) })
    }

    pub fn create_struct_gep(&self, base: &TypeRef, val: &ValueRef, index: u32) -> ValueRef {
        ValueRef::new(unsafe{ ffi::BuilderCreateStructGEP(self.ptr, base.0, val.0, index) })
    }

    pub fn create_xor_vv(&self, lhs: &ValueRef, rhs: &ValueRef) -> ValueRef {
        ValueRef::new(unsafe{ ffi::BuilderCreateXorVV(self.ptr, lhs.0, rhs.0) })
    }

    pub fn create_xor_vl(&self, lhs: &ValueRef, rhs: u64) -> ValueRef {
        ValueRef::new(unsafe{ ffi::BuilderCreateXorVL(self.ptr, lhs.0, rhs) })
    }

    pub fn create_cast(&self, op: CastOps, src: &ValueRef, dest_ty: &TypeRef) -> ValueRef {
        ValueRef::new(unsafe{ ffi::BuilderCreateCast(self.ptr, op, src.0, dest_ty.0) })
    }

    pub fn create_pointer_cast(&self, value: &ValueRef, dest_ty: &TypeRef) -> ValueRef {
        ValueRef::new(unsafe{ ffi::BuilderCreatePointerCast(self.ptr, value.0, dest_ty.0) })
    }

    pub fn create_br(&self, bb: &BasicBlock) -> ValueRef {
        ValueRef::new(unsafe{ ffi::BuilderCreateBr(self.ptr, bb.0) })
    }

    pub fn create_cond_br(&self, cond: &ValueRef, true_bb: &BasicBlock, false_bb: &BasicBlock) -> ValueRef {
        ValueRef::new(unsafe{ ffi::BuilderCreateCondBr(self.ptr, cond.0, true_bb.0, false_bb.0) })
    }

    pub fn create_add(&self, lhs: &ValueRef, rhs: &ValueRef) -> ValueRef {
        ValueRef::new(unsafe{ ffi::BuilderCreateAdd(self.ptr, lhs.0, rhs.0) })
    }

    pub fn create_sub(&self, lhs: &ValueRef, rhs: &ValueRef) -> ValueRef {
        ValueRef::new(unsafe{ ffi::BuilderCreateSub(self.ptr, lhs.0, rhs.0) })
    }

    pub fn create_mul(&self, lhs: &ValueRef, rhs: &ValueRef) -> ValueRef {
        ValueRef::new(unsafe{ ffi::BuilderCreateMul(self.ptr, lhs.0, rhs.0) })
    }

    pub fn create_div(&self, lhs: &ValueRef, rhs: &ValueRef) -> ValueRef {
        ValueRef::new(unsafe{ ffi::BuilderCreateDiv(self.ptr, lhs.0, rhs.0) })
    }

    pub fn create_rem(&self, lhs: &ValueRef, rhs: &ValueRef) -> ValueRef {
        ValueRef::new(unsafe{ ffi::BuilderCreateRem(self.ptr, lhs.0, rhs.0) })
    }

    pub fn create_icmp_eq(&self, lhs: &ValueRef, rhs: &ValueRef) -> ValueRef {
        ValueRef::new(unsafe{ ffi::BuilderCreateICmpEQ(self.ptr, lhs.0, rhs.0) })
    }

    pub fn create_icmp_ne(&self, lhs: &ValueRef, rhs: &ValueRef) -> ValueRef {
        ValueRef::new(unsafe{ ffi::BuilderCreateICmpNE(self.ptr, lhs.0, rhs.0) })
    }

    pub fn create_icmp_slt(&self, lhs: &ValueRef, rhs: &ValueRef) -> ValueRef {
        ValueRef::new(unsafe{ ffi::BuilderCreateICmpSLT(self.ptr, lhs.0, rhs.0) })
    }

    pub fn create_icmp_sle(&self, lhs: &ValueRef, rhs: &ValueRef) -> ValueRef {
        ValueRef::new(unsafe{ ffi::BuilderCreateICmpSLE(self.ptr, lhs.0, rhs.0) })
    }

    pub fn create_icmp_sgt(&self, lhs: &ValueRef, rhs: &ValueRef) -> ValueRef {
        ValueRef::new(unsafe{ ffi::BuilderCreateICmpSGT(self.ptr, lhs.0, rhs.0) })
    }

    pub fn create_icmp_sge(&self, lhs: &ValueRef, rhs: &ValueRef) -> ValueRef {
        ValueRef::new(unsafe{ ffi::BuilderCreateICmpSGE(self.ptr, lhs.0, rhs.0) })
    }

    pub fn create_global_string_pointer<S: AsRef<str>>(&self, s: S) -> ValueRef {
        ValueRef::new(unsafe{ ffi::BuilderCreateGlobalStringPointer(self.ptr, to_cstr!(s.as_ref())) })
    }
}

impl Drop for IRBuilder<'_> {
    fn drop(&mut self) {
        unsafe { ffi::DestroyIRBuilder(self.ptr) };
    }
}
