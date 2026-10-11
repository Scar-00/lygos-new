//! Backend-independent typed program. All graph edges are typed arena IDs.
pub use crate::arena::*;
use lygos_token::Location;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Primitive {
    Void,
    Bool,
    Str,
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    F32,
    F64,
}
impl Primitive {
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "void" => Self::Void,
            "bool" => Self::Bool,
            "str" => Self::Str,
            "i8" => Self::I8,
            "i16" => Self::I16,
            "i32" => Self::I32,
            "i64" => Self::I64,
            "u8" => Self::U8,
            "u16" => Self::U16,
            "u32" => Self::U32,
            "u64" => Self::U64,
            "f32" => Self::F32,
            "f64" => Self::F64,
            _ => return None,
        })
    }
    pub fn unsigned(self) -> bool {
        matches!(
            self,
            Self::Bool | Self::U8 | Self::U16 | Self::U32 | Self::U64
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SemanticType {
    Primitive(Primitive),
    Pointer {
        pointee: TypeId,
        reference: bool,
        mutable: bool,
    },
    Array {
        element: TypeId,
        length: usize,
    },
    Slice {
        element: TypeId,
    },
    Function {
        parameters: Vec<TypeId>,
        result: TypeId,
    },
    Struct(SymbolId),
    Enum(SymbolId),
}

#[derive(Debug, Default)]
pub struct TypeInterner {
    arena: Arena<SemanticType, TypeId>,
    ids: HashMap<SemanticType, TypeId>,
}
impl TypeInterner {
    pub fn intern(&mut self, typ: SemanticType) -> TypeId {
        if let Some(id) = self.ids.get(&typ) {
            return *id;
        }
        let id = self.arena.insert(typ.clone());
        self.ids.insert(typ, id);
        id
    }
    pub fn get(&self, id: TypeId) -> &SemanticType {
        &self.arena[id]
    }
    pub fn iter(&self) -> impl Iterator<Item = (TypeId, &SemanticType)> {
        self.arena.iter()
    }
    pub fn len(&self) -> usize {
        self.arena.len()
    }
    pub fn is_empty(&self) -> bool {
        self.arena.is_empty()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueCategory {
    Value,
    Place,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnaryOp {
    Negate,
    Not,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Callee {
    Direct(SymbolId),
    Indirect(ExprId),
}

#[derive(Clone, Debug)]
pub struct TypedExpr {
    pub kind: ExprKind,
    pub typ: TypeId,
    pub category: ValueCategory,
    pub span: Location,
}
#[derive(Clone, Debug)]
pub enum ExprKind {
    Integer(i32),
    Float(f64),
    String(String),
    StringLength(ExprId),
    StringData(ExprId),
    Symbol(SymbolId),
    Load(ExprId),
    AddressOf(ExprId),
    Dereference(ExprId),
    Unary {
        op: UnaryOp,
        operand: ExprId,
    },
    Binary {
        op: BinaryOp,
        left: ExprId,
        right: ExprId,
    },
    Cast(ExprId),
    Assign {
        target: ExprId,
        value: ExprId,
    },
    Call {
        callee: Callee,
        signature: TypeId,
        arguments: Vec<ExprId>,
    },
    Field {
        base: ExprId,
        field: SymbolId,
    },
    Index {
        base: ExprId,
        index: ExprId,
    },
    Struct(Vec<(SymbolId, ExprId)>),
    Array(Vec<ExprId>),
    ArrayRepeat {
        value: ExprId,
        length: usize,
    },
    SizeOf(TypeId),
    Format {
        arguments: Vec<(ExprId, SymbolId)>,
        pieces: Vec<String>,
        argument_type: TypeId,
        constructor: SymbolId,
    },
}
#[derive(Clone, Debug)]
pub struct TypedStmt {
    pub kind: StmtKind,
    pub span: Location,
}
#[derive(Clone, Debug)]
pub enum StmtKind {
    Let {
        symbol: SymbolId,
        initializer: Option<ExprId>,
    },
    Expression(ExprId),
    Return(Option<ExprId>),
    Break,
    If {
        condition: ExprId,
        then_block: BlockId,
        else_block: Option<BlockId>,
    },
    For {
        initializer: StmtId,
        condition: ExprId,
        body: BlockId,
        scope: ScopeId,
    },
}
#[derive(Clone, Debug)]
pub struct TypedBlock {
    pub statements: Vec<StmtId>,
    pub scope: ScopeId,
}
#[derive(Clone, Debug)]
pub struct TypedFunction {
    pub symbol: SymbolId,
    pub parameters: Vec<SymbolId>,
    pub body: Option<BlockId>,
    pub scope: ScopeId,
}
#[derive(Clone, Debug)]
pub struct SemanticSymbol {
    pub name: String,
    pub typ: TypeId,
    pub kind: SymbolKind,
    pub scope: ScopeId,
    pub span: Location,
}
#[derive(Clone, Debug)]
pub enum SymbolKind {
    Local {
        mutable: bool,
    },
    Parameter,
    Static {
        initializer: Option<ExprId>,
    },
    Function {
        function: FunctionId,
        link_name: String,
    },
    Struct {
        fields: Vec<SymbolId>,
    },
    Field {
        index: u32,
    },
    Enum {
        underlying: TypeId,
        variants: Vec<String>,
    },
    TypeAlias,
}
#[derive(Debug)]
pub struct SemanticScope {
    pub parent: Option<ScopeId>,
    pub symbols: HashMap<String, SymbolId>,
}

#[derive(Debug, Default)]
pub struct SemanticContext {
    pub(crate) expressions: Arena<TypedExpr, ExprId>,
    pub(crate) statements: Arena<TypedStmt, StmtId>,
    pub(crate) blocks: Arena<TypedBlock, BlockId>,
    pub(crate) functions: Arena<TypedFunction, FunctionId>,
    pub(crate) symbols: Arena<SemanticSymbol, SymbolId>,
    pub(crate) scopes: Arena<SemanticScope, ScopeId>,
    pub(crate) types: TypeInterner,
}
impl SemanticContext {
    pub fn expression(&self, id: ExprId) -> &TypedExpr {
        &self.expressions[id]
    }
    pub fn statement(&self, id: StmtId) -> &TypedStmt {
        &self.statements[id]
    }
    pub fn block(&self, id: BlockId) -> &TypedBlock {
        &self.blocks[id]
    }
    pub fn function(&self, id: FunctionId) -> &TypedFunction {
        &self.functions[id]
    }
    pub fn symbol(&self, id: SymbolId) -> &SemanticSymbol {
        &self.symbols[id]
    }
    pub fn scope(&self, id: ScopeId) -> &SemanticScope {
        &self.scopes[id]
    }
    pub fn typ(&self, id: TypeId) -> &SemanticType {
        self.types.get(id)
    }
    pub fn types(&self) -> impl Iterator<Item = (TypeId, &SemanticType)> {
        self.types.iter()
    }
    pub fn symbols(&self) -> impl Iterator<Item = (SymbolId, &SemanticSymbol)> {
        self.symbols.iter()
    }
    pub fn expressions(&self) -> impl Iterator<Item = (ExprId, &TypedExpr)> {
        self.expressions.iter()
    }
    pub fn lookup(&self, mut scope: ScopeId, name: &str) -> Option<SymbolId> {
        loop {
            let current = self.scope(scope);
            if let Some(id) = current.symbols.get(name) {
                return Some(*id);
            }
            scope = current.parent?;
        }
    }
}

/// Only semantic analysis constructs modules; lowering receives an immutable one.
#[derive(Debug)]
pub struct TypedModule {
    pub(crate) context: SemanticContext,
    pub(crate) functions: Vec<FunctionId>,
    pub(crate) statics: Vec<SymbolId>,
    pub(crate) scope: ScopeId,
}
impl TypedModule {
    pub fn context(&self) -> &SemanticContext {
        &self.context
    }
    pub fn functions(&self) -> &[FunctionId] {
        &self.functions
    }
    pub fn statics(&self) -> &[SymbolId] {
        &self.statics
    }
    pub fn scope(&self) -> ScopeId {
        self.scope
    }
}
