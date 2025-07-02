use std::{borrow::Cow, ops::{Add, Range}, path::PathBuf};

#[derive(Debug, Clone)]
pub struct Tagged<T> {
    inner: T,
    loc: Location,
}

impl<T> Tagged<T> {
    pub fn new(loc: Location, value: T) -> Tagged<T> {
        Self{ loc, inner: value }
    }

    pub fn inner(&self) -> &T {
        &self.inner
    }

    pub fn inner_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    pub fn loc(&self) -> &Location {
        &self.loc
    }
}

impl From<Token> for Tagged<String> {
    fn from(value: Token) -> Self {
        Self{ inner: value.typ.as_str().to_string(), loc: value.loc }
    }
}

#[derive(Debug, Clone)]
pub struct Location {
    pub file: PathBuf,
    pub range: Range<usize>,
}

impl Location {
    pub fn new(file: impl Into<PathBuf>, range: Range<usize>) -> Self {
        Self{ file: file.into(), range }
    }
}

impl Add for Location {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        assert_eq!(self.file, rhs.file);
        Self{ file: self.file.clone(), range: Range { start: self.range.start, end: rhs.range.end } }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenType {
    String(String),
    Integer(String), //any integer
    Float(String),   //every float is treated as double pres
    Id(String),
    Char(String),

    //general
    Equals,
    Arrow,
    Dot,
    Comma,
    Semi,
    Colon,
    Ampercent,
    Hash,
    Dollar,
    Pipe,
    Bang,

    //operators
    OpPlus,
    OpMinus,
    OpMul,
    OpDiv,
    OpMod,

    OpEqEq,
    OpNeEq,
    OpLeEq,
    OpGrEq,
    OpOr,
    OpAnd,

    OpScope,

    BraceLeft,
    BraceRight,
    CurlyLeft,
    CurlyRight,
    AngleLeft,
    AngleRight,
    ParanLeft,
    ParanRight,

    //keywords
    KwLet,
    KwMut,
    KwConst,
    KwStruct,
    KwIf,
    KwElse,
    KwFor,
    KwWhile,
    KwFn,
    KwRet,
    KwIn,
    KwInclude,
    KwImpl,
    KwType,
    KwStatic,
    KwMatch,
    KwTrait,
    KwMacro,
    KwEnum,
    KwBreak,
    KwContinue,
    //KwDefer
    //KwClaim,

    Eof,
}

impl TokenType {
    pub fn as_str(&self) -> Cow<'static, str> {
        match self {
            TokenType::String(v) => Cow::Owned(v.to_string()),
            TokenType::Integer(v) => Cow::Owned(v.to_string()),
            TokenType::Float(v) => Cow::Owned(v.to_string()),
            TokenType::Id(v) => Cow::Owned(v.to_string()),
            TokenType::Char(v) => Cow::Owned(v.to_string()),
            TokenType::Equals => Cow::Borrowed("="),
            TokenType::Arrow => Cow::Borrowed("->"),
            TokenType::Dot => Cow::Borrowed("."),
            TokenType::Comma => Cow::Borrowed(","),
            TokenType::Semi => Cow::Borrowed(";"),
            TokenType::Colon => Cow::Borrowed(":"),
            TokenType::Ampercent => Cow::Borrowed("&"),
            TokenType::Hash => Cow::Borrowed("#"),
            TokenType::Dollar => Cow::Borrowed("$"),
            TokenType::Pipe => Cow::Borrowed("|"),
            TokenType::Bang => Cow::Borrowed("!"),
            TokenType::OpPlus => Cow::Borrowed("+"),
            TokenType::OpMinus => Cow::Borrowed("-"),
            TokenType::OpMul => Cow::Borrowed("*"),
            TokenType::OpDiv => Cow::Borrowed("/"),
            TokenType::OpMod => Cow::Borrowed("%"),
            TokenType::OpEqEq => Cow::Borrowed("=="),
            TokenType::OpNeEq => Cow::Borrowed("!="),
            TokenType::OpLeEq => Cow::Borrowed("<="),
            TokenType::OpGrEq => Cow::Borrowed(">="),
            TokenType::OpOr => Cow::Borrowed("||"),
            TokenType::OpAnd => Cow::Borrowed("&&"),
            TokenType::OpScope => Cow::Borrowed("::"),
            TokenType::BraceLeft => Cow::Borrowed("["),
            TokenType::BraceRight => Cow::Borrowed("]"),
            TokenType::CurlyLeft => Cow::Borrowed("{"),
            TokenType::CurlyRight => Cow::Borrowed("}"),
            TokenType::AngleLeft => Cow::Borrowed("<"),
            TokenType::AngleRight => Cow::Borrowed(">"),
            TokenType::ParanLeft => Cow::Borrowed("("),
            TokenType::ParanRight => Cow::Borrowed(")"),
            TokenType::KwLet => Cow::Borrowed("let"),
            TokenType::KwMut => Cow::Borrowed("mut"),
            TokenType::KwConst => Cow::Borrowed("const"),
            TokenType::KwStruct => Cow::Borrowed("struct"),
            TokenType::KwIf => Cow::Borrowed("if"),
            TokenType::KwElse => Cow::Borrowed("else"),
            TokenType::KwFor => Cow::Borrowed("for"),
            TokenType::KwWhile => Cow::Borrowed("while"),
            TokenType::KwFn => Cow::Borrowed("fn"),
            TokenType::KwRet => Cow::Borrowed("return"),
            TokenType::KwIn => Cow::Borrowed("in"),
            TokenType::KwInclude => Cow::Borrowed("include"),
            TokenType::KwImpl => Cow::Borrowed("impl"),
            TokenType::KwType => Cow::Borrowed("type"),
            TokenType::KwStatic => Cow::Borrowed("static"),
            TokenType::KwMatch => Cow::Borrowed("match"),
            TokenType::KwTrait => Cow::Borrowed("trait"),
            TokenType::KwMacro => Cow::Borrowed("macro"),
            TokenType::KwEnum => Cow::Borrowed("enum"),
            TokenType::KwBreak => Cow::Borrowed("break"),
            TokenType::KwContinue => Cow::Borrowed("continue"),
            TokenType::Eof => Cow::Borrowed("Eof"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Token {
    pub typ: TokenType,
    pub loc: Location
}

impl Token {
    pub fn new(typ: TokenType, loc: Location) -> Self {
        Self{ typ, loc }
    }
}
