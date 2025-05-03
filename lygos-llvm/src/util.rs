use crate::{ffi, types::TypeRef};

#[derive(Debug, Clone, Copy)]
pub struct TargetRef(pub(crate) *mut ());
#[derive(Debug, Clone, Copy)]
pub struct TargetMachineRef(pub(crate) *mut ());
#[derive(Debug, Clone, Copy)]
pub struct DataLayoutRef(pub(crate) *mut ());

impl DataLayoutRef {
    pub fn get_type_size_in_bits(&self, ty: &TypeRef) -> usize {
        unsafe{ ffi::DataLayoutGetTypeAllocSizeInBits(self.0, ty.0) }
    }
}
