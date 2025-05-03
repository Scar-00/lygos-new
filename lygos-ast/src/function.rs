use lygos_macros::Visitable;
use lygos_token::Tagged;
use crate::{types::Type, Block};

#[derive(Debug, Clone)]
pub struct FunctionArg {
    pub id: Tagged<String>,
    pub typ: Type,
}

#[derive(Debug, Visitable)]
pub struct Function {
    pub id: Tagged<String>,
    pub name_mangeled: String,
    //obj: Option<Impl>,
    pub args: Vec<FunctionArg>,
    pub body: Block,
    pub ret_type: Type,
    pub is_def: bool,
    pub is_var_arg: bool,
    //pub ret_block: Option<llvm::BasicBlock>,
    //attrs: Vec<Attribute>,
}

impl Function {
    pub fn new(id: Tagged<String>, args: Vec<FunctionArg>, body: Block, ret_type: Type, is_def: bool, is_var_arg: bool) -> Self {
        Self{ id: id.clone(), name_mangeled: id.inner().to_string(), args, body, ret_type, is_def, is_var_arg }
    }
}
