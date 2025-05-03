use std::marker::PhantomData;

use crate::{context::Context, ffi::{self, to_cstr}, util::{DataLayoutRef, TargetMachineRef}, value::Function};

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
            ptr: unsafe{ ffi::CreateModule(to_cstr!(name.as_ref()), ctx.ptr) },
            _data: PhantomData{},
        }
    }

    pub fn print(&self) -> String {
        unsafe{ ffi::PrintModule(self.ptr) }.into()
    }

    pub fn set_data_layout(&mut self, machine: &TargetMachineRef) {
        unsafe{ ffi::ModuleSetDataLayout(self.ptr, machine.0) };
    }

    pub fn set_target_triple(&mut self, tt: impl AsRef<str>) {
        unsafe{ ffi::ModuleSetTargetTripple(self.ptr, to_cstr!(tt.as_ref())) }
    }

    pub fn get_function(&self, name: impl AsRef<str>) -> Option<Function> {
        let ret = unsafe{ ffi::ModuleGetFunction(self.ptr, to_cstr!(name.as_ref())) };
        if ret.is_null() {
            None
        }else {
            Some(Function(ret))
        }
    }

    pub fn get_data_layout(&self) -> DataLayoutRef {
        DataLayoutRef(unsafe{ ffi::ModuleGetDataLayout(self.ptr) })
    }
}
