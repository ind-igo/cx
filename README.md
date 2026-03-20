# cx

Semantic code navigation for AI agents. Replaces expensive file reads with targeted structural queries.

> **Alpha software.** This is a work in progress — expect breaking changes. Built with AI assistance (Claude).

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

In our usage, agents spend ~74% of their context budget reading files. Most of those reads are wasteful:

- **~62% aren't followed by edits** -- the agent just needs to know what's in a file
- **~46% are chain reads** -- reading A to find B to find C, averaging 3.5 files deep
- **~33% are re-reads** -- same file read multiple times per session

These numbers are from our own analysis of agent sessions, not published benchmarks. Your mileage may vary, but the pattern is consistent: agents read far more than they need to.

cx gives agents a cost ladder. Start cheap, escalate only when needed:

```
cx overview src/fees.rs       ~200 tokens   "what's in this file?"
cx definition --name calc     ~500 tokens   "show me this function"
cx references --name calc     ~1 query      "where is this used?"
```

Benchmarked on real agent workflows, cx reduces token consumption by **15-80%** depending on the task, with the biggest savings on targeted lookups and chain reads.

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
$ cx definition --name load_or_build

file: src/index.rs
signature: pub fn load_or_build(root: &Path) -> Self
range: [3412,4102]
body: ...
```

Use `--from src/foo.rs` to disambiguate when multiple files define the same name. `--max-lines` (default 200) truncates large bodies.

### References -- find all usages of a symbol

```
$ cx references --name Symbol

[18]{file,line,context}:
  src/index.rs,23,"pub exports: HashMap<PathBuf, Vec<Symbol>>,"
  src/index.rs,33,"pub struct Symbol {"
  src/query.rs,6,"use crate::index::{Index, Symbol, SymbolKind};"
  ...
```

Use `--file src/index.rs` to scope the search to a single file. Includes both definition and usage sites.

References are computed on-the-fly via AST walking (not indexed), so results are always fresh.

## How it works

On first invocation, cx builds an index (`.cx-index.db`) by parsing all source files with tree-sitter. The index stores symbols, signatures, and byte ranges for every file. Subsequent invocations incrementally update only changed files.

**Supported languages:** Rust, TypeScript/JavaScript, Python, Go, C, C++, Java, Ruby, C#, Lua, Zig, Bash, Solidity, Elixir

**Index location:** `.cx-index.db` in the project root (add to `.gitignore`)

**Project root detection:** walks up from cwd looking for `.git`. Override with `--root /path/to/project`.

**File filtering:** cx respects your `.gitignore`. To exclude additional directories from indexing, drop an empty `.cx-ignore` file inside them.

## Output format

Default output is [TOON](https://toonformat.dev) -- a token-efficient structured format. Use `--json` for JSON.

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
