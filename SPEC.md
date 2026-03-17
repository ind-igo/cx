# cx — Code Index CLI

A semantic code navigation tool for AI agents. The primary goal is to **replace file
Read operations** with targeted structural queries. Read is ~74% of agent context usage.
Grep is ~2.4%. cx addresses the large problem, not the small one.

---

## The Problem cx Solves

Agents read files for three reasons:

1. **Chain reads (46% of reads)** — Read file A, see it imports B, read B, see B
   references C, read C. Average chain is 3.5 files deep. The agent is manually walking
   the import graph because it has no other option.

2. **Context reads (62% not followed by edits)** — Agent reads an entire file just to
   understand what's in it: what functions exist, what a function signature looks like,
   what a module exports. It only needs structural info but has to pay for the whole file.

3. **Re-reads (32.5% of reads)** — Same file read multiple times per session. 85% are
   edit-verify cycles (read → edit → re-read to confirm). cx addresses these with a
   read cache that returns "unchanged" on re-reads or the full new content on changes.

## The Escalation Hierarchy

cx gives agents a natural cost ladder. Each step is more expensive, so agents have an
incentive to start cheap:

```
cx overview PATH          ~200 chars    "what's in this file?"
cx definition --name X    ~500 chars    "show me this specific function"
cx read PATH              full file     "I need the whole thing"
```

Without cx, every question costs a full file read. With cx, most questions are answered
at the first or second rung, and `cx read` at least prevents paying twice for the same
content within a session.

---

## Goals

- Replace unnecessary Read operations with targeted structural queries
- Cache full reads within a session — re-reads return "unchanged" or full new content
- Zero-config: first invocation builds and caches the index automatically
- Fast: warm queries complete in <50ms via persistent on-disk index
- Structured TOON output designed to minimize agent token consumption

## Non-Goals

- Not a grep replacement (that's 2.4% of context — not the priority)
- No import graph or cross-file reference resolution
- No type inference, no trait dispatch resolution
- No byte-range partial reads
- No diffs on changed files — just returns full new content
- No interactive/REPL mode, no language server process management

---

## Technology Stack

- **Language:** Rust
- **Parsing:** `tree-sitter` with official language grammars
- **Symbol extraction:** existing `tags.scm` queries bundled with each grammar
- **Output format:** TOON emitted directly with `format!`, no crate dependency
- **Serialization:** `serde` + `serde_json` for index persistence
- **Parallelism:** `rayon` for initial index build
- **CLI:** `clap`

### Cargo.toml dependencies

```toml
[dependencies]
tree-sitter = "0.23"
tree-sitter-rust = "0.23"
tree-sitter-typescript = "0.23"
tree-sitter-python = "0.23"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
rayon = "1"
clap = { version = "4", features = ["derive"] }
walkdir = "2"
```

No TOON crate dependency. The format is simple enough to emit directly — it's CSV with
a typed header. See the Output Format section.

---

## CLI Interface (MVP)

### Commands

```
cx overview   <file>
cx symbols    [--file <path>] [--name <glob>] [--kind <kind>]
cx definition --name <n> [--from <file>] [--max-lines <n>]
cx read       <file> [--fresh]
cx grep       <pattern> [path] [grep flags]
```

### Global flags

```
--project <path>    Project root (default: git root from cwd, then cwd)
--json              Emit JSON instead of TOON
```

### Symbol kinds

`fn` `struct` `enum` `trait` `type` `const` `class` `interface` `method` `module`

---

## Output Format

All output is **TOON** emitted directly as formatted strings. Use `--json` for JSON.
Errors go to stderr. Exit codes: 0 success, 1 error, 2 no results.

### TOON format primer

Arrays of uniform objects use tabular format:

```
name[N]{field1,field2,field3}:
  value1,value2,value3
  value1,value2,value3
```

Single objects use scalar (YAML-like) format:

```
key1: value1
multiline_key: |
  line one
  line two
```

Values containing commas or newlines are quoted. Implement with `format!` — see the
Implementation section.

### cx overview

The primary use case. Call this before deciding whether to read a file.

```
cx overview src/fees.rs
```

```
symbols[4]{name,kind,signature}:
  calculate_fee,fn,"fn calculate_fee(amount: U256, tier: FeeTier) -> U256"
  calculate_base_fee,fn,"fn calculate_base_fee(tier: FeeTier) -> U256"
  FeeTier,enum,"enum FeeTier"
  FeeConfig,struct,"struct FeeConfig"
```

File path is omitted from rows since it's implicit. ~200 chars for a typical file vs
~15,000 chars to read the file. Implemented as `cx symbols --file <path>` internally.

### cx symbols

```
cx symbols --kind fn --name "calc*"
```

```
symbols[3]{file,name,kind,signature}:
  src/fees.rs,calculate_fee,fn,"fn calculate_fee(amount: U256, tier: FeeTier) -> U256"
  src/fees.rs,calculate_base_fee,fn,"fn calculate_base_fee(tier: FeeTier) -> U256"
  src/router.rs,calculate_route_fee,fn,"fn calculate_route_fee(hops: u32, amount: U256) -> U256"
```

### cx definition

```
cx definition --name calculate_fee
```

```
file: src/fees.rs
signature: "fn calculate_fee(amount: U256, tier: FeeTier) -> U256"
range: [1823,2104]
body: |
  pub fn calculate_fee(amount: U256, tier: FeeTier) -> U256 {
      match tier {
          FeeTier::Low => amount * 3 / 1000,
          FeeTier::Medium => amount / 100,
          FeeTier::High => amount * 3 / 100,
      }
  }
```

If `--name` matches multiple symbols across files, return all of them. The agent
filters. Use `--from` to disambiguate: `cx definition --name foo --from src/swap.rs`
returns the `foo` that `src/swap.rs` would use.

**Truncation:** `--max-lines` (default 200) caps the body output. If a symbol body
exceeds the limit, the output is truncated and includes metadata so the agent knows:

```
file: src/engine.rs
signature: "fn process_all(ctx: &mut Context) -> Result<()>"
range: [4200,12850]
truncated: true
lines: 342
body: |
  pub fn process_all(ctx: &mut Context) -> Result<()> {
      ... (first 200 lines) ...
```

This prevents a single "God Function" from costing as much as a full file read.
The agent can use `cx read` or `cx read --fresh` if it truly needs the whole thing.

### cx read

Full file read with session-scoped cache. Use when `cx overview` and `cx definition`
aren't sufficient — when the agent needs the complete file content.

Two modes, because agents re-read files for two distinct reasons:

- **Verification re-read** — "is this file still what I think it is before I edit
  again?" The agent just edited the file and knows its contents. It only needs
  confirmation nothing else changed. `cx read` (default) handles this — "unchanged"
  is sufficient and costs almost nothing.

- **Context refresh re-read** — "I've lost track of what's in this file, I need to
  see it again." The agent needs the actual content back in context. `cx read --fresh`
  handles this — always returns full content regardless of cache.

The agent decides which it needs per call. The default is cheap; `--fresh` opts into
expensive when the content is actually needed.

**First read in session:**

```
cx read src/fees.rs
```

Returns full file content. Records `path → content_hash` in FileEntry under current
session ID.

**Re-read, file unchanged (verification):**

```
cx read src/fees.rs
```

```
status: unchanged
file: src/fees.rs
hash: a1b2c3d4
```

The `hash` field (truncated content hash, 8 hex chars) lets the agent verify it
matches its own expectation. If the agent's context was flushed but the session
persists, the hash acts as a sanity check — the agent can call `cx read --fresh`
if it doesn't recognize the hash. No content transmitted. Edit-verify cycles become
near-free.

**Re-read, file changed:**

```
cx read src/fees.rs
```

```
status: changed
file: src/fees.rs
```

Followed by full new file content. Updates cache. No diff in MVP — full content on
change. Diff is v2.

**Force fresh read (context refresh):**

```
cx read src/fees.rs --fresh
```

Always returns full file content. Updates the cache. Use when the agent needs the
content back in context regardless of whether it changed.

### cx grep

Pure subprocess passthrough. No index involvement in MVP.

```
cx grep -rn "calculate_fee" src/
```

Exec `rg` if available, fall back to `grep`. Pass all flags and arguments through
unchanged. Output is the subprocess's stdout directly. Exists for drop-in compatibility
with existing agent prompts. Index-backed fast path is v2.

---

## Index

### Location

`.cx-index` in the project root. Created on first query. Add `.cx-index` to
`.gitignore`.

### Structure

```rust
pub struct Index {
    pub version: u32,
    pub root: PathBuf,
    pub files: HashMap<PathBuf, FileEntry>,
    pub exports: HashMap<PathBuf, Vec<Symbol>>,
}

pub struct FileEntry {
    pub mtime: SystemTime,
    pub language: Language,
    pub read_cache: Option<ReadCache>,
}

pub struct ReadCache {
    pub session_id: String,    // matches current session — cache is invalid if different
    pub content_hash: u64,     // xxhash or similar, fast non-cryptographic hash
}

pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub signature: String,
    pub byte_range: (usize, usize),
    pub is_exported: bool,
}

pub enum SymbolKind {
    Fn, Struct, Enum, Trait, Type, Const, Class, Interface, Method, Module,
}

pub enum Language {
    Rust, TypeScript, Python, Unknown,
}
```

### Session ID

Session ID determines read cache validity. A new session means all cached reads are
stale — the agent has no prior context, so it should always get full file content on
first read.

**Derivation:**

1. Check `/tmp/cx-session-$PPID` — if it exists, read its contents as the session ID
2. If missing, generate a UUID, write it to `/tmp/cx-session-$PPID`, use that value

`$PPID` is the parent process ID of the cx invocation. This correctly scopes the
session to the agent process: subagents spawned as separate processes get a different
PPID and therefore a fresh session, which is correct — a subagent has no prior context
and should receive full reads.

Temp files are cleaned up automatically by the OS. No explicit cleanup needed.

### Index Lifecycle

1. On any `cx` invocation, attempt to load `.cx-index`
2. If version mismatch or missing: rebuild from scratch
3. Stat all indexed files, collect changed/new/deleted paths
4. Re-parse only affected files with tree-sitter in parallel (rayon)
5. Update `exports` and `mtime` for affected files (do not clear `read_cache`)
6. Write updated index atomically: serialize to `.cx-index.tmp`, rename to `.cx-index`
7. Answer query

**First invocation:** crawl from project root. Skip: `target/`, `node_modules/`,
`.git/`, `dist/`, `__pycache__/`, any directory containing a `.cx-ignore` file.

**Project root detection:** walk up from cwd looking for `.git`. Use that directory.
Fall back to cwd if no `.git` found.

**Path resolution:** all file arguments are resolved relative to cwd, not the project
root. The project root is only used for locating `.cx-index` and determining the
crawl boundary. Paths stored in the index are relative to the project root.

### Read Cache Behavior

On `cx read <file>`:

1. If `--fresh` flag set: skip cache entirely, return full content, update cache, done
2. Load index, check `files[path].read_cache`
3. If `read_cache` is None or `session_id` doesn't match current session: cache miss
4. If cache hit: hash current file content
   - Hash matches `content_hash`: return "unchanged" response with truncated hash
   - Hash differs: return full new content, update `content_hash` in index
5. On cache miss: return full content, store `{session_id, content_hash}` in FileEntry
6. Write updated index

---

## Language Modules

Each language module implements:

```rust
pub trait LanguageModule: Send + Sync {
    fn language(&self) -> Language;
    fn extensions(&self) -> &[&str];
    fn extract_symbols(&self, tree: &Tree, source: &[u8]) -> Vec<Symbol>;
}
```

### Symbol extraction with tags.scm

All three languages use the `tags.scm` bundled with their tree-sitter grammar crate.
These are the same queries GitHub uses for code navigation. They emit `@definition.*`
captures covering functions, classes, methods, types, etc.

```rust
// Verify actual API against each grammar crate's docs on crates.io before implementing.
// Some expose TAGGING_QUERY as a &str constant.
// Others require loading from the grammar's queries/ directory.
let mut parser = Parser::new();
parser.set_language(&tree_sitter_rust::LANGUAGE.into()).unwrap();
let query = Query::new(&language, tags_query_str).unwrap();
```

### Rust module

- **Extensions:** `.rs`
- **Exported:** symbol has a `pub` visibility modifier (`visibility_modifier` node as
  child or ancestor in the tree)
- **Signature:** for functions, slice source bytes from function start to the opening
  `{`. For structs/enums/traits, slice the declaration line.

### TypeScript module

- **Extensions:** `.ts`, `.tsx`, `.js`, `.jsx`
- **Exported:** node is inside an `export_statement` or preceded by `export` keyword

### Python module

- **Extensions:** `.py`
- **Exported:** all top-level definitions are exported unless name starts with `_`

---

## Query Implementation

### overview

Dispatch to `symbols` with `file` set. Single-file path: omit the `file` column from
TOON output since it's implicit. Implement as a clap alias — no separate code path.

### symbols

1. If `--file` given: return `exports[file]`, filter by `--name` glob and `--kind`
2. Otherwise: iterate all files in `exports`, apply same filters
3. Glob: `*` matches any substring, `?` matches one character, case-sensitive
4. For single-file query, omit `file` column from output

### definition

1. Collect all symbols where `name == --name`
2. If `--from` given: prefer same-file symbols, otherwise return all matches
3. For each match: read file bytes, slice `symbol.byte_range`, return raw UTF-8
4. If body exceeds `--max-lines` (default 200): truncate and add `truncated: true` + `lines: N`
5. Do not re-parse — use the stored byte range directly

### read

See Read Cache Behavior in the Index section above.

---

## TOON Output Implementation

Implement directly in `output.rs` — no external crate.

```rust
pub fn toon_table(name: &str, fields: &[&str], rows: &[Vec<String>]) -> String {
    let mut out = format!("{}[{}]{{{}}}:\n", name, rows.len(), fields.join(","));
    for row in rows {
        let cells: Vec<String> = row.iter().map(|v| toon_escape(v)).collect();
        out.push_str(&format!("  {}\n", cells.join(",")));
    }
    out
}

pub fn toon_escape(value: &str) -> String {
    if value.contains(',') || value.contains('\n') || value.contains('"') {
        format!("\"{}\"", value.replace('"', "\\\""))
    } else {
        value.to_string()
    }
}

pub fn toon_scalar(fields: &[(&str, &str)]) -> String {
    fields.iter().map(|(k, v)| {
        if v.contains('\n') {
            format!("{}: |\n{}\n", k,
                v.lines().map(|l| format!("  {}", l)).collect::<Vec<_>>().join("\n"))
        } else {
            format!("{}: {}\n", k, v)
        }
    }).collect()
}
```

---

## Project Structure

```
cx/
├── Cargo.toml
├── src/
│   ├── main.rs              # CLI entry, clap definitions, dispatch
│   ├── index.rs             # Index struct, load/save/invalidate, session ID
│   ├── query.rs             # symbols, definition, read query logic
│   ├── output.rs            # TOON and JSON formatting
│   ├── grep.rs              # subprocess passthrough for cx grep
│   ├── language/
│   │   ├── mod.rs           # LanguageModule trait, extension → language detection
│   │   ├── rust.rs
│   │   ├── typescript.rs
│   │   └── python.rs
│   └── util/
│       ├── git.rs           # walk up to find .git for project root
│       └── glob.rs          # simple * and ? glob (~20 lines)
```

---

## Error Handling

- **Parse error on a single file:** log warning to stderr, skip file, continue
- **File not found during query:** log to stderr, return empty results
- **No results:** exit code 2, print nothing to stdout
- **Invalid arguments:** clap handles, exit code 1
- **Index version mismatch:** silently rebuild

---

## Testing

```bash
cargo build --release

# overview
cx overview src/main.rs

# definition
cx definition --name calculate_fee
cx definition --name foo --from src/swap.rs

# read cache
cx read src/fees.rs        # first read: full content
cx read src/fees.rs        # re-read unchanged: "status: unchanged"
# edit the file
cx read src/fees.rs        # re-read changed: full new content
cx read src/fees.rs --fresh  # always full content, regardless of cache

# symbols
cx symbols --kind fn --name "execute*"
cx symbols --file src/amm.rs

# grep passthrough
cx grep -rn calculate_fee src/
```

---

## Claude Code Hook Integration

A PreToolUse hook makes `cx grep` adoption automatic — existing agent prompts that use
grep or rg work without modification.

```json
{
  "hooks": {
    "PreToolUse": [{
      "matcher": "Bash",
      "hooks": [{ "type": "command", "command": "python3 /path/to/cx_hook.py" }]
    }]
  }
}
```

```python
# cx_hook.py — rewrites grep/rg invocations to cx grep
import json, sys, re

data = json.load(sys.stdin)
cmd = data.get("tool_input", {}).get("command", "")

if re.match(r'\s*(grep|rg)\s', cmd):
    cmd = re.sub(r'\s*(grep|rg)\s', ' cx grep ', cmd, count=1)
    data["tool_input"]["command"] = cmd

print(json.dumps(data))
```

---

## System Prompt Snippet for Agents

```
cx — semantic code index. Use instead of reading files where possible.

The escalation hierarchy (start cheap):
  cx overview PATH          — file table of contents, ~200 chars. Use before Read.
  cx definition --name X    — just the function/type body. Use instead of Read.
  cx read PATH              — full file with session cache. Use when you need everything.

cx overview PATH
  All symbols + signatures for a file. Run this before deciding to Read a file.

cx symbols [--file PATH] [--name GLOB] [--kind KIND]
  Search symbols across project. KIND: fn|struct|enum|trait|type|const|class|interface

cx definition --name NAME [--from FILE] [--max-lines N]
  Get a function/type body without reading the whole file.
  --from FILE disambiguates when multiple files define the same name.
  --max-lines N (default 200) truncates large bodies. Check truncated: true in output.

cx read PATH [--fresh]
  Full file with session cache. Returns "status: unchanged" + hash if already read
  this session and file hasn't changed — use for edit-verify cycles. Use --fresh when
  you need the content back in context regardless of whether it changed.

cx grep [grep flags] PATTERN [PATH]
  grep-compatible passthrough to rg.

Output is TOON: arrays as "name[N]{fields}:" with one row per line, scalars as "key: value".
```
