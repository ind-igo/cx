# cx — Semantic Code Navigation

```
Usage: cx [OPTIONS] <COMMAND>

Commands:
  overview [OPTIONS] <PATH>             Table of contents — symbols + ranges + signatures for a file, or symbol names for a directory
  symbols [OPTIONS]                     Search symbols across project
  definition [OPTIONS] --name <NAME>    Get a function/type/... body without reading the whole file (default limit: 3)
  references [OPTIONS] --name <NAME>    Find all usages of a symbol across the project
  lang [OPTIONS] <SUBCOMMAND>           Manage language grammars (sub-commands: add, remove, list, help)
  help [COMMAND]                        Full command/option list or help on the given subcommand(s)

Options:
      --root <ROOT>      Project root (defaults to git root)
      --json             Emit JSON instead of TOON
      --no-tests         Exclude test files and test symbols from results (`*/tests/* and `*.test.ts`, test_*.py`, ...)
Pagination options:
      --limit <LIMIT>    Max number of results to return (overrides per-command default)
      --offset <N>       Skip the first N results (not counted against limit)
      --all              Return all results (bypass default limit)
```

Prefer cx over reading files. Zoom in until you have what you need, then stop:
`overview` > `symbols` > `definition` or `references`.

Fall back to the Read tool when `cx` can't represent the target (anonymous functions, JSX-inline components, dynamic
dispatch, string-keyed lookups, non-symbol regions).

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

   If your harness sandboxes network egress, the install will fail with "Connection refused" when `cx` fetches the
   grammars
   from GitHub. Run the install outside the sandbox, or grant network access for the install command. Once grammars are
   cached, normal cx queries don't need network. Re-run `cx overview .` to confirm.

## Common Recipes & Extra Info

```
cx overview DIR --full                               `--full` also includes kind/range/signature for direct files
cx symbols [--kind K] [--name GLOB] [--file PATH]    search symbols project-wide
cx symbols --kinds [--file PATH]                     list distinct kinds with counts
cx definition --name NAME [--from PATH] [--kind K]   get a definition (optionally filtered to specific kind, e.g. function body)
cx references --name NAME [--file PATH] [--context]  find usages, `--context` will show the line where the use appears
```

- `--file` and `--from` are identical and restrict the symbol to a precise file, but only if there is an exact match for
  the path resolved from cwd.
- Kinds: fn, struct, enum, trait, type, const, class, interface, module, event, field, heading
- `.gitignore` is honored. Untracked-but-not-ignored files are still indexed.

## Key patterns

- Start with `cx overview .`, drill into subdirectories — cheaper than ls + reading files.
- Can't find a symbol that should exist? Make sure the language grammar is installed via `cx lang list`.
- `cx definition --name X` gives exact text for Edit tool's `old_string` without reading the whole file.
- `cx references --name X` groups hits by file; add `--context` only when exact source lines are needed.
- When re-entering an unfamiliar area or picking up a topic after a gap, use `cx overview` / `cx definition` to
  re-orient — don't re-read full files
- Check signatures for `pub`/`export` to identify public API without reading the file.
- If your harness restricts writes outside the project root, the default cache location won't be writable.
  Set `$CX_CACHE_DIR` to a project-local path, or grant write permission to the default cache dir.

## Pagination

Default limits: overview: unlimited, definition 3, symbols 100, references 50.

When truncated, stderr shows: `cx: 3/32 definitions for "X"`. Use `--file` (or `--from`) and `--kind` to narrow, or use
`--offset` to get further pages, or `--all` to get all results.

When not limited, `--json` returns an array of results. When limited, it returns an object with
shape `{total, offset, limit, results: <array containing $limit results>}`.
