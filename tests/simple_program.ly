/*struct T {
    data: *u8;
    len: u64;
}

impl str {
    fn len(&self) -> u64 {
        return ((:*T)&self)->len;
    }

    fn as_ptr(&self) -> *u8 {
        return ((:*T)self)->data;
    }

    fn from_parts(ptr: *u8, len: u64) -> Self {
        let s = T${ ptr, len };
        return (*((:*str)(&s)));
    }

    fn to_string(&self) -> String {
        return String::from(*self);
    }
}

struct String {
    data: *u8;
    len: u32;
    cap: u32;
}

impl String {
    fn new(size: u32) -> String {
        let s = String ${
            .data = malloc(((:u64)size) * sizeof$(u8)),
            .len = (:u32)0,
            .cap = size,
        };
        return s;
    }

    fn from(s: str) -> String {
        let len: u32 = s.len();
        let ptr = s.as_ptr();
        let string = String::new(len);
        memcpy(string.data, ptr, ((:u64)len) * sizeof$(u8));
        string.len = len;
        return string;
    }

    fn grow(&mut self) {
        if self.cap == (:u32)0 {
            self.cap = (:u32)1;
        }
        self.cap = self.cap * (:u32)2;
        self.data = realloc(self.data, ((:u64)self.cap) * sizeof$(u8));
    }

    fn push(&mut self, char: u8) {
        if self.len == self.cap {
            self.grow();
        }
        self.data[self.len] = char;
        self.len = self.len + (:u32)1;
    }

    fn push_str(&mut self, other: str) {
        let len: u32 = other.len();
        let ptr = other.as_ptr();
        for let i: u32 = 0 in i < len {
            self.push(ptr[i]);
            i = i + (:u32)1;
        }
        return;
    }

    fn eq(&self, other: String) -> bool {
        if self.len != other.len {
            return (:bool)0;
        }
        for let i: u32 = 0 in i < self.len {
            if self.data[i] != other.data[i] {
                return (:bool)0;
            }
            i = i + (:u32)1;
        }
        return (:bool)1;
    }

    fn eq_ptr(&self, ptr: str) -> bool {
        let other = String::from(ptr);
        let eq = self.eq(other);
        //other.drop();
        return eq;
    }

    fn contains(&self, matchee: u8) -> bool {
        for let mut i: u32 = 0 in i < self.len {
            let data = self.data;
            if data[i] == matchee {
                return (:bool)1;
            }
            i = i + (:u32)1;
        }
        return (:bool)0;
    }

    fn as_str(&self) -> str {
        return str::from_parts(self.data, (:u64)self.len);
    }
}

enum FormattingError {
    None,
    T,
}

struct Formatter {
    buf: &String;
}

impl Formatter {
    fn new(buffer: &String) -> Self {
        return Formatter ${ &buffer };
    }

    fn write_str(&self, slice: str) -> FormattingError {
        let buf: &String = self.buf;
        buf.push_str(slice);
        return FormattingError::None;
    }

    /*fn debug_struct(&self, name: str) -> DebugStructBuilder {
        self.write_str(name);
        let this: DebugStructBuilder = { &self, (:bool)0 };
        return this;
    }*/
}

struct Argument {
    value: &i8;
    formatter: fn(&i8, &Formatter) -> FormattingError;
}

impl Argument {
    fn fmt(&self, fmt: &mut Formatter) -> FormattingError {
        (self.formatter)(self.value, fmt);
    }
}

struct Arguments {
    args: *Argument;
    arg_count: u32;
    pieces: *str;
    pieces_count: u32;
}

impl Arguments {
    fn new(args: *Argument, args_count: u32, pieces: *str, pieces_count: u32) -> Self {
        return Arguments ${ args, args_count, pieces, pieces_count };
    }

    fn estimate_length(&self) -> u32 {
        let mut len: u32 = 0;
        for let i: u32 = 0 in i < self.pieces_count {
            let piece = self.pieces[i];
            len = len + (:u32)piece.len();
            i = i + (:u32)1;
        }
        return len;
    }
}

enum ProcDescriptor {
    Stdin,
    Stdout,
    Stderr,
}

fn format_(args: Arguments) -> String {
    let len = args.estimate_length();
    let buf = String::new(len);
    let formatter = Formatter::new(&buf);


    let idx = 0;
    for let i: u32 = 0 in i < args.arg_count {
        let arg = args.args[i];
        formatter.write_str(args.pieces[i]);
        arg.fmt(&formatter);
        i = i + (:u32)1;
        idx = idx + 1;
    }


    if ((:u32)idx) < args.pieces_count {
        formatter.write_str(args.pieces[idx]);
    }
    return buf;
}

fn write(file: u32, ptr: *u8, len: u64) -> i64;

fn print_(s: str) {
    write((:u32)ProcDescriptor::Stdout, s.as_ptr(), s.len());
}

fn print_fmt_(args: Arguments) {
    let buf = format_(args);
    let s = buf.as_str();
    write((:u32)ProcDescriptor::Stdout, s.as_ptr(), s.len());
    //buf.drop();
}

fn malloc(size: u64) -> *u8;
fn memcpy(dest: *u8, src: *u8, size: u64) -> *u8;
fn realloc(ptr: *u8, size: u64) -> *u8;

fn main() -> i32 {
    /*let y = [1, 2, 3, 4, 5];
    for let i: u32 = 0 in i < (:u32)10 {
        i = i + (:u32)1;
    }
    return y[4];*/
    format_args$("test");
    //print_("test1");
    return 10;
}*/

fn add(a: i32, b: i32) -> i32 {
    return a + b;
}
