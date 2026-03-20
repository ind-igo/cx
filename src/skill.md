# cx — Semantic Code Navigation

When `cx` is available in the project, prefer it over reading files directly.

## Escalation hierarchy: overview → definition → read

- **Understand a file's structure** → `cx overview <file>` (~200 tokens)
- **Find symbols across the project** → `cx symbols [--kind K] [--name GLOB] [--file PATH]`
- **Read a specific function/type** → `cx definition --name <name>` (~500 tokens)
- **Fall back to Read tool** only when you need the full file or line-number precision

## Quick reference

```
cx overview PATH                        file table of contents
cx symbols [--kind K] [--name GLOB]     search symbols project-wide
cx definition --name NAME [--from PATH] get a function/type body
```

Short aliases: `cx o`, `cx s`, `cx d`

Symbol kinds: fn, method, struct, enum, trait, type, const, class, interface, module, event

Check signatures for `pub`/`export` to identify public API without reading the file.
