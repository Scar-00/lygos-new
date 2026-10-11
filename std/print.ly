// Formatting runtime extracted from tests/simple_program.ly.
// String's data/length prefix is the descriptor borrowed by &str.
fn malloc(size:u64)->*mut i8;
fn realloc(data:*mut i8,size:u64)->*mut i8;
fn free(data:*mut i8);
struct String { data:*mut i8; len:u64; cap:u64; }
impl String {
    fn new(size:u64)->Self { return Self${malloc(size),(:u64)0,size}; }
    fn grow(&mut self) {
        if self.cap==(:u64)0 { self.cap=(:u64)1; }
        self.cap=self.cap*(:u64)2;
        self.data=realloc(self.data,self.cap);
    }
    fn push(&mut self,char:i8) {
        if self.len==self.cap { self.grow(); }
        self.data[self.len]=char; self.len=self.len+(:u64)1;
    }
    fn push_str(&mut self,other:&str) {
        for let mut i:u64=0 in i<other.len() { self.push(other.as_ptr()[i]); i=i+(:u64)1; }
    }
    fn as_str(&self)->&str { return (:&str)self; }
    fn drop(&mut self) { free(self.data); }
}

enum FormattingError {
    None,
    T,
}

struct DebugStructBuilder {
    formatter: &mut Formatter;
    has_fields: bool;
}

struct Formatter {
    buf: &mut String;
}

impl Formatter {
    fn new(buffer: &mut String) -> Self {
        let this = Self${ buffer };
        return this;
    }

    fn write_str(&mut self, slice: &str) -> FormattingError {
        self.buf.push_str(slice);
        return FormattingError::None;
    }

    fn debug_struct(&mut self, name: &str) -> DebugStructBuilder {
        self.write_str(name);
        return DebugStructBuilder${
            self,
            (:bool)0
        };
    }
}

struct Argument {
    value: &i8;
    formatter: fn(&i8, &mut Formatter) -> FormattingError;
}

impl Argument {
    fn fmt(&self, fmt: &mut Formatter) -> FormattingError {
        return (self.formatter)(self.value, fmt);
    }
}

/*
    [Info]: this structure stores references to local variables
            returning it from a function will lead to UB(undefined behavior)
*/
struct Arguments {
    args: *mut Argument;
    arg_count: u32;
    pieces: *mut str;
    pieces_count: u32;
}

impl Arguments {
    fn new(args: *mut Argument, args_count: u32, pieces: *mut str, pieces_count: u32) -> Self {
        let this = Self${ args, args_count, pieces, pieces_count };
        return this;
    }

    fn estimate_length(&self) -> u64 {
        let mut len: u64 = 0;
        for let mut i: u32 = 0 in i < self.pieces_count {
            let piece = &self.pieces[i];
            len = len + piece.len();
            i = i + (:u32)1;
        }
        return len;
    }
}

fn format_(args: Arguments) -> String {
    let len = args.estimate_length();
    let mut buf = String::new(len);
    let mut formatter = Formatter::new(&mut buf);

    let mut idx:u32 = 0;
    for let mut i: u32 = 0 in i < args.arg_count {
        let arg = args.args[i];
        formatter.write_str(&args.pieces[i]);
        arg.fmt(&mut formatter);
        i = i + (:u32)1;
        idx = idx + (:u32)1;
    }


    if ((:u32)idx) < args.pieces_count {
        formatter.write_str(&args.pieces[idx]);
    }
    return buf;
}

impl DebugStructBuilder {
    fn field(&mut self, name: &str, args: Arguments) -> &mut Self {
        let fmt = self.formatter;
        let mut prefix = "{ ";
        if self.has_fields == (:bool)1 {
            prefix = ", ";
        }
        fmt.write_str(prefix);
        fmt.write_str(name);
        fmt.write_str(": ");
        let arg = args.args[0];
        (arg.formatter)(arg.value, fmt);
        self.has_fields = (:bool)1;
        return self;
    }

    fn finish(&mut self) {
        if self.has_fields != (:bool)0 {
            let fmt = self.formatter;
            fmt.write_str(" }");
        }
    }
}


// Convert the full unsigned range, including the magnitude of i64::MIN.
fn unsigned_fmt(value:u64,negative:bool,fmt:&mut Formatter)->FormattingError {
    let mut digits=[(:i8)0;20];
    let mut count:u64=0;
    let mut n=value;
    for let pending=0 in count<(:u64)20 {
        digits[count]=(:i8)(n%(:u64)10)+(:i8)48;
        count=count+(:u64)1; n=n/(:u64)10;
        if n==(:u64)0 { break; }
    }
    if negative { fmt.buf.push('-'); }
    for let pending=0 in count>(:u64)0 {
        count=count-(:u64)1; fmt.buf.push(digits[count]);
    }
    return FormattingError::None;
}
fn i64_fmt(value:&i64,fmt:&mut Formatter)->FormattingError {
    let mut magnitude=(:u64)*value;
    let negative=*value<(:i64)0;
    if negative { magnitude=(:u64)0-magnitude; }
    return unsigned_fmt(magnitude,negative,fmt);
}
fn u64_fmt(value:&u64,fmt:&mut Formatter)->FormattingError { return unsigned_fmt(*value,(:bool)0,fmt); }
fn i32_fmt(value:&i32,fmt:&mut Formatter)->FormattingError { let n=(:i64)*value; return i64_fmt(&n,fmt); }
fn i16_fmt(value:&i16,fmt:&mut Formatter)->FormattingError { let n=(:i64)*value; return i64_fmt(&n,fmt); }
fn i8_fmt(value:&i8,fmt:&mut Formatter)->FormattingError { let n=(:i64)*value; return i64_fmt(&n,fmt); }
fn u32_fmt(value:&u32,fmt:&mut Formatter)->FormattingError { let n=(:u64)*value; return u64_fmt(&n,fmt); }
fn u16_fmt(value:&u16,fmt:&mut Formatter)->FormattingError { let n=(:u64)*value; return u64_fmt(&n,fmt); }
fn u8_fmt(value:&u8,fmt:&mut Formatter)->FormattingError { let n=(:u64)*value; return u64_fmt(&n,fmt); }
fn bool_fmt(v: &bool, fmt: &mut Formatter) -> FormattingError {
    let mut s = "false";
    if *v != (:bool)0 {
        s = "true";
    }
    return fmt.write_str(s);
}

fn str_fmt(value:&str,fmt:&mut Formatter)->FormattingError { return fmt.write_str(value); }
fn ptr_fmt(value:&i8,fmt:&mut Formatter)->FormattingError { let address=(:u64)value; return u64_fmt(&address,fmt); }
impl String {
    fn fmt(&self,fmt:&mut Formatter)->FormattingError { return fmt.write_str(self.as_str()); }
    fn fmt_debug(&self,fmt:&mut Formatter)->FormattingError {
        let mut debug=fmt.debug_struct("String");
        debug.field("content",format_args$("{}",self.as_str()));
        debug.field("len",format_args$("{}",self.len));
        debug.field("cap",format_args$("{}",self.cap));
        debug.finish();
        return FormattingError::None;
    }
}
enum ProcDescriptor {
    Stdin,
    Stdout,
    Stderr,
}

fn write(file: u32, ptr: *const i8, len: u64) -> i64;

fn print_(s: &str) {
    write((:u32)ProcDescriptor::Stdout, s.as_ptr(), s.len());
}

fn eprint_(s: &str) {
    write((:u32)ProcDescriptor::Stderr, s.as_ptr(), s.len());
}

fn print_fmt_(args: Arguments) {
    let mut buf = format_(args);
    let s = buf.as_str();
    write((:u32)ProcDescriptor::Stdout, s.as_ptr(), s.len());
    buf.drop();
}

fn eprint_fmt_(args: Arguments) {
    let mut buf = format_(args);
    let s = buf.as_str();
    write((:u32)ProcDescriptor::Stderr, s.as_ptr(), s.len());
    buf.drop();
}
macro println {
    () -> {
        print_("\n");
    }
    (string: $) -> {
        print_($string);
        print_("\n");
    }
    (fmt: $, args: $[]) -> {
        print_fmt_(format_args$($fmt, $args));
        print_("\n");
    }
}

macro print {
    (string: $) -> {
        print_($string);
    }
    (fmt: $, args: $[]) -> {
        print_fmt_(format_args$($fmt, $args));
    }
}

