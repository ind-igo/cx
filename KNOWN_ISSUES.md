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

**Status (2026-03-28)**: Tested v1.3.3 — still broken for crates.io consumers. The
`C_SYMBOL_OVERRIDES` table is generated at build time from `sources/language_definitions.json`,
which is **not included** in the crates.io package. So on crates.io builds the overrides are
empty and `c_symbol_for("csharp")` falls through to `"csharp"`.

- **v1.2.0–1.3.1**: tarball uses raw names (`libtree_sitter_csharp.dylib`), empty overrides
  means download extracts correctly. Our symlink hack still needed for `get_language("c_sharp")`
  to find the file via dlsym. cx works.
- **v1.3.2+**: tarball switched to c_symbol names (`libtree_sitter_c_sharp.dylib`), but empty
  overrides means the extractor looks for `libtree_sitter_csharp.dylib`. Download silently
  extracts nothing, `get_language` fails. cx broken.

The v1.3.2/1.3.3 changelog claims this is fixed, but the fix only works when building from
the full git repo.

**Tracking**: Follow-up to https://github.com/kreuzberg-dev/tree-sitter-language-pack/issues/80

## tree-sitter-language-pack: cache invalidated on every crate version bump

The grammar cache is keyed by `CARGO_PKG_VERSION` (e.g. `~/Library/Caches/tree-sitter-language-pack/v1.1.4/libs/`).
Every crate version bump creates a new empty cache directory, forcing re-download of all grammars.
The cache should be keyed by tree-sitter ABI version or manifest hash instead.

**Workaround**: cx calls `configure()` at startup with a custom cache dir (`~/Library/Caches/cx/grammars/`
on macOS) to bypass the version-keyed path entirely. Grammars survive crate version bumps.
cx still pins `tree-sitter-language-pack = "=1.3.1"` but only because of the C# issue above.

**Status (2026-03-28)**: Issue #84 was closed as completed on 2026-03-26, but tested v1.3.3 and
the cache is still keyed by `CARGO_PKG_VERSION` via `env!("CARGO_PKG_VERSION")` in
`effective_cache_dir()` and `DownloadManager::default_cache_dir()`. Not fixed upstream.

**Tracking**: https://github.com/kreuzberg-dev/tree-sitter-language-pack/issues/84
