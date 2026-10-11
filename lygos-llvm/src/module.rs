use std::{marker::PhantomData, path::Path};

use crate::{
    context::Context,
    ffi::{self, to_cstr},
    util::{DataLayoutRef, TargetMachineRef},
    value::{Function, ValueRef},
};

pub struct Module<'ctx> {
    pub(crate) ptr: *mut (),
    _data: PhantomData<&'ctx Context>,
}

impl Drop for Module<'_> {
    fn drop(&mut self) {
        unsafe { ffi::DestroyModule(self.ptr) };
    }
}

impl<'ctx> Module<'ctx> {
    pub fn new(name: impl AsRef<str>, ctx: &'ctx Context) -> Self {
        Self {
            ptr: unsafe { ffi::CreateModule(to_cstr!(name.as_ref()), ctx.ptr) },
            _data: PhantomData {},
        }
    }

    pub fn from_file(path: impl AsRef<Path>, ctx: &'ctx Context) -> Option<Self> {
        let path = path.as_ref().display().to_string();
        let res = unsafe { ffi::CreateModule(to_cstr!(path), ctx.ptr) };
        if res.is_null() {
            return None;
        }
        Some(Self {
            ptr: res,
            _data: PhantomData {},
        })
    }

    pub fn print(&self) -> String {
        unsafe { ffi::PrintModule(self.ptr) }.into()
    }

    /// Create a string constant even when there is no current function block.
    pub fn create_global_string(&self, value: &str) -> ValueRef {
        let bytes = value.as_bytes();
        ValueRef::new(unsafe {
            ffi::ModuleCreateGlobalStringBytes(self.ptr, bytes.as_ptr(), bytes.len())
        })
    }

    pub fn set_data_layout(&mut self, machine: &TargetMachineRef) {
        unsafe { ffi::ModuleSetDataLayout(self.ptr, machine.0) };
    }

    pub fn set_target_triple(&mut self, tt: impl AsRef<str>) {
        unsafe { ffi::ModuleSetTargetTripple(self.ptr, to_cstr!(tt.as_ref())) }
    }

    pub fn get_function(&self, name: impl AsRef<str>) -> Option<Function> {
        let ret = unsafe { ffi::ModuleGetFunction(self.ptr, to_cstr!(name.as_ref())) };
        if ret.is_null() {
            None
        } else {
            Some(Function(ret))
        }
    }

    pub fn get_data_layout(&self) -> DataLayoutRef {
        DataLayoutRef(unsafe { ffi::ModuleGetDataLayout(self.ptr) })
    }
}
