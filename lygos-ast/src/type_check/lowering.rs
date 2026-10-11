//! Convert validated syntax plus retained type facts into an owned typed graph.
//! Name/field/receiver resolution ends here; LLVM lowering never sees source AST.
use super::*;
use crate::typed::{self as ir, *};

pub(super) fn lower(ast: &AST, checker: &TypeChecker) -> Result<TypedModule> {
    let AST::Module(module) = ast else {
        unreachable!()
    };
    let mut builder = Builder::new(checker);
    builder.declarations(&module.body)?;
    for ast in &module.body.body {
        match ast {
            AST::Function(function) => builder.function(function, None)?,
            AST::Impl(item) => {
                let owner = checker
                    .canonical(&path(item.typ.loc().clone(), item.typ.inner()))?
                    .get_name();
                for node in &item.body.body {
                    if let AST::Function(function) = node {
                        builder.function(function, Some(&owner))?;
                    }
                }
            }
            AST::StaticLiteral(item) => {
                let symbol = builder
                    .context
                    .lookup(builder.module_scope, item.id.inner())
                    .unwrap();
                let initializer = item
                    .value
                    .as_deref()
                    .map(|value| builder.value(value))
                    .transpose()?;
                builder.context.symbols.get_mut(symbol).unwrap().kind =
                    SymbolKind::Static { initializer };
            }
            _ => {}
        }
    }
    Ok(TypedModule {
        context: builder.context,
        functions: builder.functions,
        statics: builder.statics,
        scope: builder.module_scope,
    })
}

struct Builder<'a> {
    checker: &'a TypeChecker,
    context: SemanticContext,
    module_scope: ScopeId,
    scope: ScopeId,
    functions: Vec<FunctionId>,
    statics: Vec<SymbolId>,
    named_types: HashMap<String, SymbolId>,
    methods: HashMap<(String, String), SymbolId>,
}
impl<'a> Builder<'a> {
    fn new(checker: &'a TypeChecker) -> Self {
        let mut context = SemanticContext::default();
        let module_scope = context.scopes.insert(SemanticScope {
            parent: None,
            symbols: HashMap::new(),
        });
        Self {
            checker,
            context,
            module_scope,
            scope: module_scope,
            functions: Vec::new(),
            statics: Vec::new(),
            named_types: HashMap::new(),
            methods: HashMap::new(),
        }
    }
    fn scope(&mut self, parent: ScopeId) -> ScopeId {
        self.context.scopes.insert(SemanticScope {
            parent: Some(parent),
            symbols: HashMap::new(),
        })
    }
    fn symbol(
        &mut self,
        name: &str,
        typ: TypeId,
        kind: SymbolKind,
        span: Location,
        register: bool,
    ) -> SymbolId {
        let id = self.context.symbols.insert(SemanticSymbol {
            name: name.into(),
            typ,
            kind,
            scope: self.scope,
            span,
        });
        if register {
            self.context
                .scopes
                .get_mut(self.scope)
                .unwrap()
                .symbols
                .insert(name.into(), id);
        }
        id
    }
    fn primitive(&mut self, primitive: Primitive) -> TypeId {
        self.context
            .types
            .intern(SemanticType::Primitive(primitive))
    }
    fn typ(&mut self, typ: &Type) -> Result<TypeId> {
        let canonical = self.checker.canonical(typ)?;
        let kind = match canonical {
            Type::Path(path) => {
                if let Some(primitive) = Primitive::from_name(&path.path) {
                    SemanticType::Primitive(primitive)
                } else {
                    let symbol = *self
                        .named_types
                        .get(&path.path)
                        .ok_or_else(|| TypeError::new(path.loc, "unresolved semantic type"))?;
                    if self.checker.enums.contains_key(&path.path) {
                        SemanticType::Enum(symbol)
                    } else {
                        SemanticType::Struct(symbol)
                    }
                }
            }
            Type::Pointer(pointer) => SemanticType::Pointer {
                pointee: self.typ(&pointer.typ)?,
                reference: pointer.is_ref,
                mutable: pointer.is_mut,
            },
            Type::Array(array) => SemanticType::Array {
                element: self.typ(&array.typ)?,
                length: array.elems,
            },
            Type::Slice(slice) => SemanticType::Slice {
                element: self.typ(&slice.typ)?,
            },
            Type::FuncPtr(function) => SemanticType::Function {
                parameters: function
                    .params
                    .iter()
                    .map(|typ| self.typ(typ))
                    .collect::<Result<_>>()?,
                result: self.typ(&function.ret_type)?,
            },
        };
        Ok(self.context.types.intern(kind))
    }
    fn declarations(&mut self, block: &Block) -> Result<()> {
        let void = self.primitive(Primitive::Void);
        for ast in &block.body {
            let (name, kind) = match ast {
                AST::StructDef(item) => (&item.id, SymbolKind::Struct { fields: Vec::new() }),
                AST::EnumDef(item) => (
                    &item.id,
                    SymbolKind::Enum {
                        underlying: void,
                        variants: item.variants.clone(),
                    },
                ),
                _ => continue,
            };
            let symbol = self.symbol(name.inner(), void, kind, name.loc().clone(), true);
            self.named_types.insert(name.inner().clone(), symbol);
        }
        for ast in &block.body {
            match ast {
                AST::StructDef(item) => {
                    let owner = self.named_types[item.id.inner()];
                    let typ = self.typ(&path(item.id.loc().clone(), item.id.inner()))?;
                    self.context.symbols.get_mut(owner).unwrap().typ = typ;
                    let mut fields = Vec::new();
                    for (index, field) in item.fields.iter().enumerate() {
                        let typ = self.typ(&field.typ)?;
                        fields.push(self.symbol(
                            field.id.inner(),
                            typ,
                            SymbolKind::Field {
                                index: index as u32,
                            },
                            field.id.loc().clone(),
                            false,
                        ));
                    }
                    self.context.symbols.get_mut(owner).unwrap().kind =
                        SymbolKind::Struct { fields };
                }
                AST::EnumDef(item) => {
                    let owner = self.named_types[item.id.inner()];
                    let typ = self.typ(&path(item.id.loc().clone(), item.id.inner()))?;
                    let underlying = self.typ(&item.typ)?;
                    let symbol = self.context.symbols.get_mut(owner).unwrap();
                    symbol.typ = typ;
                    symbol.kind = SymbolKind::Enum {
                        underlying,
                        variants: item.variants.clone(),
                    };
                }
                AST::TypeAlias(item) => {
                    let typ = self.typ(&item.typ)?;
                    self.symbol(
                        item.id.inner(),
                        typ,
                        SymbolKind::TypeAlias,
                        item.id.loc().clone(),
                        true,
                    );
                }
                AST::StaticLiteral(item) => {
                    let typ = self.typ(&item.typ)?;
                    let symbol = self.symbol(
                        item.id.inner(),
                        typ,
                        SymbolKind::Static { initializer: None },
                        item.id.loc().clone(),
                        true,
                    );
                    self.statics.push(symbol);
                }
                _ => {}
            }
        }
        for ast in &block.body {
            match ast {
                AST::Function(function) => {
                    self.declare_function(function, None)?;
                }
                AST::Impl(item) => {
                    let owner = self
                        .checker
                        .canonical(&path(item.typ.loc().clone(), item.typ.inner()))?
                        .get_name();
                    for ast in &item.body.body {
                        if let AST::Function(function) = ast {
                            self.declare_function(function, Some(&owner))?;
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
    fn declare_function(&mut self, function: &Function, owner: Option<&str>) -> Result<SymbolId> {
        let name = function.id.inner();
        let existing = if let Some(owner) = owner {
            self.methods.get(&(owner.into(), name.clone())).copied()
        } else {
            self.context.lookup(self.module_scope, name)
        };
        if let Some(symbol) = existing {
            return Ok(symbol);
        }
        let params = function
            .args
            .iter()
            .map(|arg| self.typ(&arg.typ))
            .collect::<Result<Vec<_>>>()?;
        let result = self.typ(&function.ret_type)?;
        let typ = self.context.types.intern(SemanticType::Function {
            parameters: params,
            result,
        });
        let scope = self.scope(self.module_scope);
        let function_id = FunctionId::from_index(self.context.functions.len() as u32);
        let link_name = owner
            .map(|owner| format!("{owner}::{name}"))
            .unwrap_or_else(|| name.clone());
        let symbol = self.symbol(
            name,
            typ,
            SymbolKind::Function {
                function: function_id,
                link_name,
            },
            function.id.loc().clone(),
            owner.is_none(),
        );
        let id = self.context.functions.insert(TypedFunction {
            symbol,
            parameters: Vec::new(),
            body: None,
            scope,
        });
        self.functions.push(id);
        if let Some(owner) = owner {
            self.methods.insert((owner.into(), name.clone()), symbol);
        }
        Ok(symbol)
    }
    fn function(&mut self, function: &Function, owner: Option<&str>) -> Result<()> {
        let symbol = self.declare_function(function, owner)?;
        let SymbolKind::Function { function: id, .. } = self.context.symbol(symbol).kind else {
            unreachable!()
        };
        if !function.is_def {
            return Ok(());
        }
        self.scope = self.context.function(id).scope;
        let mut params = Vec::new();
        for arg in &function.args {
            let typ = self.typ(&arg.typ)?;
            params.push(self.symbol(
                arg.id.inner(),
                typ,
                SymbolKind::Parameter,
                arg.id.loc().clone(),
                true,
            ));
        }
        let body = self.block(&function.body, false).map_err(|mut error| {
            error.message = format!("in function `{}`: {}", function.id.inner(), error.message);
            error
        })?;
        let target = self.context.functions.get_mut(id).unwrap();
        target.parameters = params;
        target.body = Some(body);
        self.scope = self.module_scope;
        Ok(())
    }
    fn block(&mut self, block: &Block, nested: bool) -> Result<BlockId> {
        let previous = self.scope;
        if nested {
            self.scope = self.scope(previous);
        }
        let scope = self.scope;
        let statements = block
            .body
            .iter()
            .map(|ast| self.statement(ast))
            .collect::<Result<_>>()?;
        self.scope = previous;
        Ok(self.context.blocks.insert(TypedBlock { statements, scope }))
    }
    fn statement(&mut self, ast: &AST) -> Result<StmtId> {
        let kind = match ast {
            AST::VarDecl(item) => {
                let typ = self.checked_type(ast)?;
                let initializer = item
                    .value
                    .as_deref()
                    .map(|value| {
                        let value = self.value(value)?;
                        Ok(self.convert(value, typ, location(ast)))
                    })
                    .transpose()?;
                let symbol = self.symbol(
                    item.id.inner(),
                    typ,
                    SymbolKind::Local {
                        mutable: !item.cnst,
                    },
                    item.id.loc().clone(),
                    true,
                );
                StmtKind::Let {
                    symbol,
                    initializer,
                }
            }
            AST::ReturnExpr(item) => StmtKind::Return(
                item.value
                    .as_deref()
                    .map(|value| self.value(value))
                    .transpose()?,
            ),
            AST::BreakExpr(_) => StmtKind::Break,
            AST::IfStmt(item) => StmtKind::If {
                condition: self.value(&item.cond)?,
                then_block: self.block(&item.then_body, true)?,
                else_block: item
                    .else_body
                    .as_ref()
                    .map(|block| self.block(block, true))
                    .transpose()?,
            },
            AST::ForStmt(item) => {
                let previous = self.scope;
                self.scope = self.scope(previous);
                let scope = self.scope;
                let initializer = self.statement(&item.var)?;
                let condition = self.value(&item.cond)?;
                let body = self.block(&item.body, true)?;
                self.scope = previous;
                StmtKind::For {
                    initializer,
                    condition,
                    body,
                    scope,
                }
            }
            _ => StmtKind::Expression(self.value(ast)?),
        };
        Ok(self.context.statements.insert(TypedStmt {
            kind,
            span: location(ast),
        }))
    }
    fn checked_type(&mut self, ast: &AST) -> Result<TypeId> {
        let typ = self
            .checker
            .expression_types
            .get(&(ast as *const AST as usize))
            .ok_or_else(|| TypeError::new(location(ast), "missing semantic type fact"))?;
        self.typ(typ)
    }
    fn add(
        &mut self,
        kind: ExprKind,
        typ: TypeId,
        category: ValueCategory,
        span: Location,
    ) -> ExprId {
        self.context.expressions.insert(TypedExpr {
            kind,
            typ,
            category,
            span,
        })
    }
    fn value(&mut self, ast: &AST) -> Result<ExprId> {
        let expr = self.expression(ast)?;
        Ok(self.load(expr))
    }
    fn load(&mut self, expr: ExprId) -> ExprId {
        let value = self.context.expression(expr);
        if value.category == ValueCategory::Value {
            expr
        } else {
            self.add(
                ExprKind::Load(expr),
                value.typ,
                ValueCategory::Value,
                value.span.clone(),
            )
        }
    }
    fn convert(&mut self, value: ExprId, typ: TypeId, span: Location) -> ExprId {
        if self.context.expression(value).typ == typ {
            value
        } else {
            self.add(ExprKind::Cast(value), typ, ValueCategory::Value, span)
        }
    }
    fn dereference(&mut self, value: ExprId) -> ExprId {
        let SemanticType::Pointer { pointee, .. } =
            *self.context.typ(self.context.expression(value).typ)
        else {
            unreachable!()
        };
        let value = self.load(value);
        self.add(
            ExprKind::Dereference(value),
            pointee,
            ValueCategory::Place,
            self.context.expression(value).span.clone(),
        )
    }
    fn address(&mut self, value: ExprId, typ: TypeId) -> ExprId {
        self.add(
            ExprKind::AddressOf(value),
            typ,
            ValueCategory::Value,
            self.context.expression(value).span.clone(),
        )
    }
    fn field(&self, typ: TypeId, name: &str) -> SymbolId {
        let SemanticType::Struct(owner) = self.context.typ(typ) else {
            unreachable!()
        };
        let SymbolKind::Struct { fields } = &self.context.symbol(*owner).kind else {
            unreachable!()
        };
        *fields
            .iter()
            .find(|id| self.context.symbol(**id).name == name)
            .unwrap()
    }
    fn call(
        &mut self,
        symbol: SymbolId,
        mut args: Vec<ExprId>,
        source: &[AST],
        typ: TypeId,
        span: Location,
    ) -> Result<ExprId> {
        for arg in source {
            args.push(self.value(arg)?);
        }
        Ok(self.add(
            ExprKind::Call {
                callee: Callee::Direct(symbol),
                signature: self.context.symbol(symbol).typ,
                arguments: args,
            },
            typ,
            ValueCategory::Value,
            span,
        ))
    }
    fn expression(&mut self, ast: &AST) -> Result<ExprId> {
        let typ = self.checked_type(ast)?;
        let span = location(ast);
        let mut category = ValueCategory::Value;
        let kind = match ast {
            AST::Identifier(item) => {
                let symbol = self.context.lookup(self.scope, item.id.inner()).unwrap();
                if !matches!(
                    self.context.symbol(symbol).kind,
                    SymbolKind::Function { .. }
                ) {
                    category = ValueCategory::Place;
                }
                ExprKind::Symbol(symbol)
            }
            AST::NumberLiteral(item) => match item.typ {
                NumberType::Int => ExprKind::Integer(item.value.inner().parse().unwrap()),
                NumberType::Char => ExprKind::Integer(
                    unescape_literal(item.value.inner()).unwrap().as_bytes()[0] as i32,
                ),
                _ => ExprKind::Float(item.value.inner().parse().unwrap()),
            },
            AST::StringLiteral(item) => {
                ExprKind::String(unescape_literal(item.value.inner()).unwrap())
            }
            AST::UnaryExpr(item) => {
                if item.op.inner() == "-"
                    && matches!(&*item.obj, AST::NumberLiteral(number) if number.value.inner() == "2147483648")
                {
                    return Ok(self.add(ExprKind::Integer(i32::MIN), typ, category, span));
                }
                match item.op.inner().as_str() {
                    "&" | "&mut" => ExprKind::AddressOf(self.expression(&item.obj)?),
                    "*" => {
                        category = ValueCategory::Place;
                        ExprKind::Dereference(self.value(&item.obj)?)
                    }
                    "+" => return self.value(&item.obj),
                    op => ExprKind::Unary {
                        op: if op == "!" {
                            UnaryOp::Not
                        } else {
                            UnaryOp::Negate
                        },
                        operand: self.value(&item.obj)?,
                    },
                }
            }
            AST::BinaryExpr(item) => ExprKind::Binary {
                op: match item.op.inner().as_str() {
                    "+" => ir::BinaryOp::Add,
                    "-" => ir::BinaryOp::Sub,
                    "*" => ir::BinaryOp::Mul,
                    "/" => ir::BinaryOp::Div,
                    "%" => ir::BinaryOp::Rem,
                    "==" => ir::BinaryOp::Eq,
                    "!=" => ir::BinaryOp::Ne,
                    "<" => ir::BinaryOp::Lt,
                    "<=" => ir::BinaryOp::Le,
                    ">" => ir::BinaryOp::Gt,
                    ">=" => ir::BinaryOp::Ge,
                    "&&" => ir::BinaryOp::And,
                    "||" => ir::BinaryOp::Or,
                    _ => unreachable!(),
                },
                left: self.value(&item.lhs)?,
                right: self.value(&item.rhs)?,
            },
            AST::AssignmentExpr(item) => ExprKind::Assign {
                target: self.expression(&item.lhs)?,
                value: self.value(&item.rhs)?,
            },
            AST::CastExpr(item) => ExprKind::Cast(self.value(&item.obj)?),
            AST::ArrayLiteral(item) if item.repeat.is_some() => ExprKind::ArrayRepeat {
                value: self.value(&item.values[0])?,
                length: item.repeat.unwrap(),
            },
            AST::ArrayLiteral(item) => ExprKind::Array(
                item.values
                    .iter()
                    .map(|ast| self.value(ast))
                    .collect::<Result<_>>()?,
            ),
            AST::StructLiteral(item) => {
                let mut inits = Vec::new();
                let SemanticType::Struct(owner) = self.context.typ(typ) else {
                    unreachable!()
                };
                let SymbolKind::Struct { fields } = &self.context.symbol(*owner).kind else {
                    unreachable!()
                };
                let fields = fields.clone();
                for (index, (name, value)) in item.inits.iter().enumerate() {
                    let field = name
                        .as_ref()
                        .map(|name| self.field(typ, name.inner()))
                        .unwrap_or(fields[index]);
                    inits.push((field, self.value(value)?));
                }
                ExprKind::Struct(inits)
            }
            AST::AccessExpr(item) => {
                let base = self.expression(&item.obj)?;
                let base = if matches!(
                    self.context.typ(self.context.expression(base).typ),
                    SemanticType::Pointer { .. } | SemanticType::Slice { .. }
                ) {
                    self.load(base)
                } else {
                    base
                };
                category = ValueCategory::Place;
                ExprKind::Index {
                    base,
                    index: self.value(&item.index)?,
                }
            }
            AST::MemberExpr(MemberExpr::Member { obj, member, deref }) => {
                let base = self.expression(obj)?;
                let obj_type = self.context.expression(base).typ;
                let base = if *deref
                    || matches!(
                        self.context.typ(obj_type),
                        SemanticType::Pointer {
                            reference: true,
                            ..
                        }
                    ) {
                    self.dereference(base)
                } else {
                    base
                };
                let AST::Identifier(name) = &**member else {
                    unreachable!()
                };
                let field = self.field(self.context.expression(base).typ, name.id.inner());
                category = ValueCategory::Place;
                ExprKind::Field { base, field }
            }
            AST::CallExpr(item) => {
                let function = self.value(&item.caller)?;
                let callee = match self.context.expression(function).kind {
                    ExprKind::Symbol(symbol)
                        if matches!(
                            self.context.symbol(symbol).kind,
                            SymbolKind::Function { .. }
                        ) =>
                    {
                        Callee::Direct(symbol)
                    }
                    _ => Callee::Indirect(function),
                };
                ExprKind::Call {
                    callee,
                    signature: self.context.expression(function).typ,
                    arguments: item
                        .args
                        .iter()
                        .map(|arg| self.value(arg))
                        .collect::<Result<_>>()?,
                }
            }
            AST::MemberCallExpr(item) => {
                let receiver_type =
                    self.checker.expression_types[&(&*item.obj as *const AST as usize)].clone();
                if matches!(&receiver_type, Type::Pointer(p) if p.is_ref && matches!(&*p.typ, Type::Path(t) if t.path == "str"))
                {
                    if let AST::CallExpr(call) = &*item.r#fn {
                        if let AST::Identifier(name) = &*call.caller {
                            if matches!(name.id.inner().as_str(), "len" | "as_ptr") {
                                let receiver = self.value(&item.obj)?;
                                let kind = if name.id.inner() == "len" {
                                    ExprKind::StringLength(receiver)
                                } else {
                                    ExprKind::StringData(receiver)
                                };
                                return Ok(self.add(kind, typ, ValueCategory::Value, span));
                            }
                        }
                    }
                }
                let obj_type =
                    self.checker.expression_types[&(&*item.obj as *const AST as usize)].clone();
                let owner = self
                    .checker
                    .member_owner(obj_type, item.deref, span.clone())?;
                let AST::CallExpr(call) = &*item.r#fn else {
                    unreachable!()
                };
                let AST::Identifier(name) = &*call.caller else {
                    unreachable!()
                };
                let symbol = self.methods[&(owner, name.id.inner().clone())];
                let SemanticType::Function { parameters, .. } =
                    self.context.typ(self.context.symbol(symbol).typ)
                else {
                    unreachable!()
                };
                let expected = parameters[0];
                let mut receiver = self.expression(&item.obj)?;
                let actual = self.context.expression(receiver).typ;
                if matches!(self.context.typ(actual), SemanticType::Pointer { .. }) {
                    receiver = if matches!(self.context.typ(expected), SemanticType::Pointer { .. })
                    {
                        self.load(receiver)
                    } else {
                        let place = self.dereference(receiver);
                        self.load(place)
                    };
                } else if matches!(self.context.typ(expected), SemanticType::Pointer { .. }) {
                    receiver = self.address(receiver, expected);
                } else {
                    receiver = self.load(receiver);
                }
                receiver = self.convert(receiver, expected, span.clone());
                return self.call(symbol, vec![receiver], &call.args, typ, span);
            }
            AST::ResolutionExpr(item) => {
                let AST::Identifier(owner) = &*item.obj else {
                    unreachable!()
                };
                let owner = self
                    .checker
                    .canonical(&path(owner.id.loc().clone(), owner.id.inner()))?
                    .get_name();
                if let Some((_, variants)) = self.checker.enums.get(&owner) {
                    let AST::Identifier(name) = &*item.member else {
                        unreachable!()
                    };
                    ExprKind::Integer(
                        variants
                            .iter()
                            .position(|variant| variant == name.id.inner())
                            .unwrap() as i32,
                    )
                } else {
                    let AST::CallExpr(call) = &*item.member else {
                        unreachable!()
                    };
                    let AST::Identifier(name) = &*call.caller else {
                        unreachable!()
                    };
                    let symbol = self.methods[&(owner, name.id.inner().clone())];
                    return self.call(symbol, Vec::new(), &call.args, typ, span);
                }
            }
            AST::MacroCall(item) if item.id.inner() == "sizeof" => {
                let name = item.args[0][0].typ.as_str();
                ExprKind::SizeOf(self.typ(&path(span.clone(), &name))?)
            }
            AST::MacroCall(item) => return self.format(item, typ, span),
            _ => return error(span, "unsupported typed expression"),
        };
        Ok(self.add(kind, typ, category, span))
    }
    fn format(&mut self, item: &MacroCall, typ: TypeId, span: Location) -> Result<ExprId> {
        let source = item.parsed_args.as_ref().unwrap();
        let AST::StringLiteral(format) = &source[0] else {
            unreachable!()
        };
        let (pieces, debug) =
            parse_format_string(&unescape_literal(format.value.inner()).unwrap()).unwrap();
        let argument_type = self.typ(&path(
            span.clone(),
            &(item.format_namespace.clone() + "Argument"),
        ))?;
        let constructor = self
            .methods
            .get(&(item.format_namespace.clone() + "Arguments", "new".into()))
            .copied()
            .ok_or_else(|| TypeError::new(span.clone(), "format_args requires `Arguments::new`"))?;
        self.format_layout(argument_type, constructor, typ, &span)?;
        let mut arguments = Vec::new();
        for (ast, debug) in source[1..].iter().zip(debug) {
            let mut value = self.value(ast)?;
            let mut typ = self.checker.expression_types[&(ast as *const AST as usize)].clone();
            // String formatting borrows the referent directly. It must not format
            // the reference's address, or materialize a bare str value.
            if let Type::Pointer(pointer) = &typ {
                if pointer.is_ref && matches!(&*pointer.typ, Type::Path(path) if path.path == "str")
                {
                    value = self.dereference(value);
                    typ = *pointer.typ.clone();
                }
            }
            let name = typ.get_full_name();
            let formatter = if BaseTypes.contains(&name.as_str()) {
                self.context.lookup(
                    self.module_scope,
                    &(item.format_namespace.clone() + &name + "_fmt"),
                )
            } else if matches!(typ, Type::Pointer(_)) {
                self.context.lookup(
                    self.module_scope,
                    &(item.format_namespace.clone() + "ptr_fmt"),
                )
            } else {
                self.methods
                    .get(&(name, if debug { "fmt_debug" } else { "fmt" }.into()))
                    .copied()
            }
            .ok_or_else(|| {
                TypeError::new(
                    location(ast),
                    "no formatting function available for this type",
                )
            })?;
            self.format_signature(
                argument_type,
                formatter,
                self.context.expression(value).typ,
                &location(ast),
            )?;
            arguments.push((value, formatter));
        }
        Ok(self.add(
            ExprKind::Format {
                arguments,
                pieces,
                argument_type,
                constructor,
            },
            typ,
            ValueCategory::Value,
            span,
        ))
    }
    fn format_layout(
        &self,
        argument: TypeId,
        constructor: SymbolId,
        result: TypeId,
        span: &Location,
    ) -> Result<()> {
        let fail = || {
            TypeError::new(span.clone(), "invalid format_args runtime layout")
            .help("Argument needs an i8 pointer and a two-argument formatter; Arguments::new needs (Argument pointer, u32, str pointer, u32) -> Arguments")
        };
        let SemanticType::Struct(owner) = self.context.typ(argument) else {
            return Err(fail());
        };
        let SymbolKind::Struct { fields } = &self.context.symbol(*owner).kind else {
            return Err(fail());
        };
        if fields.len() != 2 {
            return Err(fail());
        }
        if !matches!(self.context.typ(self.context.symbol(fields[0]).typ), SemanticType::Pointer { pointee, .. }
            if matches!(self.context.typ(*pointee), SemanticType::Primitive(Primitive::I8)))
        {
            return Err(fail());
        }
        if !matches!(self.context.typ(self.context.symbol(fields[1]).typ), SemanticType::Function { parameters, .. } if parameters.len() == 2
            && parameters.iter().all(|id| matches!(self.context.typ(*id), SemanticType::Pointer { .. })))
        {
            return Err(fail());
        }
        let SemanticType::Function {
            parameters,
            result: ret,
        } = self.context.typ(self.context.symbol(constructor).typ)
        else {
            return Err(fail());
        };
        if parameters.len() != 4 || *ret != result {
            return Err(fail());
        }
        if !matches!(self.context.typ(parameters[0]), SemanticType::Pointer { pointee, .. } if *pointee == argument)
            || !matches!(
                self.context.typ(parameters[1]),
                SemanticType::Primitive(Primitive::U32)
            )
            || !matches!(self.context.typ(parameters[2]), SemanticType::Pointer { pointee, .. }
                if matches!(self.context.typ(*pointee), SemanticType::Primitive(Primitive::Str)))
            || !matches!(
                self.context.typ(parameters[3]),
                SemanticType::Primitive(Primitive::U32)
            )
        {
            return Err(fail());
        }
        Ok(())
    }
    fn format_signature(
        &self,
        argument: TypeId,
        formatter: SymbolId,
        value: TypeId,
        span: &Location,
    ) -> Result<()> {
        let SemanticType::Struct(owner) = self.context.typ(argument) else {
            unreachable!()
        };
        let SymbolKind::Struct { fields } = &self.context.symbol(*owner).kind else {
            unreachable!()
        };
        let SemanticType::Function {
            parameters: erased,
            result: expected,
        } = self.context.typ(self.context.symbol(fields[1]).typ)
        else {
            unreachable!()
        };
        let SemanticType::Function { parameters, result } =
            self.context.typ(self.context.symbol(formatter).typ)
        else {
            unreachable!()
        };
        if parameters.len() != 2
            || result != expected
            || parameters[1] != erased[1]
            || !matches!(self.context.typ(parameters[0]), SemanticType::Pointer { pointee, .. } if *pointee == value)
        {
            return Err(TypeError::new(span.clone(), "formatter has an incompatible signature")
                .related(self.context.symbol(formatter).span.clone(), "formatter declared here")
                .help("the formatter must accept a pointer to the value and the runtime formatter receiver, and return the runtime formatting result"));
        }
        Ok(())
    }
}
