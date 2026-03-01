use crate::{
    ffi::{self, to_cstr},
    module::Module,
};

pub struct Jit(*mut ());

impl Jit {
    pub fn new() -> Self {
        Self(unsafe { ffi::CreateJitEngine() })
    }

    pub fn add_module(&mut self, module: Module) {
        let ptr = module.ptr;
        std::mem::forget(module);
        unsafe { ffi::JitEngineAddModule(self.0, ptr) }
    }

    pub fn lookup(&mut self, name: impl AsRef<str>) -> Option<*mut ()> {
        let name = name.as_ref();
        let res = unsafe { ffi::JitEngineLookup(self.0, to_cstr!(name)) };
        if res.is_null() {
            None
        } else {
            Some(res)
        }
    }
}
