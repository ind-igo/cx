# Query Improvement Plan

Improve tree-sitter queries for under-covered languages, following the Swift/Dart/Elixir pattern: expand queries, add comprehensive tests, validate against real codebases.

## Order

1. **TypeScript** — class properties, export default, abstract methods, decorators, namespace (in progress)
2. **Python** — method vs function distinction, @property/@staticmethod/@classmethod, nested classes
3. **Go** — interfaces, const/var blocks
4. **Rust** — const/static items, associated types, impl blocks as containers
5. **Java** — constructors, records, annotations, inner classes
6. **C++** — namespaces, templates, constructors/destructors
7. **Solidity** — modifiers, error definitions
8. **Bash/Lua/Ruby** — smaller gaps, lower priority

## Validation repos

- TypeScript: `/Users/indigo/projects/baseline/baseline-monorepo/`
