struct T {
    data: *i8;
    len: u64;
}

impl str {
    fn len(&self) -> u64 {
        return ((:*T)&self)->len;
    }

    fn as_ptr(&self) -> *i8 {
        return ((:*T)self)->data;
    }

    fn from_parts(ptr: *i8, len: u64) -> Self {
        let s = T${ ptr, len };
        return (*((:*str)(&s)));
    }

    fn to_string(&self) -> String {
        return String::from(*self);
    }
}

struct String {
    data: *i8;
    len: u32;
    cap: u32;
}

impl String {
    fn new(size: u32) -> String {
        let s = String ${
            .data = malloc(((:u64)size) * sizeof$(i8)),
            .len = (:u32)0,
            .cap = size,
        };
        return s;
    }

    fn from(s: str) -> String {
        let len: u32 = s.len();
        let ptr = s.as_ptr();
        let string = String::new(len);
        memcpy(string.data, ptr, ((:u64)len) * sizeof$(i8));
        string.len = len;
        return string;
    }

    fn grow(&mut self) {
        if self.cap == (:u32)0 {
            self.cap = (:u32)1;
        }
        self.cap = self.cap * (:u32)2;
        self.data = realloc(self.data, ((:u64)self.cap) * sizeof$(i8));
    }

    fn push(&mut self, char: i8) {
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

    fn as_str(&self) -> str {
        return str::from_parts(self.data, (:u64)self.len);
    }
}

fn malloc(size: u64) -> *i8;
fn memcpy(dest: *i8, src: *i8, size: u64) -> *i8;
fn realloc(ptr: *i8, size: u64) -> *i8;

fn main() -> i32 {
    /*let y = [1, 2, 3, 4, 5];
    for let i: u32 = 0 in i < (:u32)10 {
        i = i + (:u32)1;
    }
    return y[4];*/
    return 0;
}
