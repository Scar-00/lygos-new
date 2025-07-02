use lygos_macros::Visitable;
use lygos_token::{Tagged, Token};

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum MacroArgType {
    Single,
    Variadic,
}

#[derive(Debug, Clone)]
pub struct MacroArg {
    pub id: Tagged<String>,
    pub typ: MacroArgType,
}

#[derive(Debug, Clone)]
pub struct MacroArm {
    pub conds: Vec<MacroArg>,
    pub tokens: Vec<Token>
}

#[derive(Debug, Clone, Visitable)]
pub struct Macro {
    pub id: Tagged<String>,
    pub arms: Vec<MacroArm>,
}

impl Macro {
    pub fn new(id: Tagged<String>, arms: Vec<MacroArm>) -> Self {
        Self{ id, arms }
    }
}

#[derive(Debug, Visitable)]
pub struct MacroCall {
    pub id: Tagged<String>,
    pub args: Vec<Vec<Token>>,
}

impl MacroCall {
    pub fn new(id: Tagged<String>, args: Vec<Vec<Token>>,) -> Self {
        Self{ id, args }
    }
}
