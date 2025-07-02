fn main() {
    println!("cargo:rerun-if-changed=src/bindings.cpp");
    println!("cargo:rustc-search=-Lusr/lib");
    println!("cargo:rustc-link-lib=LLVM-19");
    println!("cargo:rustc-link-lib=stdc++");

    println!("cargo:rustc-link-search={}", concat!(std::env!("CARGO_MANIFEST_DIR"), "/src"));

    cc::Build::new()
        .file("src/bindings.cpp")
        .shared_flag(true)
        .std("c++20")
        .out_dir("src")
        .compile("bindings");
}
