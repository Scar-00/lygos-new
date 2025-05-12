struct Foo {
    x: i32;
    y: i32;
}

fn malloc(size: i32) -> *Foo;

fn main() -> i32 {
    let f = Foo {
        .x = 20,
        .y = 10,
    };
    return 0;
}
