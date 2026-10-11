use std::ffi::CString;

use crate::{ffi::{self, to_cstr}, module::Module, util::{TargetMachineRef, TargetRef}};

pub struct Context {
    pub(crate) ptr: *mut (),
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        unsafe { ffi::DestroyContext(self.ptr) };
    }
}

impl Context {
    pub fn new() -> Self {
        Self{ ptr: unsafe { ffi::CreateContext() } }
    }

    pub fn get_default_target_triple() -> String {
        unsafe { ffi::GetTargetTriiple() }.into()
    }

    pub fn lookup_target(tt: impl AsRef<str>) -> TargetRef {
        let str = CString::new(tt.as_ref()).unwrap();
        TargetRef(unsafe{ ffi::LookUpTarget(str.as_c_str().as_ptr()) })
    }

    pub fn create_target_machine<S: AsRef<str>>(target: TargetRef, tt: S, cpu: S, features: S) -> TargetMachineRef {
        let tt = to_cstr!(tt.as_ref());
        let cpu = to_cstr!(cpu.as_ref());
        let features = to_cstr!(features.as_ref());
        TargetMachineRef(unsafe { ffi::CreateTargetMachine(target.0, tt, cpu, features) })
    }

    pub fn emit_obj_file(p: impl AsRef<str>, m: &Module, tm: &TargetMachineRef) -> bool {
        unsafe{ ffi::EmitObjFile(to_cstr!(p.as_ref()), m.ptr, tm.0) }
    }

    pub fn emit_asm_file(p: impl AsRef<str>, m: &Module, tm: &TargetMachineRef) -> bool {
        unsafe{ ffi::EmitAsmFile(to_cstr!(p.as_ref()), m.ptr, tm.0) }
    }

    pub fn init_all() {
        static INIT: std::sync::Once = std::sync::Once::new();
        INIT.call_once(|| unsafe { ffi::InitAll() });
    }
}
