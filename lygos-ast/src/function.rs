use crate::{Block, Impl, types::Type};
use lygos_token::Tagged;

#[derive(Debug, Clone)]
pub struct FunctionArg {
    pub id: Tagged<String>,
    pub typ: Type,
}

//  TODO(S): make ret_type optional to indicate `void` return type (way clearer)
#[derive(Debug, Clone)]
pub struct Function {
    pub id: Tagged<String>,
    pub obj: Option<Impl>,
    pub args: Vec<FunctionArg>,
    pub body: Block,
    pub ret_type: Type,
    pub is_def: bool,
    pub is_var_arg: bool,
    //attrs: Vec<Attribute>,
}

impl Function {
    pub fn new(
        id: Tagged<String>,
        obj: Option<Impl>,
        args: Vec<FunctionArg>,
        body: Block,
        ret_type: Type,
        is_def: bool,
        is_var_arg: bool,
    ) -> Self {
        Self {
            id,
            obj,
            args,
            body,
            ret_type,
            is_def,
            is_var_arg,
        }
    }
}
