#![allow(unused)]
#![allow(clippy::new_without_default)]
pub mod types;
pub mod symbols;
pub mod value;
pub mod scope;
pub use scope::*;

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

use lygos_macros::{Visitor, VisitorImpl};

pub type Exprs = Vec<AST>;

#[derive(Debug, Visitor, VisitorImpl)]
#[visitor(ASTVisitor)]
pub enum AST {
    Module(Module),
    Function(Function),
    VarDecl(VarDecl),

    AssignmentExpr(AssignmentExpr),
    MemberExpr(MemberExpr),
    CallExpr(CallExpr),
    //MemberCallExpr(MemberCallExpr),
    //AccessExpr(AccessExpr),
    //UnaryExpr(UnaryExpr),
    //ResolutionExpr(ResolutionExpr),
    //CastExpr(CastExpr),
    ReturnExpr(ReturnExpr),
    BinaryExpr(BinaryExpr),

    /*IfStmt(IfStmt),
    ForStmt(ForStmt),
    //MatchStmt(()),
    BreakExpr(BreakExpr),
    ClosureExpr(ClosureExpr),*/

    StructDef(StructDef),
    /*EnumDef(EnumDef),
    Impl(Impl),
    Trait(Trait),
    Macro(Macro),
    MacroCall(MacroCall),*/
    TypeAlias(TypeAlias),
    NumberLiteral(NumberLiteral),
    StringLiteral(StringLiteral),
    //InitializerList(InitializerListExpr),
    StructLiteral(StructLiteral),
    StaticLiteral(StaticLiteral),
    Identifier(Identifier),
}

impl AST {
    pub fn needs_load(&self) -> bool {
        match self {
            AST::Identifier(_) | AST::StructLiteral(_) => true,
            AST::MemberExpr(MemberExpr::Member { member, .. }) => {
                // !matches!(member, AST::CallExpr(_))
                true
            }
            _ => false,
        }
    }
}

#[derive(Debug)]
pub struct Block {
    pub body: Exprs,
    pub returns: bool,
    pub scope: Scope,
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
            scope: Scope::new(),
        }
    }

    pub fn from(exprs: Exprs) -> Self {
        Self {
            body: exprs,
            returns: false,
            scope: Scope::new(),
        }
    }

    pub fn push(&mut self, node: AST) {
        self.body.push(node);
    }
}
