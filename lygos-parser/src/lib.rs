use lygos_ast::{types::*, *};
use lygos_logging::Logger;
use lygos_token::{Location, Tagged, Token, TokenType};
pub mod comptime;

pub struct Parser {
    tokens: Vec<Token>,
    index: usize,
    current_impl: Option<Impl>,
    current_trait: Option<Tagged<String>>,
}

impl Parser {
    pub fn from(mut tokens: Vec<Token>) -> Self {
        if tokens
            .last()
            .is_none_or(|token| token.typ != TokenType::Eof)
        {
            let loc = tokens
                .last()
                .map(|token| token.loc.clone())
                .unwrap_or(Location::new("", 0..0));
            tokens.push(Token::new(TokenType::Eof, loc));
        }
        Self {
            tokens,
            index: 0,
            current_impl: None,
            current_trait: None,
        }
    }

    pub fn build_ast(&mut self) -> AST {
        let ast = self.build_raw_ast();
        match comptime::expand(ast) {
            Ok(ast) => ast,
            Err(error) => Logger::diagnostic(
                &error.message,
                &error
                    .related
                    .iter()
                    .map(|(loc, message)| lygos_logging::ErrorLabel::from(loc, message))
                    .chain(std::iter::once(lygos_logging::ErrorLabel::from(
                        &error.loc,
                        "compile-time expansion failed here",
                    )))
                    .collect::<Vec<_>>(),
                error.help.as_deref(),
                None,
            ),
        }
    }

    pub fn build_raw_ast(&mut self) -> AST {
        self.validate_delimiters();
        let mut block = Block::new();
        while self.at().typ != TokenType::Eof {
            block.push(self.parse_globals());
        }
        AST::Module(Module::new(block))
    }

    /// Parse a substituted macro body without inventing a source function.
    pub fn build_statements(&mut self) -> Block {
        self.validate_delimiters();
        let mut block = Block::new();
        while self.at().typ != TokenType::Eof {
            block.body.push(self.parse_stmt());
        }
        block
    }

    fn validate_delimiters(&self) {
        let mut stack: Vec<(&Token, TokenType)> = Vec::new();
        for token in &self.tokens[self.index..] {
            let closing = match token.typ {
                TokenType::ParanLeft => Some(TokenType::ParanRight),
                TokenType::BraceLeft => Some(TokenType::BraceRight),
                TokenType::CurlyLeft => Some(TokenType::CurlyRight),
                _ => None,
            };
            if let Some(closing) = closing {
                stack.push((token, closing));
                continue;
            }
            if matches!(
                token.typ,
                TokenType::ParanRight | TokenType::BraceRight | TokenType::CurlyRight
            ) {
                match stack.pop() {
                    Some((_, expected)) if token.typ == expected => {}
                    Some((opening, expected)) => lygos_logging::Logger::diagnostic(
                        "mismatched delimiter",
                        &[
                            lygos_logging::ErrorLabel::from(
                                &token.loc,
                                format!(
                                    "expected `{}`, found `{}`",
                                    expected.as_str(),
                                    token.typ.as_str()
                                ),
                            ),
                            lygos_logging::ErrorLabel::from(
                                &opening.loc,
                                "opening delimiter is here",
                            ),
                        ],
                        Some(&format!(
                            "replace this delimiter with `{}`",
                            expected.as_str()
                        )),
                        None,
                    ),
                    None => Logger::token_expected_help(
                        &token.loc,
                        "unexpected closing delimiter",
                        "there is no matching opening delimiter",
                        "remove this delimiter or add its matching opener",
                    ),
                }
            }
            if token.typ == TokenType::Eof {
                if let Some((opening, closing)) = stack.last() {
                    lygos_logging::Logger::diagnostic(
                        "unclosed delimiter",
                        &[
                            lygos_logging::ErrorLabel::from(
                                &token.loc,
                                format!("expected `{}` before end of file", closing.as_str()),
                            ),
                            lygos_logging::ErrorLabel::from(
                                &opening.loc,
                                "opening delimiter is here",
                            ),
                        ],
                        Some(&format!("add the matching `{}`", closing.as_str())),
                        None,
                    );
                }
                break;
            }
        }
    }

    #[inline]
    fn at(&self) -> &Token {
        &self.tokens[self.index]
    }

    #[inline]
    fn eat(&mut self) -> &Token {
        let tok = &self.tokens[self.index];
        if tok.typ != TokenType::Eof {
            self.index += 1;
        }
        tok
    }

    #[inline]
    fn peek(&self, offset: isize) -> &Token {
        let index = self
            .index
            .saturating_add_signed(offset)
            .min(self.tokens.len() - 1);
        &self.tokens[index]
    }

    fn parse_globals(&mut self) -> AST {
        match self.at().typ {
            TokenType::Hash => self.parse_attributes(),
            TokenType::KwImport => self.parse_import(),
            TokenType::KwStruct => self.parse_struct_decl(),
            TokenType::KwFn => self.parse_func(),
            TokenType::KwImpl => self.parse_impl(),
            TokenType::KwStatic => self.parse_static(),
            TokenType::KwTrait => self.parse_trait(),
            TokenType::KwMacro => self.parse_macro(),
            //TokenType::Hash => self.parse_pound(),
            TokenType::KwEnum => self.parse_enum(),
            TokenType::KwType => self.parse_type_def(),
            _ => self.parse_stmt(),
        }
    }

    fn parse_import(&mut self) -> AST {
        let loc = self.eat().loc.clone();
        let token = self.eat().clone();
        let (source, is_path) = match &token.typ {
            TokenType::Id(name) => (name.clone(), false),
            TokenType::String(path) => (path.clone(), true),
            _ => Logger::token_expected(
                &token.loc,
                "invalid import",
                "expected a module name or quoted file path",
            ),
        };
        let alias = if matches!(&self.at().typ, TokenType::Id(name) if name == "as") {
            self.eat();
            let alias = self.eat().clone();
            if !matches!(alias.typ, TokenType::Id(_)) {
                Logger::token_expected(
                    &alias.loc,
                    "invalid import alias",
                    "expected an identifier after `as`",
                );
            }
            alias.into()
        } else {
            let name = if is_path {
                std::path::Path::new(&source)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string()
            } else {
                source.clone()
            };
            if name.is_empty()
                || !name.chars().enumerate().all(|(i, c)| {
                    c == '_' || c.is_ascii_alphabetic() || (i > 0 && c.is_ascii_digit())
                })
            {
                Logger::token_expected(
                    &token.loc,
                    "invalid import alias",
                    "specify `as name` for this file path",
                );
            }
            Tagged::new(token.loc.clone(), name)
        };
        if self.eat().typ != TokenType::Semi {
            Logger::token_expected(
                &self.peek(-1).loc,
                "invalid import",
                "expected `;` after import",
            );
        }
        AST::Import(Import {
            loc,
            source,
            alias,
            is_path,
        })
    }

    fn parse_attributes(&mut self) -> AST {
        let mut attributes = Vec::new();
        while self.at().typ == TokenType::Hash {
            self.eat();
            if self.eat().typ != TokenType::BraceLeft {
                Logger::token_expected(
                    &self.peek(-1).loc,
                    "invalid attribute",
                    "expected `[` after `#`",
                );
            }
            let name = self.eat().clone();
            if !matches!(name.typ, TokenType::Id(_)) {
                Logger::token_expected(&name.loc, "invalid attribute", "expected attribute name");
            }
            let args = if self.at().typ == TokenType::ParanLeft {
                self.parse_macro_arguments()
            } else {
                Vec::new()
            };
            if self.eat().typ != TokenType::BraceRight {
                Logger::token_expected(&self.peek(-1).loc, "invalid attribute", "expected `]`");
            }
            attributes.push(Attribute {
                name: name.into(),
                args,
            });
        }
        let declaration = self.parse_globals();
        if !matches!(
            declaration,
            AST::Function(_)
                | AST::StructDef(_)
                | AST::EnumDef(_)
                | AST::Impl(_)
                | AST::Trait(_)
                | AST::TypeAlias(_)
                | AST::StaticLiteral(_)
                | AST::ComptimeFunction(_)
        ) {
            Logger::token_expected(
                attributes[0].name.loc(),
                "invalid attribute target",
                "attributes require a declaration",
            );
        }
        AST::Attributed {
            attributes,
            declaration: Box::new(declaration),
        }
    }

    fn parse_struct_field(&mut self) -> StructField {
        let id = self.eat().clone();

        let TokenType::Id(id_value) = &id.typ else {
            Logger::token_expected(
                &self.peek(-1).loc,
                "unexpected token found",
                "expected identifier in field declaration",
            );
        };

        if self.eat().typ != TokenType::Colon {
            Logger::token_expected(
                &self.peek(-1).loc,
                "missing type specifier",
                format!("field `{}` requires a type", id_value),
            );
        }

        let typ = self.parse_type_spec();

        if self.eat().typ != TokenType::Semi {
            Logger::token_expected(
                &self.peek(-1).loc,
                "unexpected token found",
                "expected `;` after field declaration",
            );
        }

        StructField {
            typ,
            id: Tagged::new(id.loc, id_value.clone()),
        }
    }

    fn parse_struct_decl(&mut self) -> AST {
        self.eat();

        let id = self.eat().clone();

        let TokenType::Id(id_value) = &id.typ else {
            Logger::token_expected(
                &id.loc,
                "unexpected token found",
                "expected identifier after keyword `struct`",
            );
        };

        if self.eat().typ != TokenType::CurlyLeft {
            Logger::token_expected_help(
                &self.peek(-1).loc,
                "unexpected token found",
                "expected `{` after struct identifier",
                "try adding `{`",
            );
        }

        let mut fields = Vec::new();
        while self.at().typ != TokenType::CurlyRight {
            fields.push(self.parse_struct_field());
        }
        self.eat();

        /*if self.eat().typ != TokenType::Semi {
            Logger::token_expected(&self.peek(-1).loc, "unexpected token found", "expected, `;` after struct declaration");
        }*/

        AST::StructDef(StructDef::new(
            Tagged::new(id.loc, id_value.to_string()),
            fields,
        ))
    }

    fn parse_func_arg(&mut self) -> FunctionArg {
        let mut id = self.eat().clone();

        if id.typ == TokenType::Ampercent
            || id.typ == TokenType::KwMut
            || if let TokenType::Id(value) = &id.typ {
                value == "self"
            } else {
                false
            }
        {
            if self.current_impl.is_none() && self.current_trait.is_none() {
                Logger::token_expected(
                    &id.loc,
                    "unexpected token found",
                    "member function can only be declared in an impl or trait block",
                );
            }

            let typ = if id.typ == TokenType::Ampercent {
                id = self.eat().clone();
                let mut is_mut = false;
                if id.typ == TokenType::KwMut {
                    id = self.eat().clone();
                    is_mut = true;
                }
                if self.current_impl.is_none() {
                    Type::Pointer(Pointer::new(
                        id.loc.clone(),
                        Box::new(Type::Path(Path::new(id.loc.clone(), "Self".to_owned()))),
                        true,
                        is_mut,
                    ))
                } else {
                    Type::Pointer(Pointer::new(
                        id.loc.clone(),
                        Box::new(Type::Path(Path::new(
                            id.loc.clone(),
                            self.current_impl.as_ref().unwrap().typ.inner().clone(),
                        ))),
                        true,
                        is_mut,
                    ))
                }
            } else {
                if id.typ == TokenType::KwMut {
                    id = self.eat().clone();
                }
                Type::Path(Path::new(
                    id.loc.clone(),
                    self.current_impl
                        .as_ref()
                        .map(|item| item.typ.inner().clone())
                        .unwrap_or_else(|| "Self".into()),
                ))
            };

            let TokenType::Id(value) = id.typ else {
                Logger::token_expected(&id.loc, "invalid receiver", "expected `self`");
            };
            if value != "self" {
                Logger::token_expected(&id.loc, "unexpected token found", "expected `self`")
            }

            return FunctionArg {
                id: Tagged::new(id.loc, value),
                typ,
            };
        }

        let TokenType::Id(value) = id.typ else {
            Logger::token_expected(
                &id.loc,
                "unexpected token found",
                "expected identifier in function signature",
            );
        };

        if self.eat().typ != TokenType::Colon {
            Logger::token_expected(
                &self.peek(-1).loc,
                "unexpected token found",
                "expected type for function parameter",
            );
        }

        let typ = self.parse_type_spec();

        FunctionArg {
            id: Tagged::new(id.loc, value),
            typ,
        }
    }

    fn parse_func(&mut self) -> AST {
        if self.eat().typ != TokenType::KwFn {
            Logger::token_expected(
                &self.peek(-1).loc,
                "invalid function declaration",
                "expected `fn`",
            );
        }

        let id = self.eat().clone();

        let TokenType::Id(_) = &id.typ else {
            Logger::token_expected(
                &id.loc,
                "unexpected token found",
                "expected identifier after keyword `fn`",
            );
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
                Logger::token_expected(
                    &self.peek(-1).loc,
                    "unexpected token found",
                    "function arguments need to be comma seperated",
                );
            }
        }
        self.eat();
        let ret_type = if self.at().typ == TokenType::Arrow {
            self.eat();
            self.parse_type_spec()
        } else {
            Type::Path(Path::new(
                Location::new("internal", 0..1),
                "void".to_string(),
            ))
        };

        let c_impl = if self.current_impl.is_some() {
            Some(Impl::new(
                self.current_impl.as_ref().unwrap().typ.clone(),
                Block::new(),
                self.current_impl.as_ref().unwrap().trat.clone(),
            ))
        } else {
            None
        };

        //let attributes = attr.map(|v| vec![v]).unwrap_or(Vec::new());

        if self.at().typ == TokenType::Semi {
            self.eat();
            return AST::Function(Function::new(
                id.into(),
                c_impl,
                args,
                Block::new(),
                ret_type,
                false,
                false,
            ));
        }

        if self.eat().typ != TokenType::CurlyLeft {
            Logger::token_expected(
                &self.peek(-1).loc,
                "unexpected token found",
                "expected `{` after function )signature",
            );
        }

        let mut body = Block::new();
        while self.at().typ != TokenType::CurlyRight {
            let stmt = self.parse_stmt();
            body.returns |= matches!(stmt, AST::ReturnExpr(_));
            body.body.push(stmt);
        }
        self.eat();
        AST::Function(Function::new(
            id.into(),
            c_impl,
            args,
            body,
            ret_type,
            true,
            false,
        ))
    }

    fn parse_impl(&mut self) -> AST {
        self.eat();

        let first = self.parse_name_path();
        let (struct_impl, trat) = if self.at().typ == TokenType::KwFor {
            self.eat();
            (self.parse_name_path(), Some(first))
        } else {
            (first, None)
        };

        if self.eat().typ != TokenType::CurlyLeft {
            Logger::token_expected_help(
                &self.peek(-1).loc,
                "unexpected token found",
                "expected `{`",
                "try adding `{`",
            );
        }

        let mut r#impl = Impl::new(struct_impl, Block::new(), trat);
        self.current_impl = Some(Impl::new(
            r#impl.typ.clone(),
            Block::new(),
            r#impl.trat.clone(),
        ));
        while self.at().typ != TokenType::CurlyRight {
            r#impl.body.body.push(self.parse_func());
        }
        self.eat();

        self.current_impl = None;
        AST::Impl(r#impl)
    }

    fn parse_static(&mut self) -> AST {
        self.eat();

        let id = self.eat().clone();

        if !matches!(id.typ, TokenType::Id(_)) {
            Logger::token_expected(
                &id.loc,
                "unexpected token found",
                "expected `identifier` after keyword `static`",
            );
        }

        if self.eat().typ != TokenType::Colon {
            Logger::token_expected(
                &self.peek(-1).loc,
                "type annotation missing",
                "expected type for static item",
            );
        }

        let ty = self.parse_type_spec();

        let value = if self.at().typ == TokenType::Equals {
            self.eat();
            Some(Box::new(self.parse_expr()))
        } else {
            None
        };

        if self.eat().typ != TokenType::Semi {
            Logger::token_expected_help(
                &self.peek(-1).loc,
                "unexpected token found",
                "expected `;` at the end of expression",
                "try adding `;`",
            );
        }

        AST::StaticLiteral(StaticLiteral::new(id.into(), ty, value))
    }

    fn parse_trait(&mut self) -> AST {
        self.eat();

        let id = self.eat().clone();
        let TokenType::Id(id_value) = &id.typ else {
            Logger::token_expected(
                &id.loc,
                "unexpected token found",
                "expected identifier after keyword `impl`",
            );
        };

        if self.eat().typ != TokenType::CurlyLeft {
            Logger::token_expected_help(
                &self.peek(-1).loc,
                "unexpected token found",
                "expected `{`",
                "try adding `{`",
            );
        }

        let mut trat = Trait::new(Tagged::new(id.loc.clone(), id_value.clone()), Block::new());
        self.current_trait = Some(trat.id.clone());
        while self.at().typ != TokenType::CurlyRight {
            trat.funcs.body.push(self.parse_func());
        }
        self.eat();

        self.current_trait = None;
        AST::Trait(trat)
    }

    fn parse_macro(&mut self) -> AST {
        self.eat();
        if self.at().typ == TokenType::KwFn {
            let AST::Function(function) = self.parse_func() else {
                unreachable!()
            };
            return AST::ComptimeFunction(function);
        }
        let id = self.eat().clone();
        if !matches!(id.typ, TokenType::Id(_)) {
            Logger::token_expected(&id.loc, "invalid macro declaration", "expected macro name");
        }
        if self.eat().typ != TokenType::CurlyLeft {
            Logger::token_expected(
                &self.peek(-1).loc,
                "invalid macro declaration",
                "expected `{`",
            );
        }
        let mut arms = Vec::new();
        while self.at().typ != TokenType::CurlyRight {
            if self.eat().typ != TokenType::ParanLeft {
                Logger::token_expected(&self.peek(-1).loc, "invalid macro arm", "expected `(`");
            }
            let mut conds: Vec<MacroArg> = Vec::new();
            while self.at().typ != TokenType::ParanRight {
                let name = self.eat().clone();
                if !matches!(name.typ, TokenType::Id(_)) {
                    Logger::token_expected(
                        &name.loc,
                        "invalid macro argument",
                        "expected identifier",
                    );
                }
                if self.eat().typ != TokenType::Colon || self.eat().typ != TokenType::Dollar {
                    Logger::token_expected(
                        &self.peek(-1).loc,
                        "invalid macro argument",
                        "expected `: $`",
                    );
                }
                let typ = if self.at().typ == TokenType::BraceLeft {
                    self.eat();
                    if self.eat().typ != TokenType::BraceRight {
                        Logger::token_expected(
                            &self.peek(-1).loc,
                            "invalid variadic argument",
                            "expected `]`",
                        );
                    }
                    MacroArgType::Variadic
                } else {
                    MacroArgType::Single
                };
                if conds
                    .iter()
                    .any(|arg| arg.id.inner().as_str() == name.typ.as_str())
                {
                    Logger::token_expected(
                        &name.loc,
                        "duplicate macro argument",
                        "argument names must be unique",
                    );
                }
                conds.push(MacroArg {
                    id: name.into(),
                    typ,
                });
                if self.at().typ == TokenType::ParanRight {
                    break;
                }
                if self.eat().typ != TokenType::Comma {
                    Logger::token_expected(
                        &self.peek(-1).loc,
                        "invalid macro argument",
                        "expected `,`",
                    );
                }
            }
            self.eat();
            if self.eat().typ != TokenType::Arrow || self.eat().typ != TokenType::CurlyLeft {
                Logger::token_expected(&self.peek(-1).loc, "invalid macro arm", "expected `-> {`");
            }
            let mut tokens = Vec::new();
            let mut depth = 1;
            while depth > 0 {
                let token = self.eat().clone();
                match token.typ {
                    TokenType::Eof => {
                        Logger::token_expected(&token.loc, "unterminated macro arm", "expected `}`")
                    }
                    TokenType::CurlyLeft => depth += 1,
                    TokenType::CurlyRight => depth -= 1,
                    _ => {}
                }
                if depth > 0 {
                    tokens.push(token);
                }
            }
            arms.push(MacroArm { conds, tokens });
        }
        self.eat();
        AST::Macro(Macro::new(id.into(), arms))
    }

    fn parse_enum(&mut self) -> AST {
        self.eat();

        let id = self.eat().clone();

        if !matches!(id.typ, TokenType::Id(_)) {
            Logger::token_expected(
                &id.loc,
                "unexpected token found",
                "expected identifier after keyword `enum`",
            );
        }

        let ty: Type = if self.at().typ == TokenType::Colon {
            self.eat();
            let ty = self.parse_type_spec();
            if let Type::Path(_) = &ty {
                ty
            } else {
                Logger::token_expected(
                    &ty.get_loc(),
                    "invalid enum type",
                    format!(
                        "enums may only contain primitive types but has {}",
                        ty.get_full_name()
                    )
                    .as_str(),
                );
            }
        } else {
            Type::Path(Path::new(id.loc.clone(), "u32"))
        };

        if self.eat().typ != TokenType::CurlyLeft {
            Logger::token_expected(
                &self.peek(-1).loc,
                "unexpected token found",
                "expected `{` after enum identifier",
            );
        }

        let mut variants = Vec::new();

        while self.at().typ != TokenType::CurlyRight {
            let TokenType::Id(variant) = &self.at().typ else {
                Logger::token_expected(
                    &self.at().loc,
                    "unexpected token found",
                    "expected identifier in enum declaration",
                );
            };
            variants.push(variant.clone());
            self.eat();

            if self.at().typ == TokenType::CurlyRight {
                break;
            }

            if self.eat().typ != TokenType::Comma {
                Logger::token_expected(
                    &self.peek(-1).loc,
                    "unexpected token found",
                    "enum variants need to be comma `,` seperated",
                );
            }
        }
        self.eat();

        AST::EnumDef(EnumDef::new(id.into(), variants, ty))
    }

    fn parse_type_def(&mut self) -> AST {
        self.eat();

        let id = self.eat().clone();
        let TokenType::Id(_) = id.typ else {
            Logger::token_expected(
                &id.loc,
                "unexpected token found",
                "expected identifier after keyword `type`",
            );
        };

        if self.eat().typ != TokenType::Equals {
            Logger::token_expected(
                &self.peek(-1).loc,
                "unexpected token found",
                "expected `=` after type identifier",
            );
        }

        let ty = self.parse_type_spec();

        if self.eat().typ != TokenType::Semi {
            Logger::token_expected(
                &self.peek(-1).loc,
                "unexpected token found",
                "expected `;` at the end of expression",
            );
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
                    Logger::token_expected_help(
                        &self.peek(-1).loc,
                        "unexpected token found",
                        "expected `;` at the end of expression",
                        "add `;` to mark end of expression",
                    );
                }
                node
            }
        }
    }

    fn parse_if_stmt(&mut self) -> AST {
        self.eat();
        let cond = self.parse_expr();

        if self.eat().typ != TokenType::CurlyLeft {
            Logger::token_expected(
                &self.peek(-1).loc,
                "unexpected token found",
                "expected `{` after if statement",
            );
        }

        let mut then_branch = Block::new();
        while self.at().typ != TokenType::CurlyRight {
            let stmt = self.parse_stmt();
            then_branch.returns |= matches!(stmt, AST::ReturnExpr(_));
            then_branch.body.push(stmt);
        }
        self.eat();

        let else_branch = if self.at().typ == TokenType::KwElse {
            self.eat();

            if self.at().typ == TokenType::KwIf {
                Some(Block::from(vec![self.parse_if_stmt()]))
            } else {
                if self.eat().typ != TokenType::CurlyLeft {
                    Logger::token_expected(
                        &self.peek(-1).loc,
                        "unexpected token found",
                        "expected `{` after if statement",
                    );
                }

                let mut else_branch = Block::new();
                while self.at().typ != TokenType::CurlyRight {
                    let stmt = self.parse_stmt();
                    else_branch.returns |= matches!(stmt, AST::ReturnExpr(_));
                    else_branch.body.push(stmt);
                }
                self.eat();
                Some(else_branch)
            }
        } else {
            None
        };

        AST::IfStmt(IfStmt::new(Box::new(cond), then_branch, else_branch))
    }

    fn parse_for_stmt(&mut self) -> AST {
        self.eat();

        let var = self.parse_expr();

        if let AST::VarDecl(_) = var {
        } else {
            Logger::token_expected(
                &self.peek(-1).loc,
                "unexpected token found",
                "expected expression after keyword `for`",
            );
        }

        if self.eat().typ != TokenType::KwIn {
            Logger::token_expected(
                &self.peek(-1).loc,
                "unexpected token found",
                "expected `in` after expression",
            );
        }

        let cond = self.parse_expr();

        if self.eat().typ != TokenType::CurlyLeft {
            Logger::token_expected(
                &self.peek(-1).loc,
                "unexpected token found",
                "expected `{` after for statement",
            );
        }

        let mut block = Block::new();
        while self.at().typ != TokenType::CurlyRight {
            let stmt = self.parse_stmt();
            block.returns |= matches!(stmt, AST::ReturnExpr(_));
            block.body.push(stmt);
        }
        self.eat();

        AST::ForStmt(ForStmt::new(Box::new(cond), Box::new(var), block))
    }

    fn parse_expr(&mut self) -> AST {
        match self.at().typ {
            TokenType::KwLet => self.parse_var_decl(),
            TokenType::KwRet => self.parse_ret_expr(),
            _ => self.parse_assignment_expr(),
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
            Logger::token_expected(
                &tok.loc,
                "unexpected token found",
                "expected identifier after keyword `let`",
            );
        };

        let mut op = self.at().clone();
        if matches!(op.typ, TokenType::Colon | TokenType::Equals) {
            self.eat();
        }

        if op.typ == TokenType::Colon {
            typ = Some(self.parse_type_spec());
            op = self.at().clone();
            if op.typ == TokenType::Equals {
                self.eat();
            }
        }
        match op.typ {
            TokenType::Semi | TokenType::KwIn => {
                if is_const || typ.is_none() {
                    Logger::token_expected(
                        &tok.loc,
                        "unexpected token found",
                        format!("constant value {} needs to be assigned", &var_name).as_str(),
                    );
                }
                AST::VarDecl(VarDecl::new(tok.into(), is_const, typ, None))
            }
            TokenType::Equals => {
                let expr = self.parse_expr();
                AST::VarDecl(VarDecl::new(
                    tok.into(),
                    is_const,
                    typ,
                    Some(Box::new(expr)),
                ))
            }
            _ => Logger::token_expected(
                &self.at().loc,
                "unexpected token found",
                "expected `=` or `;` after variable declaration",
            ),
        }
    }

    /*fn parse_claim_expr(&mut self) -> AST {
        return AST::ClaimExpr(ClaimExpr::new(self.eat().loc.clone(), Box::new(self.parse_expr())));
    }*/

    fn parse_ret_expr(&mut self) -> AST {
        let loc = self.eat().loc.clone();
        if self.at().typ == TokenType::Semi {
            return AST::ReturnExpr(ReturnExpr::new(loc, None));
        }
        AST::ReturnExpr(ReturnExpr::new(loc, Some(Box::new(self.parse_expr()))))
    }

    // Precedence, from weakest to strongest: assignment, ||, &&, equality,
    // comparison, addition, multiplication, prefix, postfix.
    fn parse_assignment_expr(&mut self) -> AST {
        let lhs = self.parse_binary_expr(1);
        if self.at().typ == TokenType::Equals {
            self.eat();
            if !matches!(
                &lhs,
                AST::Identifier(_) | AST::MemberExpr(_) | AST::AccessExpr(_)
            ) && !matches!(&lhs, AST::UnaryExpr(expr) if expr.op.inner() == "*")
            {
                Logger::token_expected(
                    &self.peek(-1).loc,
                    "invalid assignment target",
                    "expected a variable, field, index or dereference",
                );
            }
            let rhs = self.parse_assignment_expr();
            return AST::AssignmentExpr(AssignmentExpr::new(Box::new(lhs), Box::new(rhs)));
        }
        lhs
    }

    fn parse_binary_expr(&mut self, min_precedence: u8) -> AST {
        let mut lhs = self.parse_unary_expr();
        loop {
            let precedence = match self.at().typ {
                TokenType::OpOr => 1,
                TokenType::OpAnd => 2,
                TokenType::OpEqEq | TokenType::OpNeEq => 3,
                TokenType::AngleLeft
                | TokenType::AngleRight
                | TokenType::OpLeEq
                | TokenType::OpGrEq => 4,
                TokenType::OpPlus | TokenType::OpMinus => 5,
                TokenType::OpMul | TokenType::OpDiv | TokenType::OpMod => 6,
                _ => break,
            };
            if precedence < min_precedence {
                break;
            }
            let op = self.eat().clone();
            let rhs = self.parse_binary_expr(precedence + 1);
            lhs = AST::BinaryExpr(BinaryExpr::new(Box::new(lhs), Box::new(rhs), op.into()));
        }
        lhs
    }

    fn parse_unary_expr(&mut self) -> AST {
        if matches!(
            self.at().typ,
            TokenType::Ampercent
                | TokenType::OpMul
                | TokenType::Bang
                | TokenType::OpMinus
                | TokenType::OpPlus
        ) {
            let op = self.eat().clone();
            let mut op: Tagged<String> = op.into();
            if op.inner() == "&" && self.at().typ == TokenType::KwMut {
                self.eat();
                *op.inner_mut() = "&mut".into();
            }
            let obj = self.parse_unary_expr();
            return AST::UnaryExpr(UnaryExpr::new(op, Box::new(obj)));
        }
        if self.at().typ == TokenType::ParanLeft && self.peek(1).typ == TokenType::Colon {
            self.eat();
            self.eat();
            let typ = self.parse_type_spec();
            if self.eat().typ != TokenType::ParanRight {
                Logger::token_expected(
                    &self.peek(-1).loc,
                    "unexpected token found",
                    "expected `)` after cast type",
                );
            }
            let expr = self.parse_unary_expr();
            return AST::CastExpr(CastExpr::new(Box::new(expr), typ));
        }
        self.parse_postfix_expr()
    }

    fn parse_arguments(&mut self) -> Vec<AST> {
        self.eat(); // (
        let mut args = Vec::new();
        while self.at().typ != TokenType::ParanRight {
            args.push(self.parse_expr());
            if self.at().typ == TokenType::ParanRight {
                break;
            }
            if self.eat().typ != TokenType::Comma {
                Logger::token_expected(
                    &self.peek(-1).loc,
                    "unexpected token found",
                    "expected `,` between arguments",
                );
            }
        }
        self.eat();
        args
    }

    fn parse_macro_arguments(&mut self) -> Vec<Vec<Token>> {
        if self.eat().typ != TokenType::ParanLeft {
            Logger::token_expected(
                &self.peek(-1).loc,
                "unexpected token found",
                "expected `(` after macro name",
            );
        }
        let mut args = Vec::new();
        let mut arg = Vec::new();
        let mut delimiters = Vec::new();
        loop {
            let tok = self.at().clone();
            match tok.typ {
                TokenType::Eof => {
                    Logger::token_expected(&tok.loc, "unterminated macro call", "expected `)`")
                }
                TokenType::ParanRight if delimiters.is_empty() => {
                    self.eat();
                    if !arg.is_empty() {
                        args.push(arg);
                    }
                    break;
                }
                TokenType::Comma if delimiters.is_empty() => {
                    if arg.is_empty() {
                        Logger::token_expected(
                            &tok.loc,
                            "missing macro argument",
                            "expected tokens before `,`",
                        );
                    }
                    args.push(std::mem::take(&mut arg));
                    self.eat();
                    continue;
                }
                TokenType::ParanLeft => delimiters.push(TokenType::ParanRight),
                TokenType::BraceLeft => delimiters.push(TokenType::BraceRight),
                TokenType::CurlyLeft => delimiters.push(TokenType::CurlyRight),
                TokenType::ParanRight | TokenType::BraceRight | TokenType::CurlyRight => {
                    if delimiters.pop().as_ref() != Some(&tok.typ) {
                        Logger::token_expected(
                            &tok.loc,
                            "mismatched delimiter",
                            "invalid closing delimiter in macro call",
                        );
                    }
                }
                _ => {}
            }
            arg.push(self.eat().clone());
        }
        args
    }

    fn parse_postfix_expr(&mut self) -> AST {
        let mut obj = self.parse_paran_expr();
        loop {
            match self.at().typ {
                TokenType::ParanLeft => {
                    let args = self.parse_arguments();
                    obj = AST::CallExpr(CallExpr::new(Box::new(obj), args));
                }
                TokenType::BraceLeft => {
                    self.eat();
                    let idx = self.parse_expr();
                    if self.eat().typ != TokenType::BraceRight {
                        Logger::token_expected(
                            &self.peek(-1).loc,
                            "unexpected token found",
                            "expected `]` after index",
                        );
                    }
                    obj = AST::AccessExpr(AccessExpr::new(Box::new(obj), Box::new(idx)));
                }
                TokenType::Dot | TokenType::Arrow | TokenType::OpScope => {
                    let op = self.eat().typ.clone();
                    let member = self.eat().clone();
                    if !matches!(member.typ, TokenType::Id(_)) {
                        Logger::token_expected(
                            &member.loc,
                            "unexpected token found",
                            "expected member identifier",
                        );
                    }
                    let mut member = AST::Identifier(Identifier::new(member.into()));
                    if self.at().typ == TokenType::ParanLeft {
                        member =
                            AST::CallExpr(CallExpr::new(Box::new(member), self.parse_arguments()));
                    }
                    obj = if op == TokenType::OpScope {
                        AST::ResolutionExpr(ResolutionExpr::new(Box::new(obj), Box::new(member)))
                    } else if matches!(member, AST::CallExpr(_)) {
                        AST::MemberCallExpr(MemberCallExpr::new(
                            Box::new(obj),
                            Box::new(member),
                            op == TokenType::Arrow,
                        ))
                    } else {
                        AST::MemberExpr(MemberExpr::Member {
                            obj: Box::new(obj),
                            member: Box::new(member),
                            deref: op == TokenType::Arrow,
                        })
                    };
                }
                TokenType::Dollar => {
                    self.eat();
                    fn name(node: &AST) -> Option<Tagged<String>> {
                        match node {
                            AST::Identifier(id) => Some(id.id.clone()),
                            AST::ResolutionExpr(v) => {
                                let owner = name(&v.obj)?;
                                let member = name(&v.member)?;
                                Some(Tagged::new(
                                    owner.loc().clone(),
                                    format!("{}::{}", owner.inner(), member.inner()),
                                ))
                            }
                            _ => None,
                        }
                    }
                    let Some(id) = name(&obj) else {
                        Logger::token_expected(
                            &self.peek(-1).loc,
                            "invalid macro name",
                            "expected a name path before `$`",
                        );
                    };
                    let args = self.parse_macro_arguments();
                    let mut call = MacroCall::new(id, args);
                    if call.id.inner() == "format_args"
                        || call.id.inner().ends_with("::format_args")
                    {
                        let mut expressions = Vec::new();
                        for tokens in &call.args {
                            let mut parser = Self::from(tokens.clone());
                            parser.current_impl = self.current_impl.as_ref().map(|item| {
                                Impl::new(item.typ.clone(), Block::new(), item.trat.clone())
                            });
                            parser.current_trait = self.current_trait.clone();
                            let expr = parser.parse_expr();
                            if parser.at().typ != TokenType::Eof {
                                Logger::token_expected(
                                    &parser.at().loc,
                                    "invalid format argument",
                                    "expected one complete expression",
                                );
                            }
                            expressions.push(expr);
                        }
                        call.parsed_args = Some(expressions);
                    }
                    obj = AST::MacroCall(call);
                }
                _ => break,
            }
        }
        obj
    }

    fn parse_paran_expr(&mut self) -> AST {
        if self.at().typ == TokenType::ParanLeft {
            self.eat();
            let expr = self.parse_expr();
            if self.eat().typ != TokenType::ParanRight {
                Logger::token_expected(
                    &self.peek(-1).loc,
                    "unexpected token found",
                    "expected closing parenthesis `)`",
                );
            }
            return expr;
        }
        self.parse_primary_expr()
    }

    fn parse_name_path(&mut self) -> Tagged<String> {
        let first = self.eat().clone();
        let TokenType::Id(mut name) = first.typ else {
            Logger::token_expected(&first.loc, "invalid name path", "expected an identifier");
        };
        while self.at().typ == TokenType::OpScope {
            self.eat();
            let member = self.eat().clone();
            let TokenType::Id(part) = member.typ else {
                Logger::token_expected(
                    &member.loc,
                    "invalid name path",
                    "expected an identifier after `::`",
                );
            };
            name.push_str("::");
            name.push_str(&part);
        }
        Tagged::new(first.loc, name)
    }
    fn is_struct_literal(&self) -> bool {
        let mut index = self.index + 1;
        while self
            .tokens
            .get(index)
            .is_some_and(|t| t.typ == TokenType::OpScope)
            && self
                .tokens
                .get(index + 1)
                .is_some_and(|t| matches!(t.typ, TokenType::Id(_)))
        {
            index += 2;
        }
        self.tokens
            .get(index)
            .is_some_and(|t| t.typ == TokenType::Dollar)
            && self
                .tokens
                .get(index + 1)
                .is_some_and(|t| t.typ == TokenType::CurlyLeft)
    }

    fn parse_struct_literal(&mut self) -> AST {
        let mut struct_ident = self.parse_name_path();
        if struct_ident.inner() == "Self" {
            if let Some(item) = &self.current_impl {
                *struct_ident.inner_mut() = item.typ.inner().clone();
            } else {
                Logger::token_expected(
                    struct_ident.loc(),
                    "invalid `Self` literal",
                    "expected an impl block",
                );
            }
        }
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
                    Logger::token_expected(
                        &id.loc,
                        "unexpected token found",
                        "expected `identifier` after `.` in struct literal list",
                    );
                };

                name = Some(id.into());

                if self.eat().typ != TokenType::Equals {
                    Logger::token_expected(
                        &self.peek(-1).loc,
                        "unexpected token found",
                        "expected `=` between member and value",
                    );
                }
            }
            values.push((name, self.parse_expr()));

            if self.at().typ == TokenType::CurlyRight {
                break;
            }

            if self.eat().typ != TokenType::Comma {
                Logger::token_expected(
                    &self.peek(-1).loc,
                    "unexpected token found",
                    "struct literal list values need to be comma `,` seperated",
                );
            }
        }
        self.eat();
        AST::StructLiteral(StructLiteral::new(struct_ident, values))
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
            if self.at().typ == TokenType::Semi {
                self.eat();
                if values.len() != 1 {
                    Logger::token_expected(
                        &self.at().loc,
                        "invalid repeat initializer",
                        "use `[value; length]` with exactly one value",
                    );
                }
                let size = self.eat().clone();
                if !matches!(size.typ, TokenType::Integer(_)) {
                    Logger::token_expected(
                        &size.loc,
                        "invalid array length",
                        "repeat length must be a non-negative integer literal",
                    );
                }
                let length = size.typ.as_str().parse::<usize>().unwrap_or_else(|_| {
                    Logger::token_expected(
                        &size.loc,
                        "invalid array length",
                        "array length does not fit in usize",
                    )
                });
                if self.at().typ != TokenType::BraceRight {
                    Logger::token_expected(
                        &self.at().loc,
                        "invalid repeat initializer",
                        "expected `]` after repeat length",
                    );
                }
                let end = self.eat().loc.clone();
                let mut literal = ArrayLiteral::new(start + end, values);
                literal.repeat = Some(length);
                return AST::ArrayLiteral(literal);
            }
            if self.at().typ == TokenType::BraceRight {
                break;
            }

            if self.eat().typ != TokenType::Comma {
                Logger::token_expected(
                    &self.peek(-1).loc,
                    "unexpected token found",
                    "initializer list values need to be comma `,` seperated",
                );
            }
        }
        let end = self.eat().loc.clone();
        AST::ArrayLiteral(ArrayLiteral::new(start + end, values))
    }

    fn parse_primary_expr(&mut self) -> AST {
        let tok = self.at().clone();
        match &tok.typ {
            TokenType::Id(_) if self.is_struct_literal() => self.parse_struct_literal(),
            TokenType::Id(_) => {
                //if self.peek(1).typ == TokenType::Dollar && self.peek(2).typ == TokenType::CurlyLeft {
                //    self.parse_struct_literal()
                //}else {
                let mut token = self.eat().clone();
                if token.typ == TokenType::Id("Self".into()) {
                    if let Some(item) = &self.current_impl {
                        token.typ = TokenType::Id(item.typ.inner().clone());
                    }
                }
                AST::Identifier(Identifier::new(token.into()))
                //}
            }
            TokenType::BraceLeft => self.parse_array_literal(),
            TokenType::Integer(_) => AST::NumberLiteral(NumberLiteral::new(
                self.eat().clone().into(),
                NumberType::Int,
            )),
            TokenType::Float(_) => AST::NumberLiteral(NumberLiteral::new(
                self.eat().clone().into(),
                NumberType::Float,
            )),
            TokenType::Char(_) => AST::NumberLiteral(NumberLiteral::new(
                self.eat().clone().into(),
                NumberType::Char,
            )),
            TokenType::String(_) => {
                AST::StringLiteral(StringLiteral::new(self.eat().clone().into()))
            }
            TokenType::KwBreak => AST::BreakExpr(BreakExpr::new(self.eat().loc.clone())),
            //TokenType::KwContinue => todo!("implement parsing of `KwContinue`"),
            //TokenType::Pipe => self.parse_closure(),
            _ => {
                Logger::token_expected(
                    &self.at().loc,
                    format!("unexpected token found: `{:?}`", tok.typ).as_str(),
                    "unexpected token found",
                );
            }
        }
    }

    fn parse_type_spec(&mut self) -> Type {
        if self.at().typ == TokenType::OpMul || self.at().typ == TokenType::Ampercent {
            let pos = self.at().loc.clone();
            let is_ref = self.eat().typ == TokenType::Ampercent;
            let is_mut = if self.at().typ == TokenType::KwMut {
                self.eat();
                true
            } else if !is_ref && self.at().typ == TokenType::KwConst {
                self.eat();
                false
            } else if is_ref {
                false
            } else {
                Logger::token_expected_help(
                    &pos,
                    "raw pointer type requires a qualifier",
                    "missing `const` or `mut` after `*`",
                    "use `*const T` for read-only memory or `*mut T` for writable memory",
                );
            };
            let typ = self.parse_type_spec();
            let loc = pos + typ.get_loc();
            return Type::Pointer(Pointer::new(loc, Box::new(typ), is_ref, is_mut));
        }

        if self.at().typ == TokenType::BraceLeft {
            let start = self.eat().loc.range.start;
            let ty = self.parse_type_spec();

            let delim = self.eat();
            if delim.typ != TokenType::Semi && delim.typ != TokenType::BraceRight {
                Logger::token_expected(
                    &delim.loc,
                    "unexpected token found",
                    "expected `;` or `]` after type",
                );
            }

            if delim.typ == TokenType::BraceRight {
                return Type::Slice(ArraySlice::new(ty.get_loc(), ty));
            }

            let TokenType::Integer(_) = self.at().typ else {
                Logger::token_expected(
                    &self.at().loc,
                    "non constant in static context",
                    "expected constant value",
                );
            };

            let size = self.eat().clone();
            let elems = size.typ.as_str().parse::<usize>().unwrap_or_else(|_| {
                Logger::token_expected(
                    &size.loc,
                    "invalid array size",
                    "array size does not fit in usize",
                )
            });

            if self.eat().typ != TokenType::BraceRight {
                Logger::token_expected(
                    &self.peek(-1).loc,
                    "unexpected token found",
                    "expected closing brace `]`",
                );
            }

            return Type::Array(Array::new(
                Location::new(
                    self.peek(-1).loc.file.clone(),
                    start..self.peek(-1).loc.range.end,
                ),
                ty,
                elems,
            ));
        }

        if self.at().typ == TokenType::KwFn {
            let start = self.eat().loc.range.start;
            if self.eat().typ != TokenType::ParanLeft {
                Logger::token_expected(
                    &self.peek(-1).loc,
                    "unexpected token found",
                    "expected `(` after keyword `fn`",
                );
            }

            let mut params = Vec::new();
            while self.at().typ != TokenType::ParanRight {
                params.push(self.parse_type_spec());
                if self.at().typ == TokenType::ParanRight {
                    break;
                }
                if self.eat().typ != TokenType::Comma {
                    Logger::token_expected(
                        &self.peek(-1).loc,
                        "unexpected token found",
                        "function params have to be comma seperated",
                    );
                }
            }
            self.eat();

            let ret_type = if self.at().typ == TokenType::Arrow {
                self.eat();
                self.parse_type_spec()
            } else {
                Type::Path(Path::new(
                    Location::new("internal", 0..1),
                    "void".to_string(),
                ))
            };

            return Type::FuncPtr(FuncPtr::new(
                Location::new(
                    self.peek(-1).loc.file.clone(),
                    start..self.peek(-1).loc.range.end,
                ),
                params,
                ret_type,
            ));
        }

        let tok = self.eat().clone();
        let TokenType::Id(mut path) = tok.typ else {
            Logger::token_expected(&tok.loc, "invalid type", "expected a type identifier");
        };
        let path_loc = tok.loc;
        while self.at().typ == TokenType::OpScope {
            self.eat();
            let member = self.eat().clone();
            let TokenType::Id(name) = member.typ else {
                Logger::token_expected(
                    &member.loc,
                    "invalid type path",
                    "expected an identifier after `::`",
                );
            };
            path.push_str("::");
            path.push_str(&name);
        }
        if path == "Self" {
            if self.current_impl.is_none() && self.current_trait.is_none() {
                Logger::token_expected(
                    &path_loc,
                    "unexpected token found",
                    "encountered type `Self` outside of `impl` or `trait` block",
                );
            }

            if !self.current_impl.is_none() {
                path = self.current_impl.as_ref().unwrap().typ.inner().into();
            }
        }
        Type::Path(Path::new(path_loc, path))
    }
}
