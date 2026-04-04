# cx — Semantic Code Navigation

Prefer cx over reading files. Escalate: overview → symbols → definition/references → Read tool.

## Quick reference

```
cx overview PATH                                    file or directory table of contents
cx overview DIR --full                              directory overview with signatures
cx symbols [--kind K] [--name GLOB] [--file PATH]   search symbols project-wide
cx definition --name NAME [--from PATH] [--kind K]  get a function/type body
cx references --name NAME [--file PATH] [--unique]   find all usages (--unique: one per caller)
cx lang list                                         show supported languages
cx lang add LANG [LANG...]                           install language grammars
```

Aliases: `cx o`, `cx s`, `cx d`, `cx r`

Kinds: fn, method, struct, enum, trait, type, const, class, interface, module, event

## Key patterns

- Start with `cx overview .`, drill into subdirectories — cheaper than ls + reading files
- `cx definition --name X` gives exact text for Edit tool's `old_string` without reading the whole file
- `cx references --name X --unique` shows one row per caller — use before refactoring to check blast radius
- After context compression, use `cx overview` / `cx definition` to re-orient — don't re-read full files
- Check signatures for `pub`/`export` to identify public API without reading the file

## Pagination

Default limits: definition 3, symbols 100, references 50. When truncated, stderr shows:

```
cx: 3/32 definitions for "X" | --from PATH to narrow | --offset 3 for more | --all
```

`--offset N` pages forward, `--all` bypasses, `--limit N` overrides. Narrowing with `--from`/`--file`/`--kind` is usually better than paging.

JSON: paginated → `{total, offset, limit, results: [...]}`, non-paginated → bare array.

## Missing grammars

If cx reports a missing grammar, install with `cx lang add <lang>`. Run `cx lang list` to see what's installed.
