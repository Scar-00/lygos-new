//! Compile macro control flow into the shared typed IR and execute it in ORC.
//! Values cross the host ABI as checked, context-local handles, never AST pointers.
use super::*;
use lygos_ast::type_check::TypeChecker;
use lygos_codegen::CodeGenerator;
use lygos_llvm::{builder::IRBuilder, context::Context, jit::Jit, module::Module};

pub(super) struct Program {
    jit: Jit,
    sites: Vec<Site>,
    entries: HashMap<String, String>,
}

#[derive(Clone)]
enum Operation {
    Literal(Value),
    Quote(AST),
    Unary(String),
    Binary(String),
    Cast(Type),
    Host(String),
    Method(String),
    Array,
    Append,
    Repeat(usize),
    Index,
    Check(Type, String),
    Assign(Option<Type>),
    Condition,
    Input(usize),
    Enter(String, Location),
    Tick,
    MissingReturn(String),
}
struct Site {
    operation: Operation,
    arity: usize,
}

#[derive(Clone)]
struct Local {
    name: String,
    mutable: bool,
    typ: Option<Type>,
}
struct Compiler<'a> {
    executor: &'a Executor,
    sites: Vec<Site>,
    names: HashMap<String, String>,
    scopes: Vec<HashMap<String, Local>>,
    serial: usize,
    loops: usize,
    ret_type: Option<Type>,
}

impl Program {
    pub(super) fn compile(executor: &Executor, expression: Option<&AST>) -> Result<Self> {
        Context::init_all();
        let mut names = executor.functions.keys().cloned().collect::<Vec<_>>();
        names.sort();
        let mut compiler = Compiler {
            executor,
            sites: Vec::new(),
            names: names
                .iter()
                .enumerate()
                .map(|(i, name)| (name.clone(), format!("__ct_fn_{i}")))
                .collect(),
            scopes: Vec::new(),
            serial: 0,
            loops: 0,
            ret_type: None,
        };
        let mut source = String::from(
            "fn __ct_host(site:i32, context:u64, a:u64, b:u64, c:u64)->u64;\n\
             fn __ct_failed(context:u64)->bool;\nfn __ct_leave(context:u64);\n",
        );
        for name in &names {
            let function = &executor.functions[name];
            source.push_str(&compiler.function(function).map_err(|mut error| {
                error.related.push((
                    function.id.loc().clone(),
                    format!("while compiling compile-time function `{name}`"),
                ));
                error
            })?);
        }
        let mut entries = HashMap::new();
        for (index, name) in names.iter().enumerate() {
            let entry = format!("__ct_entry_{index}");
            entries.insert(name.clone(), entry.clone());
            source.push_str(&format!("fn {entry}(context:u64)->u64 {{\n"));
            let mut args = Vec::new();
            for i in 0..executor.functions[name].args.len() {
                // Wrapper guards do not leave a frame: they have not entered one.
                let site = compiler.site(Operation::Input(i), 0);
                let temp = compiler.fresh();
                source.push_str(&format!(
                    "let {temp}=__ct_host({site},context,(:u64)0,(:u64)0,(:u64)0);\n"
                ));
                args.push(temp);
            }
            source.push_str(&format!(
                "return {}(context{});\n}}\n",
                compiler.names[name],
                args.iter().map(|arg| format!(",{arg}")).collect::<String>()
            ));
        }
        if let Some(expression) = expression {
            compiler.scopes = vec![HashMap::new()];
            compiler.ret_type = None;
            let mut body = String::new();
            let value = compiler.expr(expression, &mut body)?;
            // The expression entry also has a frame so error guards can leave it.
            let enter = compiler.site(
                Operation::Enter("constant argument".into(), executor.callsite.clone()),
                0,
            );
            source.push_str(&format!(
                "fn __ct_expression(context:u64)->u64 {{\n\
                __ct_host({enter},context,(:u64)0,(:u64)0,(:u64)0);\n\
                if __ct_failed(context) {{ return (:u64)0; }}\n\
                {body}__ct_leave(context); return {value};\n}}\n"
            ));
            entries.insert(String::new(), "__ct_expression".into());
        }
        let raw = Parser::from(Lexer::from(source.chars().collect()).get_tokens()).build_raw_ast();
        let typed = TypeChecker::analyze(&raw).map_err(|errors| {
            TypeError::new(
                executor.callsite.clone(),
                format!(
                    "could not lower compile-time program: {}",
                    errors
                        .iter()
                        .map(|e| e.message.as_str())
                        .collect::<Vec<_>>()
                        .join("; ")
                ),
            )
        })?;
        let context = Context::new();
        let mut jit = Jit::try_new().map_err(|e| TypeError::new(executor.callsite.clone(), e))?;
        let mut module = Module::new("comptime", &context);
        jit.configure_module(&mut module);
        let builder = IRBuilder::new(&context);
        CodeGenerator::new(&context, &module, &builder)
            .generate(&typed)
            .map_err(|e| TypeError::new(executor.callsite.clone(), e))?;
        unsafe {
            for (name, address) in [
                ("__ct_host", host as *const () as *mut ()),
                ("__ct_failed", failed as *const () as *mut ()),
                ("__ct_leave", leave as *const () as *mut ()),
            ] {
                jit.define_symbol(name, address)
                    .map_err(|e| TypeError::new(executor.callsite.clone(), e))?;
            }
        }
        jit.add_module(module)
            .map_err(|e| TypeError::new(executor.callsite.clone(), e))?;
        Ok(Self {
            jit,
            sites: compiler.sites,
            entries,
        })
    }

    pub(super) fn execute(
        &self,
        executor: &mut Executor,
        name: &str,
        inputs: Vec<Value>,
    ) -> Result<Value> {
        let entry = self.entries.get(name).ok_or_else(|| {
            TypeError::new(
                executor.callsite.clone(),
                "missing compile-time entry point",
            )
        })?;
        let pointer = self.jit.lookup_address(entry).ok_or_else(|| {
            TypeError::new(
                executor.callsite.clone(),
                format!(
                    "LLVM could not resolve compile-time entry point: {}",
                    self.jit.last_error()
                ),
            )
        })?;
        let mut frame = Frame {
            executor,
            sites: &self.sites,
            inputs,
            values: vec![Value::Void],
            error: None,
            stack: Vec::new(),
            stored_units: 0,
        };
        let entry: unsafe extern "C" fn(u64) -> u64 = unsafe { std::mem::transmute(pointer) };
        let handle = unsafe { entry(&mut frame as *mut Frame<'_> as u64) };
        if let Some(error) = frame.error {
            return Err(error);
        }
        frame.values.get(handle as usize).cloned().ok_or_else(|| {
            TypeError::new(
                frame.executor.callsite.clone(),
                "invalid compile-time result handle",
            )
        })
    }
}

impl Compiler<'_> {
    fn fresh(&mut self) -> String {
        self.serial += 1;
        format!("__ct_v_{}", self.serial)
    }
    fn site(&mut self, operation: Operation, arity: usize) -> usize {
        let id = self.sites.len();
        self.sites.push(Site { operation, arity });
        id
    }
    fn operation(&mut self, operation: Operation, args: &[String], out: &mut String) -> String {
        assert!(args.len() <= 3);
        let site = self.site(operation, args.len());
        let result = self.fresh();
        let mut values = args.to_vec();
        values.resize(3, "(:u64)0".into());
        out.push_str(&format!(
            "let {result}=__ct_host({site},context,{},{},{});\n",
            values[0], values[1], values[2]
        ));
        self.guard(out);
        result
    }
    fn guard(&self, out: &mut String) {
        out.push_str("if __ct_failed(context) { __ct_leave(context); return (:u64)0; }\n");
    }
    fn local(&self, name: &str) -> Result<Local> {
        self.scopes
            .iter()
            .rev()
            .find_map(|s| s.get(name).cloned())
            .ok_or_else(|| {
                TypeError::new(
                    self.executor.callsite.clone(),
                    format!("unknown compile-time variable `{name}`"),
                )
            })
    }
    fn function(&mut self, function: &Function) -> Result<String> {
        self.scopes = vec![HashMap::new()];
        self.ret_type = Some(function.ret_type.clone());
        let mut params = String::new();
        for param in &function.args {
            let local = Local {
                name: self.fresh(),
                mutable: false,
                typ: Some(param.typ.clone()),
            };
            params.push_str(&format!(",{}:u64", local.name));
            if self.scopes[0]
                .insert(param.id.inner().clone(), local)
                .is_some()
            {
                return self.executor.fail("duplicate compile-time parameter");
            }
        }
        let enter = self.site(
            Operation::Enter(function.id.inner().clone(), function.id.loc().clone()),
            0,
        );
        let mut out = format!(
            "fn {}(context:u64{params})->u64 {{\n\
            __ct_host({enter},context,(:u64)0,(:u64)0,(:u64)0);\n\
            if __ct_failed(context) {{ return (:u64)0; }}\n",
            self.names[function.id.inner()]
        );
        for param in &function.args {
            let local = self.local(param.id.inner())?;
            self.operation(
                Operation::Check(
                    param.typ.clone(),
                    format!(
                        "compile-time argument `{}` requires `{}`",
                        param.id.inner(),
                        param.typ.get_full_name()
                    ),
                ),
                &[local.name],
                &mut out,
            );
        }
        let terminates = self.block(&function.body, &mut out)?;
        if !terminates {
            let value = if matches!(&function.ret_type, Type::Path(path) if path.path == "void") {
                self.operation(Operation::Literal(Value::Void), &[], &mut out)
            } else {
                self.operation(
                    Operation::MissingReturn(function.id.inner().clone()),
                    &[],
                    &mut out,
                )
            };
            out.push_str(&format!("__ct_leave(context); return {value};\n"));
        }
        out.push_str("}\n");
        Ok(out)
    }
    fn block(&mut self, block: &Block, out: &mut String) -> Result<bool> {
        self.scopes.push(HashMap::new());
        let mut terminates = false;
        for node in &block.body {
            if terminates {
                break;
            }
            terminates = self.statement(node, out)?;
        }
        self.scopes.pop();
        Ok(terminates)
    }
    fn statement(&mut self, node: &AST, out: &mut String) -> Result<bool> {
        match node {
            AST::VarDecl(item) => {
                let Some(expr) = &item.value else {
                    return self
                        .executor
                        .fail("compile-time variables require an initializer");
                };
                let mut value = self.expr(expr, out)?;
                if let Some(typ) = &item.typ {
                    value = self.operation(
                        Operation::Check(
                            typ.clone(),
                            "compile-time variable initializer does not match its type".into(),
                        ),
                        &[value],
                        out,
                    );
                }
                let name = self.fresh();
                out.push_str(&format!(
                    "let {}{name}:u64={value};\n",
                    if item.cnst { "" } else { "mut " }
                ));
                if self
                    .scopes
                    .last_mut()
                    .unwrap()
                    .insert(
                        item.id.inner().clone(),
                        Local {
                            name,
                            mutable: !item.cnst,
                            typ: item.typ.clone(),
                        },
                    )
                    .is_some()
                {
                    return self
                        .executor
                        .fail("duplicate compile-time binding in this scope");
                }
            }
            AST::ReturnExpr(item) => {
                let mut value = if let Some(expr) = &item.value {
                    self.expr(expr, out)?
                } else {
                    self.operation(Operation::Literal(Value::Void), &[], out)
                };
                if let Some(typ) = &self.ret_type {
                    value = self.operation(
                        Operation::Check(
                            typ.clone(),
                            format!(
                                "compile-time function must return `{}`",
                                typ.get_full_name()
                            ),
                        ),
                        &[value],
                        out,
                    );
                }
                out.push_str(&format!("__ct_leave(context); return {value};\n"));
                return Ok(true);
            }
            AST::BreakExpr(_) => {
                if self.loops == 0 {
                    return self.executor.fail("break outside compile-time loop");
                }
                out.push_str("break;\n");
                return Ok(true);
            }
            AST::IfStmt(item) => {
                let condition = self.expr(&item.cond, out)?;
                let condition = self.operation(Operation::Condition, &[condition], out);
                out.push_str(&format!("if (:bool){condition} {{\n"));
                let then_returns = self.block(&item.then_body, out)?;
                out.push_str("}\n");
                let mut else_returns = false;
                if let Some(block) = &item.else_body {
                    out.push_str("else {\n");
                    else_returns = self.block(block, out)?;
                    out.push_str("}\n");
                }
                return Ok(then_returns && else_returns);
            }
            AST::ForStmt(item) => {
                // The original loop condition can contain arbitrary expressions
                // with side effects; put its lowering at the top of each iteration.
                out.push_str("if (:bool)1 {\n");
                self.scopes.push(HashMap::new());
                self.statement(&item.var, out)?;
                let dummy = self.fresh();
                out.push_str(&format!("for let {dummy}=0 in (:bool)1 {{\n"));
                self.operation(Operation::Tick, &[], out);
                let condition = self.expr(&item.cond, out)?;
                let condition = self.operation(Operation::Condition, &[condition], out);
                out.push_str(&format!("if !(:bool){condition} {{ break; }}\n"));
                self.loops += 1;
                self.block(&item.body, out)?;
                self.loops -= 1;
                out.push_str("}\n}\n");
                self.scopes.pop();
            }
            _ => {
                self.expr(node, out)?;
            }
        }
        Ok(false)
    }
    fn expr(&mut self, node: &AST, out: &mut String) -> Result<String> {
        let (operation, args) = match node {
            AST::Identifier(item) => {
                // Capture the value now: evaluating a later operand may assign
                // to this binding before the enclosing operation executes.
                let local = self.local(item.id.inner())?;
                let value = self.fresh();
                out.push_str(&format!("let {value}:u64={};\n", local.name));
                return Ok(value);
            }
            AST::NumberLiteral(item) if matches!(item.typ, NumberType::Int) => {
                let value = item.value.inner().parse::<i64>().map_err(|_| {
                    TypeError::new(
                        self.executor.callsite.clone(),
                        "compile-time integer exceeds i64 range",
                    )
                })?;
                (Operation::Literal(Value::Int(value)), vec![])
            }
            AST::StringLiteral(item) => (
                Operation::Literal(Value::String(
                    unescape_literal(item.value.inner())
                        .map_err(|e| TypeError::new(self.executor.callsite.clone(), e))?,
                )),
                vec![],
            ),
            AST::UnaryExpr(item) => (
                Operation::Unary(item.op.inner().clone()),
                vec![self.expr(&item.obj, out)?],
            ),
            AST::CastExpr(item) => (
                Operation::Cast(item.target_type.clone()),
                vec![self.expr(&item.obj, out)?],
            ),
            AST::BinaryExpr(item) => {
                let left = self.expr(&item.lhs, out)?;
                let op = item.op.inner().as_str();
                if matches!(op, "&&" | "||") {
                    let cond = self.operation(Operation::Condition, &[left.clone()], out);
                    let result = self.fresh();
                    out.push_str(&format!(
                        "let mut {result}:u64={left};\nif {}(:bool){cond} {{\n",
                        if op == "||" { "!" } else { "" }
                    ));
                    let right = self.expr(&item.rhs, out)?;
                    self.operation(Operation::Condition, &[right.clone()], out);
                    out.push_str(&format!("{result}={right};\n}}\n"));
                    return Ok(result);
                }
                (
                    Operation::Binary(op.into()),
                    vec![left, self.expr(&item.rhs, out)?],
                )
            }
            AST::AssignmentExpr(item) => {
                let AST::Identifier(id) = &*item.lhs else {
                    return self
                        .executor
                        .fail("compile-time assignment currently requires a variable");
                };
                let local = self.local(id.id.inner())?;
                if !local.mutable {
                    return self
                        .executor
                        .fail("cannot assign to immutable compile-time binding");
                }
                let value = self.expr(&item.rhs, out)?;
                let checked = self.operation(
                    Operation::Assign(local.typ),
                    &[local.name.clone(), value],
                    out,
                );
                out.push_str(&format!("{}={checked};\n", local.name));
                return Ok(checked);
            }
            AST::ArrayLiteral(item) => {
                if let Some(length) = item.repeat {
                    if length > 10000 {
                        return self
                            .executor
                            .fail("compile-time array exceeds 10000 elements");
                    }
                    (
                        Operation::Repeat(length),
                        vec![self.expr(&item.values[0], out)?],
                    )
                } else {
                    let mut array = self.operation(Operation::Array, &[], out);
                    for value in &item.values {
                        let value = self.expr(value, out)?;
                        array = self.operation(Operation::Append, &[array, value], out);
                    }
                    return Ok(array);
                }
            }
            AST::AccessExpr(item) => (
                Operation::Index,
                vec![self.expr(&item.obj, out)?, self.expr(&item.index, out)?],
            ),
            AST::MemberCallExpr(item) => {
                let receiver = self.expr(&item.obj, out)?;
                let AST::CallExpr(call) = &*item.r#fn else {
                    return self
                        .executor
                        .fail("expected a compile-time node method call");
                };
                let AST::Identifier(name) = &*call.caller else {
                    return self
                        .executor
                        .fail("expected a compile-time node method name");
                };
                let mut args = vec![receiver];
                for arg in &call.args {
                    args.push(self.expr(arg, out)?);
                }
                if args.len() > 3 {
                    return self.executor.fail("too many compile-time method arguments");
                }
                (Operation::Method(name.id.inner().clone()), args)
            }
            AST::ResolutionExpr(item) => {
                let AST::Identifier(owner) = &*item.obj else {
                    return self.executor.fail("invalid compile-time namespace");
                };
                if owner.id.inner() != "ast" {
                    return self
                        .executor
                        .fail("only the ast namespace is available at compile time");
                }
                let AST::CallExpr(call) = &*item.member else {
                    return self
                        .executor
                        .fail("compile-time namespace access requires a call");
                };
                let AST::Identifier(name) = &*call.caller else {
                    return self.executor.fail("invalid compile-time callee");
                };
                let mut args = Vec::new();
                for arg in &call.args {
                    args.push(self.expr(arg, out)?);
                }
                if args.len() > 3 {
                    return self
                        .executor
                        .fail("too many compile-time AST operation arguments");
                }
                (Operation::Host(name.id.inner().clone()), args)
            }
            AST::CallExpr(call) => {
                let name = match &*call.caller {
                    AST::Identifier(id) => id.id.inner().clone(),
                    AST::ResolutionExpr(path) => {
                        let (AST::Identifier(owner), AST::Identifier(member)) =
                            (&*path.obj, &*path.member)
                        else {
                            return self.executor.fail("invalid compile-time callee");
                        };
                        format!("{}::{}", owner.id.inner(), member.id.inner())
                    }
                    _ => {
                        return self
                            .executor
                            .fail("compile-time calls require a named function");
                    }
                };
                let mut args = Vec::new();
                for arg in &call.args {
                    args.push(self.expr(arg, out)?);
                }
                return self.call(&name, args, out);
            }
            AST::MacroCall(call) => {
                let Some(function) = self.executor.functions.get(call.id.inner()) else {
                    return self.executor.fail("unknown nested compile-time macro");
                };
                if function.args.len() != call.args.len() {
                    return self.executor.fail("nested macro argument count mismatch");
                }
                let mut args = Vec::new();
                for (param, tokens) in function.args.iter().zip(&call.args) {
                    let expr = self.executor.parse_tokens(tokens.clone())?;
                    if matches!(&param.typ, Type::Path(path) if matches!(path.path.as_str(), "Ast" | "AstExpr" | "AstDecl"))
                    {
                        args.push(self.operation(Operation::Quote(expr), &[], out));
                    } else {
                        args.push(self.expr(&expr, out)?);
                    }
                }
                return self.call(call.id.inner(), args, out);
            }
            _ => {
                return self.executor.fail(
                    "this language construct is not supported during compile-time execution",
                );
            }
        };
        Ok(self.operation(operation, &args, out))
    }
    fn call(&mut self, name: &str, args: Vec<String>, out: &mut String) -> Result<String> {
        if let Some(name) = name.strip_prefix("ast::") {
            if args.len() > 3 {
                return self
                    .executor
                    .fail("too many compile-time AST operation arguments");
            }
            return Ok(self.operation(Operation::Host(name.into()), &args, out));
        }
        let Some(function) = self.executor.functions.get(name) else {
            return self
                .executor
                .fail(format!("unknown compile-time function `{name}`"));
        };
        if function.args.len() != args.len() {
            return self.executor.fail(format!(
                "compile-time function `{name}` expects {} arguments",
                function.args.len()
            ));
        }
        let result = self.fresh();
        out.push_str(&format!(
            "let {result}={}(context{});\n",
            self.names[name],
            args.iter().map(|arg| format!(",{arg}")).collect::<String>()
        ));
        self.guard(out);
        Ok(result)
    }
}

// Frame exists only while the native entry point runs. Callbacks receive its
// explicit address; parallel compiler invocations never share execution state.
struct Frame<'a> {
    executor: &'a mut Executor,
    sites: &'a [Site],
    inputs: Vec<Value>,
    values: Vec<Value>,
    error: Option<TypeError>,
    stack: Vec<(Location, String)>,
    stored_units: usize,
}
unsafe extern "C" fn failed(context: u64) -> bool {
    unsafe { (&*(context as *const Frame<'_>)).error.is_some() }
}
unsafe extern "C" fn leave(context: u64) {
    unsafe {
        (&mut *(context as *mut Frame<'_>)).stack.pop();
    }
}
unsafe extern "C" fn host(site: i32, context: u64, a: u64, b: u64, c: u64) -> u64 {
    let frame = unsafe { &mut *(context as *mut Frame<'_>) };
    if frame.error.is_some() {
        return 0;
    }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        frame.operation(site, [a, b, c])
    }));
    let result = result.unwrap_or_else(|_| {
        Err(TypeError::new(
            frame.executor.callsite.clone(),
            "compile-time host operation failed",
        ))
    });
    match result {
        Ok(handle) => handle,
        Err(mut error) => {
            error
                .related
                .extend(frame.stack.iter().rev().map(|(loc, name)| {
                    (
                        loc.clone(),
                        format!("while executing compile-time function `{name}`"),
                    )
                }));
            frame.error = Some(error);
            0
        }
    }
}
impl Frame<'_> {
    fn operation(&mut self, site: i32, handles: [u64; 3]) -> Result<u64> {
        self.executor.tick()?;
        let site = self.sites.get(site as usize).ok_or_else(|| {
            TypeError::new(
                self.executor.callsite.clone(),
                "invalid compile-time host call site",
            )
        })?;
        if matches!(site.operation, Operation::Append) {
            // Array literal construction owns its temporary exclusively. Extend
            // it in place rather than retaining every intermediate prefix.
            let value = self
                .values
                .get(handles[1] as usize)
                .cloned()
                .ok_or_else(|| {
                    TypeError::new(
                        self.executor.callsite.clone(),
                        "invalid compile-time value handle",
                    )
                })?;
            let cost = value_units(&value);
            if cost > 8_000_000usize.saturating_sub(self.stored_units) {
                return self
                    .executor
                    .fail("compile-time value storage limit exceeded (8 MB units)");
            }
            let Some(Value::Array(values)) = self.values.get_mut(handles[0] as usize) else {
                return self.executor.fail("expected compile-time array");
            };
            if values.len() >= 10000 {
                return self
                    .executor
                    .fail("compile-time array exceeds 10000 elements");
            }
            values.push(value);
            self.stored_units += cost;
            return Ok(handles[0]);
        }
        let mut args = Vec::new();
        for handle in handles.iter().take(site.arity) {
            args.push(self.values.get(*handle as usize).cloned().ok_or_else(|| {
                TypeError::new(
                    self.executor.callsite.clone(),
                    "invalid compile-time value handle",
                )
            })?);
        }
        let e = &mut self.executor;
        let value = match &site.operation {
            Operation::Literal(value) => value.clone(),
            Operation::Quote(node) => e.syntax(node.clone())?,
            Operation::Input(index) => {
                self.inputs.get(*index).cloned().ok_or_else(|| {
                    TypeError::new(e.callsite.clone(), "missing compile-time input")
                })?
            }
            Operation::Enter(name, loc) => {
                if self.stack.len() >= 16 {
                    return e.fail("compile-time call depth exceeded (16)");
                }
                self.stack.push((loc.clone(), name.clone()));
                Value::Void
            }
            Operation::Tick => Value::Void,
            Operation::MissingReturn(name) => {
                return e.fail(format!(
                    "compile-time function `{name}` did not return a value"
                ));
            }
            Operation::Condition => return Ok(u64::from(e.boolean(&args[0])?)),
            Operation::Check(typ, message) => {
                if !e.compatible(typ, &args[0]) {
                    return e.fail(message.clone());
                }
                args.remove(0)
            }
            Operation::Assign(typ) => {
                if std::mem::discriminant(&args[0]) != std::mem::discriminant(&args[1]) {
                    return e.fail("compile-time assignment changes the variable's type");
                }
                if typ.as_ref().is_some_and(|typ| !e.compatible(typ, &args[1])) {
                    return e.fail("compile-time assignment does not match the declared type");
                }
                args.remove(1)
            }
            Operation::Host(name) => e.host(name, args)?,
            Operation::Method(name) => {
                let receiver = args.remove(0);
                if matches!(receiver, Value::Field(_) | Value::Type(_) | Value::Array(_)) {
                    e.meta_method(receiver, name, args)?
                } else {
                    e.node(&receiver)?;
                    let arity = match name.as_str() {
                        "name" | "kind" | "field_count" | "clone" | "emit" | "as_expr"
                        | "as_decl" | "fields" => 0,
                        "rename" | "replace" | "field_name" | "field_type" | "field" => 1,
                        "add_field" | "binary" => 2,
                        _ => return e.fail(format!("unknown Ast method `{name}`")),
                    };
                    if args.len() != arity {
                        return e.fail(format!(
                            "Ast::{name} expects {arity} arguments, found {}",
                            args.len()
                        ));
                    }
                    args.insert(0, receiver);
                    if name == "binary" {
                        args.swap(0, 1);
                    }
                    e.host(name, args)?
                }
            }
            Operation::Array => Value::Array(Vec::new()),
            Operation::Append => unreachable!("handled before cloning host arguments"),
            Operation::Repeat(length) => Value::Array(vec![args[0].clone(); *length]),
            Operation::Index => {
                let index = e.integer(&args[1])?;
                let Value::Array(values) = &args[0] else {
                    return e.fail("compile-time indexing requires an array");
                };
                values.get(index as usize).cloned().ok_or_else(|| {
                    TypeError::new(e.callsite.clone(), "compile-time array index out of bounds")
                })?
            }
            Operation::Cast(typ) => {
                if matches!(typ, Type::Path(path) if path.path == "bool") {
                    Value::Bool(e.integer(&args[0])? != 0)
                } else if e.compatible(typ, &args[0]) {
                    args.remove(0)
                } else {
                    return e.fail("unsupported compile-time cast");
                }
            }
            Operation::Unary(op) => match op.as_str() {
                "-" => Value::Int(e.integer(&args[0])?.checked_neg().ok_or_else(|| {
                    TypeError::new(e.callsite.clone(), "compile-time integer overflow")
                })?),
                "+" => Value::Int(e.integer(&args[0])?),
                "!" => Value::Bool(!e.boolean(&args[0])?),
                _ => {
                    return e
                        .fail("pointer operations are unavailable during compile-time execution");
                }
            },
            Operation::Binary(op) => binary(e, op, &args[0], &args[1])?,
        };
        // Native temporaries retain host values until the invocation returns.
        // Bound that storage separately from the instruction and syntax limits.
        let cost = value_units(&value);
        if cost > 8_000_000usize.saturating_sub(self.stored_units) {
            return e.fail("compile-time value storage limit exceeded (8 MB units)");
        }
        self.stored_units += cost;
        let handle = self.values.len() as u64;
        self.values.push(value);
        Ok(handle)
    }
}
fn value_units(value: &Value) -> usize {
    match value {
        Value::String(text) => 32usize.saturating_add(text.len()),
        Value::Array(values) => values.iter().fold(32usize, |cost, value| {
            cost.saturating_add(value_units(value))
        }),
        _ => 32,
    }
}
fn binary(e: &Executor, op: &str, left: &Value, right: &Value) -> Result<Value> {
    if op == "+" {
        if let (Value::String(a), Value::String(b)) = (left, right) {
            if a.len() + b.len() > 1_000_000 {
                return e.fail("compile-time string exceeds 1 MB");
            }
            return Ok(Value::String(a.clone() + b));
        }
    }
    if matches!(op, "==" | "!=") {
        if std::mem::discriminant(left) != std::mem::discriminant(right) {
            return e.fail("incompatible compile-time comparison operands");
        }
        return Ok(Value::Bool(if op == "==" {
            left == right
        } else {
            left != right
        }));
    }
    let a = e.integer(left)?;
    let b = e.integer(right)?;
    Ok(match op {
        "<" => Value::Bool(a < b),
        ">" => Value::Bool(a > b),
        "<=" => Value::Bool(a <= b),
        ">=" => Value::Bool(a >= b),
        "+" | "-" | "*" | "/" | "%" => Value::Int(
            match op {
                "+" => a.checked_add(b),
                "-" => a.checked_sub(b),
                "*" => a.checked_mul(b),
                "/" => a.checked_div(b),
                _ => a.checked_rem(b),
            }
            .ok_or_else(|| {
                TypeError::new(
                    e.callsite.clone(),
                    "compile-time arithmetic overflow or division by zero",
                )
            })?,
        ),
        _ => return e.fail("unsupported compile-time operator"),
    })
}
