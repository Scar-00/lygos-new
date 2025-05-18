struct Foo {
    x: i32;
    y: i32;
}

/*impl Foo {
    fn test(&self) -> i32 {
        return self.y;
    }
}*/

type Bar = Foo;

enum FooBar {
    Test,
    Test1,
}

fn malloc(size: i32) -> *Bar;

fn main() -> i32 {
    let y = [1, 2, 3, 4, 5];
    return y[0];
}
