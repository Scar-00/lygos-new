//! Semantic validation before LLVM lowering. This pass never allocates LLVM
//! values, so invalid programs can be diagnosed without entering the backend.
use std::collections::{HashMap, HashSet};

use lygos_token::Location;

use crate::{types::*, *};
mod lowering;

#[derive(Debug)]
pub struct TypeError {
    pub loc: Location,
    pub message: String,
    pub primary_label: Option<String>,
    pub related: Vec<(Location, String)>,
    pub help: Option<String>,
}

impl TypeError {
    pub fn new(loc: Location, message: impl Into<String>) -> Self {
        Self {
            loc,
            message: message.into(),
            primary_label: None,
            related: Vec::new(),
            help: None,
        }
    }
    fn label(mut self, message: impl Into<String>) -> Self {
        self.primary_label = Some(message.into());
        self
    }
    fn related(mut self, loc: Location, message: impl Into<String>) -> Self {
        self.related.push((loc, message.into()));
        self
    }
    fn help(mut self, message: impl Into<String>) -> Self {
        self.help = Some(message.into());
        self
    }
}

impl std::fmt::Display for TypeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{}: {}",
            self.loc.file.display(),
            self.loc.range.start,
            self.message
        )
    }
}

impl std::error::Error for TypeError {}

type Result<T> = std::result::Result<T, TypeError>;

fn error<T>(loc: Location, message: impl Into<String>) -> Result<T> {
    Err(TypeError::new(loc, message))
}

#[derive(Clone)]
struct Binding {
    typ: Type,
    mutable: bool,
    initialized: bool,
    loc: Location,
}

#[derive(Clone)]
struct Signature {
    params: Vec<Type>,
    ret: Type,
    receiver: bool,
    defined: bool,
    loc: Location,
}

impl Signature {
    fn from_function(function: &Function) -> Self {
        Self {
            params: function.args.iter().map(|arg| arg.typ.clone()).collect(),
            ret: function.ret_type.clone(),
            receiver: function
                .args
                .first()
                .is_some_and(|arg| arg.id.inner() == "self"),
            defined: function.is_def,
            loc: function.id.loc().clone(),
        }
    }
}

#[derive(Default)]
pub struct TypeChecker {
    aliases: HashMap<String, Type>,
    structs: HashMap<String, Vec<StructField>>,
    enums: HashMap<String, (Type, Vec<String>)>,
    functions: HashMap<String, Signature>,
    methods: HashMap<(String, String), Signature>,
    traits: HashMap<String, HashMap<String, Signature>>,
    scopes: Vec<HashMap<String, Binding>>,
    return_type: Option<Type>,
    loop_depth: usize,
    // Transient identity keys for this immutable source AST only. No source
    // addresses are retained in the resulting ID-based semantic context.
    expression_types: HashMap<usize, Type>,
}

impl TypeChecker {
    pub fn check(ast: &AST) -> Result<()> {
        Self::check_all(ast).into_iter().next().map_or(Ok(()), Err)
    }

    /// Collect errors from independent function bodies. A declaration error stops
    /// body checking because signatures and layouts may then be unreliable.
    pub fn check_all(ast: &AST) -> Vec<TypeError> {
        Self::analyze(ast).err().unwrap_or_default()
    }

    pub fn analyze(ast: &AST) -> std::result::Result<crate::typed::TypedModule, Vec<TypeError>> {
        let checker = Self::validate(ast)?;
        lowering::lower(ast, &checker).map_err(|error| vec![error])
    }

    fn validate(ast: &AST) -> std::result::Result<Self, Vec<TypeError>> {
        let AST::Module(module) = ast else {
            return Err(vec![TypeError::new(
                Location::new("", 0..0),
                "expected a module",
            )]);
        };
        let mut checker = Self::default();
        checker.scopes.push(HashMap::new());
        if let Err(error) = checker
            .collect(&module.body)
            .and_then(|_| checker.validate_declarations(&module.body))
        {
            return Err(vec![error]);
        }
        let mut errors = Vec::new();
        for ast in &module.body.body {
            match ast {
                AST::Function(function) => checker.check_body(function, &mut errors),
                AST::Impl(item) => {
                    for ast in &item.body.body {
                        if let AST::Function(function) = ast {
                            checker.check_body(function, &mut errors);
                        }
                    }
                }
                AST::StaticLiteral(item) => {
                    if let Some(value) = &item.value {
                        let result = checker
                            .expr(value)
                            .and_then(|actual| checker.expect(&item.typ, &actual, location(value)))
                            .and_then(|_| {
                                if constant_initializer(value) {
                                    Ok(())
                                } else {
                                    error(
                                        location(value),
                                        "static initializer must be a constant expression",
                                    )
                                }
                            });
                        if let Err(error) = result {
                            errors.push(error);
                        }
                    }
                }
                _ => {}
            }
        }
        if errors.is_empty() {
            Ok(checker)
        } else {
            Err(errors)
        }
    }

    fn check_body(&mut self, function: &Function, errors: &mut Vec<TypeError>) {
        if let Err(mut error) = self.function(function) {
            error.message = format!("in function `{}`: {}", function.id.inner(), error.message);
            errors.push(error);
        }
        // A failed body may have exited inside a nested scope or a loop. Restore
        // the module context before checking the next independent body.
        self.scopes.truncate(1);
        self.return_type = None;
        self.loop_depth = 0;
    }

    fn collect(&mut self, block: &Block) -> Result<()> {
        let mut names = HashSet::new();
        for ast in &block.body {
            let name = match ast {
                AST::TypeAlias(item) => Some(&item.id),
                AST::StructDef(item) => Some(&item.id),
                AST::EnumDef(item) => Some(&item.id),
                AST::Trait(item) => Some(&item.id),
                AST::StaticLiteral(item) => Some(&item.id),
                AST::Function(item) => Some(&item.id),
                AST::Macro(item) => Some(&item.id),
                AST::Impl(_) => None,
                _ => {
                    return error(
                        location(ast),
                        "only declarations are allowed at module scope",
                    );
                }
            };
            if let Some(name) = name {
                let repeated_function =
                    matches!(ast, AST::Function(_)) && self.functions.contains_key(name.inner());
                if BaseTypes.contains(&name.inner().as_str())
                    || (!names.insert(name.inner().clone()) && !repeated_function)
                {
                    return error(
                        name.loc().clone(),
                        format!("duplicate declaration `{}`", name.inner()),
                    );
                }
            }
            match ast {
                AST::TypeAlias(item) => {
                    self.aliases
                        .insert(item.id.inner().clone(), item.typ.clone());
                }
                AST::StructDef(item) => {
                    self.structs
                        .insert(item.id.inner().clone(), item.fields.clone());
                }
                AST::EnumDef(item) => {
                    self.enums.insert(
                        item.id.inner().clone(),
                        (item.typ.clone(), item.variants.clone()),
                    );
                }
                AST::StaticLiteral(item) => {
                    self.scopes[0].insert(
                        item.id.inner().clone(),
                        Binding {
                            typ: item.typ.clone(),
                            mutable: false,
                            initialized: true,
                            loc: item.id.loc().clone(),
                        },
                    );
                }
                AST::Function(item) => {
                    let signature = Signature::from_function(item);
                    if let Some(previous) = self.functions.get(item.id.inner()) {
                        if previous.defined && signature.defined {
                            return error(
                                item.id.loc().clone(),
                                format!("duplicate function definition `{}`", item.id.inner()),
                            );
                        }
                    }
                    self.functions
                        .entry(item.id.inner().clone())
                        .and_modify(|old| old.defined |= signature.defined)
                        .or_insert(signature);
                }
                AST::Trait(item) => {
                    let mut methods = HashMap::new();
                    for ast in &item.funcs.body {
                        let AST::Function(function) = ast else {
                            return error(location(ast), "expected a trait function");
                        };
                        if methods
                            .insert(
                                function.id.inner().clone(),
                                Signature::from_function(function),
                            )
                            .is_some()
                        {
                            return error(function.id.loc().clone(), "duplicate trait method");
                        }
                    }
                    self.traits.insert(item.id.inner().clone(), methods);
                }
                _ => {}
            }
        }
        // Collect methods after all type declarations, permitting forward references.
        for ast in &block.body {
            if let AST::Impl(item) = ast {
                let owner = self.normalize(
                    &Type::Path(Path::new(item.typ.loc().clone(), item.typ.inner())),
                    &mut Vec::new(),
                )?;
                let Type::Path(owner) = owner else {
                    return error(item.typ.loc().clone(), "impl requires a named type");
                };
                for ast in &item.body.body {
                    let AST::Function(function) = ast else {
                        return error(location(ast), "expected an impl function");
                    };
                    if self
                        .methods
                        .insert(
                            (owner.path.clone(), function.id.inner().clone()),
                            Signature::from_function(function),
                        )
                        .is_some()
                    {
                        return error(function.id.loc().clone(), "duplicate method");
                    }
                }
            }
        }
        Ok(())
    }

    fn normalize(&self, typ: &Type, visiting: &mut Vec<String>) -> Result<Type> {
        let mut typ = typ.clone();
        match &mut typ {
            Type::Path(path) => {
                if let Some(target) = self.aliases.get(&path.path) {
                    if visiting.contains(&path.path) {
                        return error(
                            path.loc.clone(),
                            format!("cyclic type alias `{}`", path.path),
                        );
                    }
                    visiting.push(path.path.clone());
                    let result = self.normalize(target, visiting);
                    visiting.pop();
                    return result;
                }
                if !BaseTypes.contains(&path.path.as_str())
                    && !self.structs.contains_key(&path.path)
                    && !self.enums.contains_key(&path.path)
                {
                    return error(path.loc.clone(), format!("unknown type `{}`", path.path));
                }
            }
            Type::Pointer(pointer) => {
                pointer.typ = Box::new(self.normalize(&pointer.typ, visiting)?);
            }
            Type::Array(array) => {
                array.typ = Box::new(self.normalize(&array.typ, visiting)?);
            }
            Type::Slice(slice) => {
                slice.typ = Box::new(self.normalize(&slice.typ, visiting)?);
            }
            Type::FuncPtr(function) => {
                function.params = function
                    .params
                    .iter()
                    .map(|typ| self.normalize(typ, visiting))
                    .collect::<Result<_>>()?;
                function.ret_type = Box::new(self.normalize(&function.ret_type, visiting)?);
            }
        }
        Ok(typ)
    }

    fn canonical(&self, typ: &Type) -> Result<Type> {
        self.normalize(typ, &mut Vec::new())
    }

    fn expect(&self, expected: &Type, actual: &Type, loc: Location) -> Result<()> {
        let expected = self.canonical(expected)?;
        let actual = self.canonical(actual)?;
        if !expected.matches(&actual) {
            return Err(TypeError::new(
                loc,
                format!(
                    "expected `{}`, found `{}`",
                    expected.get_full_name(),
                    actual.get_full_name()
                ),
            )
            .label(format!(
                "this expression has type `{}`",
                actual.get_full_name()
            ))
            .related(
                expected.get_loc(),
                format!(
                    "type `{}` required by this declaration",
                    expected.get_full_name()
                ),
            ));
        }
        Ok(())
    }

    fn value_type(&self, typ: &Type) -> Result<()> {
        let typ = self.canonical(typ)?;
        match &typ {
            Type::Path(path) if path.path == "void" => {
                error(path.loc.clone(), "void is not a value type")
            }
            Type::Path(path) if path.path == "str" => Err(TypeError::new(
                path.loc.clone(),
                "`str` is reference-only and cannot be held by value",
            )
            .help(
                "use `&str` to hold a string reference; string literals already have type `&str`",
            )),
            Type::Pointer(pointer) if matches!(&*pointer.typ, Type::Path(path) if path.path == "str") => {
                Ok(())
            }
            Type::Pointer(pointer) => self.value_type(&pointer.typ),
            Type::Array(array) => self.value_type(&array.typ),
            Type::Slice(slice) => self.value_type(&slice.typ),
            Type::FuncPtr(function) => {
                for param in &function.params {
                    self.value_type(param)?;
                }
                if !is_void(&function.ret_type) {
                    self.value_type(&function.ret_type)?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn signature(&self, function: &Function) -> Result<()> {
        let mut names = HashSet::new();
        for (index, arg) in function.args.iter().enumerate() {
            if !names.insert(arg.id.inner()) {
                return error(arg.id.loc().clone(), "duplicate parameter");
            }
            if arg.id.inner() == "self" && index != 0 {
                return error(arg.id.loc().clone(), "self must be the first parameter");
            }
            self.value_type(&arg.typ)?;
        }
        let ret = self.canonical(&function.ret_type)?;
        if !is_void(&ret) {
            self.value_type(&ret)?;
        }
        Ok(())
    }

    fn validate_declarations(&self, block: &Block) -> Result<()> {
        let mut trait_impls = HashSet::new();
        for ast in &block.body {
            match ast {
                AST::TypeAlias(item) => {
                    self.canonical(&item.typ)?;
                }
                AST::StructDef(item) => {
                    let mut names = HashSet::new();
                    for field in &item.fields {
                        if !names.insert(field.id.inner()) {
                            return error(field.id.loc().clone(), "duplicate field");
                        }
                        self.value_type(&field.typ)?;
                    }
                    self.layout(
                        &Type::Path(Path::new(item.id.loc().clone(), item.id.inner())),
                        &mut Vec::new(),
                    )?;
                }
                AST::EnumDef(item) => {
                    let typ = self.canonical(&item.typ)?;
                    if !is_integer(&typ) {
                        return error(
                            item.typ.get_loc(),
                            "enum representation must be an integer type",
                        );
                    }
                    let mut names = HashSet::new();
                    for variant in &item.variants {
                        if !names.insert(variant) {
                            return error(item.id.loc().clone(), "duplicate enum variant");
                        }
                    }
                }
                AST::StaticLiteral(item) => self.value_type(&item.typ)?,
                AST::Function(function) => {
                    self.signature(function)?;
                    let previous = &self.functions[function.id.inner()];
                    self.same_signature(
                        previous,
                        &Signature::from_function(function),
                        function.id.loc().clone(),
                    )?;
                }
                AST::Impl(item) => {
                    for ast in &item.body.body {
                        if let AST::Function(function) = ast {
                            self.signature(function)?;
                        }
                    }
                    if let Some(trait_name) = &item.trat {
                        let Some(requirements) = self.traits.get(trait_name.inner()) else {
                            return error(trait_name.loc().clone(), "unknown trait");
                        };
                        let owner = self.canonical(&Type::Path(Path::new(
                            item.typ.loc().clone(),
                            item.typ.inner(),
                        )))?;
                        let Type::Path(owner_path) = &owner else {
                            unreachable!()
                        };
                        if !trait_impls
                            .insert((owner_path.path.clone(), trait_name.inner().clone()))
                        {
                            return error(
                                trait_name.loc().clone(),
                                "duplicate trait implementation",
                            );
                        }
                        for (name, requirement) in requirements {
                            // A method from a different impl cannot satisfy this impl's contract.
                            let function = item
                                .body
                                .body
                                .iter()
                                .find_map(|ast| match ast {
                                    AST::Function(function) if function.id.inner() == name => {
                                        Some(function)
                                    }
                                    _ => None,
                                })
                                .ok_or_else(|| {
                                    TypeError::new(
                                        item.typ.loc().clone(),
                                        format!("missing trait method `{name}`"),
                                    )
                                })?;
                            let expected = Signature {
                                params: requirement
                                    .params
                                    .iter()
                                    .map(|typ| substitute_self(typ, &owner))
                                    .collect(),
                                ret: substitute_self(&requirement.ret, &owner),
                                ..requirement.clone()
                            };
                            self.same_signature(
                                &expected,
                                &Signature::from_function(function),
                                function.id.loc().clone(),
                            )?;
                        }
                    }
                }
                AST::Trait(item) => {
                    // Validate symbolic Self using a known concrete placeholder.
                    let owner = Type::Path(Path::new(item.id.loc().clone(), "i32"));
                    for ast in &item.funcs.body {
                        if let AST::Function(function) = ast {
                            let mut names = HashSet::new();
                            for (index, arg) in function.args.iter().enumerate() {
                                if !names.insert(arg.id.inner()) {
                                    return error(
                                        arg.id.loc().clone(),
                                        "duplicate trait parameter",
                                    );
                                }
                                if arg.id.inner() == "self" && index != 0 {
                                    return error(
                                        arg.id.loc().clone(),
                                        "self must be the first parameter",
                                    );
                                }
                                self.value_type(&substitute_self(&arg.typ, &owner))?;
                            }
                            let ret =
                                self.canonical(&substitute_self(&function.ret_type, &owner))?;
                            if !is_void(&ret) {
                                self.value_type(&ret)?;
                            }
                            if function.is_def {
                                return error(
                                    function.id.loc().clone(),
                                    "default trait bodies are not supported",
                                );
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn same_signature(
        &self,
        expected: &Signature,
        actual: &Signature,
        loc: Location,
    ) -> Result<()> {
        if expected.params.len() != actual.params.len() || expected.receiver != actual.receiver {
            return error(loc, "incompatible function signature");
        }
        for (expected, actual) in expected.params.iter().zip(&actual.params) {
            self.expect(expected, actual, loc.clone())?;
        }
        self.expect(&expected.ret, &actual.ret, loc)
    }

    fn layout(&self, typ: &Type, visiting: &mut Vec<String>) -> Result<()> {
        match self.canonical(typ)? {
            Type::Path(path) => {
                if let Some(fields) = self.structs.get(&path.path) {
                    if visiting.contains(&path.path) {
                        return error(path.loc, format!("infinitely sized type `{}`", path.path));
                    }
                    visiting.push(path.path);
                    for field in fields {
                        self.layout(&field.typ, visiting)?;
                    }
                    visiting.pop();
                }
            }
            Type::Array(array) => self.layout(&array.typ, visiting)?,
            // Pointers, function pointers and slices break by-value layout cycles.
            _ => {}
        }
        Ok(())
    }

    fn function(&mut self, function: &Function) -> Result<()> {
        if !function.is_def {
            return Ok(());
        }
        self.return_type = Some(self.canonical(&function.ret_type)?);
        self.scopes.push(HashMap::new());
        for arg in &function.args {
            self.scopes.last_mut().unwrap().insert(
                arg.id.inner().clone(),
                Binding {
                    typ: arg.typ.clone(),
                    mutable: true,
                    initialized: true,
                    loc: arg.id.loc().clone(),
                },
            );
        }
        let returns = self.block(&function.body, false)?;
        self.scopes.pop();
        if !is_void(self.return_type.as_ref().unwrap()) && !returns {
            return error(function.id.loc().clone(), "not all paths return a value");
        }
        self.return_type = None;
        Ok(())
    }

    fn block(&mut self, block: &Block, nested: bool) -> Result<bool> {
        if nested {
            self.scopes.push(HashMap::new());
        }
        let mut returns = false;
        for ast in &block.body {
            let statement_returns = self.statement(ast)?;
            returns |= statement_returns;
        }
        if nested {
            self.scopes.pop();
        }
        Ok(returns)
    }

    fn statement(&mut self, ast: &AST) -> Result<bool> {
        match ast {
            AST::VarDecl(item) => {
                let inferred = item
                    .value
                    .as_deref()
                    .map(|value| self.expr(value))
                    .transpose()?;
                let typ = item.typ.as_ref().or(inferred.as_ref()).ok_or_else(|| {
                    TypeError::new(
                        item.id.loc().clone(),
                        "cannot infer an uninitialized variable",
                    )
                })?;
                self.value_type(typ)?;
                if let (Some(annotation), Some(actual)) = (&item.typ, &inferred) {
                    // Preserve numeric conversions on annotated declarations. Other
                    // conversions require an explicit cast in the source.
                    if !(is_numeric(&self.canonical(annotation)?)
                        && is_numeric(&self.canonical(actual)?))
                    {
                        self.expect(annotation, actual, item.id.loc().clone())?;
                    }
                }
                if item.cnst && inferred.is_none() {
                    return error(
                        item.id.loc().clone(),
                        "immutable variable needs an initializer",
                    );
                }
                let typ = typ.clone();
                self.expression_types
                    .insert(ast as *const AST as usize, self.canonical(&typ)?);
                self.scopes.last_mut().unwrap().insert(
                    item.id.inner().clone(),
                    Binding {
                        typ,
                        mutable: !item.cnst,
                        initialized: inferred.is_some(),
                        loc: item.id.loc().clone(),
                    },
                );
            }
            AST::ReturnExpr(item) => {
                let expected = self
                    .return_type
                    .clone()
                    .ok_or_else(|| TypeError::new(item.loc.clone(), "return outside a function"))?;
                let actual = if let Some(value) = &item.value {
                    self.expr(value)?
                } else {
                    path(item.loc.clone(), "void")
                };
                self.expect(&expected, &actual, item.loc.clone())?;
                return Ok(true);
            }
            AST::IfStmt(item) => {
                let cond = self.expr(&item.cond)?;
                self.expect(
                    &path(location(&item.cond), "bool"),
                    &cond,
                    location(&item.cond),
                )?;
                let before = self.scopes.clone();
                let then_returns = self.block(&item.then_body, true)?;
                let then_state = self.scopes.clone();
                self.scopes = before.clone();
                let else_returns = if let Some(block) = &item.else_body {
                    self.block(block, true)?
                } else {
                    false
                };
                let else_state = self.scopes.clone();
                self.scopes = before;
                for (index, scope) in self.scopes.iter_mut().enumerate() {
                    for (name, binding) in scope {
                        let then_initialized = then_state[index][name].initialized;
                        let else_initialized = else_state[index][name].initialized;
                        binding.initialized = if then_returns {
                            else_initialized
                        } else if else_returns {
                            then_initialized
                        } else {
                            then_initialized && else_initialized
                        };
                    }
                }
                return Ok(then_returns && else_returns);
            }
            AST::ForStmt(item) => {
                self.scopes.push(HashMap::new());
                self.statement(&item.var)?;
                let cond = self.expr(&item.cond)?;
                self.expect(
                    &path(location(&item.cond), "bool"),
                    &cond,
                    location(&item.cond),
                )?;
                let before_body = self.scopes.clone();
                self.loop_depth += 1;
                self.block(&item.body, true)?;
                self.loop_depth -= 1;
                self.scopes = before_body;
                self.scopes.pop();
            }
            AST::BreakExpr(item) => {
                if self.loop_depth == 0 {
                    return error(item.loc.clone(), "break outside a loop");
                }
            }
            _ => {
                self.expr(ast)?;
            }
        }
        Ok(false)
    }

    fn binding(&self, name: &str) -> Option<&Binding> {
        self.scopes.iter().rev().find_map(|scope| scope.get(name))
    }

    fn member_owner(&self, typ: Type, deref: bool, loc: Location) -> Result<String> {
        let typ = self.canonical(&typ)?;
        let base = match typ {
            Type::Pointer(pointer) if deref || pointer.is_ref => *pointer.typ,
            _ if deref => return error(loc, "arrow access requires a pointer"),
            typ => typ,
        };
        let Type::Path(owner) = base else {
            return error(loc, "member access requires a named value type");
        };
        Ok(owner.path)
    }

    fn expr(&mut self, ast: &AST) -> Result<Type> {
        let typ = self.expr_inner(ast)?;
        let typ = self.canonical(&typ)?;
        if matches!(&typ, Type::Path(path) if path.path == "str") {
            return Err(TypeError::new(
                location(ast),
                "`str` is reference-only and cannot be held by value",
            )
            .help("keep the `&str` reference instead of dereferencing it"));
        }
        self.expression_types
            .insert(ast as *const AST as usize, typ.clone());
        Ok(typ)
    }

    fn expr_inner(&mut self, ast: &AST) -> Result<Type> {
        let loc = location(ast);
        match ast {
            AST::Identifier(item) => {
                if let Some(binding) = self.binding(item.id.inner()) {
                    if !binding.initialized {
                        return Err(TypeError::new(
                            loc,
                            format!("variable `{}` may be uninitialized", item.id.inner()),
                        )
                        .label("value is read here")
                        .related(
                            binding.loc.clone(),
                            "declared without a definite initializer",
                        )
                        .help("assign a value on every path before reading this variable"));
                    }
                    return self.canonical(&binding.typ);
                }
                if let Some(function) = self.functions.get(item.id.inner()) {
                    return Ok(Type::FuncPtr(FuncPtr::new(
                        function.loc.clone(),
                        function.params.clone(),
                        function.ret.clone(),
                    )));
                }
                error(loc, format!("unknown value `{}`", item.id.inner()))
            }
            AST::NumberLiteral(item) => {
                let name = match item.typ {
                    NumberType::Int => "i32",
                    NumberType::Char => "i8",
                    NumberType::Float => "f32",
                    NumberType::Double => "f64",
                };
                if matches!(item.typ, NumberType::Int) && item.value.inner().parse::<i32>().is_err()
                {
                    return error(loc, "integer literal does not fit in i32");
                }
                if matches!(item.typ, NumberType::Char) {
                    let value = unescape_literal(item.value.inner())
                        .map_err(|message| TypeError::new(loc.clone(), message))?;
                    if value.len() != 1 || !value.is_ascii() {
                        return error(loc, "character literal must contain one ASCII character");
                    }
                }
                Ok(path(loc, name))
            }
            AST::StringLiteral(item) => {
                unescape_literal(item.value.inner())
                    .map_err(|message| TypeError::new(loc.clone(), message))?;
                Ok(Type::Pointer(Pointer::new(
                    loc.clone(),
                    Box::new(path(loc, "str")),
                    true,
                    false,
                )))
            }
            AST::UnaryExpr(item) => {
                if item.op.inner() == "-"
                    && matches!(&*item.obj, AST::NumberLiteral(number) if matches!(number.typ, NumberType::Int) && number.value.inner() == "2147483648")
                {
                    return Ok(path(loc, "i32"));
                }
                let typ = if matches!(item.op.inner().as_str(), "&" | "&mut") {
                    // Borrow storage directly; this does not load an unsized str
                    // value when indexing an array of string descriptors.
                    let typ = self.expr_inner(&item.obj)?;
                    let typ = self.canonical(&typ)?;
                    self.expression_types
                        .insert(&*item.obj as *const AST as usize, typ.clone());
                    typ
                } else {
                    self.expr(&item.obj)?
                };
                match item.op.inner().as_str() {
                    "*" => match typ {
                        Type::Pointer(pointer) => Ok(*pointer.typ),
                        _ => error(loc, "dereference requires a pointer"),
                    },
                    "&" | "&mut" => {
                        if !is_place(&item.obj) {
                            return error(
                                loc,
                                "taking an address requires a variable, field, index or dereference",
                            );
                        }
                        let mutable = item.op.inner() == "&mut";
                        if mutable {
                            self.require_mutable(&item.obj)?;
                        }
                        Ok(Type::Pointer(Pointer::new(
                            loc,
                            Box::new(typ),
                            true,
                            mutable,
                        )))
                    }
                    "!" => {
                        self.expect(&path(loc.clone(), "bool"), &typ, loc.clone())?;
                        Ok(path(loc, "bool"))
                    }
                    "+" | "-" if is_numeric(&typ) => Ok(typ),
                    _ => error(loc, "invalid unary operand"),
                }
            }
            AST::BinaryExpr(item) => {
                let lhs = self.expr(&item.lhs)?;
                let before_rhs = self.scopes.clone();
                let rhs = self.expr(&item.rhs)?;
                if matches!(item.op.inner().as_str(), "&&" | "||") {
                    // The RHS can be skipped; assignments there are not definite.
                    self.scopes = before_rhs;
                }
                self.expect(&lhs, &rhs, loc.clone())?;
                match item.op.inner().as_str() {
                    "&&" | "||" => {
                        self.expect(&path(loc.clone(), "bool"), &lhs, loc.clone())?;
                        Ok(path(loc, "bool"))
                    }
                    "==" | "!="
                        if is_numeric(&lhs)
                            || is_bool(&lhs)
                            || matches!(lhs, Type::Pointer(_))
                            || self.is_enum_type(&lhs) =>
                    {
                        Ok(path(loc, "bool"))
                    }
                    "<" | ">" | "<=" | ">=" if is_numeric(&lhs) => Ok(path(loc, "bool")),
                    "+" | "-" | "*" | "/" if is_numeric(&lhs) => Ok(lhs),
                    "%" if is_integer(&lhs) => Ok(lhs),
                    _ => error(loc, "invalid binary operands"),
                }
            }
            AST::AssignmentExpr(item) => {
                self.require_mutable(&item.lhs)?;
                let expected = if let AST::Identifier(id) = &*item.lhs {
                    self.canonical(&self.binding(id.id.inner()).unwrap().typ)?
                } else {
                    self.expr(&item.lhs)?
                };
                self.expression_types
                    .insert(&*item.lhs as *const AST as usize, expected.clone());
                let actual = self.expr(&item.rhs)?;
                self.expect(&expected, &actual, loc)?;
                if let AST::Identifier(id) = &*item.lhs {
                    let binding = self
                        .scopes
                        .iter_mut()
                        .rev()
                        .find_map(|scope| scope.get_mut(id.id.inner()))
                        .unwrap();
                    binding.initialized = true;
                }
                Ok(expected)
            }
            AST::CastExpr(item) => {
                let actual = self.expr(&item.obj)?;
                let target = self.canonical(&item.target_type)?;
                let actual_numeric =
                    is_numeric(&actual) || is_bool(&actual) || self.is_enum_type(&actual);
                let target_numeric =
                    is_numeric(&target) || is_bool(&target) || self.is_enum_type(&target);
                let valid = target.matches(&actual)
                    || (actual_numeric && target_numeric)
                    || (matches!(actual, Type::Pointer(_))
                        && (matches!(target, Type::Pointer(_)) || is_integer(&target)))
                    || (is_integer(&actual) && matches!(target, Type::Pointer(_)));
                if !valid {
                    return error(
                        loc,
                        format!(
                            "invalid cast from `{}` to `{}`",
                            actual.get_full_name(),
                            target.get_full_name()
                        ),
                    );
                }
                Ok(target)
            }
            AST::ArrayLiteral(item) => {
                let Some(first) = item.values.first() else {
                    return error(loc, "cannot infer the element type of an empty array");
                };
                let typ = self.expr(first)?;
                self.value_type(&typ)?;
                for value in &item.values[1..] {
                    let actual = self.expr(value)?;
                    self.expect(&typ, &actual, location(value))?;
                }
                Ok(Type::Array(Array::new(
                    loc,
                    typ,
                    item.repeat.unwrap_or(item.values.len()),
                )))
            }
            AST::AccessExpr(item) => {
                let obj = self.expr(&item.obj)?;
                let index = self.expr(&item.index)?;
                if !is_integer(&index) {
                    return error(location(&item.index), "index must have an integer type");
                }
                match obj {
                    Type::Array(array) => {
                        if let AST::NumberLiteral(index) = &*item.index {
                            if index
                                .value
                                .inner()
                                .parse::<usize>()
                                .is_ok_and(|index| index >= array.elems)
                            {
                                return error(loc, "array index is out of bounds");
                            }
                        }
                        Ok(*array.typ)
                    }
                    Type::Pointer(pointer) => Ok(*pointer.typ),
                    Type::Slice(slice) => Ok(*slice.typ),
                    _ => error(loc, "indexing requires an array, slice or pointer"),
                }
            }
            AST::MemberExpr(MemberExpr::Member { obj, member, deref }) => {
                let typ = self.expr(obj)?;
                let owner = self.member_owner(typ, *deref, loc.clone())?;
                let AST::Identifier(member) = &**member else {
                    return error(loc, "expected a field name");
                };
                let field = self
                    .structs
                    .get(&owner)
                    .and_then(|fields| {
                        fields
                            .iter()
                            .find(|field| field.id.inner() == member.id.inner())
                    })
                    .ok_or_else(|| {
                        TypeError::new(
                            member.id.loc().clone(),
                            format!("unknown field `{}` on `{owner}`", member.id.inner()),
                        )
                    })?;
                self.canonical(&field.typ)
            }
            AST::CallExpr(item) => {
                let callee = self.expr(&item.caller)?;
                let Type::FuncPtr(function) = callee else {
                    return error(loc, "value is not callable");
                };
                self.arguments(
                    &function.params,
                    &item.args,
                    loc,
                    &callee_name(&item.caller),
                )?;
                self.canonical(&function.ret_type)
            }
            AST::MemberCallExpr(item) => {
                let receiver = self.expr(&item.obj)?;
                if matches!(&receiver, Type::Pointer(p) if p.is_ref && matches!(&*p.typ, Type::Path(t) if t.path == "str"))
                {
                    if let AST::CallExpr(call) = &*item.r#fn {
                        if let AST::Identifier(name) = &*call.caller {
                            if matches!(name.id.inner().as_str(), "len" | "as_ptr") {
                                if !call.args.is_empty() {
                                    return error(loc, "string accessor expects no arguments");
                                }
                                return Ok(if name.id.inner() == "len" {
                                    path(loc, "u64")
                                } else {
                                    Type::Pointer(Pointer::new(
                                        loc.clone(),
                                        Box::new(path(loc, "i8")),
                                        false,
                                        false,
                                    ))
                                });
                            }
                        }
                    }
                }
                let owner = self.member_owner(receiver.clone(), item.deref, loc.clone())?;
                let AST::CallExpr(call) = &*item.r#fn else {
                    return error(loc, "expected a method call");
                };
                let AST::Identifier(name) = &*call.caller else {
                    return error(loc, "expected a method name");
                };
                let signature = self
                    .methods
                    .get(&(owner.clone(), name.id.inner().clone()))
                    .cloned()
                    .ok_or_else(|| {
                        TypeError::new(
                            name.id.loc().clone(),
                            format!("unknown method `{}` on `{owner}`", name.id.inner()),
                        )
                    })?;
                if !signature.receiver {
                    return error(loc, "associated function must be called with `::`");
                }
                if let Type::Pointer(pointer) = self.canonical(&signature.params[0])? {
                    if pointer.is_mut {
                        self.require_mutable_receiver(
                            &item.obj,
                            &receiver,
                            "call a mutable method",
                            location(ast),
                        )?;
                    }
                    self.expect(&pointer.typ, &path(loc.clone(), &owner), loc.clone())?;
                } else {
                    self.expect(
                        &signature.params[0],
                        &path(loc.clone(), &owner),
                        loc.clone(),
                    )?;
                }
                self.arguments(
                    &signature.params[1..],
                    &call.args,
                    loc,
                    &format!("{owner}::{}", name.id.inner()),
                )?;
                self.canonical(&signature.ret)
            }
            AST::ResolutionExpr(item) => {
                let AST::Identifier(owner) = &*item.obj else {
                    return error(loc, "nested namespaces are not supported");
                };
                let owner_type = self.canonical(&path(owner.id.loc().clone(), owner.id.inner()))?;
                let Type::Path(owner_path) = &owner_type else {
                    return error(loc, "resolution requires a named type");
                };
                if let Some((_, variants)) = self.enums.get(&owner_path.path) {
                    let AST::Identifier(variant) = &*item.member else {
                        return error(loc, "expected an enum variant");
                    };
                    if !variants.contains(variant.id.inner()) {
                        return error(variant.id.loc().clone(), "unknown enum variant");
                    }
                    return Ok(owner_type);
                }
                let AST::CallExpr(call) = &*item.member else {
                    return error(loc, "expected an associated function call");
                };
                let AST::Identifier(name) = &*call.caller else {
                    return error(loc, "expected a function name");
                };
                let signature = self
                    .methods
                    .get(&(owner_path.path.clone(), name.id.inner().clone()))
                    .cloned()
                    .ok_or_else(|| {
                        TypeError::new(name.id.loc().clone(), "unknown associated function")
                    })?;
                self.arguments(
                    &signature.params,
                    &call.args,
                    loc,
                    &format!("{}::{}", owner_path.path, name.id.inner()),
                )?;
                self.canonical(&signature.ret)
            }
            AST::StructLiteral(item) => {
                let typ = self.canonical(&path(item.id.loc().clone(), item.id.inner()))?;
                let Type::Path(owner) = &typ else {
                    return error(loc, "struct literal requires a named struct");
                };
                let fields = self
                    .structs
                    .get(&owner.path)
                    .cloned()
                    .ok_or_else(|| TypeError::new(loc.clone(), "expected a struct type"))?;
                if item.inits.len() != fields.len() {
                    return error(
                        loc,
                        "struct initializer must supply every field exactly once",
                    );
                }
                let named = item.inits.first().is_some_and(|(name, _)| name.is_some());
                let mut used = HashSet::new();
                for (index, (name, value)) in item.inits.iter().enumerate() {
                    if name.is_some() != named {
                        return error(
                            location(value),
                            "cannot mix named and positional initializers",
                        );
                    }
                    let index = if let Some(name) = name {
                        fields
                            .iter()
                            .position(|field| field.id.inner() == name.inner())
                            .ok_or_else(|| {
                                TypeError::new(name.loc().clone(), "unknown initializer field")
                            })?
                    } else {
                        index
                    };
                    if !used.insert(index) {
                        return error(location(value), "duplicate initializer field");
                    }
                    let actual = self.expr(value)?;
                    self.expect(&fields[index].typ, &actual, location(value))?;
                }
                Ok(typ)
            }
            AST::MacroCall(item) => match item.id.inner().as_str() {
                "sizeof" if item.args.len() == 1 => {
                    if item.args[0].len() != 1 {
                        return error(loc, "sizeof currently requires a named type");
                    }
                    let token = &item.args[0][0];
                    let lygos_token::TokenType::Id(name) = &token.typ else {
                        return error(token.loc.clone(), "sizeof requires a type name");
                    };
                    let typ = self.canonical(&path(token.loc.clone(), name))?;
                    self.value_type(&typ)?;
                    Ok(path(loc, "u64"))
                }
                "format_args" if !item.args.is_empty() => {
                    let expressions = item.parsed_args.as_ref().ok_or_else(|| {
                        TypeError::new(
                            loc.clone(),
                            "format arguments must be parsed before checking",
                        )
                    })?;
                    let Some(AST::StringLiteral(format)) = expressions.first() else {
                        return error(
                            loc,
                            "format_args requires a string literal as its first argument",
                        );
                    };
                    let format = unescape_literal(format.value.inner())
                        .map_err(|message| TypeError::new(loc.clone(), message))?;
                    let (_, placeholders) = parse_format_string(&format)
                        .map_err(|message| TypeError::new(loc.clone(), message))?;
                    if expressions.len() != placeholders.len() + 1 {
                        return error(loc, "format placeholder and argument counts differ");
                    }
                    for expr in &expressions[1..] {
                        self.expr(expr)?;
                    }
                    self.canonical(&path(loc, &(item.format_namespace.clone() + "Arguments")))
                }
                "sizeof" | "format_args" => error(loc, "invalid intrinsic argument count"),
                _ => error(loc, "user macro expansion is not supported"),
            },
            _ => error(loc, "unsupported expression"),
        }
    }

    fn arguments(
        &mut self,
        params: &[Type],
        args: &[AST],
        loc: Location,
        name: &str,
    ) -> Result<()> {
        if params.len() != args.len() {
            return Err(TypeError::new(
                loc,
                format!(
                    "call to `{name}`: expected {} arguments, found {}",
                    params.len(),
                    args.len()
                ),
            )
            .label(format!("{} arguments supplied here", args.len()))
            .help(format!("supply exactly {} arguments", params.len())));
        }
        for (index, (expected, arg)) in params.iter().zip(args).enumerate() {
            let actual = self.expr(arg)?;
            self.expect(expected, &actual, location(arg))
                .map_err(|mut error| {
                    error.message =
                        format!("argument {} of `{name}`: {}", index + 1, error.message);
                    error
                })?;
        }
        Ok(())
    }

    fn require_mutable_receiver(
        &mut self,
        ast: &AST,
        typ: &Type,
        operation: &str,
        loc: Location,
    ) -> Result<()> {
        match typ {
            Type::Pointer(pointer) if pointer.is_mut => Ok(()),
            Type::Pointer(pointer) => {
                let writable = if pointer.is_ref {
                    format!("&mut {}", pointer.typ.get_full_name())
                } else {
                    format!("*mut {}", pointer.typ.get_full_name())
                };
                Err(TypeError::new(loc, format!("cannot {operation} through `{}`", typ.get_full_name()))
                    .label(format!("this operation requires `{writable}`"))
                    .related(pointer.loc.clone(), format!("read-only type `{}` declared here", typ.get_full_name()))
                    .help(format!("use `{writable}` for writable memory; `let mut` only permits reassigning the pointer binding")))
            }
            _ => self.require_mutable_for(ast, operation),
        }
    }

    fn require_mutable(&mut self, ast: &AST) -> Result<()> {
        self.require_mutable_for(ast, "write")
    }

    fn require_mutable_for(&mut self, ast: &AST, operation: &str) -> Result<()> {
        match ast {
            AST::Identifier(item) => {
                let Some(binding) = self.binding(item.id.inner()) else {
                    return error(item.id.loc().clone(), "unknown assignment target");
                };
                if !binding.mutable {
                    let method = operation == "call a mutable method";
                    let message = if method {
                        format!(
                            "cannot call a mutable method on an immutable variable `{}`",
                            item.id.inner()
                        )
                    } else {
                        format!("cannot modify an immutable variable `{}`", item.id.inner())
                    };
                    return Err(TypeError::new(item.id.loc().clone(), message)
                        .label(if method {
                            "this method requires a mutable receiver"
                        } else {
                            "assignment requires a mutable binding"
                        })
                        .related(binding.loc.clone(), "immutable binding declared here")
                        .help(format!(
                            "declare `{}` with `let mut` to allow mutation",
                            item.id.inner()
                        )));
                }
                Ok(())
            }
            AST::MemberExpr(MemberExpr::Member { obj, .. })
            | AST::AccessExpr(AccessExpr { obj, .. }) => {
                let typ = self.expr(obj)?;
                self.require_mutable_receiver(obj, &typ, operation, location(ast))
            }
            AST::UnaryExpr(item) if item.op.inner() == "*" => {
                let typ = self.expr(&item.obj)?;
                if !matches!(typ, Type::Pointer(_)) {
                    return error(location(ast), "dereference requires a pointer");
                }
                self.require_mutable_receiver(&item.obj, &typ, operation, location(ast))
            }
            _ => error(location(ast), "invalid assignment target"),
        }
    }

    fn is_enum_type(&self, typ: &Type) -> bool {
        matches!(typ, Type::Path(path) if self.enums.contains_key(&path.path))
    }
}

fn path(loc: Location, name: &str) -> Type {
    Type::Path(Path::new(loc, name))
}
fn is_void(typ: &Type) -> bool {
    matches!(typ, Type::Path(path) if path.path == "void")
}
fn is_bool(typ: &Type) -> bool {
    matches!(typ, Type::Path(path) if path.path == "bool")
}
fn is_integer(typ: &Type) -> bool {
    matches!(typ, Type::Path(path) if matches!(path.path.as_str(), "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64"))
}
fn is_numeric(typ: &Type) -> bool {
    is_integer(typ)
        || matches!(typ, Type::Path(path) if matches!(path.path.as_str(), "f32" | "f64"))
}
fn is_place(ast: &AST) -> bool {
    matches!(
        ast,
        AST::Identifier(_) | AST::AccessExpr(_) | AST::MemberExpr(_)
    ) || matches!(ast, AST::UnaryExpr(item) if item.op.inner() == "*")
}
fn constant_initializer(ast: &AST) -> bool {
    match ast {
        AST::NumberLiteral(_) | AST::StringLiteral(_) => true,
        AST::CastExpr(item) => constant_initializer(&item.obj),
        AST::UnaryExpr(item) if matches!(item.op.inner().as_str(), "+" | "-" | "!") => {
            constant_initializer(&item.obj)
        }
        AST::ResolutionExpr(item) => matches!(&*item.member, AST::Identifier(_)),
        _ => false,
    }
}

fn substitute_self(typ: &Type, owner: &Type) -> Type {
    let mut typ = typ.clone();
    match &mut typ {
        Type::Path(path) if path.path == "Self" => return owner.clone(),
        Type::Pointer(pointer) => pointer.typ = Box::new(substitute_self(&pointer.typ, owner)),
        Type::Array(array) => array.typ = Box::new(substitute_self(&array.typ, owner)),
        Type::Slice(slice) => slice.typ = Box::new(substitute_self(&slice.typ, owner)),
        Type::FuncPtr(function) => {
            function.params = function
                .params
                .iter()
                .map(|typ| substitute_self(typ, owner))
                .collect();
            function.ret_type = Box::new(substitute_self(&function.ret_type, owner));
        }
        _ => {}
    }
    typ
}

fn location(ast: &AST) -> Location {
    match ast {
        AST::Function(item) => item.id.loc().clone(),
        AST::VarDecl(item) => item.id.loc().clone(),
        AST::Identifier(item) => item.id.loc().clone(),
        AST::StructLiteral(item) => item.id.loc().clone(),
        AST::StructDef(item) => item.id.loc().clone(),
        AST::EnumDef(item) => item.id.loc().clone(),
        AST::TypeAlias(item) => item.id.loc().clone(),
        AST::StaticLiteral(item) => item.id.loc().clone(),
        AST::Trait(item) => item.id.loc().clone(),
        AST::Macro(item) => item.id.loc().clone(),
        AST::MacroCall(item) => item.id.loc().clone(),
        AST::Impl(item) => item.typ.loc().clone(),
        AST::NumberLiteral(item) => item.value.loc().clone(),
        AST::StringLiteral(item) => item.value.loc().clone(),
        AST::ArrayLiteral(item) => item.loc.clone(),
        AST::ReturnExpr(item) => item.loc.clone(),
        AST::BreakExpr(item) => item.loc.clone(),
        AST::UnaryExpr(item) => item.op.loc().clone() + location(&item.obj),
        AST::BinaryExpr(item) => item.op.loc().clone(),
        AST::AssignmentExpr(item) => location(&item.lhs),
        AST::CallExpr(item) => location(&item.caller),
        AST::MemberCallExpr(item) => location(&item.obj),
        AST::MemberExpr(MemberExpr::Member { obj, member, .. }) => location(obj) + location(member),
        AST::MemberExpr(MemberExpr::Index { obj, .. }) => location(obj),
        AST::AccessExpr(item) => {
            let mut loc = location(&item.obj) + location(&item.index);
            loc.range.end += 1;
            loc
        }
        AST::ResolutionExpr(item) => location(&item.obj),
        AST::CastExpr(item) => item.target_type.get_loc(),
        AST::IfStmt(item) => location(&item.cond),
        AST::ForStmt(item) => location(&item.cond),
        _ => Location::new("", 0..0),
    }
}

fn callee_name(ast: &AST) -> String {
    match ast {
        AST::Identifier(id) => id.id.inner().clone(),
        _ => "function pointer".into(),
    }
}
