struct Foo {
    x: i32;
    y: i32;
}

type Bar = Foo;

fn malloc(size: i32) -> *Bar;

fn foo(x: i32) -> i32 {
    if x == 10 {
        return 1;
    }
    return 0;
}

fn main() -> i32 {
    return 0;
}
