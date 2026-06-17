# cx — Semantic Code Navigation

Prefer cx over reading files. Escalate: overview → symbols → definition/references → Read tool.

## First-run checks (once per session)

1. **Is cx installed?** Run `command -v cx`. If absent, stop and ask the user to install it — do not attempt the install
   yourself. Canonical commands:
    - Homebrew: `brew tap ind-igo/cx && brew install cx`
    - Cargo: `cargo install cx-cli`
    - Shell (Linux/macOS): `curl -sL https://raw.githubusercontent.com/ind-igo/cx/master/install.sh | sh`

2. **Are this project's grammars installed?** Just run `cx overview .` as your first probe. If grammars are missing the
   output is self-diagnosing — it prints the detected project languages and the exact install command, e.g.:

   ```
   cx: no language grammars installed
   Detected languages in this project:
     typescript (37 files)
     markdown (7 files)
   Install with: cx lang add typescript markdown
   ```

   Run that suggested command **with the sandbox disabled** (`dangerouslyDisableSandbox: true`) — cx fetches grammars
   from
   GitHub releases and its Rust HTTP client doesn't honor the sandbox HTTP(S)_PROXY env vars, so it hits "Connection
   refused" inside the sandbox. Once grammars are installed, all cx queries work inside the sandbox (reads don't need
   network). Re-run `cx overview .` to confirm.

## Quick reference

```
cx overview PATH                                     file or directory table of contents
cx overview DIR --full                               directory overview with ranges + signatures
cx symbols [--kind K] [--name GLOB] [--file PATH]    search symbols project-wide
cx symbols --kinds [--file PATH]                     list distinct kinds with counts
cx definition --name NAME [--from PATH] [--kind K]   get a function/type body
cx references --name NAME [--file PATH] [--context]  usages grouped by file; --context exact lines
cx lang list                                         show supported languages
cx lang add LANG [LANG...]                           install language grammars

Global: --no-tests (exclude test files/symbols), --json, --limit N, --offset N, --all
```

Aliases: `cx o`, `cx s`, `cx d`, `cx r`

Kinds: fn, struct, enum, trait, type, const, class, interface, module, event, heading

## Key patterns

- Start with `cx overview .`, drill into subdirectories — cheaper than ls + reading files
- `cx definition --name X` gives exact text for Edit tool's `old_string` without reading the whole file
- `cx references --name X` groups hits by file; add `--context` only when exact source lines are needed
- After context compression, use `cx overview` / `cx definition` to re-orient — don't re-read full files
- Check signatures for `pub`/`export` to identify public API without reading the file

## Pagination

Default limits: definition 3, symbols 100, references 50. When truncated, stderr shows:

```
cx: 3/32 definitions for "X" | --from PATH to narrow | --offset 3 for more | --all
```

`--offset N` pages forward, `--all` bypasses, `--limit N` overrides. Narrow with `--from`/`--file`/`--kind` before
paging.

JSON: paginated → `{total, offset, limit, results: [...]}`, non-paginated → bare array.

## Missing grammars

If cx reports a missing grammar, install with `cx lang add <lang>`. Run `cx lang list` to see what's installed.
