# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- **Directory overview** — `cx overview <dir>` shows a single-level table of contents: direct files with symbol names, subdirectories with file/symbol counts. Use `--full` for detailed view with signatures.
- Test symbol filtering in directory overviews — excludes test files by path pattern (`*_test.go`, `*.test.ts`, `test_*.py`, etc.) and Rust `#[test]`/`#[cfg(test)]` inline tests
- Symbol capping at 10 per file in directory overview with kind-priority ordering (types first, then functions, then methods)

### Fixed
- `--root` flag now correctly resolves relative paths against the project root instead of cwd

### Changed
- `is_test` field added to `Symbol` (index version bumped to 5, forces reindex)

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
