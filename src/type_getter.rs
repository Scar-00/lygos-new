use lygos_ast::{symbols::Symbol, types::{Array, Path, Pointer, Type}, *};
use lygos_logging::{ErrorLabel, Logger};
use lygos_token::{Location, Tagged};

use crate::locator::Locator;

pub struct TypeGenerator {
    pub(crate) locator: Locator,
}

impl ASTVisitor for TypeGenerator {
    type Output = Type;

    fn visit_module(&mut self, node: &mut Module, scope: &mut Scope) -> Self::Output {
        node.body.body.iter_mut().for_each(|child| {
            child.accept(self, scope);
        });
        Type::Path(Path::new(Location::new("", 0..1), "void"))
    }

    fn visit_function(&mut self, node: &mut Function, scope: &mut Scope) -> Self::Output {
        node.body.body.iter_mut().for_each(|child| {
            child.accept(self, scope);
        });
        Type::Path(Path::new(Location::new("", 0..1), "void"))
    }

    fn visit_vardecl(&mut self, _: &mut VarDecl, _: &mut Scope) -> Self::Output {
        Type::Path(Path::new(Location::new("", 0..1), "void"))
    }

    fn visit_assignmentexpr(&mut self, node: &mut AssignmentExpr, scope: &mut Scope) -> Self::Output {
        node.lhs.accept(self, scope)
    }

    fn visit_memberexpr(&mut self, node: &mut MemberExpr, scope: &mut Scope) -> Self::Output {
        match node {
            MemberExpr::Member { obj, member, .. } => {
                let obj_typ = obj.accept(self, scope);

                let type_name = obj_typ.get_name();
                let obj_loc = obj.accept(&mut self.locator, scope);
                let struct_symbol = scope.get_struct(&Tagged::new(obj_loc, type_name.clone()));

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
                        ErrorLabel::from(&member.accept(&mut self.locator, scope), "unknown field"),
                        &fields_avail
                    );
                };
                struct_symbol.fields[index].typ.clone()
            }
            MemberExpr::Index { .. } => todo!(),
        }
    }

    fn visit_callexpr(&mut self, node: &mut CallExpr, scope: &mut Scope) -> Self::Output {
        let fn_name = {
            let AST::Identifier(fn_name) = &*node.caller else {
                todo!("error");
            };
            fn_name.clone()
        };
        let ty = if scope.has_symbol(fn_name.id.inner()) {
            scope.get_function(&fn_name.id).ret_type.clone()
        }else {
            node.caller.accept(self, scope)
        };
        if let Type::FuncPtr(ptr) = &ty {
            *ptr.ret_type.clone()
        }else {
            scope.get_function(&fn_name.id).ret_type.clone()
        }
    }

    fn visit_membercallexpr(&mut self, node: &mut MemberCallExpr, scope: &mut Scope) -> Self::Output {
        let obj_loc = node.obj.accept(&mut self.locator, scope);
        let struct_name = node.obj.accept(self, scope).get_name();
        let func_loc = node.r#fn.accept(&mut self.locator, scope);
        let strct = scope.get_struct(&Tagged::new(obj_loc, struct_name));
        let AST::CallExpr(call) = &*node.r#fn else {
            todo!("error");
        };
        let AST::Identifier(func_id) = &*call.caller else {
            todo!("error");
        };
        let func = strct.get_function(&func_loc, func_id.id.inner());
        func.ret_type.clone()
    }

    fn visit_accessexpr(&mut self, node: &mut AccessExpr, scope: &mut Scope) -> Self::Output {
        match node.obj.accept(self, scope) {
            Type::Array(arr) => *arr.typ,
            Type::Pointer(ptr) => *ptr.typ,
            ty => ty,
        }
    }

    fn visit_unaryexpr(&mut self,node: &mut UnaryExpr,scope: &mut Scope) -> Self::Output {
        let ty = node.obj.accept(self, scope);
        let loc = node.accept(&mut self.locator, scope);
        match node.op.inner().as_str() {
            "*" => {
                if let Type::Pointer(ptr) = &ty {
                    *ptr.typ.clone()
                }else {
                    todo!("error")
                }
            }
            "&" => {
                Type::Pointer(Pointer::new(ty.get_loc(), Box::new(ty), false, false))
            }
            "!" => {
                Type::Path(Path::new(loc, "bool".to_string()))
            }
            _ => Logger::error_msg_label(
                format!("unknown unary operator `{}`", node.op.inner()).as_str(),
                ErrorLabel::from(&loc, "unknown unary operator"),
            ),
        }
    }

    fn visit_resolutionexpr(&mut self,node: &mut ResolutionExpr,scope: &mut Scope) -> Self::Output {
        //also allow preceding ResolutionExprs
        let obj_loc = node.obj.accept(&mut self.locator, scope);
        let AST::Identifier(ident) = &*node.obj else {
            todo!("error");
        };
        if scope.is_enum(&ident.id) {
            return Type::Path(Path::new(obj_loc, ident.id.inner().clone()));
        }
        let member_loc = node.member.accept(&mut self.locator, scope);
        let strct = scope.get_struct(&ident.id);
        let AST::CallExpr(call) = &*node.member else {
            todo!("error");
        };
        let AST::Identifier(member_ident) = &*call.caller else {
            todo!("error");
        };
        let func = strct.get_function(&member_loc, member_ident.id.inner());
        func.ret_type.clone()
    }

    fn visit_castexpr(&mut self, node: &mut CastExpr, _: &mut Scope) -> Self::Output {
        node.target_type.clone()
    }

    fn visit_returnexpr(&mut self, _: &mut ReturnExpr, _: &mut Scope) -> Self::Output {
        Type::Path(Path::new(Location::new("", 0..1), "void"))
    }

    fn visit_binaryexpr(&mut self, node: &mut BinaryExpr, scope: &mut Scope) -> Self::Output {
        match node.op.inner().as_str() {
            "==" | "!=" | "<" | ">" | "<=" | ">=" => Type::Path(Path::new(node.accept(&mut self.locator, scope), "bool".to_owned())),
            _ => node.lhs.accept(self, scope),
        }
    }

    fn visit_ifstmt(&mut self,node: &mut IfStmt,scope: &mut Scope) -> Self::Output {
        Type::Path(Path::new(node.cond.accept(&mut self.locator, scope).clone(), "void"))
    }

    fn visit_forstmt(&mut self, node: &mut ForStmt, scope: &mut Scope) -> Self::Output {
        Type::Path(Path::new(node.cond.accept(&mut self.locator, scope).clone(), "void"))
    }

    fn visit_breakexpr(&mut self, node: &mut BreakExpr, _: &mut Scope) -> Self::Output {
        Type::Path(Path::new(node.loc.clone(), "void"))
    }

    fn visit_structdef(&mut self, node: &mut StructDef, _: &mut Scope) -> Self::Output {
        Type::Path(Path::new(node.id.loc().clone(), node.id.inner().clone()))
    }

    fn visit_enumdef(&mut self, node: &mut EnumDef, _: &mut Scope) -> Self::Output {
        Type::Path(Path::new(node.id.loc().clone(), "void"))
    }

    fn visit_impl(&mut self, _: &mut Impl, _: &mut Scope) -> Self::Output {
        todo!()
    }

    fn visit_trait(&mut self, _: &mut Trait, _: &mut Scope) -> Self::Output {
        todo!()
    }

    fn visit_macro(&mut self, _: &mut Macro, _: &mut Scope) -> Self::Output {
        todo!()
    }

    fn visit_macrocall(&mut self, node: &mut MacroCall, _: &mut Scope) -> Self::Output {
        match node.id.inner().as_str() {
            "format_args" => Type::Path(Path::new(node.id.loc().clone(), "Arguments")),
            "sizeof" => Type::Path(Path::new(node.id.loc().clone(), "u64")),
            _ => unreachable!(),
        }
    }

    fn visit_typealias(&mut self, node: &mut TypeAlias, _: &mut Scope) -> Self::Output {
        node.typ.clone()
    }

    fn visit_numberliteral(&mut self, node: &mut NumberLiteral, _: &mut Scope) -> Self::Output {
        let loc = node.value.loc().clone();
        match node.typ {
            NumberType::Int => Type::Path(Path::new(loc, "i32")),
            NumberType::Char => Type::Path(Path::new(loc, "i8")),
            NumberType::Float => Type::Path(Path::new(loc, "f32")),
            _ => Type::Path(Path::new(loc, "f64")),
        }
    }

    fn visit_stringliteral(&mut self, string: &mut StringLiteral, _: &mut Scope) -> Self::Output {
        Type::Path(Path::new(string.value.loc().clone(), "str"))
    }

    fn visit_structliteral(&mut self, node: &mut StructLiteral, _: &mut Scope) -> Self::Output {
        Type::Path(Path::new(node.id.loc().clone(), node.id.inner().clone()))
    }

    fn visit_arrayliteral(&mut self, node: &mut ArrayLiteral, scope: &mut Scope) -> Self::Output {
        let types = node.values.iter_mut().map(|child| child.accept(self, scope)).collect::<Vec<_>>();
        let typ = if let Some(typ) = types.first() {
            typ.clone()
        }else {
            Type::Path(Path::new(node.loc.clone(), "void"))
        };
        types.iter().for_each(|child_type| {
            if !child_type.matches(&typ) {
                todo!("error");
            }
        });

        Type::Array(Array::new(node.loc.clone(), typ, types.len()))
    }

    fn visit_staticliteral(&mut self, node: &mut StaticLiteral, _: &mut Scope) -> Self::Output {
        node.typ.clone()
    }

    fn visit_identifier(&mut self, node: &mut Identifier, scope: &mut Scope) -> Self::Output {
        let var = scope.get_variable(&node.id);
        /*if node.deref {
            if let Type::Pointer(ptr) = &var.typ {
                if ptr.is_ref {
                    return *ptr.typ.clone();
                }
            }
        }*/
        var.typ.clone()
    }

    fn visit_blockexpr(&mut self, _: &mut BlockExpr, _: &mut Scope) -> Self::Output {
        todo!()
    }
}

pub fn get_function(call: &mut CallExpr, scope: &mut Scope, mut obj: &mut Option<(&mut AST, bool)>, type_generator: &mut TypeGenerator) -> (Tagged<String>, String, Type) {
    let caller_loc = call.caller.accept(&mut type_generator.locator, scope);
    /*let AST::Identifier(caller) = &*call.caller else {
        todo!("error: {:?}", obj);
    };*/
    let caller = match &*call.caller {
        AST::Identifier(caller) => caller,
        AST::MemberExpr(MemberExpr::Member { member, .. }) => {
            let AST::Identifier(id) = &**member else {
                todo!("error");
            };
            id
        }
        _ => todo!("error"),
    };
    let fn_name = caller.id.clone();
    let fully_qualified_name = match &mut obj {
        Some(obj) => {
            let obj_loc = obj.0.accept(&mut type_generator.locator, scope);
            let AST::Identifier(obj_id) = obj.0 else {
                //  NOTE(K): this might also be a literal of any kind
                todo!("error");
            };
            let struct_name = if obj.1 { obj_id.id.clone() } else { Tagged::new(obj_loc, obj_id.accept(type_generator, scope).get_name()) };
            let strct = scope.get_struct(&struct_name);
            let r#fn = strct.get_function(&caller_loc, caller.id.inner());
            r#fn.name_mangeled.clone()
        }
        None => caller.id.inner().clone(),
    };
    /*
     * FIXME(S):
     * the mecanism for getting type of a function is flawed, breaks when calling member
     * function of type which is located in another struct -> foo.bar.baz();
     */

    let ty = match obj {
        Some(obj) => {
            let obj_loc = obj.0.accept(&mut type_generator.locator, scope);
            let AST::Identifier(obj_id) = obj.0 else {
                todo!("error");
            };
            let struct_name = if obj.1 { obj_id.id.clone() } else { Tagged::new(obj_loc, obj_id.accept(type_generator, scope).get_name()) };

            let strct = scope.get_struct(&struct_name);
            let r#fn = strct.get_function(fn_name.loc(), fn_name.inner());
            r#fn.ret_type.clone()
        }
        None => {
            if scope.has_symbol(fn_name.inner()) {
                if let Symbol::Function(_) = scope.resolve_symbol(&fn_name) {
                    scope.get_function(&fn_name).ret_type.clone()
                }else {
                    call.caller.accept(type_generator, scope)
                }
            }else {
                call.caller.accept(type_generator, scope)
            }
        }
    };
    (fn_name.clone(), fully_qualified_name, ty)
}
