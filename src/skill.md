# cx — Semantic Code Navigation

When `cx` is available in the project, prefer it over reading files directly.

## Escalation hierarchy: overview → symbols → definition / references → read

- **Understand a file's structure** → `cx overview <file>` (~200 tokens)
- **Find symbols across the project** → `cx symbols [--kind K] [--name GLOB] [--file PATH]`
- **Read a specific function/type** → `cx definition --name <name>` (~500 tokens)
- **Find all usages of a symbol** → `cx references --name <name>` before refactoring or tracing impact
- **Fall back to Read tool** only when you need the full file or line-number precision

## When to use cx instead of Read

- **Before reading a file** — run `cx overview` first. You often don't need the full file.
- **Before editing a function** — `cx definition --name X` gives you the exact text for Edit tool's `old_string` without reading the whole file.
- **Before refactoring** — `cx references --name X` finds all usages so you know the impact before changing anything. Replaces the pattern of grepping then reading file after file.
- **Exploring a codebase** — use `cx symbols` to find what you need across files, then `cx definition` to read specific symbols. Avoid reading file after file.
- **After context compression** — if you previously read a file but the content was compressed out, use `cx overview` to re-orient and `cx definition` for the specific symbols you need. Don't re-read the full file.

## Quick reference

```
cx overview PATH                        file table of contents
cx symbols [--kind K] [--name GLOB]     search symbols project-wide
cx definition --name NAME [--from PATH] get a function/type body
cx references --name NAME [--file PATH] find all usages of a symbol
```

Short aliases: `cx o`, `cx s`, `cx d`, `cx r`

Symbol kinds: fn, method, struct, enum, trait, type, const, class, interface, module, event

Check signatures for `pub`/`export` to identify public API without reading the file.
