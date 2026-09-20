# Bundled languages and local overrides

Mojo and Bend 2 parsers and these symbol queries ship inside the cx binary.
Install them just like other languages, including offline:

```sh
cx lang add mojo bend
cx lang list
cx lang remove mojo
```

Installation enables the bundled parser in the cx cache (`CX_CACHE_DIR`
applies). Removal disables it; the compiled parser remains part of the binary.
Mojo uses the pinned [vendored parser](../vendor/tree-sitter-mojo/README.md);
Bend uses the pinned `tree-sitter-bend2` Cargo dependency. Both are independent
of the language pack. `cargo build` includes them without a Tree-sitter CLI or
build-time grammar downloads.

## Custom parser overrides

The manifests in this directory are examples for replacing a bundled parser.
They are optional: normal installation only needs `cx lang add mojo bend`.
cx does not load configuration or native libraries automatically from a project
directory.

Build a parser with the [Tree-sitter CLI](https://tree-sitter.github.io/tree-sitter/cli/build.html)
and a C compiler, then install the matching definition. Run these commands from
the cx checkout, replacing `/tmp` with a suitable directory on Windows:

```sh
git clone https://github.com/dmitry-salin/tree-sitter-mojo /tmp/tree-sitter-mojo
git -C /tmp/tree-sitter-mojo checkout 429918d92978087aae3e2c4b00242b8270d9ad6f
tree-sitter build --output languages/mojo/parser.so /tmp/tree-sitter-mojo
cx lang add mojo --from languages/mojo

git clone https://github.com/amaanq/tree-sitter-bend /tmp/tree-sitter-bend
git -C /tmp/tree-sitter-bend checkout f47602e1511546e2a5b4a76e3553c68c1952ab8f
tree-sitter build --output languages/bend/parser.so /tmp/tree-sitter-bend
cx lang add bend --from languages/bend
```

The output filename `parser.so` is the name used by these manifests; the compiled
library must target your operating system and architecture. cx copies it under
the platform's library filename during installation. Neither the Mojo nor the
Bend compiler is needed to navigate code.

Mojo captures functions, methods, structs, traits, and module/associated
`comptime` declarations. Bend captures functions, datatypes, constructors, and
laws. A law and its proof both appear under their shared name, with separate
source ranges and signatures. References are textual identifier matches, as
with cx's other languages.

## Directory format

A local override contains a `language.json`, a native parser, and a symbol query:

```json
{
  "name": "bend",
  "extensions": ["bend"],
  "grammar": "bend",
  "library": "parser.so",
  "query": "symbols.scm",
  "ref_node_types": ["identifier", "scoped_identifier"],
  "sig_body_child": "body"
}
```

- `name` matches the argument to `cx lang add`. An existing name replaces that
  language's full configuration; a new name registers a new language.
- `grammar` is the suffix of the exported `tree_sitter_<grammar>` function.
- `library` and `query` are paths relative to this directory (absolute paths also
  work). The query must capture `@name` and `@definition.<kind>` using cx's
  [capture conventions](../README.md#adding-a-language).
- `extensions` replaces the complete suffix list for this language. A suffix
  owned by another language is rejected; override that language by name instead.
- `ref_node_types` names identifier nodes, including compound identifiers if
  the grammar uses them. An empty list disables references.
- Optional `sig_body_child` names a body node or field. `sig_delimiter` is an
  optional byte value, such as `123` for `{`, used when no body field is available.
- Optional `kind_overrides` contains `[capture, node_kind, symbol_kind]` triples,
  following cx's existing language configuration. An empty node kind matches any
  node. Symbol kinds include `fn`, `struct`, `trait`, `type`, and `const`.

Parser compatibility, queries, reference nodes, and body settings are checked
before activation. Install only native parsers you trust: loading a shared
library executes native code.

## Updating and removing

```sh
cx lang list
cx lang add mojo --from languages/mojo
cx lang remove mojo --override
```

Installation copies the parser, query, and settings into
`<cx cache>/overrides/` (`CX_CACHE_DIR` applies). Source directories can be removed
after installation. Editing the source requires reinstalling. An invalid
replacement leaves the active installation intact. Installed overrides take
precedence over bundled and pack parsers, and normal navigation never downloads
their parsers.

`lang list` shows the original local source path for overrides. Removing an
override restores the default configuration and any already installed bundled
or pack parser; a custom language with no default becomes unsupported again. Use
plain `cx lang add NAME` to install a missing default parser after removing an override.

Indexes rebuild on the next query when installed parser, query, or settings
contents change, even if source file timestamps have not changed. A broken
active override produces an error, rather than falling back or using stale
results. Old parser snapshots are retained so processes already using them can
finish; removing an override unregisters it without deleting those snapshots.

## Checking these definitions

The bundled languages are checked from a copied, standalone binary with an
empty cache and a blocked network proxy:

```sh
cargo test --test bundled --test overrides
```

This checks installation, removal, cache invalidation, symbol kinds, full
definitions, references, Mojo escaped identifiers, Bend's separate law/proof
definitions, and restoring a bundled parser after removing a local override.
