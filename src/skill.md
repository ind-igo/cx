# cx — semantic code navigation

Use `cx` instead of reading files when you need to understand code structure. It indexes the codebase with tree-sitter and returns symbols, signatures, and definitions at a fraction of the token cost.

## When to use

- Finding what's in a file → `cx overview`
- Searching for symbols across the project → `cx symbols`
- Reading a specific function or type body → `cx definition`
- Reading a full file (with caching) → `cx read`

Prefer cx over raw file reads. Escalate only when needed: **overview → definition → read**.

## Commands

```
cx overview PATH                       File table of contents — all symbols + signatures (~200 tokens)
cx symbols [--kind K] [--name GLOB] [--file PATH]
                                       Search symbols across the project
cx definition --name NAME [--from PATH] [--max-lines N]
                                       Get a function/type body without reading the whole file
cx read PATH                           Full file with session cache (returns "unchanged" if unmodified)
cx read PATH --fresh                   Always return full content, skip cache check
```

### Symbol kinds

`fn`, `method`, `struct`, `enum`, `trait`, `type`, `const`, `class`, `interface`, `module`, `event`

### Global flags

- `--json` — emit JSON instead of TOON
- `--root PATH` — override project root (default: git root)

## Examples

```bash
# What's in this file?
cx overview src/server.rs

# Find all structs in the project
cx symbols --kind struct

# Find functions matching a pattern
cx symbols --kind fn --name "handle_*"

# Get a function body
cx definition --name handle_request

# Get a function body, disambiguate by file
cx definition --name new --from src/config.rs

# Read a file (cached — second call returns ~20 tokens if unchanged)
cx read src/server.rs
```

## Tips

- Check signatures for `pub`/`export` to identify public API without reading the file
- Use `--name` glob patterns to narrow symbol search (e.g., `--name "*Error"`)
- `cx read` caches per-session — repeated reads of unmodified files return "unchanged" (~20 tokens)
- `cx read` automatically detects edits via content hash — no need for `--fresh` after changes

## Supported languages

Rust, TypeScript, JavaScript, Python, Go, C, C++, Java, Ruby, C#, Lua, Zig, Bash, Solidity, Elixir
