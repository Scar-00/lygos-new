use std::ffi::c_char;

#[repr(C)]
pub struct FFIString {
    data: *mut u8,
    len: u64,
}

impl From<FFIString> for String {
    fn from(value: FFIString) -> Self {
        unsafe { String::from_raw_parts(value.data, (value.len - 1) as usize, value.len as usize) }
    }
}

macro_rules! to_cstr {
    ($s: expr) => {
        {
            use std::ffi::CString;
            CString::new($s).unwrap().into_raw()
        }
    };
}

pub(crate) use to_cstr;

type CStr = *const c_char;

#[repr(C)]
pub struct FuncArgs(pub(crate) *mut *mut (), pub(crate) usize);

#[repr(C)]
pub enum CastOps {
    SExt,
    ZExt,
    FPExt,
    Trunc,
    FPToSI,
    FPToUI,
    SIToFP,
    UIToFP,
    BitCast,
    FPTrunc,
    IntToPtr,
    PtrToInt,
    AddrSpaceCast,
}

#[link(name = "bindings")]
unsafe extern "C" {
    pub fn CreateContext() -> *mut ();
    pub fn DestroyContext(_: *mut ());
    pub fn ContextSetOpaquePointers(ctx: *mut (), enable: bool);
    pub fn GetTargetTriiple() -> FFIString;
    pub fn LookUpTarget(tt: CStr) -> *mut ();
    pub fn CreateTargetMachine(target: *mut (), tt: CStr, cpu: CStr, features: CStr) -> *mut ();
    pub fn CreateModule(name: CStr, ctx: *mut ()) -> *mut ();
    pub fn DestroyModule(_: *mut ());
    pub fn ModuleSetDataLayout(m: *mut (), machine: *mut ());
    pub fn ModuleSetTargetTripple(m: *mut (), tt: CStr);
    pub fn ModuleGetDataLayout(m: *mut ()) -> *mut ();

    pub fn DataLayoutGetTypeAllocSizeInBits(dl: *mut (), ty: *mut ()) -> usize;

    pub fn ModuleGetFunction(module: *mut (), name: CStr) -> *mut ();

    pub fn CreateIRBuilder(ctx: *mut ()) -> *mut ();
    pub fn DestroyIRBuilder(builder: *mut ());

    pub fn BuilderSetInsertPoint(builder: *mut (), bb: *mut ());
    pub fn BuilderGetInsertBlock(builder: *mut ()) -> *mut ();
    pub fn BuilderSaveInsertPoint(builder: *mut ()) -> *mut ();
    pub fn BuilderRestoreInsertPoint(builder: *mut (), ip: *mut ());

    pub fn BuilderCreateStore(builder: *mut (), value: *mut (), ptr: *mut ()) -> *mut ();
    pub fn BuilderCreateLoad(builder: *mut (), ty: *mut (), value: *mut ()) -> *mut ();
    pub fn BuilderCreateAlloca(builder: *mut (), ty: *mut (), array_size: *mut ()) -> *mut ();
    pub fn BuilderCreateRet(builder: *mut (), value: *mut ()) -> *mut ();
    pub fn BuilderCreateRetVoid(builder: *mut ()) -> *mut ();
    pub fn BuilderCreatePtrCall(builder: *mut (), ty: *mut (), callee: *mut (), args: *mut *mut (), args_size: usize) -> *mut ();
    pub fn BuilderCreateCall(builder: *mut (), callee: *mut (), args: *mut *mut (), args_size: usize) -> *mut ();
    pub fn BuilderCreateGEP(builder: *mut (), base: *mut (), val: *mut (), idx_list: *mut *mut (), idx_count: usize, in_bounds: bool) -> *mut ();
    pub fn BuilderCreateExtractValue(builder: *mut (), agg: *mut (), idx_list: *mut (), idx_count: usize) -> *mut ();
    pub fn BuilderCreateStructGEP(builder: *mut (), base: *mut (), val: *mut (), index: u32) -> *mut ();
    pub fn BuilderCreateXorVV(builder: *mut (), lhs: *mut (), rhs: *mut ()) -> *mut ();
    pub fn BuilderCreateXorVL(builder: *mut (), lhs: *mut (), rhs: u64) -> *mut ();
    pub fn BuilderCreateCast(builder: *mut (), op: CastOps, src: *mut (), dest_ty: *mut ()) -> *mut ();
    pub fn BuilderCreatePointerCast(builder: *mut (), value: *mut (), dest_ty: *mut ()) -> *mut ();
    pub fn BuilderCreateAdd(builder: *mut (), lhs: *mut (), rhs: *mut ()) -> *mut ();
    pub fn BuilderCreateSub(builder: *mut (), lhs: *mut (), rhs: *mut ()) -> *mut ();
    pub fn BuilderCreateMul(builder: *mut (), lhs: *mut (), rhs: *mut ()) -> *mut ();
    pub fn BuilderCreateDiv(builder: *mut (), lhs: *mut (), rhs: *mut ()) -> *mut ();
    pub fn BuilderCreateRem(builder: *mut (), lhs: *mut (), rhs: *mut ()) -> *mut ();
    pub fn BuilderCreateICmpEQ(builder: *mut (), lhs: *mut (), rhs: *mut ()) -> *mut ();
    pub fn BuilderCreateICmpNE(builder: *mut (), lhs: *mut (), rhs: *mut ()) -> *mut ();
    pub fn BuilderCreateICmpSLT(builder: *mut (), lhs: *mut (), rhs: *mut ()) -> *mut ();
    pub fn BuilderCreateICmpSLE(builder: *mut (), lhs: *mut (), rhs: *mut ()) -> *mut ();
    pub fn BuilderCreateICmpSGT(builder: *mut (), lhs: *mut (), rhs: *mut ()) -> *mut ();
    pub fn BuilderCreateICmpSGE(builder: *mut (), lhs: *mut (), rhs: *mut ()) -> *mut ();
    pub fn BuilderCreateGlobalStringPointer(builder: *mut (), string: CStr) -> *mut ();


    pub fn BuilderCreateBr(builder: *mut (), bb: *mut ()) -> *mut ();
    pub fn BuilderCreateCondBr(builder: *mut (), cond: *mut (), true_bb: *mut (), false_bb: *mut ()) -> *mut ();

    pub fn GetPointerType(ty: *mut(), address_space: u32) -> *mut ();

    pub fn GetIntType(ctx: *mut (), width: usize) -> *mut ();
    pub fn GetConstantInt(typ: *mut (), value: i32) -> *mut ();
    pub fn GetConstantFP(ty: *mut (), value: f64) -> *mut ();
    pub fn GetConstantStruct(ty: *mut (), constatns: *mut *mut (), constatns_len: usize) -> *mut ();
    pub fn GetConstantArr(ty: *mut (), values: *mut *mut (), values_len: usize) -> *mut ();

    pub fn FunctionTypeGet(ret: *mut (), params: *mut *mut (), len: usize, is_var_arg: bool) -> *mut ();

    pub fn StructTypeCreate(elems: *mut *mut (), len: usize, name: CStr, packed: bool) -> *mut ();
    pub fn StructTypeCreateOpaque(ctx: *mut (), name: CStr) -> *mut ();
    pub fn StructTypeSetBody(ty: *mut (), elems: *mut *mut (), len: usize, packed: bool);
    pub fn StructTypeGet(ctx: *mut (), elems: *mut *mut (), elems_count: usize, packed: bool) -> *mut ();

    pub fn ArrayTypeGet(ty: *mut (), num_elems: usize) -> *mut ();

    pub fn GetVoidType(ctx: *mut ()) -> *mut ();

    pub fn FloatTypeGet(ctx: *mut ()) -> *mut ();
    pub fn DoubleTypeGet(ctx: *mut ()) -> *mut ();

    pub fn TypeIsPointerType(ty: *mut ()) -> bool;
    pub fn TypeIsIntType(ty: *mut ()) -> bool;
    pub fn TypeIsFloatingPointType(ty: *mut ()) -> bool;
    pub fn TypeIsFloatType(ty: *mut ()) -> bool;
    pub fn TypeIsDoubleType(ty: *mut ()) -> bool;
    pub fn TypeIsStructType(ty: *mut ()) -> bool;
    pub fn TypeIsArrayType(ty: *mut ()) -> bool;
    pub fn TypeCanLossLesslyBitCast(ty: *mut (), dest: *mut ()) -> bool;
    pub fn TypeGetIntBitWidth(ty: *mut ()) -> u32;

    pub fn TypeGetContainedType(ty: *mut (), index: usize) -> *mut ();
    pub fn TypeGetNumContainedTypes(ty: *mut ()) -> usize;
    pub fn TypeMatches(lhs: *mut (), rhs: *mut ()) -> bool;

    //fn TypeTryGetPointerBase(ty: *mut ()) -> *mut ();

    pub fn FunctionCreate(typ: *mut (), name: CStr, module: *mut ()) -> *mut ();
    pub fn FunctionGetArgs(func: *mut ()) -> FuncArgs;
    pub fn FunctionVerify(func: *mut ()) -> bool;
    pub fn FunctionIsVarArg(func: *mut ()) -> bool;
    pub fn FunctionGetType(func: *mut ()) -> *mut ();


    pub fn GlobalVariableCreate(m: *mut (), ty: *mut (), is_constant: bool, val: *mut ()) -> *mut ();


    pub fn CreateBasicBlock(ctx: *mut (), name: CStr, parent: *mut (), before: *mut ()) -> *mut ();
    pub fn BasicBlockGetParent(bb: *mut ()) -> *mut ();
    pub fn BasicBlockGetTerminator(bb: *mut ()) -> *mut ();
    pub fn BasicBlockRemoveFromParent(bb: *mut ());
    pub fn BasicBlockHasNUses(bb: *mut (), uses: usize) -> bool;

    pub fn PrintModule(m: *mut ()) -> FFIString;

    pub fn PrintValue(value: *mut ()) -> FFIString;
    pub fn ValueType(value: *mut ()) -> *mut ();
    pub fn ValueIsConstatnt(value: *mut ()) -> bool;

    pub fn PrintType(ty: *mut ()) -> FFIString;

    pub fn EmitObjFile(path: CStr, m: *mut (), target_machine: *mut ()) -> bool;
    pub fn EmitAsmFile(path: CStr, m: *mut (), target_machine: *mut ()) -> bool;

    pub fn InitAll();
}
