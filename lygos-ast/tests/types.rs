use lygos_ast::types::{Array, FuncPtr, Type};
use lygos_token::{Location, Tagged};

fn ty(name: &str) -> Type {
    Type::from_string(Tagged::new(Location::new("", 0..0), name.into())).unwrap()
}

#[test]
fn parses_nested_pointer_and_reference_types() {
    assert_eq!(
        ty("*mut &mut *const i32").get_full_name(),
        "*mut &mut *const i32"
    );
    assert_eq!(ty("&mut mutable").get_full_name(), "&mut mutable");
    assert_eq!(ty("&mutable").get_full_name(), "&mutable");
    for name in [
        "", "*", "*i32", "*mut", "*const", "&mut", "*[]", "12", "i32 junk",
    ] {
        assert!(
            Type::from_string(Tagged::new(Location::new("", 0..0), name.into())).is_none(),
            "{name}"
        );
    }
}

#[test]
fn compares_structure_and_qualifiers() {
    assert!(ty("*const i32").matches(&ty("*const i32")));
    assert!(!ty("*const i32").matches(&ty("&i32")));
    assert!(!ty("&i32").matches(&ty("&mut i32")));
    assert!(!ty("*const i32").matches(&ty("*mut i32")));
    assert_eq!(ty("*const *mut i32").get_full_name(), "*const *mut i32");
    assert!(!ty("*mut *const i32").matches(&ty("*const i32")));
    let a = Type::Array(Array::new(Location::new("", 0..0), ty("i32"), 2));
    let b = Type::Array(Array::new(Location::new("", 0..0), ty("i32"), 3));
    assert!(!a.matches(&b));
    let f = Type::FuncPtr(FuncPtr::new(
        Location::new("", 0..0),
        vec![ty("&mut i32")],
        ty("void"),
    ));
    let g = Type::FuncPtr(FuncPtr::new(
        Location::new("", 0..0),
        vec![ty("&i32")],
        ty("void"),
    ));
    assert!(!f.matches(&g));
    assert_eq!(f.get_name(), "fn (&mut i32) -> void");
}
