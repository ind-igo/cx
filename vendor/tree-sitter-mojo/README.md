# tree-sitter-mojo

Generated parser, scanner, and headers from
[dmitry-salin/tree-sitter-mojo](https://github.com/dmitry-salin/tree-sitter-mojo)
at `429918d92978087aae3e2c4b00242b8270d9ad6f` (v1.0.5), unmodified.
The upstream Rust binding is not published on crates.io, so these sources are
vendored to keep `cargo install` and release builds self-contained.

To update, replace `src/parser.c`, `src/scanner.c`, `src/tree_sitter/*.h`, and
`LICENSE` from a tested upstream revision, update this revision, and run
`cargo test`. `build.rs` compiles the parser into cx and fingerprints its contents
to invalidate existing indexes when it changes. No Tree-sitter CLI is needed.

Bend is compiled from the pinned `tree-sitter-bend2` dependency instead.
