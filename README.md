# cx

Semantic code explorer for AI agents. The three LSP features they actually use — symbols, definitions, and references — without running a language server.


## Install

```
cargo install cx-cli
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
cx overview src/fees.rs       ~200 tokens   "what's in this file?"
cx definition --name calc     ~200 tokens   "show me this function"
cx symbols --kind fn          ~70 tokens    "what functions exist in the codebase?"
cx references --name calc     ~1 query      "where is this used?"
```

In sessions with cx enabled, we measured **58% fewer Read calls** and **40-55% fewer tokens** spent on code navigation. The biggest wins are on chain reads and targeted lookups where `cx overview` or `cx definition` replaces a full file read.

**Why not an LSP?** Language servers are built for editors — persistent processes, 1-2GB RAM, per-language setup, and used by humans. Agents only need the ability to query the structure of their codebase. cx optimizes for that access pattern.

## Usage

### Overview -- file table of contents

```
$ cx overview src/main.rs

[9]{name,kind,signature}:
  Cli,struct,struct Cli
  Commands,enum,enum Commands
  main,fn,fn main()
  resolve_root,fn,"fn resolve_root(project: Option<PathBuf>) -> PathBuf"
  ...
```

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

[17]{file,line,kind,context}:
  src/index.rs,23,type_arguments,"pub exports: HashMap<PathBuf, Vec<Symbol>>,"
  src/index.rs,33,struct_item,"pub struct Symbol {"
  src/language/mod.rs,1,use_list,"use crate::index::{Language, Symbol, SymbolKind};"
  src/query.rs,43,field_declaration,"symbol: Symbol,"
  ...
```

The `kind` column shows the tree-sitter parent node type, indicating how the symbol is used (e.g. `struct_item` = definition, `use_list` = import, `type_arguments` = type reference).

Use `--file src/index.rs` to scope the search to a single file. Includes both definition and usage sites. Duplicate references on the same line are collapsed.

References are computed on-the-fly via AST walking (not indexed), so results are always fresh.

## How it works

On first invocation, cx builds an index (`.cx-index.db`) by parsing all source files with tree-sitter. The index stores symbols, signatures, and byte ranges for every file. Subsequent invocations incrementally update only changed files.

**Supported languages:** Rust, TypeScript/JavaScript, Python, Go, C, C++, Java, Ruby, C#, Lua, Zig, Bash, Solidity, Elixir

**Index location:** `.cx-index.db` in the project root (add to `.gitignore`)

**Project root detection:** walks up from cwd looking for `.git`. Override with `--root /path/to/project`.

**File filtering:** cx respects your `.gitignore`. To exclude additional directories from indexing, drop an empty `.cx-ignore` file inside them.

## Output format

Overview, symbols, and references use [TOON](https://toonformat.dev) -- a token-efficient structured format. Definition uses a plain-text format (metadata header + raw code body) for readability. Use `--json` for JSON on any command.

## Adding a language

cx uses tree-sitter grammars. To add a new language:

1. Add the tree-sitter grammar crate to `Cargo.toml`
2. In `src/language/mod.rs`, add:
   - A grammar function (e.g., `fn swift_grammar(_ext: &str) -> tree_sitter::Language`)
   - A query constant with tree-sitter patterns for the language's symbols
   - A query function returning the constant
   - A `LanguageConfig` entry in the `LANGUAGES` array (including `ref_node_types` for find-references support)
3. Add the language variant to `Language` enum in `src/index.rs`
4. Add tests

Here's a minimal example — adding Swift support:

```rust
// Grammar function
fn swift_grammar(_ext: &str) -> tree_sitter::Language {
    tree_sitter_swift::LANGUAGE.into()
}

// Query — capture the patterns you care about
const SWIFT_QUERY: &str = r#"
(function_declaration
  name: (simple_identifier) @name) @definition.function

(class_declaration
  name: (type_identifier) @name) @definition.class

(protocol_declaration
  name: (type_identifier) @name) @definition.interface
"#;

fn swift_query() -> &'static str { SWIFT_QUERY }

// Registry entry
LanguageConfig {
    language: Language::Swift,
    extensions: &["swift"],
    grammar: swift_grammar,
    query: swift_query,
    sig_body_child: None,
    sig_delimiter: Some(b'{'),
    kind_overrides: &[],
    ref_node_types: &["simple_identifier", "type_identifier"],
},
```

**Writing queries:** Use `tree-sitter parse` or inspect `node-types.json` in the grammar crate to discover the AST structure. Capture `@name` for the symbol name and `@definition.<kind>` for the enclosing node. Supported kinds: `function`, `method`, `class`, `interface`, `type`, `enum`, `module`, `constant`, `event`, `macro`.

**Kind overrides:** When a language maps generic capture names to specific concepts (e.g., Rust's `definition.class` → `SymbolKind::Struct`), add entries to `kind_overrides`. These are checked before the default mapping.
