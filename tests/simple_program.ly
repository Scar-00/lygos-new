/*type size_t = u64;

static true: bool = (:bool)1;
static false: bool = (:bool)0;

fn isdigit(c: i32) -> i32;
fn isalpha(c: i32) -> i32;
fn isspace(c: i32) -> i32;

fn exit(code: i32);

fn malloc(size: size_t) -> *mut i8;
fn realloc(ptr: *mut i8, size: size_t) -> *mut i8;
fn calloc(count: size_t, size: size_t) -> *mut i8;
fn free(block: *mut i8);
fn memcpy(dest: *mut i8, src: *mut i8, size: size_t) -> *mut i8;
fn memmove(dest: *mut i8, src: *mut i8, len: size_t) -> *mut i8;
fn memset(ptr: *mut i8, v: i32, n: size_t) -> *mut i8;
fn strlen(str: *mut i8) -> u32;
fn strcpy(dest: *mut i8, src: *mut i8) -> *mut i8;

type FileDescriptor = *mut i8;
fn fopen(filename: *mut i8, modes: *mut i8) -> FileDescriptor;
fn fclose(stream: FileDescriptor) -> i32;
fn fseek(stream: FileDescriptor, off: u64, whence: i32) -> i32;
fn ftell(stream: FileDescriptor) -> u64;
fn rewind(stream: FileDescriptor);
fn fread(ptr: *mut i8, size: u64, n: u64, stream: FileDescriptor) -> u64;

trait Drop {
    fn drop(&mut self);
}

trait Clone {
    fn clone(&self) -> Self;
}

trait Debug {
    fn fmt_debug(&self, fmt: &mut Formatter) -> FormattingError;
}

trait Display {
    fn fmt(&self, fmt: &mut Formatter) -> FormattingError;
}

trait Hash {
    fn hash(&self) -> size_t;
}

/* trait Alloc {
    fn alloc<T>(count: u64);
    fn realloc<T>(ptr: *mut T, new_size: u64);
    fn free<T>(ptr: *mut T);
}*/

struct T {
    data: *mut i8;
    len: u64;
}

impl str {
    fn len(&self) -> u64 {
        return ((:*mut T)&self)->len;
    }

    fn as_ptr(&self) -> *mut i8 {
        return ((:*mut T)&self)->data;
    }

    fn to_string(&self) -> String {
        return String::from(self);
    }
}

impl Hash for str {
    fn hash(&self) -> size_t {
        let p: u32  = 53;
        let m: u32 = 561634546;
        let mut hash_value: size_t = 0;
        let mut pow: size_t = 1;
        let ptr = self.as_ptr();
        for let mut i: size_t = 0 in i < (:size_t)self.len() {
            hash_value = (hash_value + ((:size_t)(ptr[i] - 'a' + (:i8)1)) * pow) % (:size_t)m;
            pow = (pow * (:size_t)p) % (:size_t)m;
            i = i + (:size_t)1;
        }
        return hash_value;
    }
}

struct String {
    data: *mut i8;
    len: u32;
    cap: u32;
}

impl Drop for String {
    fn drop(&mut self) {
        if self.data != (:*mut i8)0 {
            free(self.data);
        }
    }
}

impl String {
    fn new(size: u32) -> String {
        let s  = String${
            .data = malloc(((:size_t)size) * sizeof$(i8)),
            .len = (:u32)0,
            .cap = size,
        };
        return s;
    }

    fn from(s: &str) -> String {
        let len: u32 = s.len();
        let ptr = s.as_ptr();
        let mut string = String::new(len);
        memcpy(string.data, ptr, ((:size_t)len) * sizeof$(i8));
        string.len = len;
        return string;
    }

    fn grow(&mut self) {
        if self.cap == (:u32)0 {
            self.cap = (:u32)1;
        }
        self.cap = self.cap * (:u32)2;
        self.data = realloc(self.data, ((:size_t)self.cap) * sizeof$(i8));
    }

    fn push(&mut self, char: i8) {
        if self.len == self.cap {
            self.grow();
        }
        let len = self.len;
        self.data[len] = char;
        self.len = self.len + (:u32)1;
    }

    fn push_str(&mut self, other: &str) {
        let len: u32 = other.len();
        let ptr = other.as_ptr();
        for let mut i: u32 = 0 in i < len {
            self.push(ptr[i]);
            i = i + (:u32)1;
        }
        return;
    }

    fn eq(&self, other: String) -> bool {
        if self.len != other.len {
            return (:bool)0;
        }
        for let mut i: u32 = 0 in i < self.len {
            if self.data[i] != other.data[i] {
                return (:bool)0;
            }
            i = i + (:u32)1;
        }
        return (:bool)1;
    }

    fn eq_ptr(&self, ptr: &str) -> bool {
        let mut other = String::from(ptr);
        let eq = self.eq(other);
        other.drop();
        return eq;
    }

    fn contains(&self, matchee: i8) -> bool {
        for let mut i: u32 = 0 in i < self.len {
            let data = self.data;
            if data[i] == matchee {
                return (:bool)1;
            }
            i = i + (:u32)1;
        }
        return (:bool)0;
    }
}

impl Clone for String {
    fn clone(&self) -> String {
        let mut this = String::new(self.cap);
        let size: i32 = sizeof$(i8);
        this.len = self.len;
        strcpy(this.data, self.data);
        return this;
    }
}

fn int_to_str(buffer: *mut i8, i: i64, base: i32) -> i64;
fn ptr_to_str(buffer: *mut i8, buf_size: u64, ptr: *mut i8) -> i32;

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
    pieces: *mut &str;
    pieces_count: u32;
}

impl Arguments {
    fn new(args: *mut Argument, args_count: u32, pieces: *mut &str, pieces_count: u32) -> Self {
        let this = Self${ args, args_count, pieces, pieces_count };
        return this;
    }

    fn estimate_length(&self) -> u32 {
        let mut len: u32 = 0;
        for let mut i: u32 = 0 in i < self.pieces_count {
            let piece = self.pieces[i];
            len = len + (:u32)piece.len();
            i = i + (:u32)1;
        }
        return len;
    }
}

fn format_(args: Arguments) -> String {
    let len = args.estimate_length();
    let mut buf = String::new(len);
    let mut formatter = Formatter::new(&mut buf);

    let mut idx = 0;
    for let mut i: u32 = 0 in i < args.arg_count {
        let arg = args.args[i];
        formatter.write_str(args.pieces[i]);
        arg.fmt(&mut formatter);
        i = i + (:u32)1;
        idx = idx + 1;
    }


    if ((:u32)idx) < args.pieces_count {
        formatter.write_str(args.pieces[idx]);
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

    fn finish(&self) {
        if self.has_fields != (:bool)0 {
            let fmt = self.formatter;
            fmt.write_str(" }");
        }
    }
}

fn i64_fmt(i: &i64, fmt: &mut Formatter) -> FormattingError {
    let mut buf = [(:i8)0; 24];
    let ptr = &mut buf[0];
    let len: u64 = int_to_str(ptr, i, 10);
    let s = str::from_parts(ptr, len);
    return fmt.write_str(s);
}

fn i32_fmt(v: &i32, fmt: &mut Formatter) -> FormattingError {
    let val = (:i64)v;
    return i64_fmt(&val, fmt);
}

fn i16_fmt(v: &i16, fmt: &mut Formatter) -> FormattingError {
    let val = (:i64)v;
    return i64_fmt(&val, fmt);
}

fn i8_fmt(v: &i8, fmt: &mut Formatter) -> FormattingError {
    let val = (:i64)v;
    return i64_fmt(&val, fmt);
}

fn u64_fmt(v: &u64, fmt: &mut Formatter) -> FormattingError {
    let val = (:i64)v;
    return i64_fmt(&val, fmt);
}

fn u32_fmt(v: &u32, fmt: &mut Formatter) -> FormattingError {
    let val = (:i64)v;
    return i64_fmt(&val, fmt);
}

fn u16_fmt(v: &u16, fmt: &mut Formatter) -> FormattingError {
    let val = (:i64)v;
    return i64_fmt(&val, fmt);
}

fn u8_fmt(v: &u32, fmt: &mut Formatter) -> FormattingError {
    let val = (:i64)v;
    return i64_fmt(&val, fmt);
}

fn bool_fmt(v: &bool, fmt: &mut Formatter) -> FormattingError {
    let mut s = "false";
    if *v != (:bool)0 {
        s = "true";
    }
    return fmt.write_str(s);
}

fn ptr_fmt(v: &i8, fmt: &mut Formatter) -> FormattingError {
    let mut buf = [(:i8)0; 64];
    let ptr = &mut buf[0];
    let len: u64 = ptr_to_str(ptr, (:u64)64, (:*mut i8)v);
    let s = str::from_parts(ptr, len);
    return fmt.write_str(s);
}

impl Display for str {
    fn fmt(&self, fmt: &mut Formatter) -> FormattingError {
        return fmt.write_str(self);
    }
}

impl Display for String {
    fn fmt(&self, fmt: &mut Formatter) -> FormattingError {
        let s = self.as_str();
        return fmt.write_str(s);
    }
}

impl Debug for String {
    fn fmt_debug(&self, fmt: &mut Formatter) -> FormattingError {
        fmt.debug_struct("String")
            .field("content", format_args$("{}", self.as_str()))
            .field("len", format_args$("{}", self.len))
            .field("cap", format_args$("{}", self.cap))
            .finish();
    }
}

/* macro format {
    (fmt: $, args: $[]) -> {
        format_(format_args$($fmt, $args));
    }
}*/

enum ProcDescriptor {
    Stdin,
    Stdout,
    Stderr,
}

fn write(file: u32, ptr: *mut i8, len: u64) -> i64;

fn print_(s: &str) {
    write((:u32)ProcDescriptor::Stdout, s.as_ptr(), s.len());
}

fn eprint_(s: &str) {
    write((:u32)ProcDescriptor::Stderr, s.as_ptr(), s.len());
}

fn print_fmt_(args: Arguments) {
    let buf = format_(args);
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

macro panic {
    () -> {
        eprint_("\n");
        exit(1);
    }
    (string: $) -> {
        eprint_($string);
        eprint_("\n");
        exit(1);
    }
    (fmt: $, args: $[]) -> {
        eprint_fmt_(format_args$($fmt, $args));
        eprint_("\n");
        exit(1);
    }
}

macro todo {
    () -> {
        eprint_("[TODO]");
        exit(1);
    }

    (string: $) -> {
        eprint_("[TODO]: ");
        eprint_($string);
        eprint_("\n");
        exit(1);
    }

    (fmt: $, args: $[]) -> {
        eprint_("[TODO]: ");
        eprint_fmt_(format_args$($fmt, $args));
        eprint_("\n");
        exit(1);
    }
}


*/



fn main(argc: i32) -> i32 {
    let b = argc == 10 || (argc == 100 || argc == 20);

    return 10;
}

/* struct Vec2 {
    x: i32;
    y: i32;
}

impl Vec2 {
    fn new() -> Self {
        return .Self {
            x: 20,
            y: 0
        };
    }
}

fn add(a: i32, b: i32) -> Vec2 {
    return Vec2$ {
        x: a,
        y: b,
    };
}*/
