# Known Issues

## tree-sitter-language-pack: C# dynamic loading requires symlink workaround

**Upstream bug**: `tree-sitter-language-pack` v1.1.2 has a mismatch between the download
name (`csharp`) and the C symbol name (`c_sharp`) for the C# grammar. The dynamic loader
constructs the symbol lookup as `tree_sitter_{name}`, so:

- `get_language("csharp")` finds `libtree_sitter_csharp.dylib` but looks for symbol
  `tree_sitter_csharp` — fails (actual symbol is `tree_sitter_c_sharp`)
- `get_language("c_sharp")` looks for `libtree_sitter_c_sharp.dylib` — fails (file is
  named `libtree_sitter_csharp.dylib`)

**Workaround**: `cx lang add c_sharp` downloads the `csharp` grammar and creates a
compatibility symlink `libtree_sitter_c_sharp.dylib → libtree_sitter_csharp.dylib`. cx
then uses `get_language("c_sharp")` which finds the symlink and the correct C symbol.

**Fix**: The language pack's `c_symbol` override (in `language_definitions.json`) is only
applied during static compilation, not dynamic loading. The dynamic loader in
`registry.rs` should consult `c_symbol` when constructing the `dlsym` function name.

**Tracking**: File upstream issue at https://github.com/kreuzberg-dev/tree-sitter-language-pack.
Once fixed, remove `COMPAT_SYMLINKS` and `create_compat_symlinks`/`remove_compat_symlinks`
from `src/lang.rs`, and rename config `name: "c_sharp"` to `name: "csharp"`.
