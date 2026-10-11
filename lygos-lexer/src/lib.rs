use std::{char, collections::HashMap, fs, path::Path};

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
        map.insert("import", TokenType::KwImport);
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
    #[error("{message}")]
    Lexical { loc: Location, message: String },
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
        let file_content = fs::read_to_string(src)?;
        let mut lexer = Self::from(file_content.chars().collect());
        lexer.file = src;
        Ok(lexer)
    }

    pub fn from(mut chars: Vec<char>) -> Self {
        if chars.last() != Some(&'\0') {
            chars.push('\0');
        }
        let curr = chars[0];
        Self {
            file: Path::new(""),
            src: chars,
            curr,
            index: 0,
            line_index: 0,
        }
    }

    pub fn get_tokens(&mut self) -> Vec<Token> {
        self.try_get_tokens()
            .unwrap_or_else(|error| panic!("{error}"))
    }

    pub fn try_get_tokens(&mut self) -> Result<Vec<Token>> {
        let mut tokens = Vec::new();
        loop {
            let token = self.next_token()?;
            if token.typ == TokenType::Eof {
                tokens.push(token);
                break;
            }
            tokens.push(token);
        }
        Ok(tokens)
    }

    fn advance(&mut self) {
        if self.curr != '\0' {
            self.index += 1;
            self.curr = self.src[self.index];
            self.line_index += 1;
        }
    }

    fn number(&mut self) -> Token {
        let mut value: String = String::new();
        let start = self.line_index;
        while self.curr.is_ascii_digit() {
            value.push(self.curr);
            self.advance();
        }
        if self.curr == '.'
            && self
                .src
                .get(self.index + 1)
                .is_some_and(|ch| ch.is_ascii_digit())
        {
            value.push('.');
            self.advance();
            while self.curr.is_ascii_digit() {
                value.push(self.curr);
                self.advance();
            }
        }
        let loc = Location::new(self.file.to_path_buf(), start..self.line_index);
        if value.contains(".") {
            return Token::new(TokenType::Float(value), loc);
        }
        Token::new(TokenType::Integer(value), loc)
    }

    fn quoted(&mut self, delimiter: char) -> Result<Token> {
        let mut value = String::new();
        let start = self.line_index - 1;
        while self.curr != delimiter {
            if self.curr == '\0' {
                return self.lexical_error(start, "unterminated literal");
            }
            if self.curr == '\\' {
                value.push(self.curr);
                self.advance();
                if self.curr == '\0' {
                    return self.lexical_error(start, "unterminated escape");
                }
            }
            value.push(self.curr);
            self.advance();
        }
        self.advance();
        let loc = Location::new(self.file.to_path_buf(), start..self.line_index);
        let typ = if delimiter == '"' {
            TokenType::String(value)
        } else {
            TokenType::Char(value)
        };
        Ok(Token::new(typ, loc))
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
        let mut token = token;
        token.loc.range.end = self.line_index;
        token
    }

    /*
     *   FIXME(S): multi character values
     */

    fn lexical_error<T>(&self, start: usize, message: &str) -> Result<T> {
        Err(Error::Lexical {
            loc: Location::new(
                self.file.to_path_buf(),
                start..self.line_index.max(start + 1),
            ),
            message: message.into(),
        })
    }

    fn next_token(&mut self) -> Result<Token> {
        while self.curr != '\0' {
            while self.curr.is_whitespace() {
                self.advance();
            }

            let loc = Location::new(
                self.file.to_path_buf(),
                self.line_index..self.line_index + 1,
            );
            match self.curr {
                '.' => return Ok(self.advance_token(Token::new(TokenType::Dot, loc))),
                ',' => return Ok(self.advance_token(Token::new(TokenType::Comma, loc))),
                ';' => return Ok(self.advance_token(Token::new(TokenType::Semi, loc))),
                ':' => match self.src[self.index + 1] {
                    ':' => {
                        self.advance();
                        return Ok(self.advance_token(Token::new(TokenType::OpScope, loc)));
                    }
                    _ => return Ok(self.advance_token(Token::new(TokenType::Colon, loc))),
                },
                '+' => return Ok(self.advance_token(Token::new(TokenType::OpPlus, loc))),
                '-' => match self.src[self.index + 1] {
                    '>' => {
                        self.advance();
                        return Ok(self.advance_token(Token::new(TokenType::Arrow, loc)));
                    }
                    _ => return Ok(self.advance_token(Token::new(TokenType::OpMinus, loc))),
                },
                '*' => return Ok(self.advance_token(Token::new(TokenType::OpMul, loc))),
                '/' => match self.src[self.index + 1] {
                    '/' => {
                        while self.curr != '\n' && self.curr != '\0' {
                            self.advance();
                        }
                        continue;
                    }
                    '*' => {
                        let start = self.line_index;
                        self.advance();
                        self.advance();
                        let mut depth = 1;
                        while depth > 0 {
                            if self.curr == '\0' {
                                return self.lexical_error(start, "unterminated block comment");
                            }
                            if self.curr == '/' && self.src[self.index + 1] == '*' {
                                self.advance();
                                self.advance();
                                depth += 1;
                            } else if self.curr == '*' && self.src[self.index + 1] == '/' {
                                self.advance();
                                self.advance();
                                depth -= 1;
                            } else {
                                self.advance();
                            }
                        }
                        continue;
                    }
                    _ => return Ok(self.advance_token(Token::new(TokenType::OpDiv, loc))),
                },
                '%' => return Ok(self.advance_token(Token::new(TokenType::OpMod, loc))),
                '[' => return Ok(self.advance_token(Token::new(TokenType::BraceLeft, loc))),
                ']' => return Ok(self.advance_token(Token::new(TokenType::BraceRight, loc))),
                '{' => return Ok(self.advance_token(Token::new(TokenType::CurlyLeft, loc))),
                '}' => return Ok(self.advance_token(Token::new(TokenType::CurlyRight, loc))),
                '(' => return Ok(self.advance_token(Token::new(TokenType::ParanLeft, loc))),
                ')' => return Ok(self.advance_token(Token::new(TokenType::ParanRight, loc))),
                '#' => return Ok(self.advance_token(Token::new(TokenType::Hash, loc))),
                '$' => return Ok(self.advance_token(Token::new(TokenType::Dollar, loc))),
                '<' => match self.src[self.index + 1] {
                    '=' => {
                        self.advance();
                        return Ok(self.advance_token(Token::new(TokenType::OpLeEq, loc)));
                    }
                    _ => return Ok(self.advance_token(Token::new(TokenType::AngleLeft, loc))),
                },
                '>' => match self.src[self.index + 1] {
                    '=' => {
                        self.advance();
                        return Ok(self.advance_token(Token::new(TokenType::OpGrEq, loc)));
                    }
                    _ => {
                        return Ok(self.advance_token(Token::new(TokenType::AngleRight, loc)));
                    }
                },
                '&' => match self.src[self.index + 1] {
                    '&' => {
                        self.advance();
                        return Ok(self.advance_token(Token::new(TokenType::OpAnd, loc)));
                    }
                    _ => return Ok(self.advance_token(Token::new(TokenType::Ampercent, loc))),
                },
                '|' => match self.src[self.index + 1] {
                    '|' => {
                        self.advance();
                        return Ok(self.advance_token(Token::new(TokenType::OpOr, loc)));
                    }
                    _ => return Ok(self.advance_token(Token::new(TokenType::Pipe, loc))),
                },
                '!' => match self.src[self.index + 1] {
                    '=' => {
                        self.advance();
                        return Ok(self.advance_token(Token::new(TokenType::OpNeEq, loc)));
                    }
                    _ => return Ok(self.advance_token(Token::new(TokenType::Bang, loc))),
                },
                '=' => match self.src[self.index + 1] {
                    '=' => {
                        self.advance();
                        return Ok(self.advance_token(Token::new(TokenType::OpEqEq, loc)));
                    }
                    _ => return Ok(self.advance_token(Token::new(TokenType::Equals, loc))),
                },

                '\"' => {
                    self.advance();
                    return self.quoted('"');
                }
                '\'' => {
                    self.advance();
                    return self.quoted('\'');
                }
                '\0' => return Ok(Token::new(TokenType::Eof, loc)),
                _ => {
                    if self.curr.is_ascii_digit() {
                        return Ok(self.number());
                    }

                    if self.curr.is_alphabetic() || self.curr == '_' {
                        return Ok(self.id());
                    }

                    return self
                        .lexical_error(self.line_index, &format!("unknown token `{}`", self.curr));
                }
            }
        }
        return Ok(Token::new(
            TokenType::Eof,
            Location::new(self.file.to_path_buf(), self.line_index..self.line_index),
        ));
    }
}
