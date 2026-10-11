import print;

// Value execution, syntax generation, declaration mutation and a custom derive.
macro fn square(value: i32) -> i32 {
    return value * value;
}

macro fn twice(value: Ast) -> Ast {
    return value.binary("+", value.clone());
}

macro fn add_tag(item: Ast) {
    item.add_field("tag", "&str");
}

macro fn FieldCount(item: Ast) {
    let name = item.name();
    let count = item.field_count();
    ast::parse_item(
        "impl " + name + " { fn field_count(&self) -> i32 { return "
        + ast::to_string(count) + "; } }"
    ).emit();
}

#[add_tag]
#[derive(FieldCount)]
struct Example { value: i32; }

fn main() -> i32 {
    let example = Example${square$(3), "test"};
    print::println$("fields = {}", example.field_count());
    return 0;
}
