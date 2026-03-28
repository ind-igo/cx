# Known Issues

## tree-sitter-language-pack: C# support removed (upstream bug)

C# support was removed from cx because `tree-sitter-language-pack` has a persistent bug where
the `C_SYMBOL_OVERRIDES` table (mapping `csharp` → `c_sharp`) is generated at build time from
`sources/language_definitions.json`, which is **not included** in the crates.io package. This
causes download/load failures for any language with a c_symbol override (csharp, vb,
embeddedtemplate, nushell).

- **v1.2.0–1.3.1**: tarball uses raw names, download works, but `get_language` needs a symlink hack.
- **v1.3.2+**: tarball switched to c_symbol names, download silently extracts nothing.

C# support can be re-added once the upstream fix lands.

**Tracking**: https://github.com/kreuzberg-dev/tree-sitter-language-pack/issues/80

## tree-sitter-language-pack: cache invalidated on every crate version bump

The grammar cache is keyed by `CARGO_PKG_VERSION` (e.g. `~/Library/Caches/tree-sitter-language-pack/v1.3.1/libs/`).
Every crate version bump creates a new empty cache directory, forcing re-download of all grammars.

**Workaround**: cx calls `configure()` at startup with a custom cache dir (`~/Library/Caches/cx/grammars/`
on macOS) to bypass the version-keyed path entirely. Grammars survive crate version bumps.

**Status (2026-03-28)**: Issue #84 was closed as completed on 2026-03-26, but tested v1.3.3 and
the cache is still keyed by `CARGO_PKG_VERSION`. Not fixed upstream.

**Tracking**: https://github.com/kreuzberg-dev/tree-sitter-language-pack/issues/84
