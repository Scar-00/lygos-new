use lygos_llvm::{builder::IRBuilder, context::Context, module::Module as LLVMModule, types::{DoubleTypeRef, FloatTypeRef, FunctionTypeRef, TypeRef}, value::{BasicBlock, ConstantFP, ConstantInt, Function as LLVMFunction, ValueRef}};
use lygos_ast::{symbols::{Symbol, Variable}, types::Type, *};
use lygos_logging::{ErrorLabel, Logger};
use lygos_token::{Location, Tagged};
use crate::type_getter::TypeGenerator;

pub struct CodeGenerator<'a> {
    ctx: GenerationContext<'a>,
    type_generator: TypeGenerator,
}

impl<'a> CodeGenerator<'a> {
    pub fn new(ctx: &'a Context, module: &'a LLVMModule<'a>, builder: &'a IRBuilder<'a>) -> Self {
        Self {
            ctx: GenerationContext::new(ctx, module, builder),
            type_generator: TypeGenerator{},
        }
    }
}

impl ASTVisitor for CodeGenerator<'_> {
    type Output = Option<ValueRef>;

    fn visit_module(&mut self, node: &mut Module, _: &mut Scope) -> Self::Output {
        node.body.body.iter_mut().for_each(|child| {
            child.accept(self, &mut node.body.scope);
        });

        None
    }

    fn visit_function(&mut self, node: &mut Function, _: &mut Scope) -> Self::Output {
        let func = self.ctx.module.get_function(&node.name_mangeled).unwrap_or_else(|| {
            let params = node.args.iter_mut().map(|arg| {
                node.body.scope.parent.resolve_type(&arg.typ, &self.ctx)
            }).collect::<Vec<_>>();

            let fn_type = FunctionTypeRef::get(node.body.scope.parent.resolve_type(&node.ret_type, &self.ctx), &params, false);
            LLVMFunction::create(fn_type, &node.name_mangeled, self.ctx.module)
        });

        if !node.is_def {
            return None;
        }

        let bb = BasicBlock::new(self.ctx.ctx, "", Some(&func), None);
        self.ctx.builder.set_insert_point(&bb);

        func.args().iter().zip(node.args.iter()).for_each(|(ll_arg, arg)| {
            let alloca = self.ctx.builder.create_alloca(&ll_arg.get_type(), None);
            self.ctx.builder.create_store(ll_arg, &alloca);
            let sym = Variable::new(arg.id.loc().clone(), arg.typ.clone(), alloca, false);
            node.body.scope.add_symbol(arg.id.inner(), Symbol::Variable(sym));
        });

        let create_alloc = if let Type::Path(path) = &node.ret_type {
            path.path != "void"
        }else {
            true
        };

        if create_alloc {
            let typ = node.body.scope.parent.resolve_type(&node.ret_type, &self.ctx);
            let alloc = self.ctx.builder.create_alloca(&typ, None);
            node.body.scope.set_return_alloc(alloc);
        }

        self.ctx.current_function.set_value(node);

        node.body.body.iter_mut().for_each(|child| {
            child.accept(self, &mut node.body.scope);
        });

        if let Some(bb) = &node.ret_block {
            self.ctx.builder.set_insert_point(bb);
        }

        if let Some(alloc) = node.body.scope.get_return_alloc() {
            let typ = node.body.scope.parent.resolve_type(&node.ret_type, &self.ctx);
            let value = self.ctx.builder.create_load(&typ, &alloc);
            self.ctx.builder.create_ret(Some(&value));
        }

        func.verify();
        None
    }

    fn visit_vardecl(&mut self, node: &mut VarDecl, scope: &mut Scope) -> Self::Output {
        let Some(ast_value) = &mut node.value else {
            let Some(typ) = &node.typ else {
                todo!("error");
            };
            let llvm_typ = scope.resolve_type(typ, &self.ctx);
            let alloca = self.ctx.builder.create_alloca(&llvm_typ, None);
            let sym = Variable::new(node.id.loc().clone(), typ.clone(), alloca, node.cnst);
            scope.add_symbol(node.id.inner(), Symbol::Variable(sym));
            return None;
        };
        let typ = ast_value.accept(&mut self.type_generator, scope);
        let Some(mut value) = ast_value.accept(self, scope) else {
            todo!("error");
        };
        if ast_value.needs_load() {
            value = self.ctx.builder.create_load(&scope.resolve_type(&typ, &self.ctx), &value);
        }

        if let Some(assigned_typ) = &node.typ {
            let _assigned_typ = scope.resolve_type(assigned_typ, &self.ctx);
            todo!("create cast");
        }

        let typ = if let Some(ty) = &node.typ {
            ty
        }else {
            &typ
        };

        let alloca = self.ctx.builder.create_alloca(&value.get_type(), None);
        self.ctx.builder.create_store(&value, &alloca);
        let sym = Variable::new(node.id.loc().clone(), typ.clone(), alloca, node.cnst);
        scope.add_symbol(node.id.inner(), Symbol::Variable(sym));
        None
    }

    fn visit_assignmentexpr(&mut self, node: &mut AssignmentExpr, scope: &mut Scope) -> Self::Output {
        let Some(var) = node.lhs.accept(self, scope) else {
            todo!("error");
        };

        let Some(mut value) = node.rhs.accept(self, scope) else {
            todo!("error");
        };

        let value_typ = node.rhs.accept(&mut self.type_generator, scope);
        if node.rhs.needs_load() {
            value = self.ctx.builder.create_load(&scope.resolve_type(&value_typ, &self.ctx), &value);
        }

        let var_typ = node.lhs.accept(&mut self.type_generator, scope);
        if !var_typ.matches(&value_typ) {
            Logger::error_msg_labels(
                "missmatched types", &[
                    ErrorLabel::from(&Location::new("", 0..1), format!("expected type `{}`", var_typ.get_full_name()).as_str()),
                    ErrorLabel::from(&Location::new("", 0..1), format!("but value has type `{}`", value_typ.get_full_name()).as_str()),
            ]);
        }
        self.ctx.builder.create_store(&value, &var);
        None
    }

    fn visit_memberexpr(&mut self, node: &mut MemberExpr, scope: &mut Scope) -> Self::Output {
        match node {
            MemberExpr::Member { obj, member, deref } => {
                let Some(mut obj_value) = obj.accept(self, scope) else {
                    todo!("error");
                };
                let obj_typ = obj.accept(&mut self.type_generator, scope);
                if *deref {
                    obj_value = self.ctx.builder.create_load(&scope.resolve_type(&obj_typ, &self.ctx), &obj_value);
                }

                if let Type::Pointer(ptr) = &obj_typ {
                    if ptr.is_ref {
                        obj_value = self.ctx.builder.create_load(&scope.resolve_type(&ptr.typ, &self.ctx), &obj_value);
                    }
                }

                let type_name = obj_typ.get_name();
                let struct_symbol = scope.get_struct(&Tagged::new(Location::new("", 0..1), type_name.clone()));

                let member_name = match &**member {
                    AST::Identifier(id) => { id.id.inner() }
                    _ => todo!("member expr member type"),
                };
                let Some(index) = struct_symbol.fields.iter().position(|field| {
                    field.id.inner() == member_name
                }) else {
                    let mut fields_avail = "available fieds: [".to_string();
                    for (i, field) in struct_symbol.fields.iter().enumerate() {
                        fields_avail += &("`".to_string() + field.id.inner() + "`");
                        if i + 1 != struct_symbol.fields.len() {
                            fields_avail += ", ";
                        }
                    }
                    fields_avail += "]";
                    Logger::error_msg_label_info(
                        format!("unknown field `{}` in type `{}`", member_name, type_name),
                        ErrorLabel::from(&Location::new("", 0..1), "unknown field"),
                        &fields_avail
                    );
                };

                if !obj_value.get_type().is_pointer_ty() {
                    let alloc = self.ctx.builder.create_alloca(&obj_value.get_type(), None);
                    self.ctx.builder.create_store(&obj_value, &alloc);
                    obj_value = alloc;
                }

                let base = if *deref {
                    let Some(base) = obj_typ.get_base() else {
                        Logger::error_msg_label(
                            "cannot deref value type",
                            ErrorLabel::from(&Location::new("", 0..1), format!("cannot deref value type `{}`", obj_typ.get_full_name()).as_str())
                        );
                    };
                    base
                }else if let Type::Pointer(ptr) = &obj_typ {
                    if ptr.is_ref {
                        &*ptr.typ
                    }else {
                        &obj_typ
                    }
                }else {
                    &obj_typ
                };

                if base.get_base().is_some() {
                    Logger::error_msg_label_info(
                        "invalid level of indirection",
                        ErrorLabel::from(&Location::new("", 0..1), format!("cannot access member of type `{}`", obj_typ.get_full_name())),
                        "try dereferencing using `->` instead of `.`"
                    );
                }

                let base = scope.resolve_type(base, &self.ctx);
                Some(self.ctx.builder.create_struct_gep(&base, &obj_value, index as u32))
            },
            MemberExpr::Index { .. } => todo!(),
        }
    }

    fn visit_callexpr(&mut self, _: &mut CallExpr, _: &mut Scope) -> Self::Output {
        todo!()
    }

    fn visit_returnexpr(&mut self, node: &mut ReturnExpr, scope: &mut Scope) -> Self::Output {
        let Some(ast_value) = &mut node.value else {
            if self.ctx.current_function.is_some() {
                if let Some(bb) = &self.ctx.current_function.ret_block {
                    self.ctx.builder.create_br(bb);
                }
            }
            return None;
        };

        let typ = ast_value.accept(&mut self.type_generator, scope);
        if scope.get_return_alloc().is_none() {
            let loc = typ.get_loc();
            Logger::error_msg_label(
                format!(
                    "invalid return type `{}` for function with return type `void`",
                    typ.get_full_name()
                ).as_str(),
                ErrorLabel::from(&loc, "invalid return type"),
            );
        }

        let mut value = ast_value.accept(self, scope).unwrap();
        if ast_value.needs_load() {
            value = self.ctx.builder.create_load(&scope.resolve_type(&typ, &self.ctx), &value);
        }

        if let Some(bb) = &self.ctx.current_function.ret_block {
            self.ctx.builder.create_store(&value, &scope.get_return_alloc().unwrap());
            self.ctx.builder.create_br(bb);
            return None;
        }

        self.ctx.builder.create_store(&value, &scope.get_return_alloc().unwrap());
        None
    }

    fn visit_binaryexpr(&mut self, node: &mut BinaryExpr, scope: &mut Scope) -> Self::Output {
        let lhs_typ = node.lhs.accept(&mut self.type_generator, scope);
        let Some(mut lhs) = node.lhs.accept(self, scope) else {
            todo!("error");
        };
        let rhs_typ = node.rhs.accept(&mut self.type_generator, scope);
        let Some(mut rhs) = node.rhs.accept(self, scope) else {
            todo!("error");
        };

        if node.lhs.needs_load() {
            lhs = self.ctx.builder.create_load(&scope.resolve_type(&lhs_typ, &self.ctx), &lhs);
        }

        if node.rhs.needs_load() {
            rhs = self.ctx.builder.create_load(&scope.resolve_type(&rhs_typ, &self.ctx), &rhs);
        }

        if !lhs_typ.matches(&rhs_typ) {
            Logger::error_msg_labels(
                format!("invalid operant to binary operator `{}`", node.op.inner()).as_str(), &[
                    ErrorLabel::from(&Location::new("", 0..1), format!("left hand side has type `{}`", lhs_typ.get_full_name())),
                    ErrorLabel::from(&Location::new("", 0..1), format!("right hand side has type `{}`", rhs_typ.get_full_name())),
                ]
            )
        }

        match node.op.inner().as_str() {
            "+" => Some(self.ctx.builder.create_add(&lhs, &rhs)),
            "-" => Some(self.ctx.builder.create_sub(&lhs, &rhs)),
            "*" => Some(self.ctx.builder.create_mul(&lhs, &rhs)),
            "/" => Some(self.ctx.builder.create_div(&lhs, &rhs)),
            "%" => Some(self.ctx.builder.create_rem(&lhs, &rhs)),
            "==" => Some(self.ctx.builder.create_icmp_eq(&lhs, &rhs)),
            "!=" => Some(self.ctx.builder.create_icmp_ne(&lhs, &rhs)),
            "<" => Some(self.ctx.builder.create_icmp_slt(&lhs, &rhs)),
            "<=" => Some(self.ctx.builder.create_icmp_sle(&lhs, &rhs)),
            ">" => Some(self.ctx.builder.create_icmp_sgt(&lhs, &rhs)),
            ">=" => Some(self.ctx.builder.create_icmp_sge(&lhs, &rhs)),
            _ => Logger::error_msg_label(
                    format!("unknown binary operator `{}`", node.op.inner()).as_str(),
                    ErrorLabel::from(node.op.loc(), "unknown binary operator"),
            ),
        }
    }

    fn visit_structdef(&mut self, _: &mut StructDef, _: &mut Scope) -> Self::Output {
        None
    }

    fn visit_typealias(&mut self, _: &mut TypeAlias, _: &mut Scope) -> Self::Output {
        todo!()
    }

    fn visit_numberliteral(&mut self, node: &mut NumberLiteral, _: &mut Scope) -> Self::Output {
        match node.typ {
            NumberType::Int => {
                let value = node.value.inner().parse::<i32>().unwrap();
                Some(ConstantInt::get(&TypeRef::get_int(self.ctx.ctx, 32), value))
            },
            NumberType::Char => {
                let value = node.value.inner().clone().into_bytes()[0] as i32;
                Some(ConstantInt::get(&TypeRef::get_int(self.ctx.ctx, 8), value))
            },
            NumberType::Float => {
                let value = node.value.inner().parse::<f64>().unwrap();
                Some(ConstantFP::get(&FloatTypeRef::get(self.ctx.ctx).into(), value))
            },
            _ => {
                let value = node.value.inner().parse::<f64>().unwrap();
                Some(ConstantFP::get(&DoubleTypeRef::get(self.ctx.ctx).into(), value))
            },
        }
    }

    fn visit_stringliteral(&mut self, _: &mut StringLiteral, _: &mut Scope) -> Self::Output {
        todo!()
    }

    fn visit_structliteral(&mut self, node: &mut StructLiteral, scope: &mut Scope) -> Self::Output {
        let strct = scope.get_struct(&node.id).clone();
        let typ = node.accept(&mut self.type_generator, scope);
        let var = self.ctx.builder.create_alloca(&scope.resolve_type(&typ, &self.ctx), None);
        node.inits.iter_mut().enumerate().for_each(|(i, (id, node))| {
            let Some(mut value) = node.accept(self, scope) else {
                todo!("error");
            };
            if node.needs_load() {
                let typ = node.accept(&mut self.type_generator, scope);
                value = self.ctx.builder.create_load(&scope.resolve_type(&typ, &self.ctx), &value);
            }
            if let Some(id) = &id {
                let Some(index) = strct.fields.iter().position(|f| *f.id.inner() == *id.inner()) else {
                    todo!("");
                };
                let gep = self.ctx.builder.create_struct_gep(&scope.resolve_type(&typ, &self.ctx), &var, index as u32);
                self.ctx.builder.create_store(&value, &gep);
            }else {
                if i >= strct.fields.len() {
                    Logger::error_msg_label("out of bounds struct index", ErrorLabel::from(&Location::new("", 0..1), "out of bounds"));
                }

                let gep = self.ctx.builder.create_struct_gep(&scope.resolve_type(&typ, &self.ctx), &var, i as u32);
                self.ctx.builder.create_store(&value, &gep);
            }
        });

        Some(var)
    }

    fn visit_staticliteral(&mut self, _: &mut StaticLiteral, _: &mut Scope) -> Self::Output {
        todo!()
    }

    fn visit_identifier(&mut self, node: &mut Identifier, scope: &mut Scope) -> Self::Output {
        let var = scope.get_variable(&node.id);
        Some(var.alloca)
    }
}
