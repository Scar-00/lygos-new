use crate::{
    ffi::{self, to_cstr},
    module::Module,
};

pub struct Jit(*mut ());

impl Jit {
    pub fn new() -> Self {
        Self::try_new().expect("LLVM could not create LLJIT")
    }

    pub fn try_new() -> Result<Self, String> {
        let ptr = unsafe { ffi::CreateJitEngine() };
        if ptr.is_null() {
            Err("LLVM could not create LLJIT".into())
        } else {
            Ok(Self(ptr))
        }
    }

    pub fn last_error(&self) -> String {
        unsafe { ffi::JitEngineError(self.0) }.into()
    }

    /// Copy the JIT's host triple and layout before generating target-dependent IR.
    pub fn configure_module(&self, module: &mut Module<'_>) {
        unsafe { ffi::JitEngineConfigureModule(self.0, module.ptr) };
    }

    /// Register a host function for modules added to this engine.
    ///
    /// # Safety
    /// The address must have the ABI declared by the module and remain valid
    /// until this engine is destroyed.
    pub unsafe fn define_symbol(&mut self, name: &str, address: *mut ()) -> Result<(), String> {
        if unsafe { ffi::JitEngineDefineSymbol(self.0, to_cstr!(name), address) } {
            Ok(())
        } else {
            Err(self.last_error())
        }
    }

    pub fn add_module(&mut self, module: Module) -> Result<(), String> {
        let ptr = module.ptr;
        std::mem::forget(module);
        if unsafe { ffi::JitEngineAddModule(self.0, ptr) } {
            Ok(())
        } else {
            Err(self.last_error())
        }
    }

    pub fn lookup(&mut self, name: impl AsRef<str>) -> Option<*mut ()> {
        self.lookup_address(name)
    }

    pub fn lookup_address(&self, name: impl AsRef<str>) -> Option<*mut ()> {
        let name = name.as_ref();
        let res = unsafe { ffi::JitEngineLookup(self.0, to_cstr!(name)) };
        if res.is_null() { None } else { Some(res) }
    }
}

impl Drop for Jit {
    fn drop(&mut self) {
        unsafe { ffi::DestroyJitEngine(self.0) };
    }
}

impl Default for Jit {
    fn default() -> Self {
        Self::new()
    }
}
