//! Compile-time execution and syntax expansion, before semantic analysis.
use std::collections::HashMap;
mod native;

use crate::Parser;
use lygos_ast::{
    arena::{Arena, ArenaId},
    type_check::TypeError,
    types::*,
    *,
};
use lygos_lexer::Lexer;
use lygos_token::{Location, Tagged, Token, TokenType};

type Result<T> = std::result::Result<T, TypeError>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SyntaxId(u32);
impl ArenaId for SyntaxId {
    fn from_index(index: u32) -> Self {
        Self(index)
    }
    fn index(self) -> usize {
        self.0 as usize
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FieldId(u32);
impl ArenaId for FieldId {
    fn from_index(index: u32) -> Self {
        Self(index)
    }
    fn index(self) -> usize {
        self.0 as usize
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct MetaTypeId(u32);
impl ArenaId for MetaTypeId {
    fn from_index(index: u32) -> Self {
        Self(index)
    }
    fn index(self) -> usize {
        self.0 as usize
    }
}
#[derive(Clone)]
struct FieldHandle {
    owner: SyntaxId,
    index: usize,
    revision: u64,
}

pub const META_SOURCE: &str = include_str!("../../meta/builtins.ly");

#[derive(Clone, Debug, PartialEq)]
enum Value {
    Int(i64),
    Bool(bool),
    String(String),
    Array(Vec<Value>),
    Syntax(SyntaxId),
    Field(FieldId),
    Type(MetaTypeId),
    Void,
}
/// Expand an owned source module. Syntax handles never escape into TypedModule.
pub fn expand(mut ast: AST) -> Result<AST> {
    let AST::Module(module) = &mut ast else {
        return Ok(ast);
    };
    let mut executor = Executor {
        functions: HashMap::new(),
        nodes: Arena::default(),
        fields: Arena::default(),
        meta_types: Arena::default(),
        revisions: HashMap::new(),
        emitted: Vec::new(),
        steps: 100_000,
        syntax_budget: 100_000,
        compiled: None,
        expansion_depth: 0,
        callsite: Location::new("", 0..0),
    };
    for node in &module.body.body {
        if let AST::ComptimeFunction(function) = node {
            if !function.is_def || function.is_var_arg || function.obj.is_some() {
                return Err(TypeError::new(
                    function.id.loc().clone(),
                    "compile-time functions require a body and fixed parameters",
                ));
            }
            if matches!(
                function.id.inner().as_str(),
                "sizeof" | "format_args" | "derive" | "meta"
            ) {
                return Err(TypeError::new(
                    function.id.loc().clone(),
                    "reserved compile-time function name",
                ));
            }
            if executor
                .functions
                .insert(function.id.inner().clone(), function.clone())
                .is_some()
            {
                return Err(TypeError::new(
                    function.id.loc().clone(),
                    "duplicate compile-time function",
                ));
            }
        }
    }
    executor.expand_block(&mut module.body)?;
    while !executor.emitted.is_empty() {
        for mut emitted in std::mem::take(&mut executor.emitted) {
            executor.expand_node(&mut emitted)?;
            module.body.body.push(emitted);
        }
    }
    Ok(ast)
}

struct Executor {
    functions: HashMap<String, Function>,
    nodes: Arena<AST, SyntaxId>,
    fields: Arena<FieldHandle, FieldId>,
    meta_types: Arena<Type, MetaTypeId>,
    revisions: HashMap<SyntaxId, u64>,
    emitted: Vec<AST>,
    steps: usize,
    syntax_budget: usize,
    compiled: Option<native::Program>,
    expansion_depth: usize,
    callsite: Location,
}

impl Executor {
    fn fail<T>(&self, message: impl Into<String>) -> Result<T> {
        Err(TypeError::new(self.callsite.clone(), message))
    }
    fn tick(&mut self) -> Result<()> {
        if self.steps == 0 {
            return self.fail("compile-time execution limit exceeded (100000 steps)");
        }
        self.steps -= 1;
        Ok(())
    }
    fn syntax(&mut self, ast: AST) -> Result<Value> {
        self.tick()?;
        let cost = syntax_cost(&ast);
        if cost > 10_000 || cost > self.syntax_budget {
            return self.fail("compile-time syntax allocation limit exceeded");
        }
        self.syntax_budget -= cost;
        Ok(Value::Syntax(self.nodes.insert(ast)))
    }
    fn node(&self, value: &Value) -> Result<SyntaxId> {
        if let Value::Syntax(id) = value {
            Ok(*id)
        } else {
            self.fail("expected an Ast handle")
        }
    }
    fn text(&self, value: &Value) -> Result<String> {
        if let Value::String(value) = value {
            Ok(value.clone())
        } else {
            self.fail("expected a compile-time string")
        }
    }
    fn integer(&self, value: &Value) -> Result<i64> {
        if let Value::Int(value) = value {
            Ok(*value)
        } else {
            self.fail("expected a compile-time integer")
        }
    }
    fn boolean(&self, value: &Value) -> Result<bool> {
        if let Value::Bool(value) = value {
            Ok(*value)
        } else {
            self.fail("expected a compile-time bool")
        }
    }
    fn parse_tokens(&self, tokens: Vec<Token>) -> Result<AST> {
        if tokens.is_empty() {
            return self.fail("macro argument cannot be empty");
        }
        let mut parser = Parser::from(tokens);
        parser.validate_delimiters();
        let expression = parser.parse_expr();
        if parser.at().typ != TokenType::Eof {
            return self.fail("expected one expression in macro argument");
        }
        Ok(expression)
    }
    fn lex(&self, source: &str) -> Result<Vec<Token>> {
        if source.len() > 1_000_000 {
            return self.fail("generated syntax exceeds 1 MB");
        }
        let mut tokens = Lexer::from(source.chars().collect())
            .try_get_tokens()
            .map_err(|error| {
                TypeError::new(
                    self.callsite.clone(),
                    format!("invalid generated syntax: {error:?}"),
                )
            })?;
        for token in &mut tokens {
            token.loc = self.callsite.clone();
        }
        Ok(tokens)
    }
    fn materialize(&self, value: Value) -> Result<AST> {
        let loc = self.callsite.clone();
        Ok(match value {
            Value::Syntax(id) => self.nodes[id].clone(),
            Value::Int(value) if value >= 0 => AST::NumberLiteral(NumberLiteral::new(
                Tagged::new(loc, value.to_string()),
                NumberType::Int,
            )),
            Value::Int(value) => AST::UnaryExpr(UnaryExpr::new(
                Tagged::new(loc.clone(), "-".into()),
                Box::new(AST::NumberLiteral(NumberLiteral::new(
                    Tagged::new(loc, value.unsigned_abs().to_string()),
                    NumberType::Int,
                ))),
            )),
            Value::Bool(value) => AST::CastExpr(CastExpr::new(
                Box::new(AST::NumberLiteral(NumberLiteral::new(
                    Tagged::new(loc.clone(), if value { "1" } else { "0" }.into()),
                    NumberType::Int,
                ))),
                Type::Path(Path::new(loc, "bool")),
            )),
            Value::String(value) => {
                let escaped = value
                    .replace('\\', "\\\\")
                    .replace('"', "\\\"")
                    .replace('\n', "\\n")
                    .replace('\r', "\\r")
                    .replace('\t', "\\t")
                    .replace('\0', "\\0");
                AST::StringLiteral(StringLiteral::new(Tagged::new(loc, escaped)))
            }
            Value::Array(values) => AST::ArrayLiteral(ArrayLiteral::new(
                loc,
                values
                    .into_iter()
                    .map(|value| self.materialize(value))
                    .collect::<Result<_>>()?,
            )),
            Value::Void => return self.fail("inline macro must return a value or Ast node"),
            Value::Field(_) | Value::Type(_) => {
                return self.fail("meta field/type handles cannot become runtime expressions");
            }
        })
    }
    fn expand_block(&mut self, block: &mut Block) -> Result<()> {
        let mut output = Vec::new();
        for mut node in std::mem::take(&mut block.body) {
            if let AST::Attributed {
                attributes,
                declaration,
            } = &node
            {
                if attributes
                    .iter()
                    .any(|attribute| attribute.name.inner() == "meta")
                {
                    self.callsite = attributes[0].name.loc().clone();
                    if attributes.len() != 1
                        || !attributes[0].args.is_empty()
                        || !valid_meta_declaration(declaration)
                    {
                        return self.fail("invalid compiler meta declaration; use the signatures from meta/builtins.ly");
                    }
                    continue;
                }
            }
            if matches!(node, AST::ComptimeFunction(_)) {
                continue;
            }
            let type_name = match &node {
                AST::StructDef(item) => Some(&item.id),
                AST::EnumDef(item) => Some(&item.id),
                AST::TypeAlias(item) => Some(&item.id),
                _ => None,
            };
            if let Some(name) = type_name {
                if matches!(
                    name.inner().as_str(),
                    "Ast" | "AstExpr" | "AstDecl" | "AstField" | "AstType" | "ast"
                ) {
                    return Err(TypeError::new(
                        name.loc().clone(),
                        "compiler meta types cannot be redefined as runtime types",
                    ));
                }
            }
            self.expand_node(&mut node)?;
            output.push(node);
        }
        block.body = output;
        Ok(())
    }
    fn expand_node(&mut self, node: &mut AST) -> Result<()> {
        let expansion = matches!(node, AST::Attributed { .. })
            || matches!(node, AST::MacroCall(call) if self.functions.contains_key(call.id.inner()));
        if !expansion {
            return self.expand_inner(node);
        }
        self.tick()?;
        if self.expansion_depth >= 64 {
            return self.fail("macro expansion depth exceeded (64)");
        }
        self.expansion_depth += 1;
        let result = self.expand_inner(node);
        self.expansion_depth -= 1;
        result
    }
    fn expand_inner(&mut self, node: &mut AST) -> Result<()> {
        match node {
            AST::Attributed {
                attributes,
                declaration,
            } => {
                let mut handle = self.syntax(*declaration.clone())?;
                for attribute in attributes {
                    self.callsite = attribute.name.loc().clone();
                    if attribute.name.inner() == "derive" {
                        if attribute.args.is_empty() {
                            return self
                                .fail("derive requires at least one compile-time function name");
                        }
                        for arg in &attribute.args {
                            if arg.len() != 1 || !matches!(arg[0].typ, TokenType::Id(_)) {
                                return self
                                    .fail("derive arguments must be compile-time function names");
                            }
                            let value = self.invoke(&arg[0].typ.as_str(), vec![handle.clone()])?;
                            if value != Value::Void {
                                return self.fail("derive functions must return void and emit declarations with ast::emit");
                            }
                        }
                    } else {
                        let mut args = vec![handle.clone()];
                        for tokens in &attribute.args {
                            let expr = self.parse_tokens(tokens.clone())?;
                            args.push(self.eval(&expr)?);
                        }
                        let value = self.invoke(attribute.name.inner(), args)?;
                        if value != Value::Void {
                            self.node(&value)?;
                            handle = value;
                        }
                    }
                }
                let id = self.node(&handle)?;
                *node = self.nodes[id].clone();
                if !is_declaration(node) {
                    return self.fail("declaration attribute must produce a declaration");
                }
                self.expand_node(node)?;
            }
            AST::MacroCall(call) if self.functions.contains_key(call.id.inner()) => {
                self.callsite = call.id.loc().clone();
                let function = self.functions[call.id.inner()].clone();
                if function.args.len() != call.args.len() {
                    return self.fail(format!(
                        "macro `{}` expects {} arguments, found {}",
                        call.id.inner(),
                        function.args.len(),
                        call.args.len()
                    ));
                }
                let mut values = Vec::new();
                for (param, tokens) in function.args.iter().zip(&call.args) {
                    let mut expr = self.parse_tokens(tokens.clone())?;
                    if matches!(&param.typ, Type::Path(path) if matches!(path.path.as_str(), "Ast" | "AstExpr" | "AstDecl"))
                    {
                        values.push(self.syntax(expr)?);
                    } else {
                        self.expand_node(&mut expr)?;
                        values.push(self.eval(&expr)?);
                    }
                }
                self.callsite = call.id.loc().clone();
                let value = self.invoke(call.id.inner(), values)?;
                *node = self.materialize(value)?;
                self.expand_node(node)?;
            }
            AST::Module(module) => self.expand_block(&mut module.body)?,
            AST::Function(function) => self.expand_block(&mut function.body)?,
            AST::Impl(item) => self.expand_block(&mut item.body)?,
            AST::Trait(item) => self.expand_block(&mut item.funcs)?,
            AST::VarDecl(item) => {
                if let Some(value) = &mut item.value {
                    self.expand_node(value)?;
                }
            }
            AST::StaticLiteral(item) => {
                if let Some(value) = &mut item.value {
                    self.expand_node(value)?;
                }
            }
            AST::ReturnExpr(item) => {
                if let Some(value) = &mut item.value {
                    self.expand_node(value)?;
                }
            }
            AST::UnaryExpr(item) => self.expand_node(&mut item.obj)?,
            AST::CastExpr(item) => self.expand_node(&mut item.obj)?,
            AST::BinaryExpr(item) => {
                self.expand_node(&mut item.lhs)?;
                self.expand_node(&mut item.rhs)?;
            }
            AST::AssignmentExpr(item) => {
                self.expand_node(&mut item.lhs)?;
                self.expand_node(&mut item.rhs)?;
            }
            AST::CallExpr(item) => {
                self.expand_node(&mut item.caller)?;
                for arg in &mut item.args {
                    self.expand_node(arg)?;
                }
            }
            AST::MemberCallExpr(item) => {
                self.expand_node(&mut item.obj)?;
                self.expand_node(&mut item.r#fn)?;
            }
            AST::AccessExpr(item) => {
                self.expand_node(&mut item.obj)?;
                self.expand_node(&mut item.index)?;
            }
            AST::MemberExpr(MemberExpr::Member { obj, member, .. }) => {
                self.expand_node(obj)?;
                self.expand_node(member)?;
            }
            AST::MemberExpr(MemberExpr::Index { obj, .. }) => self.expand_node(obj)?,
            AST::ResolutionExpr(item) => self.expand_node(&mut item.obj)?,
            AST::ArrayLiteral(item) => {
                for value in &mut item.values {
                    self.expand_node(value)?;
                }
            }
            AST::StructLiteral(item) => {
                for (_, value) in &mut item.inits {
                    self.expand_node(value)?;
                }
            }
            AST::IfStmt(item) => {
                self.expand_node(&mut item.cond)?;
                self.expand_block(&mut item.then_body)?;
                if let Some(block) = &mut item.else_body {
                    self.expand_block(block)?;
                }
            }
            AST::ForStmt(item) => {
                self.expand_node(&mut item.var)?;
                self.expand_node(&mut item.cond)?;
                self.expand_block(&mut item.body)?;
            }
            AST::MacroCall(call) => {
                if let Some(args) = &mut call.parsed_args {
                    for arg in args {
                        self.expand_node(arg)?;
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
    fn compatible(&self, typ: &Type, value: &Value) -> bool {
        match typ {
            Type::Path(path) => match path.path.as_str() {
                "Ast" => matches!(value, Value::Syntax(_)),
                "AstExpr" => matches!(value, Value::Syntax(id) if is_expression(&self.nodes[*id])),
                "AstDecl" => matches!(value, Value::Syntax(id) if is_declaration(&self.nodes[*id])),
                "AstField" => matches!(value, Value::Field(_)),
                "AstType" => matches!(value, Value::Type(_)),
                "bool" => matches!(value, Value::Bool(_)),
                "void" => matches!(value, Value::Void),
                "i32" => matches!(value, Value::Int(n) if i32::try_from(*n).is_ok()),
                "i64" => matches!(value, Value::Int(_)),
                "u32" => matches!(value, Value::Int(n) if u32::try_from(*n).is_ok()),
                "u64" => matches!(value, Value::Int(n) if *n >= 0),
                _ => false,
            },
            Type::Pointer(pointer) => {
                pointer.is_ref
                    && !pointer.is_mut
                    && matches!(&*pointer.typ, Type::Path(path) if path.path == "str")
                    && matches!(value, Value::String(_))
            }
            Type::Array(array) => {
                matches!(value, Value::Array(values) if values.len() == array.elems && values.iter().all(|value| self.compatible(&array.typ, value)))
            }
            Type::Slice(slice) => {
                matches!(value, Value::Array(values) if values.iter().all(|value| self.compatible(&slice.typ, value)))
            }
            _ => false,
        }
    }
    fn invoke(&mut self, name: &str, args: Vec<Value>) -> Result<Value> {
        let Some(function) = self.functions.get(name) else {
            return self.fail(format!("unknown compile-time function `{name}`"));
        };
        if args.len() != function.args.len() {
            return self.fail(format!(
                "compile-time function `{name}` expects {} arguments",
                function.args.len()
            ));
        }
        for (param, value) in function.args.iter().zip(&args) {
            if !self.compatible(&param.typ, value) {
                return self.fail(format!(
                    "compile-time argument `{}` requires `{}`",
                    param.id.inner(),
                    param.typ.get_full_name()
                ));
            }
        }
        let program = match self.compiled.take() {
            Some(program) => program,
            None => native::Program::compile(self, None)?,
        };
        let result = program.execute(self, name, args);
        self.compiled = Some(program);
        result
    }
    fn eval(&mut self, node: &AST) -> Result<Value> {
        // Constant arguments use the same LLVM execution path as macro bodies.
        native::Program::compile(self, Some(node))?.execute(self, "", Vec::new())
    }
    fn meta_type(&mut self, typ: Type) -> Result<Value> {
        self.tick()?;
        Ok(Value::Type(self.meta_types.insert(typ)))
    }
    fn field_handle(&self, id: FieldId) -> Result<FieldHandle> {
        let handle = self.fields[id].clone();
        if self.revisions.get(&handle.owner).copied().unwrap_or(0) != handle.revision
            || !matches!(&self.nodes[handle.owner], AST::StructDef(item) if handle.index < item.fields.len())
        {
            return self.fail("AstField handle no longer refers to a struct field");
        }
        Ok(handle)
    }
    fn meta_method(&mut self, receiver: Value, method: &str, args: Vec<Value>) -> Result<Value> {
        self.tick()?;
        let arity = match (&receiver, method) {
            (Value::Field(_), "name" | "field_type") => 0,
            (Value::Field(_), "rename" | "set_type") => 1,
            (
                Value::Type(_),
                "name" | "kind" | "clone" | "is_reference" | "is_mutable" | "element" | "array_len",
            ) => 0,
            (Value::Array(_), "len") => 0,
            _ => return self.fail(format!("unknown meta method `{method}` for this receiver")),
        };
        if args.len() != arity {
            return self.fail(format!("meta method `{method}` expects {arity} arguments"));
        }
        match receiver {
            Value::Array(values) => Ok(Value::Int(values.len() as i64)),
            Value::Field(id) => {
                let handle = self.field_handle(id)?;
                let AST::StructDef(item) = &self.nodes[handle.owner] else {
                    unreachable!()
                };
                match method {
                    "name" => Ok(Value::String(item.fields[handle.index].id.inner().clone())),
                    "field_type" => self.meta_type(item.fields[handle.index].typ.clone()),
                    "rename" => {
                        let name = self.text(&args[0])?;
                        let parsed = self.parse_tokens(self.lex(&name)?)?;
                        if !matches!(parsed, AST::Identifier(_)) {
                            return self.fail("field name must be a single valid identifier");
                        }
                        let AST::StructDef(item) = self.nodes.get_mut(handle.owner).unwrap() else {
                            unreachable!()
                        };
                        *item.fields[handle.index].id.inner_mut() = name;
                        Ok(Value::Void)
                    }
                    "set_type" => {
                        let Value::Type(typ) = args[0] else {
                            return self.fail("AstField::set_type requires an AstType handle");
                        };
                        let typ = self.meta_types[typ].clone();
                        let AST::StructDef(item) = self.nodes.get_mut(handle.owner).unwrap() else {
                            unreachable!()
                        };
                        item.fields[handle.index].typ = typ;
                        Ok(Value::Void)
                    }
                    _ => unreachable!(),
                }
            }
            Value::Type(id) => {
                let typ = self.meta_types[id].clone();
                match method {
                    "name" => Ok(Value::String(typ.get_full_name())),
                    "kind" => Ok(Value::String(
                        match typ {
                            Type::Path(_) => "named",
                            Type::Pointer(p) if p.is_ref => "reference",
                            Type::Pointer(_) => "pointer",
                            Type::Array(_) => "array",
                            Type::Slice(_) => "slice",
                            Type::FuncPtr(_) => "function",
                        }
                        .into(),
                    )),
                    "clone" => self.meta_type(typ),
                    "is_reference" => Ok(Value::Bool(matches!(typ, Type::Pointer(p) if p.is_ref))),
                    "is_mutable" => Ok(Value::Bool(matches!(typ, Type::Pointer(p) if p.is_mut))),
                    "element" => match typ {
                        Type::Pointer(p) => self.meta_type(*p.typ),
                        Type::Array(a) => self.meta_type(*a.typ),
                        Type::Slice(s) => self.meta_type(*s.typ),
                        _ => self
                            .fail("AstType::element requires a pointer, reference, array or slice"),
                    },
                    "array_len" => match typ {
                        Type::Array(a) => i64::try_from(a.elems).map(Value::Int).map_err(|_| {
                            TypeError::new(
                                self.callsite.clone(),
                                "array length exceeds compile-time i64 range",
                            )
                        }),
                        _ => self.fail("AstType::array_len requires an array type"),
                    },
                    _ => unreachable!(),
                }
            }
            _ => unreachable!(),
        }
    }
    fn host(&mut self, name: &str, args: Vec<Value>) -> Result<Value> {
        self.tick()?;
        let arity = match name {
            "parse_expr" | "parse_item" | "name" | "kind" | "field_count" | "clone" | "emit"
            | "error" | "int" | "ident" | "to_string" | "parse_type" | "as_expr" | "as_decl"
            | "fields" => 1,
            "rename" | "replace" | "field_name" | "field_type" | "field" => 2,
            "add_field" | "binary" => 3,
            _ => return self.fail(format!("unknown compile-time AST operation `ast::{name}`")),
        };
        if args.len() != arity {
            return self.fail(format!("ast::{name} expects {arity} arguments"));
        }
        match name {
            "parse_type" => {
                let source = self.text(&args[0])?;
                let mut parser = Parser::from(self.lex(&source)?);
                parser.validate_delimiters();
                let typ = parser.parse_type_spec();
                if parser.at().typ != TokenType::Eof {
                    return self.fail("ast::parse_type requires exactly one type");
                }
                self.meta_type(typ)
            }
            "as_expr" | "as_decl" => {
                let id = self.node(&args[0])?;
                if (name == "as_expr" && !is_expression(&self.nodes[id]))
                    || (name == "as_decl" && !is_declaration(&self.nodes[id]))
                {
                    return self.fail(format!("Ast::{name} received the wrong node kind"));
                }
                Ok(args[0].clone())
            }
            "field" | "fields" => {
                let owner = self.node(&args[0])?;
                let AST::StructDef(item) = &self.nodes[owner] else {
                    return self.fail("field operations require a struct declaration");
                };
                let count = item.fields.len();
                if name == "field" {
                    let index = self.integer(&args[1])?;
                    if index < 0 || index as usize >= count {
                        return self.fail("AST field index out of bounds");
                    }
                    self.tick()?;
                    Ok(Value::Field(self.fields.insert(FieldHandle {
                        owner,
                        index: index as usize,
                        revision: self.revisions.get(&owner).copied().unwrap_or(0),
                    })))
                } else {
                    let mut fields = Vec::new();
                    for index in 0..count {
                        self.tick()?;
                        fields.push(Value::Field(self.fields.insert(FieldHandle {
                            owner,
                            index,
                            revision: self.revisions.get(&owner).copied().unwrap_or(0),
                        })));
                    }
                    Ok(Value::Array(fields))
                }
            }
            "parse_expr" => {
                let source = self.text(&args[0])?;
                let node = self.parse_tokens(self.lex(&source)?)?;
                self.syntax(node)
            }
            "parse_item" => {
                let source = self.text(&args[0])?;
                let AST::Module(mut module) = Parser::from(self.lex(&source)?).build_raw_ast()
                else {
                    unreachable!()
                };
                if module.body.body.len() != 1 {
                    return self.fail("ast::parse_item requires exactly one declaration");
                }
                let node = module.body.body.remove(0);
                if !is_declaration(&node) {
                    return self.fail("ast::parse_item requires a declaration");
                }
                self.syntax(node)
            }
            "clone" => {
                let id = self.node(&args[0])?;
                self.syntax(self.nodes[id].clone())
            }
            "replace" => {
                let id = self.node(&args[0])?;
                let replacement = self.node(&args[1])?;
                *self.nodes.get_mut(id).unwrap() = self.nodes[replacement].clone();
                *self.revisions.entry(id).or_default() += 1;
                Ok(Value::Void)
            }
            "emit" => {
                let id = self.node(&args[0])?;
                if !is_declaration(&self.nodes[id]) {
                    return self.fail("ast::emit requires a runtime declaration");
                }
                let cost = syntax_cost(&self.nodes[id]);
                if cost > self.syntax_budget {
                    return self.fail("compile-time syntax allocation limit exceeded");
                }
                self.syntax_budget -= cost;
                self.emitted.push(self.nodes[id].clone());
                Ok(Value::Void)
            }
            "error" => self.fail(self.text(&args[0])?),
            "int" => self.syntax(self.materialize(Value::Int(self.integer(&args[0])?))?),
            "ident" => {
                let name = self.text(&args[0])?;
                let node = self.parse_tokens(self.lex(&name)?)?;
                if !matches!(node, AST::Identifier(_)) {
                    return self.fail("ast::ident requires a single valid identifier");
                }
                self.syntax(node)
            }
            "binary" => {
                let op = self.text(&args[0])?;
                if !matches!(
                    op.as_str(),
                    "+" | "-"
                        | "*"
                        | "/"
                        | "%"
                        | "=="
                        | "!="
                        | "<"
                        | ">"
                        | "<="
                        | ">="
                        | "&&"
                        | "||"
                ) {
                    return self.fail("invalid AST binary operator");
                }
                let left = self.node(&args[1])?;
                let right = self.node(&args[2])?;
                if !is_expression(&self.nodes[left]) || !is_expression(&self.nodes[right]) {
                    return self.fail("ast::binary requires expression nodes");
                }
                self.syntax(AST::BinaryExpr(BinaryExpr::new(
                    Box::new(self.nodes[left].clone()),
                    Box::new(self.nodes[right].clone()),
                    Tagged::new(self.callsite.clone(), op),
                )))
            }
            "to_string" => Ok(Value::String(match &args[0] {
                Value::Int(n) => n.to_string(),
                Value::Bool(b) => b.to_string(),
                Value::String(s) => s.clone(),
                _ => return self.fail("ast::to_string requires an integer, bool or string"),
            })),
            "kind" => {
                let id = self.node(&args[0])?;
                Ok(Value::String(
                    match self.nodes[id] {
                        AST::StructDef(_) => "struct",
                        AST::Function(_) => "function",
                        AST::EnumDef(_) => "enum",
                        AST::TypeAlias(_) => "alias",
                        AST::Identifier(_) => "identifier",
                        _ => "expression",
                    }
                    .into(),
                ))
            }
            "name" | "rename" => {
                let id = self.node(&args[0])?;
                let replacement = if name == "rename" {
                    let text = self.text(&args[1])?;
                    let node = self.parse_tokens(self.lex(&text)?)?;
                    if !matches!(node, AST::Identifier(_)) {
                        return self.fail("new name must be a single valid identifier");
                    }
                    Some(text)
                } else {
                    None
                };
                let tagged = match self.nodes.get_mut(id).unwrap() {
                    AST::StructDef(item) => &mut item.id,
                    AST::Function(item) => &mut item.id,
                    AST::EnumDef(item) => &mut item.id,
                    AST::TypeAlias(item) => &mut item.id,
                    AST::Identifier(item) => &mut item.id,
                    AST::StaticLiteral(item) => &mut item.id,
                    _ => return self.fail("AST node has no supported name"),
                };
                if let Some(replacement) = replacement {
                    *tagged.inner_mut() = replacement;
                    Ok(Value::Void)
                } else {
                    Ok(Value::String(tagged.inner().clone()))
                }
            }
            "field_count" | "field_name" | "field_type" | "add_field" => {
                let id = self.node(&args[0])?;
                let AST::StructDef(item) = &self.nodes[id] else {
                    return self.fail("field operations require a struct declaration");
                };
                if name == "field_count" {
                    return Ok(Value::Int(item.fields.len() as i64));
                }
                if name == "add_field" {
                    let field_name = self.text(&args[1])?;
                    let field_type = self.text(&args[2])?;
                    let source = format!("struct __MacroField {{ {field_name}: {field_type}; }}");
                    let AST::Module(mut module) = Parser::from(self.lex(&source)?).build_raw_ast()
                    else {
                        unreachable!()
                    };
                    let AST::StructDef(mut parsed) = module.body.body.remove(0) else {
                        unreachable!()
                    };
                    if parsed.fields.len() != 1 {
                        return self.fail("ast::add_field requires one field name and type");
                    }
                    let AST::StructDef(item) = self.nodes.get_mut(id).unwrap() else {
                        unreachable!()
                    };
                    if item.fields.len() >= 10000 || self.syntax_budget == 0 {
                        return self.fail("compile-time syntax allocation limit exceeded");
                    }
                    self.syntax_budget -= 1;
                    item.fields.push(parsed.fields.remove(0));
                    return Ok(Value::Void);
                }
                let index = self.integer(&args[1])?;
                let Some(field) = item.fields.get(index as usize) else {
                    return self.fail("AST field index out of bounds");
                };
                Ok(Value::String(if name == "field_name" {
                    field.id.inner().clone()
                } else {
                    field.typ.get_full_name()
                }))
            }
            _ => unreachable!(),
        }
    }
}

fn is_declaration(node: &AST) -> bool {
    matches!(
        node,
        AST::Function(_)
            | AST::StructDef(_)
            | AST::EnumDef(_)
            | AST::TypeAlias(_)
            | AST::StaticLiteral(_)
            | AST::Impl(_)
            | AST::Trait(_)
            | AST::Attributed { .. }
    )
}

fn is_expression(node: &AST) -> bool {
    matches!(
        node,
        AST::Identifier(_)
            | AST::NumberLiteral(_)
            | AST::StringLiteral(_)
            | AST::ArrayLiteral(_)
            | AST::StructLiteral(_)
            | AST::UnaryExpr(_)
            | AST::BinaryExpr(_)
            | AST::AssignmentExpr(_)
            | AST::CastExpr(_)
            | AST::CallExpr(_)
            | AST::MemberCallExpr(_)
            | AST::MemberExpr(_)
            | AST::AccessExpr(_)
            | AST::ResolutionExpr(_)
            | AST::MacroCall(_)
    )
}

fn valid_meta_declaration(node: &AST) -> bool {
    let AST::Module(catalog) =
        Parser::from(Lexer::from(META_SOURCE.chars().collect()).get_tokens()).build_raw_ast()
    else {
        unreachable!()
    };
    catalog.body.body.iter().any(|candidate| {
        let AST::Attributed {
            declaration: expected,
            ..
        } = candidate
        else {
            return false;
        };
        match (node, &**expected) {
            (AST::StructDef(actual), AST::StructDef(expected)) => {
                actual.id.inner() == expected.id.inner() && actual.fields.is_empty()
            }
            (AST::Impl(actual), AST::Impl(expected))
                if actual.typ.inner() == expected.typ.inner() && actual.trat.is_none() =>
            {
                actual.body.body.len() == expected.body.body.len()
                    && actual
                        .body
                        .body
                        .iter()
                        .zip(&expected.body.body)
                        .all(|(actual, expected)| {
                            let (AST::Function(actual), AST::Function(expected)) =
                                (actual, expected)
                            else {
                                return false;
                            };
                            !actual.is_def
                                && !actual.is_var_arg
                                && actual.id.inner() == expected.id.inner()
                                && actual.args.len() == expected.args.len()
                                && actual.ret_type.matches(&expected.ret_type)
                                && actual
                                    .args
                                    .iter()
                                    .zip(&expected.args)
                                    .all(|(a, b)| a.typ.matches(&b.typ))
                        })
            }
            _ => false,
        }
    })
}

fn syntax_cost(root: &AST) -> usize {
    let mut pending = vec![root];
    let mut cost = 0;
    while let Some(node) = pending.pop() {
        cost += 1;
        match node {
            AST::Module(item) => pending.extend(&item.body.body),
            AST::Function(item) | AST::ComptimeFunction(item) => {
                cost += item.args.len();
                pending.extend(&item.body.body);
            }
            AST::Impl(item) => pending.extend(&item.body.body),
            AST::Trait(item) => pending.extend(&item.funcs.body),
            AST::StructDef(item) => cost += item.fields.len(),
            AST::Attributed { declaration, .. } => pending.push(declaration),
            AST::VarDecl(item) => pending.extend(item.value.as_deref()),
            AST::StaticLiteral(item) => pending.extend(item.value.as_deref()),
            AST::ReturnExpr(item) => pending.extend(item.value.as_deref()),
            AST::UnaryExpr(item) => pending.push(&item.obj),
            AST::CastExpr(item) => pending.push(&item.obj),
            AST::BinaryExpr(item) => pending.extend([&*item.lhs, &*item.rhs]),
            AST::AssignmentExpr(item) => pending.extend([&*item.lhs, &*item.rhs]),
            AST::CallExpr(item) => {
                pending.push(&item.caller);
                pending.extend(&item.args);
            }
            AST::MemberCallExpr(item) => pending.extend([&*item.obj, &*item.r#fn]),
            AST::MemberExpr(MemberExpr::Member { obj, member, .. }) => {
                pending.extend([&**obj, &**member])
            }
            AST::MemberExpr(MemberExpr::Index { obj, .. }) => pending.push(obj),
            AST::AccessExpr(item) => pending.extend([&*item.obj, &*item.index]),
            AST::ResolutionExpr(item) => pending.extend([&*item.obj, &*item.member]),
            AST::ArrayLiteral(item) => pending.extend(&item.values),
            AST::StructLiteral(item) => pending.extend(item.inits.iter().map(|(_, node)| node)),
            AST::IfStmt(item) => {
                pending.push(&item.cond);
                pending.extend(&item.then_body.body);
                if let Some(block) = &item.else_body {
                    pending.extend(&block.body);
                }
            }
            AST::ForStmt(item) => {
                pending.extend([&*item.cond, &*item.var]);
                pending.extend(&item.body.body);
            }
            AST::MacroCall(item) => {
                cost += item.args.iter().map(Vec::len).sum::<usize>();
                if let Some(args) = &item.parsed_args {
                    pending.extend(args);
                }
            }
            _ => {}
        }
        if cost > 10_000 {
            break;
        }
    }
    cost
}
