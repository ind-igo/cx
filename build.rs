use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

fn main() {
    let source = std::path::Path::new("vendor/tree-sitter-mojo/src");
    let mut build = cc::Build::new();
    build.std("c11").include(source).warnings(false);
    build.flag_if_supported("-utf-8");
    build
        .file(source.join("parser.c"))
        .file(source.join("scanner.c"));
    build.compile("tree-sitter-mojo");

    // Invalidate indexes after a bundled parser update, including local rebuilds.
    let mut hash = DefaultHasher::new();
    for file in [
        "parser.c",
        "scanner.c",
        "tree_sitter/parser.h",
        "tree_sitter/alloc.h",
        "tree_sitter/array.h",
    ] {
        let path = source.join(file);
        println!("cargo:rerun-if-changed={}", path.display());
        std::fs::read(path).unwrap().hash(&mut hash);
    }
    println!("cargo:rerun-if-changed=Cargo.toml");
    std::fs::read("Cargo.toml").unwrap().hash(&mut hash);
    println!("cargo:rustc-env=CX_BUNDLED_GRAMMAR_HASH={}", hash.finish());
}
