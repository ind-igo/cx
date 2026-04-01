# cx — Semantic Code Navigation

When `cx` is available in the project, prefer it over reading files directly.

## Escalation hierarchy: directory overview → file overview → symbols → definition / references → read

- **Explore a directory** → `cx overview <dir>` (~20 tokens per entry)
- **Understand a file's structure** → `cx overview <file>` (~200 tokens)
- **Find symbols across the project** → `cx symbols [--kind K] [--name GLOB] [--file PATH]`
- **Read a specific function/type** → `cx definition --name <name>` (~500 tokens)
- **Find all usages of a symbol** → `cx references --name <name>` before refactoring or tracing impact
- **Fall back to Read tool** only when you need the full file or surrounding context beyond the symbol body

## When to use cx instead of Read

- **Exploring a new codebase** — start with `cx overview .` to see top-level structure, then drill into subdirectories. Cheaper than `ls` + reading files.
- **Before reading a file** — run `cx overview` first. You often don't need the full file.
- **Before editing a function** — `cx definition --name X` gives you the exact text for Edit tool's `old_string` without reading the whole file.
- **Before refactoring** — `cx references --name X` finds all usages so you know the impact before changing anything. Replaces the pattern of grepping then reading file after file.
- **Exploring a codebase** — use `cx symbols` to find what you need across files, then `cx definition` to read specific symbols. Avoid reading file after file.
- **After context compression** — if you previously read a file but the content was compressed out, use `cx overview` to re-orient and `cx definition` for the specific symbols you need. Don't re-read the full file.

## Quick reference

```
cx overview PATH                                    file or directory table of contents
cx overview DIR --full                              directory overview with signatures
cx symbols [--kind K] [--name GLOB] [--file PATH]   search symbols project-wide
cx definition --name NAME [--from PATH] [--kind K]  get a function/type body
cx references --name NAME [--file PATH]              find all usages of a symbol
cx lang list                                         show supported languages
cx lang add LANG [LANG...]                           install language grammars
```

Short aliases: `cx o`, `cx s`, `cx d`, `cx r`

Symbol kinds: fn, method, struct, enum, trait, type, const, class, interface, module, event

Check signatures for `pub`/`export` to identify public API without reading the file.

## Missing grammars

If cx reports a missing grammar (e.g. `cx: rust grammar not installed`), install it with `cx lang add rust`. Run `cx lang list` to see what's installed.
