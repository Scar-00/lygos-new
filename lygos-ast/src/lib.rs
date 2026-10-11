#![allow(unused)]
#![allow(clippy::new_without_default)]
pub mod arena;
pub mod type_check;
pub mod typed;
pub mod types;

pub mod module;
pub use module::*;
pub mod function;
pub use function::*;
pub mod call;
pub use call::*;
pub mod control_flow;
pub use control_flow::*;
pub mod binary;
pub use binary::*;
pub mod literals;
pub use literals::*;
pub mod vardecl;
pub use vardecl::*;
pub mod assignment;
pub use assignment::*;
pub mod strct;
pub use strct::*;
pub mod access;
pub use access::*;
pub mod r#trait;
pub use r#trait::*;
pub mod r#macro;
pub use r#macro::*;

pub type Exprs = Vec<AST>;

#[derive(Debug, Clone)]
pub enum AST {
    Module(Module),
    Import(Import),
    Function(Function),
    VarDecl(VarDecl),

    AssignmentExpr(AssignmentExpr),
    MemberExpr(MemberExpr),
    CallExpr(CallExpr),
    MemberCallExpr(MemberCallExpr),
    AccessExpr(AccessExpr),
    UnaryExpr(UnaryExpr),
    ResolutionExpr(ResolutionExpr),
    CastExpr(CastExpr),
    ReturnExpr(ReturnExpr),
    BinaryExpr(BinaryExpr),

    IfStmt(IfStmt),
    ForStmt(ForStmt),
    //MatchStmt(()),
    BreakExpr(BreakExpr),
    //ClosureExpr(ClosureExpr),
    BlockExpr(BlockExpr),

    StructDef(StructDef),
    EnumDef(EnumDef),
    Impl(Impl),
    Trait(Trait),
    Macro(Macro),
    ComptimeFunction(Function),
    Attributed {
        attributes: Vec<Attribute>,
        declaration: Box<AST>,
    },
    MacroCall(MacroCall),
    TypeAlias(TypeAlias),
    NumberLiteral(NumberLiteral),
    StringLiteral(StringLiteral),
    StructLiteral(StructLiteral),
    ArrayLiteral(ArrayLiteral),
    StaticLiteral(StaticLiteral),
    Identifier(Identifier),
}

#[derive(Debug, Clone)]
pub struct Block {
    pub body: Exprs,
    pub returns: bool,
}

impl Default for Block {
    fn default() -> Self {
        Self::new()
    }
}

impl Block {
    pub fn new() -> Self {
        Self {
            body: Vec::new(),
            returns: false,
        }
    }

    pub fn from(exprs: Exprs) -> Self {
        Self {
            body: exprs,
            returns: false,
        }
    }

    pub fn push(&mut self, node: AST) {
        self.body.push(node);
    }
}
