//! LLVM lowering consumes only the immutable, resolved typed program.
use lygos_ast::typed::*;
use lygos_llvm::{
    builder::{BinaryOp as LlvmBinaryOp, IRBuilder},
    context::Context,
    module::Module,
    types::{ArrayTypeRef, DoubleTypeRef, FloatTypeRef, FunctionTypeRef, StructTypeRef, TypeRef},
    value::{
        BasicBlock, CastOps, ConstantFP, ConstantInt, ConstantStruct, Function, GlobalVariable,
        ValueRef,
    },
};
use std::collections::HashMap;

pub struct CodeGenerator<'a> {
    ctx: &'a Context,
    module: &'a Module<'a>,
    builder: &'a IRBuilder<'a>,
}
impl<'a> CodeGenerator<'a> {
    pub fn new(ctx: &'a Context, module: &'a Module<'a>, builder: &'a IRBuilder<'a>) -> Self {
        Self {
            ctx,
            module,
            builder,
        }
    }
    pub fn generate(&self, program: &TypedModule) -> Result<(), String> {
        Lowering {
            ctx: self.ctx,
            module: self.module,
            builder: self.builder,
            ir: program.context(),
            types: HashMap::new(),
            values: HashMap::new(),
            functions: HashMap::new(),
            breaks: Vec::new(),
        }
        .module(program)
    }
}

struct Lowering<'a, 'ir> {
    ctx: &'a Context,
    module: &'a Module<'a>,
    builder: &'a IRBuilder<'a>,
    ir: &'ir SemanticContext,
    types: HashMap<TypeId, TypeRef>,
    values: HashMap<SymbolId, ValueRef>,
    functions: HashMap<SymbolId, Function>,
    breaks: Vec<BasicBlock>,
}
impl Lowering<'_, '_> {
    fn module(mut self, program: &TypedModule) -> Result<(), String> {
        // Reserve nominal layouts before filling fields, including recursive pointers.
        let mut structs = Vec::new();
        for (symbol, item) in self.ir.symbols() {
            if let SymbolKind::Struct { fields } = &item.kind {
                let llvm = StructTypeRef::create_opaque(self.ctx, &item.name);
                self.types.insert(item.typ, llvm.into());
                structs.push((symbol, llvm, fields.clone()));
            }
        }
        for (_, mut llvm, fields) in structs {
            let fields = fields
                .iter()
                .map(|id| self.typ(self.ir.symbol(*id).typ))
                .collect::<Vec<_>>();
            llvm.set_body(&fields, false);
        }
        // Every direct callee is declared before any body or initializer is lowered.
        for id in program.functions() {
            let function = self.ir.function(*id);
            let symbol = self.ir.symbol(function.symbol);
            let SymbolKind::Function { link_name, .. } = &symbol.kind else {
                unreachable!()
            };
            let signature = self.signature(symbol.typ);
            let llvm = Function::create(signature, link_name, self.module);
            self.functions.insert(function.symbol, llvm);
        }
        for id in program.statics() {
            let symbol = self.ir.symbol(*id);
            let SymbolKind::Static { initializer } = symbol.kind else {
                unreachable!()
            };
            let typ = self.typ(symbol.typ);
            let value = initializer.map(|id| self.expression(id));
            let global = GlobalVariable::new(self.module, &typ, value.as_ref(), true);
            self.values.insert(*id, global.into());
        }
        for id in program.functions() {
            let function = self.ir.function(*id);
            let Some(body) = function.body else { continue };
            let llvm = self.functions[&function.symbol];
            let block = BasicBlock::new(self.ctx, "entry", Some(&llvm), None);
            self.builder.set_insert_point(&block);
            for (symbol, arg) in function.parameters.iter().zip(llvm.args()) {
                let slot = self.builder.create_entry_alloca(&arg.get_type());
                self.builder.create_store(&arg, &slot);
                self.values.insert(*symbol, slot);
            }
            self.block(body);
            if self.builder.get_insert_block().get_terminator().is_none() {
                self.builder.create_ret(None);
            }
            if llvm.verify() {
                return Err(format!(
                    "LLVM verification failed for `{}`",
                    self.ir.symbol(function.symbol).name
                ));
            }
        }
        Ok(())
    }
    fn typ(&mut self, id: TypeId) -> TypeRef {
        if let Some(typ) = self.types.get(&id) {
            return *typ;
        }
        let typ = match self.ir.typ(id).clone() {
            SemanticType::Primitive(p) => match p {
                Primitive::Void => TypeRef::get_void(self.ctx),
                Primitive::Bool => TypeRef::get_int(self.ctx, 1),
                Primitive::I8 | Primitive::U8 => TypeRef::get_int(self.ctx, 8),
                Primitive::I16 | Primitive::U16 => TypeRef::get_int(self.ctx, 16),
                Primitive::I32 | Primitive::U32 => TypeRef::get_int(self.ctx, 32),
                Primitive::I64 | Primitive::U64 => TypeRef::get_int(self.ctx, 64),
                Primitive::F32 => FloatTypeRef::get(self.ctx).into(),
                Primitive::F64 => DoubleTypeRef::get(self.ctx).into(),
                Primitive::Str => StructTypeRef::get(
                    self.ctx,
                    &[self.pointer_type(), TypeRef::get_int(self.ctx, 64)],
                    false,
                )
                .into(),
            },
            SemanticType::Pointer { .. } | SemanticType::Function { .. } => self.pointer_type(),
            SemanticType::Array { element, length } => {
                ArrayTypeRef::get(self.typ(element), length).into()
            }
            SemanticType::Slice { .. } => StructTypeRef::get(
                self.ctx,
                &[self.pointer_type(), TypeRef::get_int(self.ctx, 64)],
                false,
            )
            .into(),
            SemanticType::Enum(symbol) => {
                let SymbolKind::Enum { underlying, .. } = self.ir.symbol(symbol).kind else {
                    unreachable!()
                };
                self.typ(underlying)
            }
            SemanticType::Struct(_) => unreachable!("nominal LLVM types were reserved"),
        };
        self.types.insert(id, typ);
        typ
    }
    fn pointer_type(&self) -> TypeRef {
        TypeRef::get_ptr(TypeRef::get_int(self.ctx, 8), 0)
    }
    fn signature(&mut self, id: TypeId) -> FunctionTypeRef {
        let SemanticType::Function { parameters, result } = self.ir.typ(id).clone() else {
            unreachable!()
        };
        let params = parameters
            .iter()
            .map(|id| self.typ(*id))
            .collect::<Vec<_>>();
        FunctionTypeRef::get(self.typ(result), &params, false)
    }
    fn unsigned(&self, typ: TypeId) -> bool {
        match self.ir.typ(typ) {
            SemanticType::Primitive(primitive) => primitive.unsigned(),
            SemanticType::Enum(symbol) => {
                let SymbolKind::Enum { underlying, .. } = self.ir.symbol(*symbol).kind else {
                    unreachable!()
                };
                self.unsigned(underlying)
            }
            _ => false,
        }
    }
    fn cast(&mut self, value: ValueRef, source: TypeId, target: TypeId) -> ValueRef {
        let from = value.get_type();
        let to = self.typ(target);
        if from.matches(to) {
            return value;
        }
        let op = if from.is_int_ty() && to.is_int_ty() {
            if from.get_int_bit_width() > to.get_int_bit_width() {
                CastOps::Trunc
            } else if self.unsigned(source) {
                CastOps::ZExt
            } else {
                CastOps::SExt
            }
        } else if from.is_floating_point_ty() && to.is_floating_point_ty() {
            if from.is_float_ty() {
                CastOps::FPExt
            } else {
                CastOps::FPTrunc
            }
        } else if from.is_int_ty() && to.is_floating_point_ty() {
            if self.unsigned(source) {
                CastOps::UIToFP
            } else {
                CastOps::SIToFP
            }
        } else if from.is_floating_point_ty() && to.is_int_ty() {
            if self.unsigned(target) {
                CastOps::FPToUI
            } else {
                CastOps::FPToSI
            }
        } else if from.is_pointer_ty() && to.is_int_ty() {
            CastOps::PtrToInt
        } else if from.is_int_ty() && to.is_pointer_ty() {
            CastOps::IntToPtr
        } else {
            CastOps::BitCast
        };
        self.builder.create_cast(op, &value, &to)
    }
    fn block(&mut self, id: BlockId) {
        for id in &self.ir.block(id).statements {
            if self.builder.get_insert_block().get_terminator().is_some() {
                break;
            }
            self.statement(*id);
        }
    }
    fn statement(&mut self, id: StmtId) {
        match self.ir.statement(id).kind.clone() {
            StmtKind::Let {
                symbol,
                initializer,
            } => {
                let typ = self.typ(self.ir.symbol(symbol).typ);
                let slot = self.builder.create_entry_alloca(&typ);
                if let Some(id) = initializer {
                    let value = self.expression(id);
                    self.builder.create_store(&value, &slot);
                }
                self.values.insert(symbol, slot);
            }
            StmtKind::Expression(id) => {
                self.expression(id);
            }
            StmtKind::Return(id) => {
                let value = id.map(|id| self.expression(id));
                self.builder.create_ret(value.as_ref());
            }
            StmtKind::Break => {
                self.builder.create_br(self.breaks.last().unwrap());
            }
            StmtKind::If {
                condition,
                then_block,
                else_block,
            } => {
                let condition = self.expression(condition);
                let function = self.builder.get_insert_block().get_parent();
                let then_bb = BasicBlock::new(self.ctx, "if.then", Some(&function), None);
                let else_bb = BasicBlock::new(self.ctx, "if.else", Some(&function), None);
                let merge = BasicBlock::new(self.ctx, "if.end", Some(&function), None);
                self.builder.create_cond_br(&condition, &then_bb, &else_bb);
                self.builder.set_insert_point(&then_bb);
                self.block(then_block);
                if self.builder.get_insert_block().get_terminator().is_none() {
                    self.builder.create_br(&merge);
                }
                self.builder.set_insert_point(&else_bb);
                if let Some(block) = else_block {
                    self.block(block);
                }
                if self.builder.get_insert_block().get_terminator().is_none() {
                    self.builder.create_br(&merge);
                }
                if merge.has_n_uses(0) {
                    merge.remove_from_parent();
                } else {
                    self.builder.set_insert_point(&merge);
                }
            }
            StmtKind::For {
                initializer,
                condition,
                body,
                ..
            } => {
                let function = self.builder.get_insert_block().get_parent();
                let cond = BasicBlock::new(self.ctx, "for.cond", Some(&function), None);
                let loop_body = BasicBlock::new(self.ctx, "for.body", Some(&function), None);
                let merge = BasicBlock::new(self.ctx, "for.end", Some(&function), None);
                self.statement(initializer);
                self.builder.create_br(&cond);
                self.builder.set_insert_point(&cond);
                let value = self.expression(condition);
                self.builder.create_cond_br(&value, &loop_body, &merge);
                self.builder.set_insert_point(&loop_body);
                self.breaks.push(merge);
                self.block(body);
                self.breaks.pop();
                if self.builder.get_insert_block().get_terminator().is_none() {
                    self.builder.create_br(&cond);
                }
                self.builder.set_insert_point(&merge);
            }
        }
    }
    // Evaluating a place yields its address; explicit Load nodes yield values.
    fn expression(&mut self, id: ExprId) -> ValueRef {
        let expr = self.ir.expression(id).clone();
        match expr.kind {
            ExprKind::Integer(value) => ConstantInt::get(&self.typ(expr.typ), value),
            ExprKind::Float(value) => ConstantFP::get(&self.typ(expr.typ), value),
            ExprKind::String(value) => self.string(&value, expr.typ),
            ExprKind::StringLength(receiver) | ExprKind::StringData(receiver) => {
                let pointer = self.expression(receiver);
                let SemanticType::Pointer { pointee, .. } =
                    self.ir.typ(self.ir.expression(receiver).typ)
                else {
                    unreachable!()
                };
                let string_type = self.typ(*pointee);
                let descriptor = self
                    .builder
                    .create_load(&string_type, &pointer, Some("string"));
                let index = if matches!(expr.kind, ExprKind::StringLength(_)) {
                    1
                } else {
                    0
                };
                self.builder.create_extract_value(&descriptor, &[index])
            }
            ExprKind::Symbol(symbol) => {
                if let Some(function) = self.functions.get(&symbol) {
                    (*function).into()
                } else {
                    self.values[&symbol]
                }
            }
            ExprKind::Load(place) => {
                let address = self.expression(place);
                self.builder
                    .create_load(&self.typ(expr.typ), &address, Some("load"))
            }
            ExprKind::AddressOf(place) => self.address(place),
            ExprKind::Dereference(pointer) => self.expression(pointer),
            ExprKind::Cast(value) => {
                let llvm = self.expression(value);
                self.cast(llvm, self.ir.expression(value).typ, expr.typ)
            }
            ExprKind::Unary { op, operand } => {
                let value = self.expression(operand);
                match op {
                    UnaryOp::Not => self.builder.create_xor_vl(&value, 1),
                    UnaryOp::Negate => {
                        let zero = if value.get_type().is_floating_point_ty() {
                            ConstantFP::get(&value.get_type(), 0.0)
                        } else {
                            ConstantInt::get(&value.get_type(), 0)
                        };
                        self.builder
                            .create_typed_binary(&zero, &value, LlvmBinaryOp::Sub, false)
                    }
                }
            }
            ExprKind::Binary { op, left, right } => {
                let lhs = self.expression(left);
                if matches!(op, BinaryOp::And | BinaryOp::Or) {
                    return self.logical(op, lhs, right);
                }
                let rhs = self.expression(right);
                let op = match op {
                    BinaryOp::Add => LlvmBinaryOp::Add,
                    BinaryOp::Sub => LlvmBinaryOp::Sub,
                    BinaryOp::Mul => LlvmBinaryOp::Mul,
                    BinaryOp::Div => LlvmBinaryOp::Div,
                    BinaryOp::Rem => LlvmBinaryOp::Rem,
                    BinaryOp::Eq => LlvmBinaryOp::Eq,
                    BinaryOp::Ne => LlvmBinaryOp::Ne,
                    BinaryOp::Lt => LlvmBinaryOp::Lt,
                    BinaryOp::Le => LlvmBinaryOp::Le,
                    BinaryOp::Gt => LlvmBinaryOp::Gt,
                    BinaryOp::Ge => LlvmBinaryOp::Ge,
                    _ => unreachable!(),
                };
                self.builder.create_typed_binary(
                    &lhs,
                    &rhs,
                    op,
                    self.unsigned(self.ir.expression(left).typ),
                )
            }
            ExprKind::Assign { target, value } => {
                let slot = self.expression(target);
                let value = self.expression(value);
                self.builder.create_store(&value, &slot);
                value
            }
            ExprKind::Call {
                callee,
                signature,
                arguments,
            } => {
                let function = match callee {
                    Callee::Direct(symbol) => self.functions[&symbol].into(),
                    Callee::Indirect(id) => self.expression(id),
                };
                let args = arguments
                    .iter()
                    .map(|id| self.expression(*id))
                    .collect::<Vec<_>>();
                match callee {
                    Callee::Direct(_) => self.builder.create_call(&function, &args),
                    Callee::Indirect(_) => self.builder.create_ptr_call(
                        &self.signature(signature).into(),
                        &function,
                        &args,
                    ),
                }
            }
            ExprKind::Field { base, field } => {
                let slot = self.address(base);
                let SymbolKind::Field { index } = self.ir.symbol(field).kind else {
                    unreachable!()
                };
                self.builder.create_struct_gep(
                    &self.typ(self.ir.expression(base).typ),
                    &slot,
                    index,
                )
            }
            ExprKind::Index { base, index } => {
                let index = self.expression(index);
                match self.ir.typ(self.ir.expression(base).typ).clone() {
                    SemanticType::Array { .. } => {
                        let slot = self.address(base);
                        let zero = ConstantInt::get(&TypeRef::get_int(self.ctx, 32), 0);
                        self.builder.create_gep(
                            &self.typ(self.ir.expression(base).typ),
                            &slot,
                            &[zero, index],
                            true,
                        )
                    }
                    SemanticType::Pointer { pointee, .. } => {
                        let pointer = self.expression(base);
                        self.builder
                            .create_gep(&self.typ(pointee), &pointer, &[index], true)
                    }
                    SemanticType::Slice { element } => {
                        let slice = self.expression(base);
                        let pointer = self.builder.create_extract_value(&slice, &[0]);
                        self.builder
                            .create_gep(&self.typ(element), &pointer, &[index], true)
                    }
                    _ => unreachable!(),
                }
            }
            ExprKind::Struct(inits) => {
                let typ = self.typ(expr.typ);
                let slot = self.builder.create_entry_alloca(&typ);
                for (field, value) in inits {
                    let SymbolKind::Field { index } = self.ir.symbol(field).kind else {
                        unreachable!()
                    };
                    let value = self.expression(value);
                    let pointer = self.builder.create_struct_gep(&typ, &slot, index);
                    self.builder.create_store(&value, &pointer);
                }
                self.builder.create_load(&typ, &slot, Some("aggregate"))
            }
            ExprKind::ArrayRepeat { value, length } => {
                let value = self.expression(value);
                let typ = self.typ(expr.typ);
                let slot = self.builder.create_entry_alloca(&typ);
                if length != 0 {
                    let integer = TypeRef::get_int(self.ctx, 64);
                    let zero = ConstantInt::get(&integer, 0);
                    let index_slot = self.builder.create_entry_alloca(&integer);
                    self.builder.create_store(&zero, &index_slot);
                    let function = self.builder.get_insert_block().get_parent();
                    let body = BasicBlock::new(self.ctx, "repeat.body", Some(&function), None);
                    let end = BasicBlock::new(self.ctx, "repeat.end", Some(&function), None);
                    self.builder.create_br(&body);
                    self.builder.set_insert_point(&body);
                    let index =
                        self.builder
                            .create_load(&integer, &index_slot, Some("repeat.index"));
                    let pointer = self.builder.create_gep(&typ, &slot, &[zero, index], true);
                    self.builder.create_store(&value, &pointer);
                    let one = ConstantInt::get(&integer, 1);
                    let next =
                        self.builder
                            .create_typed_binary(&index, &one, LlvmBinaryOp::Add, true);
                    self.builder.create_store(&next, &index_slot);
                    let mut count = zero;
                    let radix = ConstantInt::get(&integer, 65536);
                    for shift in [48, 32, 16, 0] {
                        count = self.builder.create_typed_binary(
                            &count,
                            &radix,
                            LlvmBinaryOp::Mul,
                            true,
                        );
                        let digit =
                            ConstantInt::get(&integer, (((length as u64) >> shift) & 65535) as i32);
                        count = self.builder.create_typed_binary(
                            &count,
                            &digit,
                            LlvmBinaryOp::Add,
                            true,
                        );
                    }
                    let more =
                        self.builder
                            .create_typed_binary(&next, &count, LlvmBinaryOp::Lt, true);
                    self.builder.create_cond_br(&more, &body, &end);
                    self.builder.set_insert_point(&end);
                }
                self.builder.create_load(&typ, &slot, Some("repeat.array"))
            }
            ExprKind::Array(values) => {
                let typ = self.typ(expr.typ);
                let slot = self.builder.create_entry_alloca(&typ);
                let zero = ConstantInt::get(&TypeRef::get_int(self.ctx, 32), 0);
                for (index, value) in values.iter().enumerate() {
                    let value = self.expression(*value);
                    let index = ConstantInt::get(&TypeRef::get_int(self.ctx, 32), index as i32);
                    let pointer = self.builder.create_gep(&typ, &slot, &[zero, index], true);
                    self.builder.create_store(&value, &pointer);
                }
                self.builder.create_load(&typ, &slot, Some("array"))
            }
            ExprKind::SizeOf(typ) => {
                let typ = self.typ(typ);
                let size = self.module.get_data_layout().get_type_size_in_bits(&typ) / 8;
                ConstantInt::get(&self.typ(expr.typ), size as i32)
            }
            ExprKind::Format {
                arguments,
                pieces,
                argument_type,
                constructor,
            } => self.format(arguments, pieces, argument_type, constructor),
        }
    }
    fn address(&mut self, id: ExprId) -> ValueRef {
        let value = self.expression(id);
        if self.ir.expression(id).category == ValueCategory::Place {
            value
        } else {
            let slot = self.builder.create_entry_alloca(&value.get_type());
            self.builder.create_store(&value, &slot);
            slot
        }
    }
    fn logical(&mut self, op: BinaryOp, lhs: ValueRef, right: ExprId) -> ValueRef {
        let function = self.builder.get_insert_block().get_parent();
        let rhs_bb = BasicBlock::new(self.ctx, "logic.rhs", Some(&function), None);
        let merge = BasicBlock::new(self.ctx, "logic.end", Some(&function), None);
        let boolean = TypeRef::get_int(self.ctx, 1);
        let slot = self.builder.create_entry_alloca(&boolean);
        self.builder.create_store(&lhs, &slot);
        if op == BinaryOp::And {
            self.builder.create_cond_br(&lhs, &rhs_bb, &merge);
        } else {
            self.builder.create_cond_br(&lhs, &merge, &rhs_bb);
        }
        self.builder.set_insert_point(&rhs_bb);
        let rhs = self.expression(right);
        self.builder.create_store(&rhs, &slot);
        self.builder.create_br(&merge);
        self.builder.set_insert_point(&merge);
        self.builder.create_load(&boolean, &slot, Some("logic"))
    }
    fn string(&mut self, value: &str, typ: TypeId) -> ValueRef {
        let SemanticType::Pointer { pointee, .. } = self.ir.typ(typ) else {
            unreachable!("string literals have reference type")
        };
        let storage_type = self.typ(*pointee);
        let initializer = ConstantStruct::get(
            storage_type,
            &[
                self.module.create_global_string(value),
                ConstantInt::get(&TypeRef::get_int(self.ctx, 64), value.len() as i32),
            ],
        );
        GlobalVariable::new(self.module, &storage_type, Some(&initializer), true).into()
    }
    fn format(
        &mut self,
        arguments: Vec<(ExprId, SymbolId)>,
        pieces: Vec<String>,
        argument_type: TypeId,
        constructor: SymbolId,
    ) -> ValueRef {
        let arg_typ = self.typ(argument_type);
        let array_typ: TypeRef = ArrayTypeRef::get(arg_typ, arguments.len()).into();
        let array = self.builder.create_entry_alloca(&array_typ);
        let integer = TypeRef::get_int(self.ctx, 32);
        let zero = ConstantInt::get(&integer, 0);
        for (index, (value, formatter)) in arguments.iter().enumerate() {
            let value = self.address(*value);
            let element = self.builder.create_gep(
                &array_typ,
                &array,
                &[zero, ConstantInt::get(&integer, index as i32)],
                true,
            );
            let value_field = self.builder.create_struct_gep(&arg_typ, &element, 0);
            self.builder.create_store(&value, &value_field);
            let fn_field = self.builder.create_struct_gep(&arg_typ, &element, 1);
            self.builder
                .create_store(&self.functions[formatter].into(), &fn_field);
        }
        let str_typ: TypeRef = StructTypeRef::get(
            self.ctx,
            &[self.pointer_type(), TypeRef::get_int(self.ctx, 64)],
            false,
        )
        .into();
        let pieces_typ: TypeRef = ArrayTypeRef::get(str_typ, pieces.len()).into();
        let piece_array = self.builder.create_entry_alloca(&pieces_typ);
        for (index, piece) in pieces.iter().enumerate() {
            let pointer = self.builder.create_gep(
                &pieces_typ,
                &piece_array,
                &[zero, ConstantInt::get(&integer, index as i32)],
                true,
            );
            let value = ConstantStruct::get(
                str_typ,
                &[
                    self.module.create_global_string(piece),
                    ConstantInt::get(&TypeRef::get_int(self.ctx, 64), piece.len() as i32),
                ],
            );
            self.builder.create_store(&value, &pointer);
        }
        let args = self
            .builder
            .create_gep(&array_typ, &array, &[zero, zero], true);
        let pieces_ptr = self
            .builder
            .create_gep(&pieces_typ, &piece_array, &[zero, zero], true);
        self.builder.create_call(
            &self.functions[&constructor].into(),
            &[
                args,
                ConstantInt::get(&integer, arguments.len() as i32),
                pieces_ptr,
                ConstantInt::get(&integer, pieces.len() as i32),
            ],
        )
    }
}
