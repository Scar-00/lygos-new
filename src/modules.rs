//! File modules are loaded once, expanded independently, and resolved before
//! semantic analysis. Canonical names keep each file's declarations distinct.
use lygos_ast::{
    arena::{Arena, ArenaId},
    type_check::TypeError,
    types::Type,
    *,
};
use lygos_lexer::Lexer;
use lygos_parser::{Parser, comptime};
use lygos_token::{Location, Tagged};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

type Result<T> = std::result::Result<T, TypeError>;
mod macros;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ModuleId(u32);
impl ArenaId for ModuleId {
    fn from_index(index: u32) -> Self {
        Self(index)
    }
    fn index(self) -> usize {
        self.0 as usize
    }
}
struct FileModule {
    path: PathBuf,
    body: Block,
    imports: HashMap<String, ModuleId>,
    exports: HashMap<String, String>,
    token_macros: HashMap<String, Macro>,
}
#[derive(Default)]
pub struct ModuleLoader {
    modules: Arena<FileModule, ModuleId>,
    files: HashMap<PathBuf, ModuleId>,
    visiting: HashSet<PathBuf>,
}
pub fn load(path: impl AsRef<Path>) -> Result<AST> {
    ModuleLoader::default().load(path)
}
impl ModuleLoader {
    pub fn load(mut self, path: impl AsRef<Path>) -> Result<AST> {
        let path = path.as_ref();
        let loc = Location::new(path, 0..0);
        let root = self.file(path, &loc)?;
        let mut output = Block::new();
        let ids = self.modules.iter().map(|(id, _)| id).collect::<Vec<_>>();
        // Finish all expansion before resolving any file, including generated
        // declarations referenced from another module.
        for id in &ids {
            let mut body = std::mem::take(&mut self.modules.get_mut(*id).unwrap().body);
            macros::Expander::new(&self.modules, *id).block(&mut body)?;
            body.body.retain(|node| !matches!(node, AST::Macro(_)));
            let AST::Module(expanded) = comptime::expand(AST::Module(Module::new(body)))? else {
                unreachable!()
            };
            let item = self.modules.get_mut(*id).unwrap();
            for node in &expanded.body.body {
                if let Some((name, external)) = declaration_name(node) {
                    item.exports.insert(
                        name.inner().clone(),
                        if id.0 == 0 || external {
                            name.inner().clone()
                        } else {
                            format!("__module_{}__{}", id.0, name.inner())
                        },
                    );
                }
            }
            item.body = expanded.body;
        }
        for id in ids {
            let mut body = std::mem::take(&mut self.modules.get_mut(id).unwrap().body);
            let mut resolver = Resolver {
                modules: &self.modules,
                module: id,
                scopes: Vec::new(),
            };
            for node in &mut body.body {
                resolver.declaration(node)?;
            }
            // Keep the entry file last; all declarations are collected before
            // checking bodies, so order does not constrain forward references.
            if id == root {
                self.modules.get_mut(id).unwrap().body = body;
            } else {
                output.body.extend(body.body);
            }
        }
        output
            .body
            .extend(std::mem::take(&mut self.modules.get_mut(root).unwrap().body).body);
        Ok(AST::Module(Module::new(output)))
    }
    fn file(&mut self, path: &Path, origin: &Location) -> Result<ModuleId> {
        let path = path.canonicalize().map_err(|error| {
            TypeError::new(
                origin.clone(),
                format!("cannot load module `{}`: {error}", path.display()),
            )
        })?;
        if self.visiting.contains(&path) {
            return Err(TypeError::new(
                origin.clone(),
                format!("module import cycle involving `{}`", path.display()),
            ));
        }
        if let Some(id) = self.files.get(&path) {
            return Ok(*id);
        }
        let mut lexer =
            Lexer::new(&path).map_err(|error| TypeError::new(origin.clone(), error.to_string()))?;
        let tokens = lexer.try_get_tokens().map_err(|error| match error {
            lygos_lexer::Error::Lexical { loc, message } => TypeError::new(loc, message),
            error => TypeError::new(origin.clone(), error.to_string()),
        })?;
        let AST::Module(module) = Parser::from(tokens).build_raw_ast() else {
            unreachable!()
        };
        self.module(path, module.body, origin)
    }
    fn module(&mut self, path: PathBuf, mut body: Block, _origin: &Location) -> Result<ModuleId> {
        let id = self.modules.insert(FileModule {
            path: path.clone(),
            body: Block::new(),
            imports: HashMap::new(),
            exports: HashMap::new(),
            token_macros: HashMap::new(),
        });
        self.files.insert(path.clone(), id);
        self.visiting.insert(path.clone());
        let mut imports = HashMap::new();
        let mut declarations = Vec::new();
        for node in body.body.drain(..) {
            let AST::Import(import) = node else {
                declarations.push(node);
                continue;
            };
            if imports.contains_key(import.alias.inner()) {
                return Err(TypeError::new(
                    import.alias.loc().clone(),
                    format!("duplicate module alias `{}`", import.alias.inner()),
                ));
            }
            let base = path.parent().unwrap_or(Path::new("."));
            let candidate = if import.is_path {
                base.join(&import.source)
            } else {
                base.join(format!("{}.ly", import.source))
            };
            let directory = base.join(&import.source).join("mod.ly");
            let target = if candidate.is_file() {
                self.file(&candidate, &import.loc)
            } else if !import.is_path && directory.is_file() {
                self.file(&directory, &import.loc)
            } else if !import.is_path && import.source == "print" {
                self.builtin_print(&import.loc)
            } else {
                Err(TypeError::new(
                    import.loc.clone(),
                    format!(
                        "cannot find module `{}` (searched `{}`{})",
                        import.source,
                        candidate.display(),
                        if import.is_path {
                            String::new()
                        } else {
                            format!(" and `{}`", directory.display())
                        }
                    ),
                ))
            };
            let target = target.map_err(|mut error| {
                error.related.push((
                    import.loc.clone(),
                    format!("imported as `{}` here", import.alias.inner()),
                ));
                error
            })?;
            imports.insert(import.alias.inner().clone(), target);
        }
        let module = Module::new(Block::from(declarations));
        let mut exports = HashMap::new();
        for node in &module.body.body {
            if let Some((name, external)) = declaration_name(node) {
                if imports.contains_key(name.inner()) {
                    return Err(TypeError::new(
                        name.loc().clone(),
                        format!("declaration conflicts with module alias `{}`", name.inner()),
                    ));
                }
                let canonical = if id.0 == 0 || external {
                    name.inner().clone()
                } else {
                    format!("__module_{}__{}", id.0, name.inner())
                };
                exports.insert(name.inner().clone(), canonical);
            }
        }
        let item = self.modules.get_mut(id).unwrap();
        item.token_macros = module
            .body
            .body
            .iter()
            .filter_map(|node| {
                if let AST::Macro(m) = node {
                    Some((m.id.inner().clone(), m.clone()))
                } else {
                    None
                }
            })
            .collect();
        item.body = module.body;
        item.imports = imports;
        item.exports = exports;
        self.visiting.remove(&path);
        Ok(id)
    }
    fn builtin_print(&mut self, origin: &Location) -> Result<ModuleId> {
        let path = PathBuf::from("<builtin:print>");
        if let Some(id) = self.files.get(&path) {
            return Ok(*id);
        }
        let source = include_str!("../std/print.ly").to_string()
            + if cfg!(windows) {
                include_str!("../std/print_windows.ly")
            } else {
                include_str!("../std/print_posix.ly")
            };
        let mut tokens = Lexer::from(source.chars().collect()).get_tokens();
        // Bundled source is embedded; use the import's location for diagnostics.
        for token in &mut tokens {
            token.loc = origin.clone();
        }
        let AST::Module(module) = Parser::from(tokens).build_raw_ast() else {
            unreachable!()
        };
        self.module(path, module.body, origin)
    }
}
fn declaration_name(node: &AST) -> Option<(&Tagged<String>, bool)> {
    match node {
        AST::Function(v) => Some((&v.id, !v.is_def)),
        AST::StructDef(v) => Some((&v.id, false)),
        AST::ComptimeFunction(v) => Some((&v.id, false)),
        AST::Macro(v) => Some((&v.id, false)),
        AST::Attributed { declaration, .. } => declaration_name(declaration),
        AST::EnumDef(v) => Some((&v.id, false)),
        AST::Trait(v) => Some((&v.id, false)),
        AST::TypeAlias(v) => Some((&v.id, false)),
        AST::StaticLiteral(v) => Some((&v.id, false)),
        _ => None,
    }
}
struct Resolver<'a> {
    modules: &'a Arena<FileModule, ModuleId>,
    module: ModuleId,
    scopes: Vec<HashSet<String>>,
}
impl Resolver<'_> {
    fn local(&self, name: &str) -> bool {
        self.scopes.iter().rev().any(|s| s.contains(name))
    }
    fn name(&self, name: &str, loc: &Location) -> Result<String> {
        let parts = name.split("::").collect::<Vec<_>>();
        if parts.len() == 1 {
            return Ok(self.modules[self.module]
                .exports
                .get(name)
                .cloned()
                .unwrap_or_else(|| name.to_string()));
        }
        let mut module = self.module;
        let mut index = 0;
        while index < parts.len() {
            if let Some(next) = self.modules[module].imports.get(parts[index]) {
                module = *next;
                index += 1;
            } else {
                break;
            }
        }
        if index == 0 {
            return Ok(name.to_string());
        }
        if index == parts.len() {
            return Err(TypeError::new(
                loc.clone(),
                "a module namespace cannot be used as a value or type",
            ));
        }
        let item = &self.modules[module];
        let Some(export) = item.exports.get(parts[index]) else {
            return Err(TypeError::new(
                loc.clone(),
                format!(
                    "module `{}` has no declaration `{}`",
                    item.path.display(),
                    parts[index]
                ),
            ));
        };
        let mut result = export.clone();
        for part in &parts[index + 1..] {
            result.push_str("::");
            result.push_str(part);
        }
        Ok(result)
    }
    fn tag(&self, tag: &mut Tagged<String>) -> Result<()> {
        *tag.inner_mut() = self.name(tag.inner(), tag.loc())?;
        Ok(())
    }
    fn typ(&self, typ: &mut Type) -> Result<()> {
        match typ {
            Type::Path(p) => p.path = self.name(&p.path, &p.loc)?,
            Type::Pointer(p) => self.typ(&mut p.typ)?,
            Type::Array(a) => self.typ(&mut a.typ)?,
            Type::Slice(s) => self.typ(&mut s.typ)?,
            Type::FuncPtr(f) => {
                for t in &mut f.params {
                    self.typ(t)?;
                }
                self.typ(&mut f.ret_type)?;
            }
        }
        Ok(())
    }
    fn function(&mut self, function: &mut Function, method: bool) -> Result<()> {
        if !method {
            self.tag(&mut function.id)?;
        }
        if let Some(owner) = &mut function.obj {
            self.tag(&mut owner.typ)?;
            if let Some(t) = &mut owner.trat {
                self.tag(t)?;
            }
        }
        self.scopes.push(HashSet::new());
        for arg in &mut function.args {
            self.typ(&mut arg.typ)?;
            self.scopes
                .last_mut()
                .unwrap()
                .insert(arg.id.inner().clone());
        }
        self.typ(&mut function.ret_type)?;
        self.block(&mut function.body)?;
        self.scopes.pop();
        Ok(())
    }
    fn declaration(&mut self, node: &mut AST) -> Result<()> {
        match node {
            AST::Function(f) => self.function(f, false)?,
            AST::StructDef(s) => {
                self.tag(&mut s.id)?;
                for f in &mut s.fields {
                    self.typ(&mut f.typ)?;
                }
            }
            AST::EnumDef(e) => {
                self.tag(&mut e.id)?;
                self.typ(&mut e.typ)?;
            }
            AST::Trait(t) => {
                self.tag(&mut t.id)?;
                for n in &mut t.funcs.body {
                    if let AST::Function(f) = n {
                        self.function(f, true)?;
                    }
                }
            }
            AST::Impl(i) => {
                self.tag(&mut i.typ)?;
                if let Some(t) = &mut i.trat {
                    self.tag(t)?;
                }
                for n in &mut i.body.body {
                    if let AST::Function(f) = n {
                        self.function(f, true)?;
                    }
                }
            }
            AST::TypeAlias(t) => {
                self.tag(&mut t.id)?;
                self.typ(&mut t.typ)?;
            }
            AST::StaticLiteral(s) => {
                self.tag(&mut s.id)?;
                self.typ(&mut s.typ)?;
                if let Some(v) = &mut s.value {
                    self.expr(v)?;
                }
            }
            _ => self.expr(node)?,
        }
        Ok(())
    }
    fn block(&mut self, block: &mut Block) -> Result<()> {
        self.scopes.push(HashSet::new());
        for node in &mut block.body {
            self.expr(node)?;
        }
        self.scopes.pop();
        Ok(())
    }
    fn expr(&mut self, node: &mut AST) -> Result<()> {
        // Resolve a whole namespace chain at once, including associated methods.
        if let AST::ResolutionExpr(resolution) = node {
            if let Some((path, args)) = resolution_path(resolution) {
                let first = path.split("::").next().unwrap();
                if !self.local(first) && self.modules[self.module].imports.contains_key(first) {
                    let loc = expression_location(&resolution.obj);
                    let canonical = self.name(&path, &loc)?;
                    let mut args = args;
                    if let Some(args) = &mut args {
                        for arg in args {
                            self.expr(arg)?;
                        }
                    }
                    *node = resolved_expression(&canonical, loc, args);
                    return Ok(());
                }
            }
        }
        match node {
            AST::Identifier(v) => {
                if !self.local(v.id.inner()) {
                    if self.modules[self.module].imports.contains_key(v.id.inner()) {
                        return Err(TypeError::new(
                            v.id.loc().clone(),
                            "a module namespace cannot be used as a value",
                        ));
                    }
                    self.tag(&mut v.id)?;
                }
            }
            AST::VarDecl(v) => {
                if let Some(t) = &mut v.typ {
                    self.typ(t)?;
                }
                if let Some(e) = &mut v.value {
                    self.expr(e)?;
                }
                self.scopes.last_mut().unwrap().insert(v.id.inner().clone());
            }
            AST::ReturnExpr(v) => {
                if let Some(e) = &mut v.value {
                    self.expr(e)?;
                }
            }
            AST::UnaryExpr(v) => self.expr(&mut v.obj)?,
            AST::CastExpr(v) => {
                self.typ(&mut v.target_type)?;
                self.expr(&mut v.obj)?;
            }
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
                if let AST::CallExpr(call) = &mut *v.r#fn {
                    for a in &mut call.args {
                        self.expr(a)?;
                    }
                }
            }
            AST::ResolutionExpr(v) => {
                self.expr(&mut v.obj)?;
                if let AST::CallExpr(call) = &mut *v.member {
                    for a in &mut call.args {
                        self.expr(a)?;
                    }
                }
            }
            AST::MemberExpr(MemberExpr::Member { obj, .. })
            | AST::MemberExpr(MemberExpr::Index { obj, .. }) => self.expr(obj)?,
            AST::AccessExpr(v) => {
                self.expr(&mut v.obj)?;
                self.expr(&mut v.index)?;
            }
            AST::StructLiteral(v) => {
                self.tag(&mut v.id)?;
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
                self.scopes.push(HashSet::new());
                self.expr(&mut v.var)?;
                self.expr(&mut v.cond)?;
                self.block(&mut v.body)?;
                self.scopes.pop();
            }
            AST::MacroCall(v) => {
                if v.id.inner() == "format_args" || v.id.inner().ends_with("::format_args") {
                    let runtime = if let Some(namespace) =
                        v.id.inner().strip_suffix("::format_args")
                    {
                        let mut module = self.module;
                        for part in namespace.split("::") {
                            module = *self.modules[module].imports.get(part).ok_or_else(|| {
                                TypeError::new(
                                    v.id.loc().clone(),
                                    format!("unknown module `{part}`"),
                                )
                            })?;
                        }
                        module
                    } else if self.modules[self.module].exports.contains_key("Arguments") {
                        self.module
                    } else {
                        let candidates = self.modules[self.module]
                            .imports
                            .values()
                            .copied()
                            .filter(|id| {
                                self.modules[*id].exports.contains_key("Arguments")
                                    && self.modules[*id].exports.contains_key("Argument")
                            })
                            .collect::<HashSet<_>>();
                        if candidates.len() > 1 {
                            return Err(TypeError::new(
                                v.id.loc().clone(),
                                "ambiguous formatting runtime; use `module::format_args$(...)`",
                            ));
                        }
                        candidates.into_iter().next().unwrap_or(self.module)
                    };
                    *v.id.inner_mut() = "format_args".into();
                    if v.format_namespace.is_empty() {
                        v.format_namespace = if runtime.0 == 0 {
                            String::new()
                        } else {
                            format!("__module_{}__", runtime.0)
                        };
                    }
                }
                if v.id.inner() == "sizeof" && v.args.len() == 1 {
                    let tokens = &v.args[0];
                    let name = tokens.iter().map(|t| t.typ.as_str()).collect::<String>();
                    let canonical = self.name(&name, v.id.loc())?;
                    if tokens.iter().enumerate().all(|(i, t)| {
                        if i % 2 == 0 {
                            matches!(t.typ, lygos_token::TokenType::Id(_))
                        } else {
                            t.typ == lygos_token::TokenType::OpScope
                        }
                    }) {
                        v.args[0] = vec![lygos_token::Token::new(
                            lygos_token::TokenType::Id(canonical),
                            v.id.loc().clone(),
                        )];
                    }
                }
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
fn expression_location(node: &AST) -> Location {
    match node {
        AST::Identifier(v) => v.id.loc().clone(),
        AST::ResolutionExpr(v) => expression_location(&v.obj),
        _ => Location::new("", 0..0),
    }
}
fn expression_path(node: &AST) -> Option<String> {
    match node {
        AST::Identifier(v) => Some(v.id.inner().clone()),
        AST::ResolutionExpr(v) => Some(format!(
            "{}::{}",
            expression_path(&v.obj)?,
            expression_path(&v.member)?
        )),
        _ => None,
    }
}
fn resolution_path(resolution: &ResolutionExpr) -> Option<(String, Option<Vec<AST>>)> {
    let owner = expression_path(&resolution.obj)?;
    match &*resolution.member {
        AST::Identifier(v) => Some((format!("{owner}::{}", v.id.inner()), None)),
        AST::CallExpr(c) => Some((
            format!("{owner}::{}", expression_path(&c.caller)?),
            Some(c.args.clone()),
        )),
        _ => None,
    }
}
fn resolved_expression(name: &str, loc: Location, args: Option<Vec<AST>>) -> AST {
    let (owner, member) = name
        .split_once("::")
        .map_or((None, name), |(a, b)| (Some(a), b));
    let mut expression = AST::Identifier(Identifier::new(Tagged::new(
        loc.clone(),
        member.to_string(),
    )));
    if let Some(args) = args {
        expression = AST::CallExpr(CallExpr::new(Box::new(expression), args));
    }
    if let Some(owner) = owner {
        AST::ResolutionExpr(ResolutionExpr::new(
            Box::new(AST::Identifier(Identifier::new(Tagged::new(
                loc,
                owner.to_string(),
            )))),
            Box::new(expression),
        ))
    } else {
        expression
    }
}
