use crate::AST;
use lygos_token::{Tagged, Token};

#[derive(Debug, Clone)]
pub struct Attribute {
    pub name: Tagged<String>,
    pub args: Vec<Vec<Token>>,
}

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
    pub tokens: Vec<Token>,
}

#[derive(Debug, Clone)]
pub struct Macro {
    pub id: Tagged<String>,
    pub arms: Vec<MacroArm>,
}

impl Macro {
    pub fn new(id: Tagged<String>, arms: Vec<MacroArm>) -> Self {
        Self { id, arms }
    }
}

#[derive(Debug, Clone)]
pub struct MacroCall {
    pub id: Tagged<String>,
    pub args: Vec<Vec<Token>>,
    /// Expression arguments of built-in formatting calls, checked before lowering.
    pub parsed_args: Option<Vec<AST>>,
    /// Canonical namespace of the runtime used by format_args$.
    pub format_namespace: String,
}

impl MacroCall {
    pub fn new(id: Tagged<String>, args: Vec<Vec<Token>>) -> Self {
        Self {
            id,
            args,
            parsed_args: None,
            format_namespace: String::new(),
        }
    }
}

pub fn parse_format_string(format: &str) -> Result<(Vec<String>, Vec<bool>), String> {
    let mut chars = format.chars().peekable();
    let mut pieces = Vec::new();
    let mut debug = Vec::new();
    let mut piece = String::new();
    while let Some(ch) = chars.next() {
        match ch {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
                piece.push('{');
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
                piece.push('}');
            }
            '{' => {
                let mut placeholder = String::new();
                loop {
                    match chars.next() {
                        Some('}') => break,
                        Some(ch) => placeholder.push(ch),
                        None => return Err("unterminated format placeholder".into()),
                    }
                }
                debug.push(match placeholder.as_str() {
                    "" => false,
                    ":?" => true,
                    _ => {
                        return Err(format!(
                            "unsupported format placeholder `{{{placeholder}}}`"
                        ));
                    }
                });
                pieces.push(std::mem::take(&mut piece));
            }
            '}' => return Err("unmatched `}` in format string".into()),
            ch => piece.push(ch),
        }
    }
    pieces.push(piece);
    Ok((pieces, debug))
}
