use lygos_util as util;
use lygos_logging::Logger;
use lygos_ast::{types::*, *};
use lygos_token::{Location, Tagged, Token, TokenType};

pub struct Parser {
    tokens: Vec<Token>,
    index: usize,
    //preprocessor: Preprocessor,
    current_impl: util::Pointer<Impl>,
    current_trait: util::Pointer<Trait>,
    current_block: util::Pointer<Block>,
}

impl Parser {
    pub fn from(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            index: 0,
            //preprocessor: Preprocessor::new(),
            current_impl: util::Pointer::new(),
            current_trait: util::Pointer::new(),
            current_block: util::Pointer::new(),
        }
    }

    pub fn build_ast(&mut self) -> AST {
        let mut block = Block::new();
        loop {
            block.push(self.parse_globals());
            if self.at().typ == TokenType::Eof {
                break;
            }
        }
        AST::Module(Module::new(block))
    }

    #[inline]
    fn at(&self) -> &Token {
        &self.tokens[self.index]
    }

    #[inline]
    fn eat(&mut self) -> &Token {
        let tok = &self.tokens[self.index];
        self.index += 1;
        tok
    }

    #[inline]
    fn peek(&self, offset: isize) -> &Token {
        let index = (self.index as isize + offset) as usize;
        &self.tokens[index]
    }

    fn parse_globals(&mut self) -> AST {
        match self.at().typ {
            TokenType::KwStruct => self.parse_struct_decl(),
            TokenType::KwFn => self.parse_func(),
            TokenType::KwImpl => self.parse_impl(),
            TokenType::KwStatic => self.parse_static(),
            TokenType::KwTrait => self.parse_trait(),
            //TokenType::KwMacro => self.parse_macro(),
            //TokenType::Hash => self.parse_pound(),
            TokenType::KwEnum => self.parse_enum(),
            TokenType::KwType => self.parse_type_def(),
            _ => self.parse_stmt(),
        }
    }

    fn parse_struct_field(&mut self) -> StructField {
        let id = self.eat().clone();

        let TokenType::Id(id_value) = &id.typ else {
            Logger::token_expected(&self.peek(-1).loc, "unexpected token found", "expected identifier in field declaration");
        };

        if self.eat().typ != TokenType::Colon {
            Logger::token_expected(&self.peek(-1).loc, "missing type specifier", format!("field `{}` requires a type", id_value));
        }

        let typ = self.parse_type_spec();

        if self.eat().typ != TokenType::Semi {
            Logger::token_expected(&self.peek(-1).loc, "unexpected token found", "expected `;` after field declaration");
        }

        StructField{ typ, id: Tagged::new(id.loc, id_value.clone()) }
    }

    fn parse_struct_decl(&mut self) -> AST {
        self.eat();

        let id = self.eat().clone();

        let TokenType::Id(id_value) = &id.typ else {
            Logger::token_expected(&id.loc, "unexpected token found", "expected identifier after keyword `struct`");
        };

        if self.eat().typ != TokenType::CurlyLeft {
            Logger::token_expected_help(&self.peek(-1).loc, "unexpected token found", "expected `{` after struct identifier", "try adding `{`");
        }

        let mut fields = Vec::new();
        while self.at().typ != TokenType::CurlyRight {
            fields.push(self.parse_struct_field());
        }
        self.eat();

        /*if self.eat().typ != TokenType::Semi {
            Logger::token_expected(&self.peek(-1).loc, "unexpected token found", "expected, `;` after struct declaration");
        }*/

        AST::StructDef(StructDef::new(Tagged::new(id.loc, id_value.to_string()), fields))
    }

    fn parse_func_arg(&mut self) -> FunctionArg {
        let mut id = self.eat().clone();

        if id.typ == TokenType::Ampercent || id.typ == TokenType::KwMut || if let TokenType::Id(value) = &id.typ {
            value == "self"
        }else { false } {
            if self.current_impl.is_null() && self.current_trait.is_null() {
                Logger::token_expected(&id.loc, "unexpected token found", "member function can only be declared in an impl or trait block");
            }

            let typ = if id.typ == TokenType::Ampercent {
                id = self.eat().clone();
                let mut is_mut = false;
                if id.typ == TokenType::KwMut {
                    id = self.eat().clone();
                    is_mut = true;
                }
                if self.current_impl.is_null() {
                    Type::Pointer(Pointer::new(id.loc.clone(), Box::new(Type::Path(Path::new(id.loc.clone(), "".to_owned()))), true, is_mut))
                }else {
                    Type::Pointer(Pointer::new(id.loc.clone(), Box::new(Type::Path(Path::new(id.loc.clone(), self.current_impl.typ.inner().clone()))), true, is_mut))
                }
            }else {
                Type::Path(Path::new(id.loc.clone(), "".to_string()))
            };

            let TokenType::Id(value) = id.typ else {
                todo!("error");
            };
            if value != "self" {
                Logger::token_expected(&id.loc, "unexpected token found", "expected `self`")
            }

            return FunctionArg{ id: Tagged::new(id.loc, value), typ };
        }

        let TokenType::Id(value) = id.typ else {
            Logger::token_expected(&id.loc, "unexpected token found", "expected identifier in function signature");
        };

        if self.eat().typ != TokenType::Colon {
            Logger::token_expected(&self.peek(-1).loc, "unexpected token found", "expected type for function parameter");
        }

        let typ = self.parse_type_spec();

        FunctionArg{ id: Tagged::new(id.loc, value), typ }
    }

    fn parse_func(&mut self) -> AST {
        self.eat();

        let id = self.eat().clone();

        let TokenType::Id(_) = &id.typ else {
            Logger::token_expected(&id.loc, "unexpected token found", "expected identifier after keyword `fn`");
        };

        if self.eat().typ != TokenType::ParanLeft {
            Logger::token_expected(&self.peek(-1).loc, "unexpected token found", "expected `(`");
        }

        let mut args = Vec::new();

        while self.at().typ != TokenType::ParanRight {
            args.push(self.parse_func_arg());
            if self.at().typ == TokenType::ParanRight {
                break;
            }

            if self.eat().typ != TokenType::Comma {
                Logger::token_expected(&self.peek(-1).loc, "unexpected token found", "function arguments need to be comma seperated");
            }
        }
        self.eat();
        let ret_type = if self.at().typ == TokenType::Arrow {
            self.eat();
            self.parse_type_spec()
        }else {
            Type::Path(Path::new(Location::new("internal", 0..1), "void".to_string()))
        };

        let c_impl = if self.current_impl.is_some() {
            Some(Impl::new(self.current_impl.typ.clone(), Block::new(), self.current_impl.trat.clone()))
        }else {
            None
        };

        //let attributes = attr.map(|v| vec![v]).unwrap_or(Vec::new());

        if self.at().typ == TokenType::Semi {
            self.eat();
            return AST::Function(Function::new(id.into(), c_impl, args, Block::new(), ret_type, false, false));
        }

        if self.eat().typ != TokenType::CurlyLeft {
            Logger::token_expected(&self.peek(-1).loc, "unexpected token found", "expected `{` after function )signature");
        }

        let mut body = Block::new();
        while self.at().typ != TokenType::CurlyRight {
            self.current_block = util::Pointer::from(&body);
            body.body.push(self.parse_stmt());
        }
        self.eat();
        AST::Function(Function::new(id.into(), c_impl, args, body, ret_type, true, false))
    }

    fn parse_impl(&mut self) -> AST {
        self.eat();

        let mut trat = None;
        if matches!(self.at().typ, TokenType::Id(_)) && self.peek(1).typ == TokenType::KwFor {
            trat = Some(self.eat().clone());
            self.eat();
        }

        let struct_impl = self.eat().clone();

        if !matches!(struct_impl.typ, TokenType::Id(_)) {
            Logger::token_expected(&struct_impl.loc, "unexpected token found", "expected identifier after keyword `impl`");
        }

        if self.eat().typ != TokenType::CurlyLeft {
            Logger::token_expected_help(&self.peek(-1).loc, "unexpected token found", "expected `{`", "try adding `{`");
        }

        let mut r#impl = Impl::new(struct_impl.into(), Block::new(), trat.map(|v| v.into()));
        self.current_impl = util::Pointer::from(&r#impl);
        while self.at().typ != TokenType::CurlyRight {
            self.current_block = util::Pointer::from(&r#impl.body);
            r#impl.body.body.push(self.parse_func());
        }
        self.eat();

        self.current_impl = util::Pointer::new();
        AST::Impl(r#impl)
    }

    fn parse_static(&mut self) -> AST {
        self.eat();

        let id = self.eat().clone();

        if !matches!(id.typ, TokenType::Id(_)) {
            Logger::token_expected(&id.loc, "unexpected token found", "expected `identifier` after keyword `static`");
        }

        if self.eat().typ != TokenType::Colon {
            Logger::token_expected(&self.peek(-1).loc, "type annotation missing", "expected type for static item");
        }

        let ty = self.parse_type_spec();

        let value = if self.at().typ == TokenType::Equals {
            self.eat();
            Some(Box::new(self.parse_expr()))
        }else {
            None
        };

        if self.eat().typ != TokenType::Semi {
            Logger::token_expected_help(&self.peek(-1).loc, "unexpected token found", "expected `;` at the end of expression", "try adding `;`");
        }

        AST::StaticLiteral(StaticLiteral::new(id.into(), ty, value))
    }

    fn parse_trait(&mut self) -> AST {
        self.eat();

        let id = self.eat().clone();
        let TokenType::Id(id_value) = &id.typ else {
            Logger::token_expected(&id.loc, "unexpected token found", "expected identifier after keyword `impl`");
        };

        if self.eat().typ != TokenType::CurlyLeft {
            Logger::token_expected_help(&self.peek(-1).loc, "unexpected token found", "expected `{`", "try adding `{`");
        }

        let mut trat = Trait::new(Tagged::new(id.loc.clone(), id_value.clone()), Block::new());
        self.current_trait = util::Pointer::from(&trat);
        while self.at().typ != TokenType::CurlyRight {
            self.current_block = util::Pointer::from(&trat.funcs);
            trat.funcs.body.push(self.parse_func());
        }
        self.eat();

        self.current_trait = util::Pointer::new();
        AST::Trait(trat)
    }

    /*fn parse_macro(&mut self) -> AST {
        self.eat();

        let id = self.eat().clone();
        if id.typ != TokenType::Id {
            token_expected(&id.loc, "unexpected token found", "expected identifier after keyword `macro`");
        }

        if self.eat().typ != TokenType::CurlyLeft {
            token_expected_help(&self.peek(-1).loc, "unexpected token found", "expected `{` after macro name", "try adding `{`");
        }

        let mut arms = Vec::new();

        while self.at().typ != TokenType::CurlyRight {
            if self.eat().typ != TokenType::ParanLeft {
                token_expected_help(&self.peek(-1).loc, "unexpected token found", "expected `(`", "try adding `(`");
            }

            let mut conds = Vec::new();
            while self.at().typ != TokenType::ParanRight {
                if self.at().typ != TokenType::Id {
                    token_expected(&self.at().loc, "unexpected token found", "expected identifier");
                }

                let tok_name = self.eat().clone();

                if self.eat().typ != TokenType::Colon {
                    token_expected_help(&self.peek(-1).loc,
                                            "unexpected token found",
                                            format!("expected `:` after macro arg `{}`", tok_name.value).as_str(),
                                            "try adding `:`"
                                        );
                }

                let typ = if self.at().typ == TokenType::Dollar {
                    self.eat();
                    if self.at().typ == TokenType::BraceLeft {
                        self.eat();
                        let ty = MacroArgType::Variadic;
                        if self.eat().typ != TokenType::BraceRight {
                            token_expected_help(&self.peek(-1).loc, "unexpected token found", "expected closing bracked `}`", "try adding `}`");
                        }
                        ty
                    }else {
                        MacroArgType::Single
                    }
                }else {
                    todo!();
                };

                for MacroArg { id, .. } in &conds {
                    if *id.inner() == tok_name.value {
                        token_expected(&tok_name.loc, "douplicate symbol found", format!("found duplicate identifier in macro arm param `{}`", tok_name.value).as_str());
                    }
                }
                conds.push(MacroArg { id: tok_name.into(), typ });
                if self.at().typ == TokenType::ParanRight {
                    break;
                }
                if self.at().typ != TokenType::Comma {
                    token_expected(&self.at().loc, "unexpected token found", "macro args need to be `,` seperated");
                }else {
                    self.eat();
                }
            }
            self.eat();

            let mut block = Vec::new();

            if self.eat().typ != TokenType::Arrow {
                token_expected(&self.peek(-1).loc, "unexpected token found", "expected `->` after arm decl");
            }

            if self.eat().typ != TokenType::CurlyLeft {
                token_expected_help(&self.peek(-1).loc, "unexpected token found", "expected `{` after macro arm delimiter", "try adding `{`");
            }

            let mut braces = 0;
            loop {
                let tok = self.at();
                if tok.typ == TokenType::CurlyLeft {
                    braces += 1;
                }
                if tok.typ == TokenType::CurlyRight {
                    braces -= 1;
                }
                block.push(self.eat().clone());
                if self.at().typ == TokenType::CurlyRight && braces == 0 {
                    break;
                }
            }
            self.eat();
            arms.push(MacroArm{ conds, tokens: block });
        }
        self.eat();

        let mac = Macro::new(id.into(), arms);

        self.preprocessor.add_macro(mac.clone());
        return AST::Macro(mac);
    }

    fn parse_attribute(&mut self) -> AST {
        //self.eat();
        if self.at().typ != TokenType::Id {
            token_expected(&self.at().loc, "unexpected token found", "expected identifier");
        }
        let ident = Identifier::new(Tagged::from(self.eat().clone()));
        if self.eat().typ != TokenType::BraceRight {
            token_expected(&self.peek(-1).loc, "unexpected token found", "expected `]` at the end of attribute declaration");
        }
        let attribute = Attribute::new(ident.clone());
        return self.parse_func(Some(attribute));
    }

    fn parse_pound(&mut self) -> AST {
        let start_index = self.index;
        self.eat();
        match &self.eat().value.as_str() {
            &"include" => {
                let is_def = if self.at().typ == TokenType::AngleLeft { true } else { false };
                let tok = if is_def { self.eat(); self.eat().clone() } else { self.eat().clone() };
                if is_def && self.eat().typ != TokenType::AngleRight {
                    token_expected(&tok.loc, "unexpected token found", "expected closing bracked `>`");
                }
                let stream = self.preprocessor.expand_include(IncludeFile{ path: Tagged::new(tok.loc, tok.value.into()), is_default_include_path: is_def });
                let len = self.index - start_index;
                self.index = start_index;
                for _ in 0..len {
                    self.tokens.remove(self.index);
                }

                for t in stream.0.iter().rev() {
                    self.tokens.insert(self.index, t.clone());
                }
                return self.parse_globals();
            },
            &"[" => {
                return self.parse_attribute();
            }
            _ => token_expected(&self.at().loc, "unexpected token found", "invalid value after `#`"),
        }
    }*/

    fn parse_enum(&mut self) -> AST {
        self.eat();

        let id = self.eat().clone();

        if !matches!(id.typ, TokenType::Id(_)) {
            Logger::token_expected(&id.loc, "unexpected token found", "expected identifier after keyword `enum`");
        }

        let ty: Type = if self.at().typ == TokenType::Colon {
            self.eat();
            let ty = self.parse_type_spec();
            if let Type::Path(_) = &ty {
                ty
            }else {
                Logger::token_expected(&ty.get_loc(), "invalid enum type", format!("enums may only contain primitive types but has {}", ty.get_full_name()).as_str());
            }
        }else {
            Type::Path(Path::new(id.loc.clone(), "u32"))
        };

        if self.eat().typ != TokenType::CurlyLeft {
            Logger::token_expected(&self.peek(-1).loc, "unexpected token found", "expected `{` after enum identifier");
        }

        let mut variants = Vec::new();

        while self.at().typ != TokenType::CurlyRight {
            let TokenType::Id(variant) = &self.at().typ else {
                Logger::token_expected(&self.at().loc, "unexpected token found", "expected identifier in enum declaration");
            };
            variants.push(variant.clone());
            self.eat();

            if self.at().typ == TokenType::CurlyRight {
                break;
            }

            if self.eat().typ != TokenType::Comma {
                Logger::token_expected(&self.peek(-1).loc, "unexpected token found", "enum variants need to be comma `,` seperated");
            }
        }
        self.eat();

        AST::EnumDef(EnumDef::new(id.into(), variants, ty))
    }

    fn parse_type_def(&mut self) -> AST {
        self.eat();

        let id = self.eat().clone();
        let TokenType::Id(_) = id.typ else {
            Logger::token_expected(&id.loc, "unexpected token found", "expected identifier after keyword `type`");
        };

        if self.eat().typ != TokenType::Equals {
            Logger::token_expected(&self.peek(-1).loc, "unexpected token found", "expected `=` after type identifier");
        }

        let ty = self.parse_type_spec();

        if self.eat().typ != TokenType::Semi {
            Logger::token_expected(&self.peek(-1).loc, "unexpected token found", "expected `;` at the end of expression");
        }

        AST::TypeAlias(TypeAlias::new(id.into(), ty))
    }

    fn parse_stmt(&mut self) -> AST {
        match self.at().typ {
            TokenType::KwIf => self.parse_if_stmt(),
            TokenType::KwFor => self.parse_for_stmt(),
            //TokenType::KwMatch => self.parse_match_stmt(),
            _ => {
                let node = self.parse_expr();
                if self.eat().typ != TokenType::Semi {
                    Logger::token_expected_help(&self.peek(-1).loc, "unexpected token found", "expected `;` at the end of expression", "add `;` to mark end of expression");
                }
                node
            }
        }
    }

    fn parse_if_stmt(&mut self) -> AST {
        self.eat();
        let cond = self.parse_expr();

        if self.eat().typ != TokenType::CurlyLeft {
            Logger::token_expected(&self.peek(-1).loc, "unexpected token found", "expected `{` after if statement");
        }

        let mut then_branch = Block::new();
        while self.at().typ != TokenType::CurlyRight {
            self.current_block = util::Pointer::from(&then_branch);
            then_branch.body.push(self.parse_stmt());
        }
        self.eat();

        let else_branch = if self.at().typ == TokenType::KwElse {
            self.eat();

            if self.eat().typ != TokenType::CurlyLeft {
                Logger::token_expected(&self.peek(-1).loc, "unexpected token found", "expected `{` after if statement");
            }

            let mut else_branch = Block::new();
            while self.at().typ != TokenType::CurlyRight {
                self.current_block = util::Pointer::from(&else_branch);
                else_branch.body.push(self.parse_stmt());
            }
            self.eat();
            Some(else_branch)
        }else {
            None
        };

        AST::IfStmt(IfStmt::new(Box::new(cond), then_branch, else_branch))
    }

    fn parse_for_stmt(&mut self) -> AST {
        self.eat();

        let var = self.parse_expr();

        if let AST::VarDecl(_) = var {} else {
            Logger::token_expected(&self.peek(-1).loc, "unexpected token found", "expected expression after keyword `for`");
        }

        if self.eat().typ != TokenType::KwIn {
            Logger::token_expected(&self.peek(-1).loc, "unexpected token found", "expected `in` after expression");
        }

        let cond = self.parse_expr();


        if self.eat().typ != TokenType::CurlyLeft {
            Logger::token_expected(&self.peek(-1).loc, "unexpected token found", "expected `{` after for statement");
        }

        let mut block = Block::new();
        while self.at().typ != TokenType::CurlyRight {
            self.current_block = util::Pointer::from(&block);
            block.body.push(self.parse_stmt());
        }
        self.eat();

        AST::ForStmt(ForStmt::new(Box::new(cond), Box::new(var), block))
    }

    /*fn parse_match_stmt(&mut self) -> AST {
        todo!();
    }

    /*fn parse_block_expr(&mut self) -> AST {
        self.eat();
        let mut block = Block::new();
        let mut returns = false;
        while self.at().typ != TokenType::CurlyRight {
            self.current_block = containers::Pointer::from(&block);
            let expr = self.parse_stmt();
            if let AST::ClaimExpr(_) = &expr {
                if self.at().typ != TokenType::CurlyRight {
                    token_expected(expr.loc(), "unexpected token found", "expression after claim");
                }
                block.body.push(expr);
                returns = true;
                break;
            }
            block.body.push(expr);
        }
        self.eat();
        return AST::BlockExpr(BlockExpr::new(block, returns));
    }*/*/

    fn parse_expr(&mut self) -> AST {
        match self.at().typ {
            TokenType::KwLet => self.parse_var_decl(),
            TokenType::KwRet => self.parse_ret_expr(),
            TokenType::Id(_) if self.peek(1).typ == TokenType::Dollar && self.peek(2).typ == TokenType::CurlyLeft => {
                self.parse_struct_literal()
            },
            _ => self.parse_cond_expr(),
        }
    }

    fn parse_var_decl(&mut self) -> AST {
        self.eat();

        let mut is_const = true;
        let mut typ = None;
        let mut tok = self.eat().clone();
        if tok.typ == TokenType::KwMut {
            is_const = false;
            tok = self.eat().clone();
        }

        let TokenType::Id(var_name) = &tok.typ else {
            Logger::token_expected(&tok.loc, "unexpected token found", "expected identifier after keyword `let`");
        };

        let mut op = self.eat().clone();

        if op.typ == TokenType::Colon {
            typ = Some(self.parse_type_spec());
            op = self.at().clone();
            if op.typ == TokenType::Equals {
                self.eat();
            }
        }
        match op.typ {
            TokenType::Semi => {
                if is_const {
                    Logger::token_expected(&tok.loc, "unexpected token found", format!("constant value {} needs to be assigned", &var_name).as_str());
                }
                AST::VarDecl(VarDecl::new(tok.into(), is_const, typ, None))
            }
            TokenType::Equals => {
                let expr = self.parse_expr();
                AST::VarDecl(VarDecl::new(tok.into(), is_const, typ, Some(Box::new(expr))))
            }
            _ => Logger::token_expected(&self.at().loc, "unexpected token found", "expected `=` or `;` after variable declaration"),
        }
    }

    /*fn parse_claim_expr(&mut self) -> AST {
        return AST::ClaimExpr(ClaimExpr::new(self.eat().loc.clone(), Box::new(self.parse_expr())));
    }*/

    fn parse_ret_expr(&mut self) -> AST {
        let loc = self.eat().loc.clone();
        self.current_block.as_mut().returns = true;
        if self.at().typ == TokenType::Semi {
            return AST::ReturnExpr(ReturnExpr::new(loc, None));
        }
        AST::ReturnExpr(ReturnExpr::new(loc, Some(Box::new(self.parse_expr()))))
    }

    fn parse_cond_expr(&mut self) -> AST {
        let mut lhs = self.parse_additive_expr();
        while self.at().typ == TokenType::OpEqEq
           || self.at().typ == TokenType::OpNeEq
           || self.at().typ == TokenType::AngleLeft
           || self.at().typ == TokenType::AngleRight
           || self.at().typ == TokenType::OpLeEq
           || self.at().typ == TokenType::OpGrEq {
               let op = self.eat().clone();
               let rhs = self.parse_additive_expr();
               lhs = AST::BinaryExpr(BinaryExpr::new(Box::new(lhs), Box::new(rhs), op.into()));
               while self.at().typ == TokenType::OpAnd || self.at().typ == TokenType::OpOr {
                   let op = self.eat().clone();
                   lhs = AST::BinaryExpr(BinaryExpr::new(Box::new(lhs), Box::new(self.parse_cond_expr()), op.into()));
               }
        }
        lhs
    }

    fn parse_additive_expr(&mut self) -> AST {
        let mut lhs = self.parse_mult_expr();
        while self.at().typ == TokenType::OpPlus || self.at().typ == TokenType::OpMinus {
            let op = self.eat().clone();
            let rhs = self.parse_mult_expr();
            lhs = AST::BinaryExpr(BinaryExpr::new(Box::new(lhs), Box::new(rhs), op.into()));
        }
        lhs
    }

    fn parse_mult_expr(&mut self) -> AST {
        let mut lhs = self.parse_assignment_expr();
        while self.at().typ == TokenType::OpMul || self.at().typ == TokenType::OpDiv || self.at().typ == TokenType::OpMod {
            let op = self.eat().clone();
            let rhs = self.parse_assignment_expr();
            lhs = AST::BinaryExpr(BinaryExpr::new(Box::new(lhs), Box::new(rhs), op.into()));
        }
        lhs
    }

    fn parse_assignment_expr(&mut self) -> AST {
        let lhs = self.parse_index_expr();
        if self.at().typ == TokenType::Equals {
            self.eat();
            let rhs = self.parse_expr();
            return AST::AssignmentExpr(AssignmentExpr::new(Box::new(lhs), Box::new(rhs)));
        }
        lhs
    }

    fn parse_index_expr(&mut self) -> AST {
        let mut obj = self.parse_member_expr();
        while self.at().typ == TokenType::BraceLeft {
            self.eat();
            let idx = self.parse_expr();
            if self.eat().typ != TokenType::BraceRight {
                Logger::token_expected(&self.peek(-1).loc, "unexpected token found", "expected closing bracket `]`");
            }
            obj = AST::AccessExpr(AccessExpr::new(Box::new(obj), Box::new(idx)));
        }
        obj
    }

    fn parse_member_expr(&mut self) -> AST {
        let mut obj = self.parse_call_expr();
        while self.at().typ == TokenType::Dot || self.at().typ == TokenType::Arrow {
            let deref = self.eat().typ == TokenType::Arrow;
            let member = self.parse_call_expr();
            obj = if let AST::CallExpr(_) = member {
                AST::MemberCallExpr(MemberCallExpr::new(Box::new(obj), Box::new(member), deref))
            }else {
                AST::MemberExpr(MemberExpr::Member{ obj: Box::new(obj), member: Box::new(member), deref })
            };
        }
        obj
    }

    fn parse_call_expr(&mut self) -> AST {
        let mut callee = self.parse_resolution_expr();
        let is_macro = if self.at().typ == TokenType::Dollar { self.eat(); true } else { false };

        if is_macro && self.at().typ == TokenType::ParanLeft {
            //let start_index = self.index - 2;
            self.eat();
            let mut args = Vec::new();
            while self.at().typ != TokenType::ParanRight {
                let mut arg = Vec::new();
                let mut depth: usize = 0;
                while self.at().typ != TokenType::Comma {
                    let tok = self.at();
                    if tok.typ == TokenType::ParanLeft {
                        depth += 1;
                    }
                    if tok.typ == TokenType::ParanRight {
                        depth -= 1;
                    }
                    arg.push(self.eat().clone());
                    if self.at().typ == TokenType::ParanRight && depth == 0 {
                        break;
                    }
                }
                args.push(arg);

                if self.at().typ == TokenType::ParanRight {
                    break;
                }

                if self.eat().typ != TokenType::Comma {
                    Logger::token_expected(&self.peek(-1).loc, "unexpected token found", "macro call parameters need to be comma seperated");
                }
            }
            self.eat();
            let AST::Identifier(id) = &callee else {
                todo!();
            };
            let call = MacroCall::new(id.id.clone(), args);
            /*if let Some(stream) = self.preprocessor.expand_macro(&mut call) {
                let len = self.index - start_index;
                self.index = start_index;
                for _ in 0..len + 1 {
                    self.tokens.remove(self.index);
                }

                for t in stream.0.into_iter().rev() {
                    self.tokens.insert(self.index, t);
                }
                callee = self.parse_globals();

                /*
                 *  FIXME(S):
                 *  figure out what to do with the `;` at the end of macro invocation, after the expantion
                 *
                 */
                self.tokens.insert(self.index, Token::new(";", TokenType::Semi, Loc::new("inte".into(), 0, 1)));
            }else {*/
                callee = AST::MacroCall(call);
            //}
        }

        if self.at().typ == TokenType::ParanLeft {
            self.eat();
            let mut args = Block::new();
            while self.at().typ != TokenType::ParanRight {
                args.body.push(self.parse_expr());
                if self.at().typ == TokenType::ParanRight {
                    break;
                }
                if self.eat().typ != TokenType::Comma {
                    Logger::token_expected(&self.peek(-1).loc, "unexpected token found", "function parameters need to be comma seperated");
                }
            }
            self.eat();

            callee = AST::CallExpr(CallExpr::new(Box::new(callee), args.body));
        }
        callee
    }

    fn parse_resolution_expr(&mut self) -> AST {
        let mut obj = self.parse_unary_expr();
        while self.at().typ == TokenType::OpScope {
            self.eat();
            let member = self.parse_expr();
            obj = AST::ResolutionExpr(ResolutionExpr::new(Box::new(obj), Box::new(member)));
        }
        obj
    }

    fn parse_unary_expr(&mut self) -> AST {
        if self.at().typ == TokenType::Ampercent
        || self.at().typ == TokenType::OpMul
        || self.at().typ == TokenType::Bang {
            let op = self.eat().clone();
            let obj = self.parse_expr();
            return AST::UnaryExpr(UnaryExpr::new(op.into(), Box::new(obj)));
        }
        self.parse_paran_expr()
    }

    fn parse_paran_expr(&mut self) -> AST {
        if self.at().typ == TokenType::ParanLeft {
            self.eat();
            if self.at().typ == TokenType::Colon {
                self.eat();
                let typ = self.parse_type_spec();
                if self.eat().typ != TokenType::ParanRight {
                    Logger::token_expected(&self.peek(-1).loc, "unexpected token found", "expected closing paran `)`");
                }
                let expr = self.parse_expr();
                return AST::CastExpr(CastExpr::new(Box::new(expr), typ));
            }
            let expr = self.parse_expr();
            if self.eat().typ != TokenType::ParanRight {
                Logger::token_expected(&self.peek(-1).loc, "unexpected token found", "expected closing paran `)`");
            }
            return expr;
        }
        self.parse_primary_expr()
    }

    /*fn parse_closure(&mut self) -> AST {
        let loc = self.eat().loc.clone();
        let mut args = Vec::new();
        while self.at().typ != TokenType::Pipe {
            args.push(self.parse_func_arg());
            if self.at().typ == TokenType::ParanRight {
                break;
            }

            if self.eat().typ != TokenType::Comma {
                token_expected(&self.peek(-1).loc, "unexpected token found", "closure arguments need to be comma seperated");
            }
        }
        self.eat();
        let ret_type = if self.at().typ == TokenType::Arrow {
            self.eat();
            self.parse_type_spec()
        }else {
            Type::Path(Path::new(Loc::new("internal".into(), 0, 1), "void".to_string()))
        };

        if self.eat().typ != TokenType::CurlyLeft {
            token_expected(&self.peek(-1).loc, "unexpected token found", "expected `{` after for statement");
        }
        let mut body = Block::new();
        while self.at().typ != TokenType::CurlyRight {
            self.current_block = containers::Pointer::from(&body);
            body.body.push(self.parse_stmt());
        }
        self.eat();

        return AST::ClosureExpr(ClosureExpr::new(loc.clone(), Function::new(Tagged::new(loc, "".into()), None, args, body, ret_type, true, false, Vec::new())));
    }*/

    fn parse_struct_literal(&mut self) -> AST {
        let struct_ident = self.eat().clone();
        if !matches!(struct_ident.typ, TokenType::Id(_)) {
            unreachable!();
        };
        self.eat();
        if TokenType::CurlyLeft != self.eat().typ {
            unreachable!();
        }
        let mut values: Vec<(Option<Tagged<String>>, AST)> = Vec::new();
        while self.at().typ != TokenType::CurlyRight {
            let mut name = None;
            if self.at().typ == TokenType::Dot {
                self.eat();
                let id = self.eat().clone();
                let TokenType::Id(_) = id.typ else {
                    Logger::token_expected(&id.loc, "unexpected token found", "expected `identifier` after `.` in struct literal list");
                };

                name = Some(id.into());

                if self.eat().typ != TokenType::Equals {
                    Logger::token_expected(&self.peek(-1).loc, "unexpected token found", "expected `=` between member and value");
                }
            }
            values.push((name, self.parse_expr()));

            if self.at().typ == TokenType::CurlyRight {
                break;
            }

            if self.eat().typ != TokenType::Comma {
                Logger::token_expected(&self.peek(-1).loc, "unexpected token found", "struct literal list values need to be comma `,` seperated");
            }
        }
        self.eat();
        AST::StructLiteral(StructLiteral::new(struct_ident.into(), values))
    }

    fn parse_array_literal(&mut self) -> AST {
        let start_tok = self.eat().clone();
        if TokenType::BraceLeft != start_tok.typ {
            unreachable!();
        }
        let start = start_tok.loc;
        let mut values: Vec<AST> = Vec::new();
        while self.at().typ != TokenType::BraceRight {
            values.push(self.parse_expr());
            if self.at().typ == TokenType::BraceRight {
                break;
            }

            if self.eat().typ != TokenType::Comma {
                Logger::token_expected(&self.peek(-1).loc, "unexpected token found", "initializer list values need to be comma `,` seperated");
            }
        }
        let end = self.eat().loc.clone();
        AST::ArrayLiteral(ArrayLiteral::new(start + end, values))
    }

    fn parse_primary_expr(&mut self) -> AST {
        let tok = self.at().clone();
        match &tok.typ {
            TokenType::Id(_) => {
                //if self.peek(1).typ == TokenType::Dollar && self.peek(2).typ == TokenType::CurlyLeft {
                //    self.parse_struct_literal()
                //}else {
                    AST::Identifier(Identifier::new(self.eat().clone().into()))
                //}
            },
            TokenType::BraceLeft => {
                self.parse_array_literal()
            },
            TokenType::Integer(_) => AST::NumberLiteral(NumberLiteral::new(self.eat().clone().into(), NumberType::Int)),
            TokenType::Float(_) => AST::NumberLiteral(NumberLiteral::new(self.eat().clone().into(), NumberType::Float)),
            TokenType::Char(_) => AST::NumberLiteral(NumberLiteral::new(self.eat().clone().into(), NumberType::Char)),
            TokenType::String(_) => AST::StringLiteral(StringLiteral::new(self.eat().clone().into())),
            TokenType::KwBreak => AST::BreakExpr(BreakExpr::new(self.eat().loc.clone())),
            //TokenType::KwContinue => todo!("implement parsing of `KwContinue`"),
            //TokenType::Pipe => self.parse_closure(),
            _ => {
                Logger::token_expected(&self.at().loc, format!("unexpected token found: `{:?}`", tok.typ).as_str(), "unexpected token found");
            }
        }
    }

    fn parse_type_spec(&mut self) -> Type {
        if self.at().typ == TokenType::OpMul || self.at().typ == TokenType::Ampercent {
            let pos = &self.at().loc.clone();
            let is_ref = self.eat().typ == TokenType::Ampercent;
            let is_mut = if self.at().typ == TokenType::KwMut { self.eat(); true } else { false };

            return Type::Pointer(Pointer::new(pos.clone(), Box::new(self.parse_type_spec()), is_ref, is_mut))
        }

        if self.at().typ == TokenType::BraceLeft {
            let start = self.eat().loc.range.start;
            let ty = self.parse_type_spec();

            let delim = self.eat();
            if delim.typ != TokenType::Semi && delim.typ != TokenType::BraceRight {
                Logger::token_expected(&delim.loc, "unexpected token found", "expected `;` or `]` after type");
            }

            if delim.typ == TokenType::BraceRight {
                return Type::Slice(ArraySlice::new(ty.get_loc(), ty));
            }

            let TokenType::Integer(_) = self.at().typ else {
                Logger::token_expected(&self.at().loc, "non constant in static context", "expected constant value");
            };

            let elems = self.eat().typ.as_str().parse::<usize>().unwrap();

            if self.eat().typ != TokenType::BraceRight {
                Logger::token_expected(&self.peek(-1).loc, "unexpected token found", "expected closing brace `]`");
            }

            return Type::Array(Array::new(Location::new(self.peek(-1).loc.file.clone(), start..self.peek(-1).loc.range.end), ty, elems));
        }

        if self.at().typ == TokenType::KwFn {
            let start = self.eat().loc.range.start;
            if self.eat().typ != TokenType::ParanLeft {
                Logger::token_expected(&self.peek(-1).loc, "unexpected token found", "expected `(` after keyword `fn`");
            }

            let mut params = Vec::new();
            while self.at().typ != TokenType::ParanRight {
                params.push(self.parse_type_spec());
                if self.at().typ == TokenType::ParanRight {
                    break;
                }
                if self.eat().typ != TokenType::Comma {
                    Logger::token_expected(&self.peek(-1).loc, "unexpected token found", "function params have to be comma seperated");
                }
            }
            self.eat();

            let ret_type = if self.at().typ == TokenType::Arrow {
                self.eat();
                self.parse_type_spec()
            }else {
                Type::Path(Path::new(Location::new("internal", 0..1), "void".to_string()))
            };

            return Type::FuncPtr(FuncPtr::new(Location::new(self.peek(-1).loc.file.clone(), start..self.peek(-1).loc.range.end), params, ret_type));
        }

        let path_loc = self.at().loc.clone();
        let mut path = self.eat().typ.as_str();
        if path == "Self" {
            if self.current_impl.is_null() && self.current_trait.is_null() {
                Logger::token_expected(&path_loc, "unexpected token found", "encountered type `Self` outside of `impl` or `trait` block");
            }

            if !self.current_impl.is_null() {
                path = self.current_impl.as_ref().typ.inner().into();
            }
        }
        Type::Path(Path::new(path_loc, path))
    }
}
