#include <llvm/IR/BasicBlock.h>
#include <llvm/IR/DerivedTypes.h>
#include <llvm/IR/Function.h>
#include <llvm/IR/Type.h>
#include <llvm/IR/Value.h>
#include <llvm/Support/raw_ostream.h>
#include <llvm/IR/IRBuilder.h>
#include <llvm/IR/LLVMContext.h>
#include <llvm/IR/Module.h>
#include <llvm/IR/Verifier.h>
#include <llvm/Support/CodeGen.h>
//#include <llvm/Support/Host.h>
#include <llvm/Support/TargetSelect.h>
#include <llvm/TargetParser/Host.h>
#include <llvm/Target/TargetMachine.h>
#include <llvm/IR/PassManager.h>
#include <llvm/Transforms/Scalar.h>
#include <llvm/Transforms/Utils.h>
#include <llvm/Passes/PassBuilder.h>
#include <llvm/Bitcode/BitcodeWriter.h>
#include <llvm/Bitstream/BitstreamWriter.h>
#include <llvm/IR/Mangler.h>
#include <llvm/IR/LegacyPassManager.h>
#include <llvm/IR/LegacyPassManagers.h>
#include <llvm/Transforms/Scalar.h>
#include <llvm/Support/TargetSelect.h>
#include <llvm/Target/TargetMachine.h>
#include <llvm/MC/TargetRegistry.h>
#include <llvm/Linker/Linker.h>

struct FFIString {
    char *data;
    size_t len;
};

template<typename T>
struct FFIArray {
    T *data;
    size_t len;
};

template<typename T>
FFIArray<T> FFIArray_Create(size_t size) {
    return { .data = malloc(size * sizeof(T)), .len = size };
}

FFIString FFIString_Create(std::string s) {
    FFIString str;
    str.len = s.size() + 1;
    str.data = (char *)calloc(str.len, sizeof(char));
    std::memcpy(str.data, s.data(), str.len * sizeof(char));
    return str;
}

enum ExternCastOps {
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
};

llvm::Instruction::CastOps convert_ops(ExternCastOps op) {
    switch(op) {
        case SExt: return llvm::Instruction::CastOps::SExt;
        case ZExt: return llvm::Instruction::CastOps::ZExt;
        case FPExt: return llvm::Instruction::CastOps::FPExt;
        case Trunc: return llvm::Instruction::CastOps::Trunc;
        case FPToSI: return llvm::Instruction::CastOps::FPToSI;
        case FPToUI: return llvm::Instruction::CastOps::FPToUI;
        case SIToFP: return llvm::Instruction::CastOps::SIToFP;
        case UIToFP: return llvm::Instruction::CastOps::UIToFP;
        case BitCast: return llvm::Instruction::CastOps::BitCast;
        case FPTrunc: return llvm::Instruction::CastOps::FPTrunc;
        case IntToPtr: return llvm::Instruction::CastOps::IntToPtr;
        case PtrToInt: return llvm::Instruction::CastOps::PtrToInt;
        case AddrSpaceCast: return llvm::Instruction::CastOps::AddrSpaceCast;
    }
    __builtin_unreachable();
}

extern "C" {
    llvm::LLVMContext *CreateContext() {
        return new llvm::LLVMContext();
    }

    void DestroyContext(llvm::LLVMContext *ctx) {
        delete ctx;
    }

    void ContextSetOpaquePointers(llvm::LLVMContext *ctx, bool enable) {
        (void)ctx;
        (void)enable;
        //ctx->setOpaquePointers(enable);
    }

    llvm::Module *CreateModule(const char *name, llvm::LLVMContext *ctx) {
        return new llvm::Module(name, *ctx);
    }

    void DestroyModule(llvm::Module *mod) {
        delete mod;
    }

    FFIString PrintModule(llvm::Module *mod) {
        std::string s;
        llvm::raw_string_ostream os(s);
        mod->print(os, nullptr);
        return FFIString_Create(s);
    }

    llvm::Function *ModuleGetFunction(llvm::Module *mod, const char *name) {
        return mod->getFunction(name);
    }

    void ModuleSetDataLayout(llvm::Module *mod, llvm::TargetMachine *machine) {
        mod->setDataLayout(machine->createDataLayout());
    }

    void ModuleSetTargetTripple(llvm::Module *mod, const char *tt) {
        mod->setTargetTriple(tt);
    }

    const llvm::DataLayout *ModuleGetDataLayout(llvm::Module *mod) {
        return &mod->getDataLayout();
    }

    size_t DataLayoutGetTypeAllocSizeInBits(llvm::DataLayout *dl, llvm::Type *ty) {
        return dl->getTypeSizeInBits(ty).getFixedValue();
    }

    llvm::IRBuilder<> *CreateIRBuilder(llvm::LLVMContext *ctx) {
        return new llvm::IRBuilder<>(*ctx);
    }

    void DestroyIRBuilder(llvm::IRBuilder<> *builder) {
        delete builder;
    }

    void BuilderSetInsertPoint(llvm::IRBuilder<> *builder, llvm::BasicBlock *bb) {
        builder->SetInsertPoint(bb);
    }

    llvm::BasicBlock *BuilderGetInsertBlock(llvm::IRBuilder<> *builder) {
        return builder->GetInsertBlock();
    }

    //todo release this
    llvm::IRBuilderBase::InsertPoint *BuilderSaveInsertPoint(llvm::IRBuilder<> *builder) {
        auto ip = (llvm::IRBuilderBase::InsertPoint *)malloc(sizeof(llvm::IRBuilderBase::InsertPoint));
        *ip = builder->saveIP();
        return ip;
    }

    void BuilderRestoreInsertPoint(llvm::IRBuilder<> *builder, llvm::IRBuilderBase::InsertPoint *ip) {
        builder->restoreIP(*ip);
    }

    llvm::Value *BuilderCreateLoad(llvm::IRBuilder<> *builder, llvm::Type *ty, llvm::Value *value, const char *debug_name) {
        return builder->CreateLoad(ty, value, debug_name);
    }

    llvm::Value *BuilderCreateStore(llvm::IRBuilder<> *builder, llvm::Value *value, llvm::Value *ptr) {
        return builder->CreateStore(value, ptr);
    }

    llvm::Value *BuilderCreateAlloca(llvm::IRBuilder<> *builder, llvm::Type *ty, llvm::Value *array_size) {
        return builder->CreateAlloca(ty, array_size);
    }

    llvm::Value *BuilderCreateRet(llvm::IRBuilder<> *builder, llvm::Value *value) {
        return builder->CreateRet(value);
    }

    llvm::Value *BuilderCreateRetVoid(llvm::IRBuilder<> *builder) {
        return builder->CreateRetVoid();
    }

    llvm::Value *BuilderCreatePtrCall(llvm::IRBuilder<> *builder, llvm::FunctionType *ty, llvm::Value *callee, llvm::Value **args, size_t args_size) {
        std::vector<llvm::Value*> args_arr;
        for(size_t i = 0; i < args_size; i++) {
            args_arr.push_back(args[i]);
        }
        return builder->CreateCall(ty, callee, args_arr);
    }

    llvm::Value *BuilderCreateCall(llvm::IRBuilder<> *builder, llvm::Function *callee, llvm::Value **args, size_t args_size) {
        std::vector<llvm::Value*> args_arr;
        for(size_t i = 0; i < args_size; i++) {
            args_arr.push_back(args[i]);
        }
        return builder->CreateCall(callee, args_arr);
    }

    llvm::Value *BuilderCreateGEP(llvm::IRBuilder<> *builder, llvm::Type *base, llvm::Value *val, llvm::Value **idx_list, size_t idx_count, bool in_bounds) {
        std::vector<llvm::Value*> IdxList;
        for(size_t i = 0; i < idx_count; i++) {
            IdxList.push_back(idx_list[i]);
        }
        return builder->CreateGEP(base, val, IdxList, "", in_bounds);
    }

    llvm::Value *BuilderCreateExtractValue(llvm::IRBuilder<> *builder, llvm::Value *agg, unsigned int *idx_list, size_t idx_count) {
        std::vector<unsigned int> IdxList;
        for(size_t i = 0; i < idx_count; i++) {
            IdxList.push_back(idx_list[i]);
        }
        //llvm::Intrinsic::expect
        //builder->CreateIntrinsic()
        return builder->CreateExtractValue(agg, IdxList);
    }

    llvm::Value *BuilderCreateStructGEP(llvm::IRBuilder<> *builder, llvm::Type *base, llvm::Value *val, unsigned int index) {
        return builder->CreateStructGEP(base, val, index);
    }

    llvm::Value *BuilderCreateXorVV(llvm::IRBuilder<> *builder, llvm::Value *lhs, llvm::Value *rhs) {
        return builder->CreateXor(lhs, rhs);
    }

    llvm::Value *BuilderCreateXorVL(llvm::IRBuilder<> *builder, llvm::Value *lhs, uint64_t rhs) {
        return builder->CreateXor(lhs, rhs);
    }

    llvm::Value *BuilderCreateCast(llvm::IRBuilder<> *builder, ExternCastOps op, llvm::Value *src, llvm::Type *dest_ty) {
        return builder->CreateCast(convert_ops(op), src, dest_ty);
    }

    llvm::Value *BuilderCreatePointerCast(llvm::IRBuilder<> *builder, llvm::Value *value, llvm::Type *dest_ty) {
        return builder->CreatePointerCast(value, dest_ty);
    }

    llvm::Value *BuilderCreateBr(llvm::IRBuilder<> *builder, llvm::BasicBlock *bb) {
        return builder->CreateBr(bb);
    }

    llvm::Value *BuilderCreateCondBr(llvm::IRBuilder<> *builder, llvm::Value *cond, llvm::BasicBlock *true_bb, llvm::BasicBlock *false_bb) {
        return builder->CreateCondBr(cond, true_bb, false_bb);
    }

    llvm::Value *BuilderCreateAdd(llvm::IRBuilder<> *builder, llvm::Value *lhs, llvm::Value *rhs) {
        return builder->CreateAdd(lhs, rhs);
    }

    llvm::Value *BuilderCreateSub(llvm::IRBuilder<> *builder, llvm::Value *lhs, llvm::Value *rhs) {
        return builder->CreateSub(lhs, rhs);
    }

    llvm::Value *BuilderCreateMul(llvm::IRBuilder<> *builder, llvm::Value *lhs, llvm::Value *rhs) {
        return builder->CreateMul(lhs, rhs);
    }

    llvm::Value *BuilderCreateDiv(llvm::IRBuilder<> *builder, llvm::Value *lhs, llvm::Value *rhs) {
        return builder->CreateSDiv(lhs, rhs);
    }

    llvm::Value *BuilderCreateRem(llvm::IRBuilder<> *builder, llvm::Value *lhs, llvm::Value *rhs) {
        return builder->CreateSRem(lhs, rhs);
    }

    llvm::Value *BuilderCreateICmpEQ(llvm::IRBuilder<> *builder, llvm::Value *lhs, llvm::Value *rhs) {
        return builder->CreateICmpEQ(lhs, rhs);
    }

    llvm::Value *BuilderCreateICmpNE(llvm::IRBuilder<> *builder, llvm::Value *lhs, llvm::Value *rhs) {
        return builder->CreateICmpNE(lhs, rhs);
    }

    llvm::Value *BuilderCreateICmpSLT(llvm::IRBuilder<> *builder, llvm::Value *lhs, llvm::Value *rhs) {
        return builder->CreateICmpSLT(lhs, rhs);
    }

    llvm::Value *BuilderCreateICmpSLE(llvm::IRBuilder<> *builder, llvm::Value *lhs, llvm::Value *rhs) {
        return builder->CreateICmpSLE(lhs, rhs);
    }

    llvm::Value *BuilderCreateICmpSGT(llvm::IRBuilder<> *builder, llvm::Value *lhs, llvm::Value *rhs) {
        return builder->CreateICmpSGT(lhs, rhs);
    }

    llvm::Value *BuilderCreateICmpSGE(llvm::IRBuilder<> *builder, llvm::Value *lhs, llvm::Value *rhs) {
        return builder->CreateICmpSGE(lhs, rhs);
    }

    llvm::Value *BuilderCreateGlobalStringPointer(llvm::IRBuilder<> *builder, const char *string) {
        return builder->CreateGlobalStringPtr(string);
    }

    llvm::Type *GetIntType(llvm::LLVMContext *ctx, size_t width) {
        return llvm::Type::getIntNTy(*ctx, width);
    }

    llvm::PointerType *GetPointerType(llvm::Type *type, unsigned int address_space) {
        return llvm::PointerType::get(type, address_space);
    }

    bool TypeIsPointerType(llvm::Type *ty) {
        return ty->isPointerTy();
    }

    bool TypeIsIntType(llvm::Type *ty) {
        return ty->isIntegerTy();
    }

    bool TypeIsFloatingPointType(llvm::Type *ty) {
        return ty->isFloatingPointTy();
    }

    bool TypeIsFloatType(llvm::Type *ty) {
        return ty->isFloatTy();
    }

    bool TypeIsDoubleType(llvm::Type *ty) {
        return ty->isDoubleTy();
    }

    bool TypeIsStructType(llvm::Type *ty) {
        return ty->isStructTy();
    }

    bool TypeIsArrayType(llvm::Type *ty) {
        return ty->isArrayTy();
    }

    bool TypeCanLossLesslyBitCast(llvm::Type *ty, llvm::Type *dest) {
        return ty->canLosslesslyBitCastTo(dest);
    }

    uint32_t TypeGetIntBitWidth(llvm::Type *ty) {
        return ty->getIntegerBitWidth();
    }

    llvm::Type *TypeGetContainedType(llvm::Type *ty, size_t index) {
        return ty->getContainedType(index);
    }

    size_t TypeGetNumContainedTypes(llvm::Type *ty) {
        return ty->getNumContainedTypes();
    }

    bool TypeMatches(llvm::Type *lhs, llvm::Type *rhs) {
        return lhs == rhs;
    }

    //this returns NULL on failure
    /*llvm::Type *TypeTryGetPointerBase(llvm::Type *ty) {
        if(!ty->isPointerTy())
            return nullptr;
        return ((llvm::PointerType *)ty)->getContainedType(0);
    }*/

    llvm::FunctionType *FunctionTypeGet(llvm::Type *ret, llvm::Type **params, size_t params_len, bool is_var_arg) {
        std::vector<llvm::Type*> params_arr;
        for(size_t i = 0; i < params_len; i++) {
            params_arr.push_back(params[i]);
        }
        return llvm::FunctionType::get(ret, params_arr, is_var_arg);
    }

    llvm::StructType *StructTypeCreate(llvm::Type **elems, size_t elems_count, const char *name, bool packed) {
        std::vector<llvm::Type*> elems_arr;
        for(size_t i = 0; i < elems_count; i++) {
            elems_arr.push_back(elems[i]);
        }
        return llvm::StructType::create(elems_arr, name, packed);
    }

    llvm::StructType *StructTypeCreateOpaque(llvm::LLVMContext *ctx,  const char *name) {
        return llvm::StructType::create(*ctx, name);
    }

    void StructTypeSetBody(llvm::StructType *self, llvm::Type **elems, size_t elems_count, bool packed) {
        std::vector<llvm::Type*> elems_arr;
        for(size_t i = 0; i < elems_count; i++) {
            elems_arr.push_back(elems[i]);
        }
        self->setBody(elems_arr, packed);
    }

    llvm::StructType *StructTypeGet(llvm::LLVMContext *ctx, llvm::Type **elems, size_t elems_count, bool packed) {
        std::vector<llvm::Type*> elems_arr;
        for(size_t i = 0; i < elems_count; i++) {
            elems_arr.push_back(elems[i]);
        }
        return llvm::StructType::get(*ctx, elems_arr, packed);
    }

    llvm::ArrayType *ArrayTypeGet(llvm::Type *ty, size_t elems_count) {
        return llvm::ArrayType::get(ty, elems_count);
    }

    llvm::Type *GetVoidType(llvm::LLVMContext *ctx) {
        return llvm::Type::getVoidTy(*ctx);
    }

    llvm::Type *FloatTypeGet(llvm::LLVMContext *ctx) {
        return llvm::Type::getFloatTy(*ctx);
    }

    llvm::Type *DoubleTypeGet(llvm::LLVMContext *ctx) {
        return llvm::Type::getDoubleTy(*ctx);
    }


    llvm::Function *FunctionCreate(llvm::FunctionType *type, const char *name, llvm::Module *mod) {
        return llvm::Function::Create(type, llvm::Function::LinkageTypes::ExternalLinkage, name, mod);
    }

    struct ArgsFn{ llvm::Value **data; size_t size; };

    ArgsFn FunctionGetArgs(llvm::Function *func) {
        ArgsFn args{ .data = (llvm::Value **)malloc(func->arg_size() * sizeof(llvm::Value *)), .size = func->arg_size() };
        auto fn_args = func->arg_begin();
        for(size_t i = 0; i < func->arg_size(); i++) {
            args.data[i] = &*fn_args + i;
        }
        return args;
    }

    bool FunctionVerify(llvm::Function *func) {
        return llvm::verifyFunction(*func, &llvm::errs());
    }

    bool FunctionIsVarArg(llvm::Function *func) {
        return func->isVarArg();
    }

    llvm::Type *FunctionGetType(llvm::Function *func) {
        return func->getType();
    }

    llvm::GlobalVariable *GlobalVariableCreate(llvm::Module *mod, llvm::Type *ty, bool is_constant, llvm::Constant *value) {
        auto v = value;
        if(v == nullptr) {
            v = llvm::ConstantAggregateZero::get(ty);
        }
        return new llvm::GlobalVariable(*mod, ty, is_constant, llvm::GlobalVariable::InternalLinkage, value);
    }

    llvm::Constant *GetConstantInt(llvm::Type *typ, int value) {
        return llvm::ConstantInt::get(typ, value);
    }

    llvm::Constant *GetConstantFP(llvm::Type *typ, double value) {
        return llvm::ConstantFP::get(typ, value);
    }

    llvm::Constant *GetConstantStruct(llvm::StructType *ty, llvm::Constant **constants, size_t constants_len) {
        std::vector<llvm::Constant*> constants_arr;
        for(size_t i = 0; i < constants_len; i++) {
            constants_arr.push_back(constants[i]);
        }
        return llvm::ConstantStruct::get(ty, constants_arr);
    }

    llvm::Constant *GetConstantArr(llvm::ArrayType *type, llvm::Constant **values, size_t values_len) {
        std::vector<llvm::Constant*> constants_arr;
        for(size_t i = 0; i < values_len; i++) {
            constants_arr.push_back(values[i]);
        }
        return llvm::ConstantArray::get(type, constants_arr);
    }

    void InitAll(void) {
        llvm::InitializeAllTargetInfos();
        llvm::InitializeAllTargets();
        llvm::InitializeAllTargetMCs();
        llvm::InitializeAllAsmPrinters();
        llvm::InitializeAllAsmParsers();
    }

    FFIString GetTargetTriiple(void) {
        auto tt = llvm::sys::getDefaultTargetTriple();
        return FFIString_Create(tt.data());
    }

    const llvm::Target *LookUpTarget(const char *tt) {
        std::string tt_string = tt;
        std::string err;
        auto target = llvm::TargetRegistry::lookupTarget(tt_string, err);
        if(!target)
            printf("[llvm-error]: %s\n", err.c_str());
        return target;
        return NULL;
    }

    llvm::TargetMachine *CreateTargetMachine(llvm::Target *target, const char *tt, const char *cpu, const char *features) {
        llvm::TargetOptions opt;
        auto rm = llvm::Reloc::PIC_;
        return target->createTargetMachine(tt, cpu, features, opt, rm);
    }

    llvm::BasicBlock *CreateBasicBlock(llvm::LLVMContext *ctx, const char *name, llvm::Function *parent, llvm::BasicBlock *before) {
        return llvm::BasicBlock::Create(*ctx, name, parent, before);
    }

    llvm::Function *BasicBlockGetParent(llvm::BasicBlock *bb) {
        return bb->getParent();
    }

    llvm::Instruction *BasicBlockGetTerminator(llvm::BasicBlock *bb) {
        return bb->getTerminator();
    }

    void BasicBlockRemoveFromParent(llvm::BasicBlock *bb) {
        bb->removeFromParent();
    }

    bool BasicBlockHasNUses(llvm::BasicBlock *bb, size_t uses) {
        return bb->hasNUses(uses);
    }

    FFIString PrintValue(llvm::Value *value) {
        std::string s;
        llvm::raw_string_ostream os(s);
        value->print(os);
        return FFIString_Create(s);
    }

    llvm::Type *ValueType(llvm::Value *value) {
        return value->getType();
    }

    bool ValueIsConstatnt(llvm::Value *value) {
        return llvm::isa<llvm::Constant>(value);
    }

    FFIString PrintType(llvm::Type *type) {
        std::string s;
        llvm::raw_string_ostream os(s);
        type->print(os);
        return FFIString_Create(s);
    }

    bool EmitObjFile(const char *path, llvm::Module *mod, llvm::TargetMachine *target_machine) {
        llvm::legacy::PassManager pass_manager;

        std::error_code e;
        llvm::raw_fd_ostream os{path, e};
        //
        if(target_machine->addPassesToEmitFile(pass_manager, os, nullptr, llvm::CodeGenFileType::ObjectFile/*llvm::CGFT_ObjectFile*/, false)) {
            return false;
        }
        pass_manager.run(*mod);
        os.flush();
        return true;
    }

    bool EmitAsmFile(const char *path, llvm::Module *mod, llvm::TargetMachine *target_machine) {
        llvm::legacy::PassManager pass_manager;

        std::error_code e;
        llvm::raw_fd_ostream os{path, e};
        if(target_machine->addPassesToEmitFile(pass_manager, os, nullptr, llvm::CodeGenFileType::ObjectFile)) {
            return false;
        }
        pass_manager.run(*mod);
        os.flush();
        return true;
    }
}
