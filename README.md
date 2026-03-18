# cx

Semantic code navigation for AI agents. Replaces expensive file reads with targeted structural queries.

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
cx read src/fees.rs           full file     "I need everything"
```

Benchmarked on real agent workflows, cx reduces token consumption by **15-80%** depending on the task, with the biggest savings on targeted lookups and re-reads.

## Install

```
cargo install --path .
```

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

### Read -- full file with session cache

```
$ cx read src/main.rs          # first call: returns full content
$ cx read src/main.rs          # second call: "status: unchanged" (~20 tokens)
$ cx read src/main.rs --fresh  # bypass cache, always full content
```

Sessions are scoped to the parent process. A new terminal gets a fresh session.

## How it works

On first invocation, cx builds an index (`.cx-index`) by parsing all source files with tree-sitter. The index stores symbols, signatures, and byte ranges for every file. Subsequent invocations incrementally update only changed files.

**Supported languages:** Rust, TypeScript/JavaScript, Python, Go, C, C++, Java, Ruby, C#, Lua, Zig, Bash, Solidity, Elixir

**Index location:** `.cx-index` in the project root (add to `.gitignore`)

**Project root detection:** walks up from cwd looking for `.git`

## Output format

Default output is [TOON](https://toonformat.dev) -- a token-efficient structured format. Use `--json` for JSON.

## Agent integration

Add this to your agent's system prompt:

```
cx -- semantic code index. Use instead of reading files where possible.

  cx overview PATH              file table of contents (~200 tokens)
  cx symbols [--kind K] [--name GLOB] [--file PATH]
                                search symbols across project
  cx definition --name NAME     function/type body without full file read
  cx read PATH [--fresh]        full file with session cache

Escalation hierarchy: overview → definition → read
- Use `cx symbols --kind fn` then check signatures for `pub`/`export` to find public API
- Use overview to find functions, definition to read bodies
- Fall back to `cx read` only when you need the full file
```

