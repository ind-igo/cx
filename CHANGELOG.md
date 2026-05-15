# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.7.1] - 2026-05-15

### Added
- Objective-C support for `.m` and `.mm` files, including classes, protocols, methods, and C functions.

## [0.7.0] - 2026-05-14

### Added
- Markdown heading navigation: `.md`, `.markdown`, and `.mdown` files are indexed by headings, and `definition` returns the selected heading section.
- Line ranges in `overview` output for more precise navigation.
- `cx symbols --kinds` to list available symbol kinds with counts.
- Directory paths in `--file` and `--from` filters.
- C++ header declaration indexing.
- Windows ARM64 release support.

### Changed
- References now default to the compact grouped summary; exact matching lines are available with `--context`.
- Overview includes test files and test symbols by default; use `--no-tests` to exclude them.
- Absolute path arguments now derive the project root from the provided path instead of only the current working directory.
- Full index crawling is parallelized.
- Query coverage expanded across TypeScript, Python, Go, Rust, Java, C++, C, Solidity, Ruby, Lua, Bash, and Zig.
- `SymbolKind::Method` was collapsed into `fn` for simpler output.

### Fixed
- Directory overview and symbol/definition filtering now handle current-working-directory and absolute-path resolution more consistently.
- C++ declaration-only headers at nested paths are indexed correctly.

## [0.6.3] - 2026-04-04

### Added
- `CX_CACHE_DIR` env var to override the cache location (#14) — enables cx in sandboxed agents (Codex, Claude Code) that restrict writes outside the workspace

## [0.6.2] - 2026-04-04

### Added
- **Pagination** (#15): Global `--limit`, `--offset`, `--all` flags across all query commands
  - Default limits: definition (3), symbols (100), references (50)
  - Compact stderr hint when truncated: `cx: 3/32 definitions for "X" | --from PATH to narrow | --offset 3 for more | --all`
  - JSON uses `{total, offset, limit, results}` envelope when paginated, bare array otherwise
  - `--all` and `--limit` are mutually exclusive (enforced by clap)
- Definition results sorted by symbol priority (types first) before pagination

### Changed
- Definition paginates before reading bodies from disk (avoids wasted I/O on large match sets)
- Skill prompt trimmed from ~1000 to ~350 tokens

## [0.6.1] - 2026-04-02

### Added
- **Dart language support** (#9, requested by @evanscai): classes (sealed/base/interface/mixin), mixins, extensions, extension types, enums, functions, methods, getters/setters, constructors (named/factory), operators, type aliases
- **Comprehensive Swift support** (based on #11 by @upupc): actors, extensions, properties, subscripts, enum bodies, init/deinit (#10, #12)
- **Elixir enhancements** (#6 by @RamXX): `@type`/`@typep`/`@opaque`, `@callback`, `defimpl`
- **Directory overview** (#8, reported by @it-ony): `cx overview dir/` — single-level table of contents with symbol names, `--full` for signatures
- Test symbol filtering in directory overviews — excludes test files by path pattern and Rust `#[test]`/`#[cfg(test)]` inline tests

### Changed
- Language module refactored into focused files (`queries/*.rs`, `extract.rs`, `tests.rs`)
- `RwLock` + thread-local `Parser` for better parallel indexing performance
- Symbol dedup now prefers later (more specific) query matches for same byte range
- Index version bumped to 6 (forces reindex)

### Fixed
- `--root` flag now correctly resolves relative paths against the project root instead of cwd


## [0.6.0] - 2026-03-30

### Changed
- **Breaking:** Index database moved from `.cx-index.db` in the repo root to `~/.cache/cx/indexes/`. No more repo pollution or `.gitignore` dance.

### Added
- `cx cache path` — print the index cache path for the current project
- `cx cache clean` — delete the cached index for the current project

### Removed
- `.cx-index.db` repo-local index file
- Gitignore warning on first run

### Fixed
- Flaky incremental update tests on filesystems with coarse (1-second) mtime granularity

## [0.5.0] - 2026-03-25

### Added
- `cx lang add <languages>` — download and install language grammars on demand
- `cx lang remove <languages>` — remove installed grammars
- `cx lang list` — show supported languages and install status
- Actionable warnings when grammars are missing during indexing
- First-run UX: shows detected languages with file counts and install command

### Changed
- Grammars are now dynamically loaded via `tree-sitter-language-pack` instead of
  statically linking 14 `tree-sitter-{lang}` crates
- `Language` enum replaced with string-based language identification
- `FileEntry` serialized with bincode (index version bumped to 4, forces reindex)
- tree-sitter upgraded from 0.25 to 0.26
- Zig and Python queries updated for newer grammar versions
- `find_references` now returns `Result` and propagates `NotInstalled` errors
- Release binary reduced from ~25MB to ~7MB

### Removed
- Static dependency on 14 individual `tree-sitter-{lang}` crates

## [0.4.5] - 2026-03-24

### Changed
- Updated Cargo.lock for redb 3 upgrade

## [0.4.4] - 2026-03-23

### Fixed
- x86_64 macOS build runner configuration

### Added
- Release workflow and install script
- Concurrent read access via redb 3 upgrade
