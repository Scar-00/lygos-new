use lygos_llvm::value::BasicBlock;
use lygos_macros::Visitable;
use lygos_token::Tagged;
use crate::{symbols, types::Type, Block, Impl};

#[derive(Debug, Clone)]
pub struct FunctionArg {
    pub id: Tagged<String>,
    pub typ: Type,
}

//  TODO(S): make ret_type optional to indicate `void` return type (way clearer)
#[derive(Debug, Visitable)]
pub struct Function {
    pub id: Tagged<String>,
    pub name_mangeled: String,
    pub obj: Option<Impl>,
    pub args: Vec<FunctionArg>,
    pub body: Block,
    pub ret_type: Type,
    pub is_def: bool,
    pub is_var_arg: bool,
    pub ret_block: Option<BasicBlock>,
    //attrs: Vec<Attribute>,
}

impl Function {
    pub fn new(id: Tagged<String>, obj: Option<Impl>, args: Vec<FunctionArg>, body: Block, ret_type: Type, is_def: bool, is_var_arg: bool) -> Self {
        Self{ id: id.clone(), name_mangeled: id.inner().to_string(), obj, args, body, ret_type, is_def, is_var_arg, ret_block: None }
    }
}

#[allow(clippy::from_over_into)]
impl Into<symbols::Function> for &Function {
    fn into(self) -> symbols::Function {
        symbols::Function::new(self.id.clone(),
                                self.name_mangeled.clone(),
                                self.args.clone(),
                                self.ret_type.clone(),
                                self.is_def
                            )
    }
}

#[allow(clippy::from_over_into)]
impl Into<symbols::Function> for &mut Function {
    fn into(self) -> symbols::Function {
        symbols::Function::new(self.id.clone(),
                                self.name_mangeled.clone(),
                                self.args.clone(),
                                self.ret_type.clone(),
                                self.is_def
                            )
    }
}
