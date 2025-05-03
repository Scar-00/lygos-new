use std::{char, collections::HashMap, fs, path::{Path, PathBuf}};

lazy_static::lazy_static! {
    static ref KeyWords: HashMap<&'static str, TokenType> = {
        let mut map = HashMap::new();
        map.insert("let", TokenType::KwLet);
        map.insert("mut", TokenType::KwMut);
        map.insert("const", TokenType::KwConst);
        map.insert("struct", TokenType::KwStruct);
        map.insert("if", TokenType::KwIf);
        map.insert("else", TokenType::KwElse);
        map.insert("for", TokenType::KwFor);
        map.insert("while", TokenType::KwWhile);
        map.insert("fn", TokenType::KwFn);
        map.insert("return", TokenType::KwRet);
        map.insert("in", TokenType::KwIn);
        map.insert("include", TokenType::KwInclude);
        map.insert("impl", TokenType::KwImpl);
        map.insert("type", TokenType::KwType);
        map.insert("static", TokenType::KwStatic);
        map.insert("match", TokenType::KwMatch);
        map.insert("trait", TokenType::KwTrait);
        map.insert("macro", TokenType::KwMacro);
        map.insert("enum", TokenType::KwEnum);
        map.insert("break", TokenType::KwBreak);
        map.insert("continue", TokenType::KwContinue);
        //map.insert("claim", TokenType::KwClaim);

        map
    };
}

use lygos_token::*;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

pub struct Lexer<'file> {
    file: &'file Path,
    src: Vec<char>,
    curr: char,
    index: usize,
    line_index: usize,
}

impl<'file> Lexer<'file> {
    pub fn new(src: &'file Path) -> Result<Self> {
        let mut file_content = fs::read_to_string(src)?;
        file_content.push('\0');
        let chars = file_content.chars().collect::<Vec<_>>();
        let curr = chars[0];
        Ok(Self{
            file: src,
            src: chars,
            curr,
            index: 0,
            line_index: 0
        })
    }

    pub fn get_tokens(&mut self) -> Vec<Token> {
        let mut tokens = Vec::new();
        loop {
            let token = self.next_token();
            if token.typ == TokenType::Eof {
                tokens.push(token);
                break;
            }
            tokens.push(token);
        }
        tokens
    }

    fn advance(&mut self) {
        self.index += 1;
        self.curr = self.src[self.index];
        self.line_index += 1;
    }

    fn number(&mut self) -> Token {
        let mut value: String = String::new();
        let start = self.line_index;
        while self.curr.is_ascii_digit() || self.curr == '.' {
            value.push(self.curr);
            self.advance();
        }
        let loc = Location::new(self.file.to_path_buf(), start..self.line_index);
        if value.contains(".") {
            return Token::new(TokenType::Float(value), loc);
        }
        Token::new(TokenType::Integer(value), loc)
    }

    fn string(&mut self) -> Token {
        let mut value: String = String::new();
        let start = self.line_index;
        while self.curr != '\"' {
            value.push(self.curr);
            self.advance();
        }
        let loc = Location::new(self.file.to_path_buf(), start..self.line_index);
        self.advance();
        Token::new(TokenType::String(value), loc)
    }

    fn char(&mut self) -> Token {
        let mut value: String = String::new();
        let start = self.line_index;
        while self.curr != '\'' {
            value.push(self.curr);
            self.advance();
        }
        let loc = Location::new(self.file.to_path_buf(), start..self.line_index);
        self.advance();
        Token::new(TokenType::Char(value), loc)
    }

    fn id(&mut self) -> Token {
        let mut value: String = String::new();
        let start = self.line_index;
        while self.curr.is_alphanumeric() || self.curr == '_' {
            value.push(self.curr);
            self.advance();
        }
        let loc = Location::new(self.file.to_path_buf(), start..self.line_index);
        if let Some(typ) = KeyWords.get(&value.as_str()) {
            return Token::new(typ.clone(), loc);
        }

        Token::new(TokenType::Id(value), loc)
    }

    fn advance_token(&mut self, token: Token) -> Token {
        self.advance();
        token
    }

    /*
    *   FIXME(S): multi character values
    */

    fn next_token(&mut self) -> Token {
        while self.curr != '\0' {
            while self.curr.is_whitespace() {
                self.advance();
            }

            let loc = Location::new(self.file.to_path_buf(), self.line_index..self.line_index + 1);
            match self.curr {
                '.' => return self.advance_token(Token::new(TokenType::Dot, loc)),
                ',' => return self.advance_token(Token::new(TokenType::Comma, loc)),
                ';' => return self.advance_token(Token::new(TokenType::Semi, loc)),
                ':' => match self.src[self.index + 1] {
                    ':' => {
                        self.advance();
                        return self.advance_token(Token::new(TokenType::OpScope, loc));
                    }
                    _ => return self.advance_token(Token::new(TokenType::Colon, loc)),
                },
                '+' => return self.advance_token(Token::new(TokenType::OpPlus, loc)),
                '-' => match self.src[self.index + 1] {
                    '>' => {
                        self.advance();
                        return self.advance_token(Token::new(TokenType::Arrow, loc));
                    }
                    _ => return self.advance_token(Token::new(TokenType::OpMinus, loc)),
                },
                '*' => return self.advance_token(Token::new(TokenType::OpMul, loc)),
                '/' => match self.src[self.index + 1] {
                    '/' => {
                        while self.curr != '\n' {
                            self.advance();
                        }
                        continue;
                    },
                    '*' => {
                        let mut braces = 0;
                        loop {
                            if self.curr == '/' && self.src[self.index + 1] == '*' {
                                self.advance();
                                self.advance();
                                braces += 1;
                            }
                            if self.curr == '*' && self.src[self.index + 1] == '/' {
                                self.advance();
                                self.advance();
                                braces -= 1;
                                if braces == 0 {
                                    break;
                                }
                            }
                            self.advance();
                            if self.curr == '\0' {
                                break;
                            }
                        }
                        continue;
                    },
                    _ => return self.advance_token(Token::new(TokenType::OpDiv, loc)),
                },
                '%' => return self.advance_token(Token::new(TokenType::OpMod, loc)),
                '[' => return self.advance_token(Token::new(TokenType::BraceLeft, loc)),
                ']' => return self.advance_token(Token::new(TokenType::BraceRight, loc)),
                '{' => return self.advance_token(Token::new(TokenType::CurlyLeft, loc)),
                '}' => return self.advance_token(Token::new(TokenType::CurlyRight, loc)),
                '(' => return self.advance_token(Token::new(TokenType::ParanLeft, loc)),
                ')' => return self.advance_token(Token::new(TokenType::ParanRight, loc)),
                '#' => return self.advance_token(Token::new(TokenType::Hash, loc)),
                '$' => return self.advance_token(Token::new(TokenType::Dollar, loc)),
                '<' => match self.src[self.index + 1] {
                    '=' => {
                        self.advance();
                        return self.advance_token(Token::new(TokenType::OpLeEq, loc));
                    }
                    _ => return self.advance_token(Token::new(TokenType::AngleLeft, loc)),
                },
                '>' => match self.src[self.index + 1] {
                    '=' => {
                        self.advance();
                        return self.advance_token(Token::new(TokenType::OpGrEq, loc));
                    }
                    _ => {
                        return self.advance_token(Token::new(TokenType::AngleRight, loc))
                    }
                },
                '&' => match self.src[self.index + 1] {
                    '&' => {
                        self.advance();
                        return self.advance_token(Token::new(TokenType::OpAnd, loc));
                    }
                    _ => return self.advance_token(Token::new(TokenType::Ampercent, loc)),
                },
                '|' => match self.src[self.index + 1] {
                    '|' => {
                        self.advance();
                        return self.advance_token(Token::new(TokenType::OpOr, loc));
                    }
                    _ => return self.advance_token(Token::new(TokenType::Pipe, loc)),
                },
                '!' => match self.src[self.index + 1] {
                    '=' => {
                        self.advance();
                        return self.advance_token(Token::new(TokenType::OpNeEq, loc));
                    }
                    _ => return self.advance_token(Token::new(TokenType::Bang, loc)),
                },
                '=' => match self.src[self.index + 1] {
                    '=' => {
                        self.advance();
                        return self.advance_token(Token::new(TokenType::OpEqEq, loc));
                    }
                    _ => return self.advance_token(Token::new(TokenType::Equals, loc)),
                },

                '\"' => {
                    self.advance();
                    return self.string();
                }
                '\'' => {
                    self.advance();
                    return self.char();
                }
                '\0' => return Token::new(TokenType::Eof, loc),
                _ => {
                    if self.curr.is_ascii_digit() {
                        return self.number();
                    }

                    if self.curr.is_alphabetic() {
                        return self.id();
                    }

                    panic!("unknown token found `{}`", self.curr);
                }
            }
        }
        return Token::new(TokenType::Eof, Location::new(PathBuf::new(), self.line_index..self.line_index + 1));
    }
}
