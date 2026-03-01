use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn run_llvm_config(args: &[&str]) -> String {
    let llvm_config = std::env::var("LLVM_CONFIG_PATH").unwrap_or_else(|_| "llvm-config".into());
    let out = Command::new(&llvm_config)
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("failed to run {llvm_config}: {e}"));
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        panic!("llvm-config {:?} failed: {stderr}", args);
    }

    String::from_utf8(out.stdout)
        .unwrap_or_else(|e| panic!("invalid utf8 from llvm-config: {e}"))
        .trim()
        .to_string()
}

fn lib_name(token: &str) -> Option<String> {
    if let Some(name) = token.strip_prefix("-l") {
        return Some(name.to_string());
    }

    let path = Path::new(token);
    if path.extension().and_then(|e| e.to_str()) == Some("lib") {
        let stem = path.file_stem()?.to_str()?.to_string();
        if cfg!(target_os = "windows") {
            return Some(stem);
        }
        return Some(stem.strip_prefix("lib").unwrap_or(&stem).to_string());
    }

    None
}

fn lib_exists_in_search_paths(name: &str, lib_dir: &str) -> bool {
    let filename = format!("{name}.lib");
    let direct = Path::new(lib_dir).join(&filename);
    if direct.exists() {
        return true;
    }

    if let Ok(lib_env) = std::env::var("LIB") {
        for p in lib_env.split(';').filter(|s| !s.is_empty()) {
            if Path::new(p).join(&filename).exists() {
                return true;
            }
        }
    }

    false
}

fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn write_compile_commands(
    manifest_dir: &Path,
    source: &Path,
    compiler: &Path,
    include_dir: &str,
    out_dir: &Path,
) {
    let output = out_dir.join("bindings.o");
    let include_flag = if cfg!(target_env = "msvc") { "/I" } else { "-I" };
    let std_flag = if cfg!(target_env = "msvc") {
        "/std:c++20"
    } else {
        "-std=c++20"
    };

    let arguments = [
        compiler.to_string_lossy().into_owned(),
        "-c".to_string(),
        source.to_string_lossy().into_owned(),
        "-o".to_string(),
        output.to_string_lossy().into_owned(),
        include_flag.to_string(),
        include_dir.to_string(),
        std_flag.to_string(),
    ];

    let args_json = arguments
        .iter()
        .map(|a| format!("\"{}\"", json_escape(a)))
        .collect::<Vec<_>>()
        .join(", ");

    let entry = format!(
        "[\n  {{\n    \"directory\": \"{}\",\n    \"file\": \"{}\",\n    \"output\": \"{}\",\n    \"arguments\": [{}]\n  }}\n]\n",
        json_escape(&manifest_dir.to_string_lossy()),
        json_escape(&source.to_string_lossy()),
        json_escape(&output.to_string_lossy()),
        args_json
    );

    let compile_commands = manifest_dir.join("compile_commands.json");
    fs::write(compile_commands, entry).expect("failed to write compile_commands.json");
}

fn main() {
    println!("cargo:rerun-if-changed=src/bindings.cpp");
    println!("cargo:rerun-if-env-changed=LLVM_CONFIG_PATH");

    let include_dir = run_llvm_config(&["--includedir"]);
    let lib_dir = run_llvm_config(&["--libdir"]);
    let llvm_libs = run_llvm_config(&["--libs", "--link-static", "--ignore-libllvm"]);
    let system_libs = run_llvm_config(&["--system-libs"]);

    println!("cargo:rustc-link-search=native={lib_dir}");

    let mut seen = BTreeSet::new();
    for token in llvm_libs.split_whitespace().chain(system_libs.split_whitespace()) {
        if let Some(name) = lib_name(token) {
            if name == "libxml2s" && !lib_exists_in_search_paths(&name, &lib_dir) {
                println!("cargo:warning=Skipping optional system lib {name} (not found)");
                continue;
            }
            if seen.insert(name.clone()) {
                println!("cargo:rustc-link-lib={name}");
            }
        }
    }

    let mut build = cc::Build::new();
    build
        .cpp(true)
        .file("src/bindings.cpp")
        .include(&include_dir)
        .flag_if_supported("/std:c++20")
        .flag_if_supported("-std=c++20");

    if cfg!(target_env = "msvc") {
        build.static_crt(true);
    }

    let manifest_dir = PathBuf::from(
        std::env::var("CARGO_MANIFEST_DIR").expect("missing CARGO_MANIFEST_DIR"),
    );
    let source = manifest_dir.join("src").join("bindings.cpp");
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("missing OUT_DIR"));
    let compiler = build.get_compiler();

    write_compile_commands(
        &manifest_dir,
        &source,
        compiler.path(),
        &include_dir,
        &out_dir,
    );

    build.compile("bindings");
}
