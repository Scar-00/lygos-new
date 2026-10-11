//! Expand the existing token-arm macros in their definition's namespace.
use super::*;
use lygos_ast::r#macro::MacroArgType;
use lygos_token::{Token, TokenType};
pub(super) struct Expander<'a> {
    modules: &'a Arena<FileModule, ModuleId>,
    module: ModuleId,
    depth: usize,
    budget: usize,
}
impl<'a> Expander<'a> {
    pub(super) fn new(modules: &'a Arena<FileModule, ModuleId>, module: ModuleId) -> Self {
        Self {
            modules,
            module,
            depth: 0,
            budget: 10000,
        }
    }
    pub(super) fn block(&mut self, block: &mut Block) -> Result<()> {
        let mut output = Vec::new();
        for mut node in std::mem::take(&mut block.body) {
            if let AST::MacroCall(call) = &node {
                if let Some(mut expanded) = self.call(call)? {
                    self.depth += 1;
                    self.block(&mut expanded)?;
                    self.depth -= 1;
                    output.extend(expanded.body);
                    continue;
                }
            }
            self.expr(&mut node)?;
            output.push(node);
        }
        block.body = output;
        Ok(())
    }
    fn call(&mut self, call: &MacroCall) -> Result<Option<Block>> {
        let mut module = self.module;
        let canonical = self.modules.iter().find_map(|(id, file)| {
            file.token_macros
                .keys()
                .find(|name| {
                    file.exports
                        .get(*name)
                        .is_some_and(|value| value == call.id.inner())
                })
                .map(|name| (id, name.clone()))
        });
        let mut parts = call.id.inner().split("::").peekable();
        while parts.clone().count() > 1 {
            let part = parts.next().unwrap();
            let Some(next) = self.modules[module].imports.get(part) else {
                return Ok(None);
            };
            module = *next;
        }
        let source_name = parts.next().unwrap();
        let name = if let Some((id, ref name)) = canonical {
            module = id;
            name.as_str()
        } else {
            source_name
        };
        let Some(definition) = self.modules[module].token_macros.get(name) else {
            return Ok(None);
        };
        if self.depth >= 64 || self.budget == 0 {
            return Err(TypeError::new(
                call.id.loc().clone(),
                "token macro expansion limit exceeded",
            ));
        }
        self.budget -= 1;
        let arm = definition
            .arms
            .iter()
            .find(|arm| {
                let variadic = arm
                    .conds
                    .last()
                    .is_some_and(|c| c.typ == MacroArgType::Variadic);
                arm.conds
                    .iter()
                    .take(arm.conds.len().saturating_sub(1))
                    .all(|c| c.typ == MacroArgType::Single)
                    && if variadic {
                        call.args.len() >= arm.conds.len() - 1
                    } else {
                        call.args.len() == arm.conds.len()
                    }
            })
            .ok_or_else(|| {
                TypeError::new(
                    call.id.loc().clone(),
                    format!(
                        "no matching arm for macro `{}` with {} arguments",
                        call.id.inner(),
                        call.args.len()
                    ),
                )
            })?;
        let mut arguments = HashMap::new();
        for (index, param) in arm.conds.iter().enumerate() {
            let value = if param.typ == MacroArgType::Variadic {
                let mut tokens = Vec::new();
                for (i, arg) in call.args[index..].iter().enumerate() {
                    if i > 0 {
                        tokens.push(Token::new(TokenType::Comma, call.id.loc().clone()));
                    }
                    tokens.extend(arg.iter().cloned());
                }
                tokens
            } else {
                call.args[index].clone()
            };
            arguments.insert(param.id.inner().clone(), value);
        }
        let mut tokens = Vec::new();
        let mut index = 0;
        while index < arm.tokens.len() {
            let token = &arm.tokens[index];
            if token.typ == TokenType::Dollar {
                if let Some(Token {
                    typ: TokenType::Id(name),
                    ..
                }) = arm.tokens.get(index + 1)
                {
                    if let Some(value) = arguments.get(name) {
                        tokens.extend(value.iter().cloned());
                        index += 2;
                        continue;
                    }
                }
            }
            let mut token = token.clone();
            token.loc = call.id.loc().clone();
            if let TokenType::Id(name) = &token.typ {
                // Runtime helpers belong to the macro's definition file.
                // Substituted argument tokens are kept in the caller's scope.
                if let Some(canonical) = self.modules[module].exports.get(name) {
                    token.typ = TokenType::Id(canonical.clone());
                }
            }
            tokens.push(token);
            index += 1;
        }
        if tokens.len() > 100000 {
            return Err(TypeError::new(
                call.id.loc().clone(),
                "token macro expansion exceeds 100000 tokens",
            ));
        }
        let mut block = Parser::from(tokens).build_statements();
        let namespace = if module.0 == 0 {
            String::new()
        } else {
            format!("__module_{}__", module.0)
        };
        mark_formatting(&mut block, &namespace);
        Ok(Some(block))
    }
    fn expr(&mut self, node: &mut AST) -> Result<()> {
        if let AST::MacroCall(call) = node {
            if let Some(mut expanded) = self.call(call)? {
                if expanded.body.len() != 1 {
                    return Err(TypeError::new(
                        call.id.loc().clone(),
                        "a token macro used as an expression must expand to one expression",
                    ));
                }
                *node = expanded.body.pop().unwrap();
                self.depth += 1;
                self.expr(node)?;
                self.depth -= 1;
                return Ok(());
            }
        }
        match node {
            AST::Function(v) | AST::ComptimeFunction(v) => self.block(&mut v.body)?,
            AST::Impl(v) => self.block(&mut v.body)?,
            AST::Trait(v) => self.block(&mut v.funcs)?,
            AST::Attributed { declaration, .. } => self.expr(declaration)?,
            AST::VarDecl(v) => {
                if let Some(e) = &mut v.value {
                    self.expr(e)?;
                }
            }
            AST::StaticLiteral(v) => {
                if let Some(e) = &mut v.value {
                    self.expr(e)?;
                }
            }
            AST::ReturnExpr(v) => {
                if let Some(e) = &mut v.value {
                    self.expr(e)?;
                }
            }
            AST::UnaryExpr(v) => self.expr(&mut v.obj)?,
            AST::CastExpr(v) => self.expr(&mut v.obj)?,
            AST::BinaryExpr(v) => {
                self.expr(&mut v.lhs)?;
                self.expr(&mut v.rhs)?;
            }
            AST::AssignmentExpr(v) => {
                self.expr(&mut v.lhs)?;
                self.expr(&mut v.rhs)?;
            }
            AST::CallExpr(v) => {
                self.expr(&mut v.caller)?;
                for a in &mut v.args {
                    self.expr(a)?;
                }
            }
            AST::MemberCallExpr(v) => {
                self.expr(&mut v.obj)?;
                self.expr(&mut v.r#fn)?;
            }
            AST::ResolutionExpr(v) => {
                self.expr(&mut v.obj)?;
                self.expr(&mut v.member)?;
            }
            AST::MemberExpr(MemberExpr::Member { obj, member, .. }) => {
                self.expr(obj)?;
                self.expr(member)?;
            }
            AST::MemberExpr(MemberExpr::Index { obj, .. }) => self.expr(obj)?,
            AST::AccessExpr(v) => {
                self.expr(&mut v.obj)?;
                self.expr(&mut v.index)?;
            }
            AST::StructLiteral(v) => {
                for (_, e) in &mut v.inits {
                    self.expr(e)?;
                }
            }
            AST::ArrayLiteral(v) => {
                for e in &mut v.values {
                    self.expr(e)?;
                }
            }
            AST::IfStmt(v) => {
                self.expr(&mut v.cond)?;
                self.block(&mut v.then_body)?;
                if let Some(b) = &mut v.else_body {
                    self.block(b)?;
                }
            }
            AST::ForStmt(v) => {
                self.expr(&mut v.var)?;
                self.expr(&mut v.cond)?;
                self.block(&mut v.body)?;
            }
            AST::MacroCall(v) => {
                if let Some(args) = &mut v.parsed_args {
                    for a in args {
                        self.expr(a)?;
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
}
fn mark_formatting(block: &mut Block, namespace: &str) {
    fn node(ast: &mut AST, namespace: &str) {
        match ast {
            AST::MacroCall(call) => {
                if call.id.inner() == "format_args" {
                    call.format_namespace = namespace.into();
                }
                if let Some(args) = &mut call.parsed_args {
                    for a in args {
                        node(a, namespace);
                    }
                }
            }
            AST::CallExpr(c) => {
                for a in &mut c.args {
                    node(a, namespace);
                }
            }
            AST::ResolutionExpr(r) => node(&mut r.member, namespace),
            AST::ReturnExpr(r) => {
                if let Some(v) = &mut r.value {
                    node(v, namespace);
                }
            }
            AST::VarDecl(v) => {
                if let Some(v) = &mut v.value {
                    node(v, namespace);
                }
            }
            _ => {}
        }
    }
    for ast in &mut block.body {
        node(ast, namespace);
    }
}
