use lygos_ast::{symbols::Symbol, types::{Path, Type}, *};
use lygos_logging::{ErrorLabel, Logger};
use lygos_token::{Location, Tagged};

pub struct TypeGenerator;

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

    fn visit_assignmentexpr(&mut self, _: &mut AssignmentExpr, _: &mut Scope) -> Self::Output {
        todo!()
    }

    fn visit_memberexpr(&mut self, node: &mut MemberExpr, scope: &mut Scope) -> Self::Output {
        match node {
            MemberExpr::Member { obj, member, .. } => {
                let obj_typ = obj.accept(self, scope);

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

    fn visit_returnexpr(&mut self, _: &mut ReturnExpr, _: &mut Scope) -> Self::Output {
        Type::Path(Path::new(Location::new("", 0..1), "void"))
    }

    fn visit_binaryexpr(&mut self, node: &mut BinaryExpr, scope: &mut Scope) -> Self::Output {
        match node.op.inner().as_str() {
            "==" | "!=" | "<" | ">" | "<=" | ">=" => Type::Path(Path::new(Location::new("", 0..1), "bool".to_owned())),
            _ => node.lhs.accept(self, scope),
        }
    }

    fn visit_structdef(&mut self, node: &mut StructDef, _: &mut Scope) -> Self::Output {
        Type::Path(Path::new(node.id.loc().clone(), node.id.inner().clone()))
    }

    fn visit_typealias(&mut self, _: &mut TypeAlias, _: &mut Scope) -> Self::Output {
        Type::Path(Path::new(Location::new("", 0..1), "void"))
    }

    fn visit_numberliteral(&mut self, node: &mut NumberLiteral, _: &mut Scope) -> Self::Output {
        match node.typ {
            NumberType::Int => Type::Path(Path::new(Location::new("internal", 0..1), "i32")),
            NumberType::Char => Type::Path(Path::new(Location::new("internal", 0..1), "i8")),
            NumberType::Float => Type::Path(Path::new(Location::new("internal", 0..1), "f32")),
            _ => Type::Path(Path::new(Location::new("internal", 0..1), "f64")),
        }
    }

    fn visit_stringliteral(&mut self, _: &mut StringLiteral, _: &mut Scope) -> Self::Output {
        todo!()
    }

    fn visit_structliteral(&mut self, node: &mut StructLiteral, _: &mut Scope) -> Self::Output {
        Type::Path(Path::new(Location::new("", 0..1), node.id.inner().clone()))
    }

    fn visit_staticliteral(&mut self, _: &mut StaticLiteral, _: &mut Scope) -> Self::Output {
        Type::Path(Path::new(Location::new("", 0..1), "void"))
    }

    fn visit_identifier(&mut self, node: &mut Identifier, scope: &mut Scope) -> Self::Output {
        let var = scope.get_variable(&node.id);
        var.typ.clone()
    }
}

pub fn get_function(call: &mut CallExpr, scope: &mut Scope, mut obj: &mut Option<(&mut AST, bool)>, type_generator: &mut TypeGenerator) -> (String, Type) {
    let AST::Identifier(caller) = &*call.caller else {
        todo!("error");
    };
    let fn_name = &caller.id;
    let fully_qualified_name = match &mut obj {
        Some(obj) => {
            let AST::Identifier(obj_id) = obj.0 else {
                todo!("error");
            };
            let struct_name = if obj.1 { obj_id.id.clone() } else { Tagged::new(Location::new("", 0..1), obj_id.accept(type_generator, scope).get_name()) };
            let strct = scope.get_struct(&struct_name);
            let r#fn = strct.get_function(&Location::new("", 0..1), caller.id.inner());
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
            let AST::Identifier(obj_id) = obj.0 else {
                todo!("error");
            };
            let struct_name = if obj.1 { obj_id.id.clone() } else { Tagged::new(Location::new("", 0..1), obj_id.accept(type_generator, scope).get_name()) };

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
    (fully_qualified_name, ty)
}
