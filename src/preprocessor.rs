use std::collections::HashMap;

use lygos_ast::{types::{BaseTypes, Path, Pointer, Type}, *};
use lygos_llvm::{types::{ArrayTypeRef, StructTypeRef, TypeRef}, value::{ConstantInt, ConstantStruct, ValueRef}};
use lygos_logging::{ErrorLabel, Logger};
use lygos_parser::Parser;
use lygos_token::{Location, Tagged, Token, TokenType};

use crate::code_gen::CodeGenerator;

type IntrinsicCall = fn(call: &MacroCall, scope: &mut Scope, code_gen: &mut CodeGenerator) -> ValueRef;
lazy_static::lazy_static! {
    pub static ref IntrinsicMarcos: HashMap<&'static str, IntrinsicCall> = {
        let mut map = HashMap::<&'static str, IntrinsicCall>::new();
        map.insert("format_args", Preprocessor::macro_format);
        map.insert("sizeof", Preprocessor::macro_sizeof);
        /*map.insert("file", crate::ast::intrinsics::macro_file);
        map.insert("line", crate::ast::intrinsics::macro_line);
        map.insert("impl_debug", crate::ast::intrinsics::impl_debug);
        map.insert("test_macro", crate::ast::intrinsics::test_macro);*/
        map
    };
}

pub struct Preprocessor;

impl Preprocessor {
    fn get_formatting_function(call_loc: &Location, ty: Type, scope: &mut Scope, debug: bool) -> String {
        let name = ty.get_full_name();
        let tagged = Tagged::new(ty.get_loc(), name.clone());
        if BaseTypes.contains(&name.as_str()) {
            return name + "_fmt";
        }
        if let Type::Pointer(_) = ty {
            return String::from("ptr_fmt");
        }
        if scope.is_struct(&tagged) {
            if debug {
                if !scope.get_struct(&tagged).implements_trait("Debug") {
                    Logger::error_msg_label(
                        format!("type `{}` does not implement trait `Debug`", name).as_str(),
                        ErrorLabel::from(call_loc, "not formattable")
                    );
                }
                return scope.get_struct(&tagged).get_function(call_loc, "fmt_debug").name_mangeled.clone();
            }else {
                if !scope.get_struct(&tagged).implements_trait("Display") {
                    Logger::error_msg_label(
                        format!("type `{}` does not implement trait `Display`", name).as_str(),
                        ErrorLabel::from(call_loc, "not formattable")
                    );
                }
                return scope.get_struct(&tagged).get_function(call_loc, "fmt").name_mangeled.clone();
            }
        }
        Logger::error_msg_label(
            format!("cannot format type `{}` with the default formatter", name).as_str(),
            ErrorLabel::from(call_loc, "not formattable")
        );
    }

    fn macro_format(call: &MacroCall, scope: &mut Scope, code_gen: &mut CodeGenerator) -> ValueRef {
        let format_string = call.args[0][0].clone();

        let TokenType::String(format_string) = format_string.typ else {
            Logger::token_expected(&format_string.loc, "unexpected token found", "expected string literal as first argument to `format_args`");
        };

        let mut format_string = format_string.as_str();
        let mut piecies: Vec<String> = Vec::new();
        let mut args_expected = 0;
        let mut debug: Vec<bool> = Vec::new();
        while let Some(mut pos) = format_string.find('{') {
            args_expected += 1;
            piecies.push(format_string[..pos].to_string());
            if pos + 3 < format_string.len() {
                if format_string[pos+1..pos+3] == *":?" {
                    pos += 2;
                    debug.push(true);
                }else {
                    debug.push(false);
                }
            }else {
                debug.push(false);
            }
            format_string = &format_string[pos + 2..];
        }
        if !format_string.is_empty() {
            piecies.push(format_string.to_string());
        }

        if call.args.len() != args_expected + 1 {
            Logger::error_msg_label(
                format!("macro `format_args` expected `{}` args, but `{}` were provided", args_expected, call.args.len()).as_str(),
                ErrorLabel::from(call.id.loc(), "incorrect number of arguments")
            );
        }

        let mut arg_idents: Vec<AST> = Vec::new();
        for i in 1..args_expected+1 {
            let mut arg = call.args[i].clone();
            arg.push(Token::new(TokenType::Semi, Location::new("internal", 0..1)));
            let mut parser = Parser::from(arg);
            if let AST::Module(mut m) = parser.build_ast() {
                arg_idents.push(m.body.body.remove(0));
            }
        }

        let zero = ConstantInt::get(&TypeRef::get_int(code_gen.ctx.ctx, 32), 0);

        let arg_ty = Type::Path(Path::new(Location::new("internal", 0..1), "Argument"));
        let arr_ty = ArrayTypeRef::get(scope.resolve_type(&arg_ty, &code_gen.ctx), args_expected).into();

        let fmt_typ = scope.resolve_type(&scope.get_struct(&Tagged::new(Location::new("internal", 0..1), "Argument".to_string())).fields[1].typ, &code_gen.ctx);

        let args_arr = code_gen.ctx.builder.create_alloca(&arr_ty, None);
        for i in 0..args_expected {
            //getting a member_call_expr's type before its value is generated will lead to the
            //functions name to not be mangeled an therefore be invalid
            let Some(mut value_val) = arg_idents[i].accept(code_gen, scope) else {
                todo!("");
            };

            let Some(func) = code_gen.ctx.module.get_function(Self::get_formatting_function(
                    call.id.loc(),
                    arg_idents[i].accept(&mut code_gen.type_generator, scope),
                    scope,
                    debug[i])
            ) else {
                Logger::error_msg_label(
                    format!("failed to get formatting function for `{}`", Self::get_formatting_function(call.id.loc(), arg_idents[i].accept(&mut code_gen.type_generator, scope), scope, debug[i])).as_str(),
                    ErrorLabel::from(call.id.loc(), "not formattable")
                );
            };

            let arg_ty = scope.resolve_type(&arg_ty, &code_gen.ctx);
            let arg = code_gen.ctx.builder.create_alloca(&arg_ty, None);
            let type_ptr = Type::Pointer(Pointer::new(
                    Location::new("internal", 0..1),
                    Box::new(arg_idents[i].accept(&mut code_gen.type_generator, scope)),
                    false,
                    false
            ));
            let value_ty = scope.resolve_type(&type_ptr, &code_gen.ctx);
            let value = code_gen.ctx.builder.create_alloca(&value_ty, None);

            if !value_val.get_type().is_pointer_ty() {
                let alloc = code_gen.ctx.builder.create_alloca(&value_val.get_type(), None);
                code_gen.ctx.builder.create_store(&value_val, &alloc);
                value_val = alloc;
            }

            code_gen.ctx.builder.create_store(&value_val, &value);

            let value_gep = code_gen.ctx.builder.create_struct_gep(&arg_ty, &arg, 0);
            let load = code_gen.ctx.builder.create_load(&value_ty, &value, None::<&str>);
            let cast = code_gen.ctx.builder.create_pointer_cast(&load, &TypeRef::get_ptr(TypeRef::get_int(code_gen.ctx.ctx, 8), 0));
            code_gen.ctx.builder.create_store(&cast, &value_gep);

            let ptr_gep = code_gen.ctx.builder.create_struct_gep(&arg_ty, &arg, 1);
            let cast = code_gen.ctx.builder.create_pointer_cast(&func.into(), &fmt_typ);
            code_gen.ctx.builder.create_store(&cast, &ptr_gep);

            let idx = ConstantInt::get(&TypeRef::get_int(code_gen.ctx.ctx, 32), i as i32);
            let arr_index = code_gen.ctx.builder.create_gep(&arr_ty, &args_arr, &[zero, idx], true);
            code_gen.ctx.builder.create_store(&code_gen.ctx.builder.create_load(&arg_ty, &arg, None::<&str>), &arr_index);
        }
        let piece_type = StructTypeRef::get(code_gen.ctx.ctx, &[TypeRef::get_ptr(TypeRef::get_int(code_gen.ctx.ctx, 8), 0), TypeRef::get_int(code_gen.ctx.ctx, 64)], false).into();
        let pieces_ty = ArrayTypeRef::get(piece_type, piecies.len()).into();
        let pieces_arr = code_gen.ctx.builder.create_alloca(&pieces_ty, None);

        for (i, piece) in piecies.iter().enumerate() {
            let idx = ConstantInt::get(&TypeRef::get_int(code_gen.ctx.ctx, 32), i as i32);
            let gep = code_gen.ctx.builder.create_gep(&pieces_ty, &pieces_arr, &[zero, idx], true);
            let lit = ConstantStruct::get(piece_type,
                &[
                    code_gen.ctx.builder.create_global_string_pointer(piece.as_str()),
                    ConstantInt::get(&TypeRef::get_int(code_gen.ctx.ctx, 64), piece.len() as i32)
                ]
            );
            code_gen.ctx.builder.create_store(&lit, &gep);
        }

        let arr_idx = code_gen.ctx.builder.create_gep(&arr_ty, &args_arr, &[zero, zero], true);
        let pieces_idx = code_gen.ctx.builder.create_gep(&pieces_ty, &pieces_arr, &[zero, zero], true);

        let Some(func) = code_gen.ctx.module.get_function("Arguments_new") else {
            Logger::error_msg("internal", "Arguments::new is undefined");
        };
        code_gen.ctx.builder.create_call(&func.into(), &[
            arr_idx, ConstantInt::get(&TypeRef::get_int(code_gen.ctx.ctx, 32), args_expected as i32),
            pieces_idx, ConstantInt::get(&TypeRef::get_int(code_gen.ctx.ctx, 32), piecies.len() as i32),
        ])
    }

    fn macro_sizeof(call: &MacroCall, scope: &mut Scope, code_gen: &mut CodeGenerator) -> ValueRef {
        if call.args.len() != 1 {
            Logger::error_msg_label(
                "macro `sizeof` expected exactly 1 argument",
                ErrorLabel::from(call.id.loc(), "invalid number of arguments")
            );
        }

        let arg = call.args[0][0].clone();
        let loc = arg.loc;
        let TokenType::Id(type_string) = arg.typ else {
            todo!();
        };
        let ty = Type::from_string(Tagged::new(loc, type_string)).unwrap();
        let ty = scope.resolve_type(&ty, &code_gen.ctx);
        let size = code_gen.ctx.module.get_data_layout().get_type_size_in_bits(&ty);
        ConstantInt::get(&TypeRef::get_int(code_gen.ctx.ctx, 64), size as i32 / 8)
        /*arg.push(Token::new(TokenType::Semi, arg.last().unwrap().loc.clone()));
        let mut parser = Parser::from(arg.clone());
        if let AST::Module(mut m) = parser.build_ast() {
            let ident = &mut m.body.body[0];
            let AST::Identifier(ident) = ident else {
                todo!();
            };

            let ty = if scope.has_symbol(ident.id.inner()) {
                if let symbols::Symbol::Variable(_) = scope.resolve_symbol(&ident.id) {
                    ident.accept(&mut code_gen.type_generator, scope)
                }else {
                    Type::from_string(ident.id.clone()).unwrap()
                }
            }else {
        }else {
            unreachable!();
        }*/
    }
}

impl ASTVisitor for Preprocessor {
    type Output = AST;

    fn visit_module(&mut self, _: &mut Module, _: &mut Scope) -> Self::Output {
        todo!()
    }

    fn visit_function(&mut self, _: &mut Function, _: &mut Scope) -> Self::Output {
        todo!()
    }

    fn visit_vardecl(&mut self, _: &mut VarDecl, _: &mut Scope) -> Self::Output {
        todo!()
    }

    fn visit_assignmentexpr(&mut self, _: &mut AssignmentExpr, _: &mut Scope) -> Self::Output {
        todo!()
    }

    fn visit_memberexpr(&mut self, _: &mut MemberExpr, _: &mut Scope) -> Self::Output {
        todo!()
    }

    fn visit_callexpr(&mut self, _: &mut CallExpr, _: &mut Scope) -> Self::Output {
        todo!()
    }

    fn visit_membercallexpr(&mut self, _: &mut MemberCallExpr, _: &mut Scope) -> Self::Output {
        todo!()
    }

    fn visit_accessexpr(&mut self, _: &mut AccessExpr, _: &mut Scope) -> Self::Output {
        todo!()
    }

    fn visit_unaryexpr(&mut self, _: &mut UnaryExpr, _: &mut Scope) -> Self::Output {
        todo!()
    }

    fn visit_resolutionexpr(&mut self, _: &mut ResolutionExpr, _: &mut Scope) -> Self::Output {
        todo!()
    }

    fn visit_castexpr(&mut self, _: &mut CastExpr, _: &mut Scope) -> Self::Output {
        todo!()
    }

    fn visit_returnexpr(&mut self, _: &mut ReturnExpr, _: &mut Scope) -> Self::Output {
        todo!()
    }

    fn visit_binaryexpr(&mut self, _: &mut BinaryExpr, _: &mut Scope) -> Self::Output {
        todo!()
    }

    fn visit_ifstmt(&mut self, _: &mut IfStmt, _: &mut Scope) -> Self::Output {
        todo!()
    }

    fn visit_forstmt(&mut self, _: &mut ForStmt, _: &mut Scope) -> Self::Output {
        todo!()
    }

    fn visit_breakexpr(&mut self, _: &mut BreakExpr, _: &mut Scope) -> Self::Output {
        todo!()
    }

    fn visit_structdef(&mut self, _: &mut StructDef, _: &mut Scope) -> Self::Output {
        todo!()
    }

    fn visit_enumdef(&mut self, _: &mut EnumDef, _: &mut Scope) -> Self::Output {
        todo!()
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

    fn visit_macrocall(&mut self, _: &mut MacroCall, _: &mut Scope) -> Self::Output {
        todo!()
    }

    fn visit_typealias(&mut self, _: &mut TypeAlias, _: &mut Scope) -> Self::Output {
        todo!()
    }

    fn visit_numberliteral(&mut self, _: &mut NumberLiteral, _: &mut Scope) -> Self::Output {
        todo!()
    }

    fn visit_stringliteral(&mut self, _: &mut StringLiteral, _: &mut Scope) -> Self::Output {
        todo!()
    }

    fn visit_structliteral(&mut self, _: &mut StructLiteral, _: &mut Scope) -> Self::Output {
        todo!()
    }

    fn visit_arrayliteral(&mut self, _: &mut ArrayLiteral, _: &mut Scope) -> Self::Output {
        todo!()
    }

    fn visit_staticliteral(&mut self, _: &mut StaticLiteral, _: &mut Scope) -> Self::Output {
        todo!()
    }

    fn visit_identifier(&mut self, _: &mut Identifier, _: &mut Scope) -> Self::Output {
        todo!()
    }

    fn visit_blockexpr(&mut self, _: &mut BlockExpr, _: &mut Scope) -> Self::Output {
        todo!()
    }
}
