# Known Issues

## tree-sitter-language-pack: C# file path vs C symbol mismatch

The C# grammar downloads as `libtree_sitter_csharp.dylib` but `get_language("csharp")`
looks for `libtree_sitter_c_sharp.dylib` (via `c_symbol_for("csharp")` → `"c_sharp"`).
The 1.1.3 fix (#80) correctly resolves the `dlsym` name but also applies the c_symbol
override to the *file path*, breaking file lookup.

**Workaround**: `cx lang add c_sharp` creates a symlink
`libtree_sitter_c_sharp.dylib → libtree_sitter_csharp.dylib`.

**Fix**: `load_from_dir` should use the language name for the file path and `c_symbol_for`
only for the `dlsym` lookup. The file is named after the language (`csharp`), not the C
symbol (`c_sharp`).

**Tracking**: Follow-up to https://github.com/kreuzberg-dev/tree-sitter-language-pack/issues/80

## tree-sitter-language-pack: cache invalidated on every crate version bump

The grammar cache is keyed by `CARGO_PKG_VERSION` (e.g. `~/.cache/tree-sitter-language-pack/v1.1.4/libs/`).
Every crate version bump creates a new empty cache directory, forcing re-download of all grammars.
The cache should be keyed by tree-sitter ABI version or manifest hash instead.

**Workaround**: cx pins `tree-sitter-language-pack = "=1.1.4"` to avoid surprise cache invalidation.

**Tracking**: https://github.com/kreuzberg-dev/tree-sitter-language-pack/issues/84
