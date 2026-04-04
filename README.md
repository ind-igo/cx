# cx

Semantic code navigation for AI agents — file overviews, symbol search, definitions, and references — without running a language server.

> Disclaimer: Built with AI.

## Install

```bash
brew tap ind-igo/cx && brew install cx
```

Or with Cargo:

```bash
cargo install cx-cli
```

Or via the install script:

```bash
curl -sL https://raw.githubusercontent.com/ind-igo/cx/master/install.sh | sh
```

On Windows (PowerShell):

```powershell
irm https://raw.githubusercontent.com/ind-igo/cx/master/install.ps1 | iex
```

## Agent integration

`cx skill` prints a prompt that teaches any coding agent to prefer cx over raw file reads. Pipe it into whichever instructions file your agent reads:

```bash
# Claude Code (CLAUDE.md)
cx skill > ~/.claude/CX.md
# then add @CX.md to ~/.claude/CLAUDE.md

# Codex, Copilot, Zed, and other AGENTS.md-compatible tools
cx skill >> AGENTS.md
```

That's it. The prompt includes the command reference and the escalation hierarchy (overview → symbols → definition / references → read).

## Why

Agents burn most of their context reading files. We analyzed 105 of our own Claude Code sessions (73 pre-cx, 32 post-cx) and found:

- **66% of reads are chains** -- reading A to find B to find C, exploring before acting
- **37% are re-reads** -- same file read multiple times per session
- **Avg Read costs ~1,200 tokens** (median 594), and sessions average 21 reads

cx gives agents a cost ladder. Start cheap, escalate only when needed:

```
cx overview src/              ~20 tokens    "what's in this folder?"
cx overview src/fees.rs       ~200 tokens   "what's in this file?"
cx definition --name calc     ~200 tokens   "show me this function"
cx symbols --kind fn          ~70 tokens    "what functions exist in the codebase?"
cx references --name calc     ~1 query      "where is this used?"
```

In sessions with cx enabled, we measured **58% fewer Read calls** and **40-55% fewer tokens** spent on code navigation. The biggest wins are on chain reads and targeted lookups where `cx overview` or `cx definition` replaces a full file read.

**Why not an LSP?** Language servers are built for editors — persistent processes, 1-2GB RAM, per-language setup, and used by humans. Agents only need the ability to query the structure of their codebase. cx optimizes for that access pattern.

## How cx compares

| Tool | Overlap | cx difference |
|------|---------|---------------|
| **ctags** | Symbol indexing | Tree-sitter instead of regex, persistent db, built-in query CLI |
| **LSP** | Go-to-definition, find references, symbol search | No daemon, no compilation, no project setup — just parse and query |
| **ripgrep** | Finding code by name | Semantic — `cx definition --name X` vs grep-then-read-5-files |
| **Reading files** | Understanding code | `cx overview` ~200 tokens vs full file read ~thousands |

## Usage

### Overview -- file and directory table of contents

Directories show one level: direct files with symbol names, subdirectories with counts. Test files and test symbols are filtered out automatically.

```
$ cx overview .

[7]{file,symbols}:
  container/,"(3 files, 28 symbols)"
  scripts/,"(6 files, 16 symbols)"
  src/,"(19 files, 147 symbols)"
  setup.sh,"check_build_tools, check_node, detect_platform, ..."
```

Drill into a subdirectory:

```
$ cx overview src/

[7]{file,symbols}:
  language/,"(1 files, 19 symbols)"
  util/,"(3 files, 4 symbols)"
  index.rs,"Index, Symbol, SymbolKind, load_or_build, ..."
  main.rs,"Cli, Commands, main, resolve_root, ..."
```

Single file -- full symbol table with kinds and signatures:

```
$ cx overview src/main.rs

[9]{name,kind,signature}:
  Cli,struct,struct Cli
  Commands,enum,enum Commands
  main,fn,fn main()
  resolve_root,fn,"fn resolve_root(project: Option<PathBuf>) -> PathBuf"
  ...
```

Use `--full` on directories for the detailed per-file view with signatures.

### Symbols -- search across the project

```
$ cx symbols --kind fn

[15]{file,name,kind,signature}:
  src/output.rs,print_toon,fn,"pub fn print_toon<T: Serialize>(value: &T)"
  src/query.rs,symbols,fn,"pub fn symbols(...) -> i32"
  src/query.rs,definition,fn,"pub fn definition(...) -> i32"
  ...
```

Filters: `--kind`, `--name` (glob), `--file`

Public/exported symbols are identifiable from their signatures (e.g. `pub fn` in Rust, `export function` in TypeScript).

### Definition -- get a function body without reading the file

```
$ cx definition --name resolve_root

file: src/main.rs
line: 76
---
fn resolve_root(project: Option<PathBuf>) -> PathBuf {
    match project {
        Some(p) => p,
        None => {
            let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            util::git::find_project_root(&cwd)
        }
    }
}
```

Use `--from src/foo.rs` to disambiguate when multiple files define the same name. `--kind fn` filters by symbol kind. `--max-lines` (default 200) truncates large bodies.

### References -- find all usages of a symbol

```
$ cx references --name Symbol

[17]{file,line,caller,context}:
  src/index.rs,23,,FileData,"pub symbols: Vec<Symbol>,"
  src/index.rs,69,Symbol,"pub struct Symbol {"
  src/language/mod.rs,4,,"use crate::index::{Symbol, SymbolKind};"
  src/query.rs,38,SymbolRow,"symbol: &'a Symbol,"
  ...
```

The `caller` column shows which function or type encloses the reference. Use `--unique` to deduplicate by caller — one row per function that depends on the symbol:

```
$ cx references --name Symbol --unique

[6]{file,caller,line}:
  src/index.rs,FileData,23
  src/index.rs,load_entries,175
  src/language/extract.rs,extract_symbols,83
  src/language/mod.rs,parse_and_extract,325
  src/query.rs,definition,125
  src/query.rs,dir_overview,480
```

Use `--file src/index.rs` to scope the search to a single file. Includes both definition and usage sites. Duplicate references on the same line are collapsed.

References are computed on-the-fly via AST walking (not indexed), so results are always fresh.

### Pagination

Commands have default result limits to keep output bounded: definition shows 3, symbols 100, references 50. When results are truncated, cx prints a hint:

```
cx: 3/32 definitions for "OnTypeModel" | --from PATH to narrow | --offset 3 for more | --all
```

Use `--offset N` to page forward, `--all` to bypass the limit, or `--limit N` to override the default. Narrowing with `--from` / `--file` / `--kind` is usually better than paging.

With `--json`, paginated output uses `{total, offset, limit, results: [...]}`. Non-paginated output remains a bare array.

## How it works

On first invocation, cx builds an index by parsing all source files with tree-sitter. The index stores symbols, signatures, and byte ranges for every file. Subsequent invocations incrementally update only changed files.

Language grammars are downloaded on demand as shared libraries via [tree-sitter-language-pack](https://github.com/kreuzberg-dev/tree-sitter-language-pack). Install the ones you need:

```bash
cx lang add rust typescript python
cx lang list        # see what's installed
cx lang remove lua  # remove one
```

If you run cx without installing grammars first, it will tell you which ones are needed:

```
cx: no language grammars installed

Detected languages in this project:
  rust (42 files)
  typescript (18 files)

Install with: cx lang add rust typescript
```

**Supported languages:** Run `cx lang list` to see all supported languages and their install status.

**Index location:** `~/.cache/cx/indexes/` (one db per project, keyed by path hash). Run `cx cache path` to see the exact location, `cx cache clean` to delete it. Override with `CX_CACHE_DIR`.

**Project root detection:** walks up from cwd looking for `.git`. Override with `--root /path/to/project`.

**File filtering:** cx respects your `.gitignore`. To exclude additional directories from indexing, drop an empty `.cx-ignore` file inside them.

**Sandboxed environments (Codex, Claude Code, etc.):** cx writes to `~/.cache/cx` by default. If your sandbox restricts writes outside the workspace, either add `~/.cache/cx` to the sandbox's writable paths, or set `CX_CACHE_DIR` to a writable location (e.g. `CX_CACHE_DIR=/tmp/cx-cache`).

## Output format

Overview, symbols, and references use [TOON](https://toonformat.dev) -- a token-efficient structured format. Definition uses a plain-text format (metadata header + raw code body) for readability. Use `--json` for JSON on any command.

## Adding a language

cx uses tree-sitter grammars loaded dynamically via `tree-sitter-language-pack`. To add support for a new language:

1. In `src/language/mod.rs`, add:
   - A query constant with tree-sitter patterns for the language's symbols
   - A `LanguageConfig` entry in the `LANGUAGES` array
2. Add tests

The grammar itself is downloaded at runtime — no build dependency needed. Here's a minimal example — adding Swift support:

```rust
const SWIFT_QUERY: &str = r#"
(function_declaration
  name: (simple_identifier) @name) @definition.function

(class_declaration
  name: (type_identifier) @name) @definition.class

(protocol_declaration
  name: (type_identifier) @name) @definition.interface
"#;

LanguageConfig {
    name: "swift",
    extensions: &["swift"],
    grammar_override: &[],
    download_names: &[],  // empty = download name matches config name
    query: SWIFT_QUERY,
    sig_body_child: None,
    sig_delimiter: Some(b'{'),
    kind_overrides: &[],
    ref_node_types: &["simple_identifier", "type_identifier"],
},
```

**Writing queries:** Use `tree-sitter parse` or inspect `node-types.json` in the grammar to discover the AST structure. Capture `@name` for the symbol name and `@definition.<kind>` for the enclosing node. Supported kinds: `function`, `method`, `class`, `interface`, `type`, `enum`, `module`, `constant`, `event`.

**Kind overrides:** When a language maps generic capture names to specific concepts (e.g., Rust's `definition.class` → `SymbolKind::Struct`), add entries to `kind_overrides`. These are checked before the default mapping.

**Grammar names:** The `name` field must match the name used by `tree-sitter-language-pack` (check their [language list](https://github.com/kreuzberg-dev/tree-sitter-language-pack)). If the download name differs from the config name, use `download_names` (e.g., `typescript` also downloads `tsx`).
