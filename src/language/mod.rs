use crate::index::{Symbol, SymbolKind};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{LazyLock, Mutex};
use tree_sitter::{Node, Parser, Query, QueryCursor, StreamingIterator};

/// Cache compiled queries keyed by resolved grammar name (e.g. "rust", "tsx").
static QUERY_CACHE: LazyLock<Mutex<HashMap<String, Query>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

// --- Language registry ---

struct LanguageConfig {
    name: &'static str,
    extensions: &'static [&'static str],
    /// Map certain file extensions to a different grammar name (e.g. tsx → "tsx").
    grammar_override: &'static [(&'static str, &'static str)],
    /// Names to pass to `tree_sitter_language_pack::download()`. Empty = use name.
    download_names: &'static [&'static str],
    query: &'static str,
    /// Find this child node kind to determine where the body starts; signature = text before it.
    sig_body_child: Option<&'static str>,
    /// Scan for this byte to split signature from body (e.g. b'{').
    sig_delimiter: Option<u8>,
    /// (capture_name, node_kind, SymbolKind) — checked before defaults.
    /// Empty node_kind matches any node.
    kind_overrides: &'static [(&'static str, &'static str, SymbolKind)],
    /// Node kinds that represent identifier references (for find-references).
    ref_node_types: &'static [&'static str],
}

// --- Inlined TAGS_QUERY from tree-sitter-rust (pinned) ---

const RUST_QUERY: &str = r#"
(struct_item
    name: (type_identifier) @name) @definition.class

(enum_item
    name: (type_identifier) @name) @definition.class

(union_item
    name: (type_identifier) @name) @definition.class

(type_item
    name: (type_identifier) @name) @definition.class

(declaration_list
    (function_item
        name: (identifier) @name) @definition.method)

(function_item
    name: (identifier) @name) @definition.function

(trait_item
    name: (type_identifier) @name) @definition.interface

(mod_item
    name: (identifier) @name) @definition.module

(macro_definition
    name: (identifier) @name) @definition.macro
"#;

// --- Inlined TAGS_QUERY from tree-sitter-python (adapted for new grammar) ---

const PY_QUERY: &str = r#"
(module (assignment left: (identifier) @name) @definition.constant)

(class_definition
  name: (identifier) @name) @definition.class

(function_definition
  name: (identifier) @name) @definition.function
"#;

// --- Custom queries ---

const TS_QUERY: &str = r#"
(function_declaration
  name: (identifier) @name) @definition.function

(class_declaration
  name: (type_identifier) @name) @definition.class

(method_definition
  name: (property_identifier) @name) @definition.method

(interface_declaration
  name: (type_identifier) @name) @definition.interface

(type_alias_declaration
  name: (type_identifier) @name) @definition.type

(enum_declaration
  name: (identifier) @name) @definition.enum

(module
  name: (identifier) @name) @definition.module

(lexical_declaration
  (variable_declarator
    name: (identifier) @name
    value: (arrow_function))) @definition.function

(variable_declaration
  (variable_declarator
    name: (identifier) @name
    value: (arrow_function))) @definition.function
"#;

const GO_QUERY: &str = r#"
(function_declaration
  name: (identifier) @name) @definition.function

(method_declaration
  name: (field_identifier) @name) @definition.method

(type_spec
  name: (type_identifier) @name) @definition.type
"#;

const C_QUERY: &str = r#"
(function_definition
  declarator: (function_declarator
    declarator: (identifier) @name)) @definition.function

(function_definition
  declarator: (pointer_declarator
    declarator: (function_declarator
      declarator: (identifier) @name))) @definition.function

(struct_specifier
  name: (type_identifier) @name
  body: (_)) @definition.class

(enum_specifier
  name: (type_identifier) @name) @definition.enum

(type_definition
  declarator: (type_identifier) @name) @definition.type
"#;

const CPP_QUERY: &str = r#"
(function_definition
  declarator: (function_declarator
    declarator: (identifier) @name)) @definition.function

(function_definition
  declarator: (pointer_declarator
    declarator: (function_declarator
      declarator: (identifier) @name))) @definition.function

(function_definition
  declarator: (function_declarator
    declarator: (field_identifier) @name)) @definition.method

(function_definition
  declarator: (function_declarator
    declarator: (qualified_identifier
      name: (identifier) @name))) @definition.method

(struct_specifier
  name: (type_identifier) @name
  body: (_)) @definition.class

(class_specifier
  name: (type_identifier) @name) @definition.class

(enum_specifier
  name: (type_identifier) @name) @definition.enum

(type_definition
  declarator: (type_identifier) @name) @definition.type
"#;

const JAVA_QUERY: &str = r#"
(class_declaration
  name: (identifier) @name) @definition.class

(method_declaration
  name: (identifier) @name) @definition.method

(interface_declaration
  name: (identifier) @name) @definition.interface

(enum_declaration
  name: (identifier) @name) @definition.enum
"#;

const RUBY_QUERY: &str = r#"
(method
  name: (_) @name) @definition.method

(singleton_method
  name: (_) @name) @definition.method

(class
  name: (constant) @name) @definition.class

(module
  name: (constant) @name) @definition.module
"#;

const LUA_QUERY: &str = r#"
(function_declaration
  name: (identifier) @name) @definition.function

(function_declaration
  name: (dot_index_expression
    field: (identifier) @name)) @definition.function

(function_declaration
  name: (method_index_expression
    method: (identifier) @name)) @definition.method
"#;

const ZIG_QUERY: &str = r#"
(Decl
  (FnProto
    (IDENTIFIER) @name)) @definition.function

(Decl
  (VarDecl
    (IDENTIFIER) @name
    (ErrorUnionExpr
      (SuffixExpr
        (ContainerDecl
          (ContainerDeclType
            "struct")))))) @definition.class

(Decl
  (VarDecl
    (IDENTIFIER) @name
    (ErrorUnionExpr
      (SuffixExpr
        (ContainerDecl
          (ContainerDeclType
            "enum")))))) @definition.enum

(Decl
  (VarDecl
    (IDENTIFIER) @name
    (ErrorUnionExpr
      (SuffixExpr
        (ContainerDecl
          (ContainerDeclType
            "union")))))) @definition.class

(Decl
  (VarDecl
    (IDENTIFIER) @name
    (ErrorUnionExpr
      (SuffixExpr
        (ErrorSetDecl))))) @definition.enum
"#;

const BASH_QUERY: &str = r#"
(function_definition
  name: (word) @name) @definition.function
"#;

const SOL_QUERY: &str = r#"
(contract_declaration
  name: (identifier) @name) @definition.class

(interface_declaration
  name: (identifier) @name) @definition.interface

(library_declaration
  name: (identifier) @name) @definition.module

(function_definition
  name: (identifier) @name) @definition.function

(struct_declaration
  name: (identifier) @name) @definition.class

(enum_declaration
  name: (identifier) @name) @definition.enum

(event_definition
  name: (identifier) @name) @definition.event
"#;

const ELIXIR_QUERY: &str = r#"
(call
  target: (identifier) @_keyword
  (arguments (alias) @name)
  (#any-of? @_keyword "defmodule" "defprotocol" "defimpl")) @definition.module

(call
  target: (identifier) @_keyword
  (arguments
    [(identifier) @name
     (call target: (identifier) @name)
     (binary_operator left: (call target: (identifier) @name))])
  (#any-of? @_keyword "def" "defp" "defmacro" "defmacrop" "defguard" "defguardp" "defdelegate")) @definition.function

(unary_operator
  operand: (call
    target: (identifier) @_keyword
    (arguments
      (binary_operator
        left: (identifier) @name)))
  (#any-of? @_keyword "type" "typep" "opaque")) @definition.type

(unary_operator
  operand: (call
    target: (identifier) @_keyword
    (arguments
      (binary_operator
        left: (call target: (identifier) @name)))
    (#eq? @_keyword "callback"))) @definition.method
"#;

const SWIFT_QUERY: &str = r#"
; --- Type declarations ---

(class_declaration
  "class"
  name: (type_identifier) @name) @definition.class

(class_declaration
  "struct"
  name: (type_identifier) @name) @definition.struct

(class_declaration
  "enum"
  name: (type_identifier) @name) @definition.enum

(class_declaration
  "actor"
  name: (type_identifier) @name) @definition.class

(class_declaration
  "extension"
  name: _ @name) @definition.module

(protocol_declaration
  name: (type_identifier) @name) @definition.interface

(typealias_declaration
  name: (type_identifier) @name) @definition.type

; --- Methods (functions inside type bodies) ---

(class_body
  (function_declaration
    name: (simple_identifier) @name) @definition.method)

(enum_class_body
  (function_declaration
    name: (simple_identifier) @name) @definition.method)

(protocol_body
  (protocol_function_declaration
    name: (simple_identifier) @name) @definition.method)

; --- Init / Deinit ---

(class_body
  (init_declaration
    name: _ @name) @definition.method)

(enum_class_body
  (init_declaration
    name: _ @name) @definition.method)

(class_body
  (deinit_declaration
    "deinit" @name) @definition.method)

; --- Subscripts ---

(class_body
  (subscript_declaration
    "subscript" @name) @definition.method)

(enum_class_body
  (subscript_declaration
    "subscript" @name) @definition.method)

; --- Properties ---

(class_body
  (property_declaration
    name: (pattern
      bound_identifier: (simple_identifier) @name)) @definition.constant)

(enum_class_body
  (property_declaration
    name: (pattern
      bound_identifier: (simple_identifier) @name)) @definition.constant)

(protocol_body
  (protocol_property_declaration
    name: (pattern
      bound_identifier: (simple_identifier) @name)) @definition.constant)

; --- Top-level functions ---

(function_declaration
  name: (simple_identifier) @name) @definition.function
"#;

// --- Registry ---

static LANGUAGES: &[LanguageConfig] = &[
    LanguageConfig {
        name: "rust",
        extensions: &["rs"],
        grammar_override: &[],
        download_names: &[],
        query: RUST_QUERY,
        sig_body_child: None,
        sig_delimiter: Some(b'{'),
        kind_overrides: &[
            ("definition.class", "struct_item", SymbolKind::Struct),
            ("definition.class", "enum_item", SymbolKind::Enum),
            ("definition.class", "union_item", SymbolKind::Struct),
            ("definition.class", "type_item", SymbolKind::Type),
            ("definition.class", "", SymbolKind::Struct),
            ("definition.interface", "", SymbolKind::Trait),
            ("definition.macro", "", SymbolKind::Fn),
        ],
        ref_node_types: &["identifier", "type_identifier", "field_identifier"],
    },
    LanguageConfig {
        name: "typescript",
        extensions: &["ts", "tsx", "js", "jsx"],
        grammar_override: &[("tsx", "tsx"), ("jsx", "tsx")],
        download_names: &["typescript", "tsx"],
        query: TS_QUERY,
        sig_body_child: None,
        sig_delimiter: Some(b'{'),
        kind_overrides: &[],
        ref_node_types: &["identifier", "type_identifier", "property_identifier", "shorthand_property_identifier", "shorthand_property_identifier_pattern"],
    },
    LanguageConfig {
        name: "python",
        extensions: &["py"],
        grammar_override: &[],
        download_names: &[],
        query: PY_QUERY,
        sig_body_child: Some("block"),
        sig_delimiter: None,
        kind_overrides: &[],
        ref_node_types: &["identifier"],
    },
    LanguageConfig {
        name: "go",
        extensions: &["go"],
        grammar_override: &[],
        download_names: &[],
        query: GO_QUERY,
        sig_body_child: None,
        sig_delimiter: Some(b'{'),
        kind_overrides: &[],
        ref_node_types: &["identifier", "type_identifier", "field_identifier"],
    },
    LanguageConfig {
        name: "c",
        extensions: &["c"],
        grammar_override: &[],
        download_names: &[],
        query: C_QUERY,
        sig_body_child: None,
        sig_delimiter: Some(b'{'),
        kind_overrides: &[
            ("definition.class", "", SymbolKind::Struct),
        ],
        ref_node_types: &["identifier", "type_identifier", "field_identifier"],
    },
    LanguageConfig {
        name: "cpp",
        extensions: &["cpp", "cc", "cxx", "h", "hpp", "hxx", "hh"],
        grammar_override: &[],
        download_names: &[],
        query: CPP_QUERY,
        sig_body_child: None,
        sig_delimiter: Some(b'{'),
        kind_overrides: &[],
        ref_node_types: &["identifier", "type_identifier", "field_identifier"],
    },
    LanguageConfig {
        name: "java",
        extensions: &["java"],
        grammar_override: &[],
        download_names: &[],
        query: JAVA_QUERY,
        sig_body_child: None,
        sig_delimiter: Some(b'{'),
        kind_overrides: &[],
        ref_node_types: &["identifier"],
    },
    LanguageConfig {
        name: "ruby",
        extensions: &["rb"],
        grammar_override: &[],
        download_names: &[],
        query: RUBY_QUERY,
        sig_body_child: None,
        sig_delimiter: None,
        kind_overrides: &[],
        ref_node_types: &["identifier", "constant"],
    },
    LanguageConfig {
        name: "lua",
        extensions: &["lua"],
        grammar_override: &[],
        download_names: &[],
        query: LUA_QUERY,
        sig_body_child: None,
        sig_delimiter: None,
        kind_overrides: &[],
        ref_node_types: &["identifier"],
    },
    LanguageConfig {
        name: "zig",
        extensions: &["zig"],
        grammar_override: &[],
        download_names: &[],
        query: ZIG_QUERY,
        sig_body_child: None,
        sig_delimiter: Some(b'{'),
        kind_overrides: &[
            ("definition.class", "Decl", SymbolKind::Struct),
        ],
        ref_node_types: &["IDENTIFIER"],
    },
    LanguageConfig {
        name: "bash",
        extensions: &["sh", "bash"],
        grammar_override: &[],
        download_names: &[],
        query: BASH_QUERY,
        sig_body_child: None,
        sig_delimiter: Some(b'{'),
        kind_overrides: &[],
        ref_node_types: &["word"],
    },
    LanguageConfig {
        name: "solidity",
        extensions: &["sol"],
        grammar_override: &[],
        download_names: &[],
        query: SOL_QUERY,
        sig_body_child: None,
        sig_delimiter: Some(b'{'),
        kind_overrides: &[],
        ref_node_types: &["identifier"],
    },
    LanguageConfig {
        name: "elixir",
        extensions: &["ex", "exs"],
        grammar_override: &[],
        download_names: &[],
        query: ELIXIR_QUERY,
        sig_body_child: None,
        sig_delimiter: None,
        kind_overrides: &[],
        ref_node_types: &["identifier", "alias"],
    },
    LanguageConfig {
        name: "swift",
        extensions: &["swift"],
        grammar_override: &[],
        download_names: &[],
        query: SWIFT_QUERY,
        sig_body_child: None,
        sig_delimiter: Some(b'{'),
        kind_overrides: &[
            ("definition.struct", "", SymbolKind::Struct),
            ("definition.enum", "", SymbolKind::Enum),
        ],
        ref_node_types: &["simple_identifier", "type_identifier"],
    },
];

// --- Errors ---

#[derive(Debug)]
pub enum LangError {
    NotInstalled(String),
    ParseFailed,
}

impl std::fmt::Display for LangError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LangError::NotInstalled(name) => write!(f, "{} grammar not installed — run: cx lang add {}", name, name),
            LangError::ParseFailed => write!(f, "parse failed"),
        }
    }
}

// --- Public API ---

/// Detect language config name from file extension.
pub fn detect_language(path: &Path) -> Option<&'static str> {
    let ext = path.extension().and_then(|e| e.to_str())?;
    LANGUAGES
        .iter()
        .find(|c| c.extensions.contains(&ext))
        .map(|c| c.name)
}

/// Return all supported language config names.
pub fn supported_languages() -> Vec<&'static str> {
    LANGUAGES.iter().map(|c| c.name).collect()
}

/// Return the primary file extension for a language config name.
pub fn primary_extension(lang: &str) -> &str {
    LANGUAGES.iter()
        .find(|c| c.name == lang)
        .and_then(|c| c.extensions.first().copied())
        .unwrap_or(lang)
}

/// Return the download names for a language (for `cx lang add`).
pub fn download_names_for(lang: &str) -> Vec<&'static str> {
    LANGUAGES.iter()
        .find(|c| c.name == lang)
        .map(|c| {
            if c.download_names.is_empty() {
                vec![c.name]
            } else {
                c.download_names.to_vec()
            }
        })
        .unwrap_or_default()
}

pub struct Reference {
    pub line: usize, // 1-based
    pub parent_kind: String,
}

/// Resolve the grammar name for a given config + file extension.
fn resolve_grammar_name(config: &LanguageConfig, ext: &str) -> &'static str {
    for &(e, grammar) in config.grammar_override {
        if e == ext {
            return grammar;
        }
    }
    config.name
}

/// Look up config, create parser, and parse source into a tree.
fn parse_source(lang: &str, source: &[u8], path: &Path) -> Result<(&'static LanguageConfig, tree_sitter::Tree, &'static str), LangError> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let config = LANGUAGES.iter().find(|c| c.name == lang).ok_or_else(|| LangError::NotInstalled(lang.to_string()))?;
    let grammar_name = resolve_grammar_name(config, ext);

    let ts_lang = tree_sitter_language_pack::get_language(grammar_name)
        .map_err(|_| LangError::NotInstalled(config.name.to_string()))?;

    let mut parser = Parser::new();
    parser.set_language(&ts_lang).map_err(|_| LangError::ParseFailed)?;
    let tree = parser.parse(source, None).ok_or(LangError::ParseFailed)?;
    Ok((config, tree, grammar_name))
}

/// Parse source and find all identifier nodes whose text matches `name`.
pub fn find_references(lang: &str, source: &[u8], path: &Path, name: &str) -> Result<Vec<Reference>, LangError> {
    let (config, tree, _) = parse_source(lang, source, path)?;

    let mut refs = Vec::new();
    let mut stack = vec![tree.root_node()];
    while let Some(node) = stack.pop() {
        if node.child_count() == 0
            && config.ref_node_types.contains(&node.kind())
            && node.utf8_text(source).ok() == Some(name)
        {
            let parent_kind = node
                .parent()
                .map(|p| p.kind().to_string())
                .unwrap_or_default();
            refs.push(Reference {
                line: node.start_position().row + 1,
                parent_kind,
            });
        }
        for i in (0..node.child_count()).rev() {
            if let Some(child) = node.child(i as u32) {
                stack.push(child);
            }
        }
    }

    Ok(refs)
}

/// Parse a file and extract symbols for the given language.
/// `path` is used to distinguish .tsx from .ts for grammar selection.
pub fn parse_and_extract(lang: &str, source: &[u8], path: &Path) -> Result<Vec<Symbol>, LangError> {
    let (config, tree, grammar_name) = parse_source(lang, source, path)?;

    let mut cache = QUERY_CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let query = cache.entry(grammar_name.to_string()).or_insert_with(|| {
        Query::new(&tree.language(), config.query).expect("query compilation failed")
    });

    Ok(extract_symbols(config, query, &tree, source))
}

// --- Test detection ---

/// Check if a symbol node represents a test (Rust `#[test]`/`#[cfg(test)]`, Zig `test` decl).
fn detect_test_symbol(lang: &str, node: Node, source: &[u8]) -> bool {
    match lang {
        "rust" => has_rust_test_attribute(node, source),
        // Zig test blocks use `TestDecl` node type — these are not captured by our
        // symbol queries, so they never appear in the index. No filtering needed.
        _ => false,
    }
}

/// Walk previous siblings looking for `#[test]` or `#[cfg(test)]` attribute items.
/// Also checks if the node is inside a `#[cfg(test)] mod`.
fn has_rust_test_attribute(node: Node, source: &[u8]) -> bool {
    // Check immediate preceding attribute siblings
    let mut sibling = node.prev_named_sibling();
    while let Some(sib) = sibling {
        if sib.kind() == "attribute_item" {
            if let Ok(text) = sib.utf8_text(source) {
                if is_test_attribute(text) {
                    return true;
                }
            }
        } else {
            break; // attributes are contiguous
        }
        sibling = sib.prev_named_sibling();
    }

    // Check if we're inside a #[cfg(test)] mod by walking up the tree
    let mut parent = node.parent();
    while let Some(p) = parent {
        if p.kind() == "mod_item" {
            let mut sib = p.prev_named_sibling();
            while let Some(s) = sib {
                if s.kind() == "attribute_item" {
                    if let Ok(text) = s.utf8_text(source) {
                        if text.contains("cfg(test)") {
                            return true;
                        }
                    }
                } else {
                    break;
                }
                sib = s.prev_named_sibling();
            }
        }
        parent = p.parent();
    }

    false
}

/// Check if an attribute text represents a test attribute.
/// Matches `#[test]`, `#[tokio::test]`, `#[rstest]`, etc. but not `#[attestation]`.
fn is_test_attribute(text: &str) -> bool {
    let trimmed = text.trim_start_matches("#[").trim_end_matches(']');
    // Exact match: #[test]
    if trimmed == "test" { return true; }
    // Path-qualified: #[tokio::test], #[tokio::test(...)]
    if trimmed.ends_with("::test") || trimmed.contains("::test(") { return true; }
    // cfg(test)
    if trimmed.contains("cfg(test)") { return true; }
    false
}

// --- Generic extractor ---

fn extract_symbols(
    config: &LanguageConfig,
    query: &Query,
    tree: &tree_sitter::Tree,
    source: &[u8],
) -> Vec<Symbol> {
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(query, tree.root_node(), source);
    let capture_names = query.capture_names();

    let mut symbols = Vec::new();

    while let Some(m) = matches.next() {
        let mut name_node: Option<Node> = None;
        let mut def_node: Option<Node> = None;
        let mut def_kind: Option<&str> = None;

        for capture in m.captures {
            let cname = capture_names[capture.index as usize];
            if cname == "name" {
                name_node = Some(capture.node);
            } else if cname.starts_with("definition.") {
                def_node = Some(capture.node);
                def_kind = Some(cname);
            }
        }

        let (Some(name_n), Some(def_n), Some(kind_str)) = (name_node, def_node, def_kind)
        else {
            continue;
        };

        let name = match name_n.utf8_text(source) {
            Ok(s) => s.to_string(),
            Err(_) => continue,
        };

        let kind = match resolve_kind(config, kind_str, &def_n) {
            Some(k) => k,
            None => continue,
        };

        let byte_range = (def_n.start_byte(), def_n.end_byte());
        let signature = build_signature(config, def_n, source);
        let is_test = detect_test_symbol(config.name, def_n, source);

        symbols.push(Symbol {
            name,
            kind,
            signature,
            byte_range,
            is_test,
        });
    }

    deduplicate(symbols)
}

fn resolve_kind(config: &LanguageConfig, capture_name: &str, node: &Node) -> Option<SymbolKind> {
    let node_kind = node.kind();

    // Exact match on (capture_name, node_kind)
    for &(cap, nk, sk) in config.kind_overrides {
        if cap == capture_name && nk == node_kind {
            return Some(sk);
        }
    }
    // Wildcard match (empty node_kind)
    for &(cap, nk, sk) in config.kind_overrides {
        if cap == capture_name && nk.is_empty() {
            return Some(sk);
        }
    }

    // Defaults
    match capture_name {
        "definition.function" => Some(SymbolKind::Fn),
        "definition.method" => Some(SymbolKind::Method),
        "definition.class" => Some(SymbolKind::Class),
        "definition.interface" => Some(SymbolKind::Interface),
        "definition.type" => Some(SymbolKind::Type),
        "definition.enum" => Some(SymbolKind::Enum),
        "definition.module" => Some(SymbolKind::Module),
        "definition.constant" => Some(SymbolKind::Const),
        "definition.event" => Some(SymbolKind::Event),
        "definition.macro" => Some(SymbolKind::Fn),
        _ => None,
    }
}

fn build_signature(config: &LanguageConfig, node: Node, source: &[u8]) -> String {
    let start = node.start_byte();
    let end = node.end_byte();
    let text = &source[start..end];

    // Strategy 1: find body child node, take text before it
    if let Some(body_kind) = config.sig_body_child {
        let mut walker = node.walk();
        for child in node.children(&mut walker) {
            if child.kind() == body_kind {
                let sig_text = &source[start..child.start_byte()];
                let sig = String::from_utf8_lossy(sig_text)
                    .trim()
                    .trim_end_matches(':')
                    .trim()
                    .to_string();
                if !sig.is_empty() {
                    return sig;
                }
            }
        }
    }

    // Strategy 2: scan for delimiter byte
    if let Some(delim) = config.sig_delimiter
        && let Some(pos) = text.iter().position(|&b| b == delim) {
            let sig = String::from_utf8_lossy(&text[..pos]).trim().to_string();
            if !sig.is_empty() {
                return sig;
            }
        }

    // Strategy 3: for arrow functions, truncate at =>
    if let Some(pos) = text.windows(2).position(|w| w == b"=>") {
        let sig = String::from_utf8_lossy(&text[..pos + 2]).trim().to_string();
        if !sig.is_empty() {
            return sig;
        }
    }

    // Fallback: first line, strip trailing delimiters
    let first_line = text
        .iter()
        .position(|&b| b == b'\n')
        .map(|p| &text[..p])
        .unwrap_or(text);

    String::from_utf8_lossy(first_line)
        .trim()
        .trim_end_matches('{')
        .trim_end_matches(':')
        .trim()
        .to_string()
}

fn deduplicate(symbols: Vec<Symbol>) -> Vec<Symbol> {
    let mut seen: HashMap<(usize, usize), usize> = HashMap::new();
    let mut deduped: Vec<Symbol> = Vec::new();

    for sym in symbols {
        if let Some(&idx) = seen.get(&sym.byte_range) {
            if deduped[idx].kind == SymbolKind::Fn && sym.kind == SymbolKind::Method {
                deduped[idx] = sym;
            }
        } else {
            seen.insert(sym.byte_range, deduped.len());
            deduped.push(sym);
        }
    }

    deduped
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::Once;

    static INIT: Once = Once::new();

    fn init_grammar_cache() {
        INIT.call_once(|| {
            let config = tree_sitter_language_pack::PackConfig {
                cache_dir: Some(crate::lang::grammar_cache_dir()),
                ..Default::default()
            };
            tree_sitter_language_pack::configure(&config)
                .expect("failed to configure grammar cache");
        });
    }

    fn extract(lang: &str, src: &str, file: &str) -> Vec<Symbol> {
        init_grammar_cache();
        parse_and_extract(lang, src.as_bytes(), &PathBuf::from(file)).unwrap()
    }

    // --- Rust ---

    #[test]
    fn rust_function() {
        let src = "pub fn calculate_fee(amount: u64) -> u64 {\n    amount * 3 / 1000\n}";
        let syms = extract("rust", src, "test.rs");
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "calculate_fee");
        assert_eq!(syms[0].kind, SymbolKind::Fn);
        assert!(!syms[0].signature.contains('{'));
        assert!(syms[0].signature.contains("pub fn"));
    }

    #[test]
    fn rust_struct() {
        let src = "pub struct FeeConfig {\n    pub rate: u64,\n}";
        let syms = extract("rust", src, "test.rs");
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "FeeConfig");
        assert_eq!(syms[0].kind, SymbolKind::Struct);
    }

    #[test]
    fn rust_enum() {
        let src = "pub enum FeeTier {\n    Low,\n    High,\n}";
        let syms = extract("rust", src, "test.rs");
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "FeeTier");
        assert_eq!(syms[0].kind, SymbolKind::Enum);
    }

    #[test]
    fn rust_trait() {
        let src = "pub trait Configurable {\n    fn configure(&self);\n}";
        let syms = extract("rust", src, "test.rs");
        let trait_sym = syms.iter().find(|s| s.name == "Configurable").unwrap();
        assert_eq!(trait_sym.kind, SymbolKind::Trait);
    }

    #[test]
    fn rust_multiple_symbols() {
        let src = "pub fn foo() {}\nfn bar() {}\npub struct Baz;";
        let syms = extract("rust", src, "test.rs");
        assert!(syms.len() >= 3);
        let names: Vec<&str> = syms.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"foo"));
        assert!(names.contains(&"bar"));
        assert!(names.contains(&"Baz"));
    }

    #[test]
    fn rust_byte_range() {
        let src = "pub fn test_func() -> u32 { 42 }";
        let syms = extract("rust", src, "test.rs");
        assert_eq!(syms.len(), 1);
        let (start, end) = syms[0].byte_range;
        assert!(start < end);
        assert!(end <= src.len());
        assert!(src[start..end].contains("test_func"));
    }

    // --- TypeScript ---

    #[test]
    fn ts_function() {
        let src = "function greet(name: string): string { return name; }";
        let syms = extract("typescript", src, "test.ts");
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "greet");
        assert_eq!(syms[0].kind, SymbolKind::Fn);
    }

    #[test]
    fn ts_class() {
        let src = "export class UserService {\n  getName() { return 'test'; }\n}";
        let syms = extract("typescript", src, "test.ts");
        let class = syms.iter().find(|s| s.name == "UserService").unwrap();
        assert_eq!(class.kind, SymbolKind::Class);
        let method = syms.iter().find(|s| s.name == "getName").unwrap();
        assert_eq!(method.kind, SymbolKind::Method);
    }

    #[test]
    fn ts_interface() {
        let src = "export interface Config {\n  host: string;\n  port: number;\n}";
        let syms = extract("typescript", src, "test.ts");
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "Config");
        assert_eq!(syms[0].kind, SymbolKind::Interface);
    }

    #[test]
    fn ts_arrow_function() {
        let src = "const add = (a: number, b: number) => a + b;";
        let syms = extract("typescript", src, "test.ts");
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "add");
        assert_eq!(syms[0].kind, SymbolKind::Fn);
        assert!(syms[0].signature.contains("const add"), "should include const: {}", syms[0].signature);
        assert!(!syms[0].signature.contains("a + b"), "should not include body: {}", syms[0].signature);
    }

    #[test]
    fn ts_tsx() {
        let src = "export function App() { return <div />; }";
        let syms = extract("typescript", src, "test.tsx");
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "App");
    }

    // --- Python ---

    #[test]
    fn py_function() {
        let src = "def greet(name: str) -> str:\n    return f'Hello, {name}'";
        let syms = extract("python", src, "test.py");
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "greet");
        assert_eq!(syms[0].kind, SymbolKind::Fn);
        assert!(syms[0].signature.contains("-> str"), "should preserve return type: {}", syms[0].signature);
        assert!(syms[0].signature.contains("name: str"), "should preserve param types: {}", syms[0].signature);
    }

    #[test]
    fn py_class() {
        let src = "class UserService:\n    def get_name(self):\n        return 'test'";
        let syms = extract("python", src, "test.py");
        let class = syms.iter().find(|s| s.name == "UserService").unwrap();
        assert_eq!(class.kind, SymbolKind::Class);
    }

    #[test]
    fn py_constant() {
        let src = "MAX_SIZE = 100";
        let syms = extract("python", src, "test.py");
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "MAX_SIZE");
        assert_eq!(syms[0].kind, SymbolKind::Const);
    }

    #[test]
    fn py_multiple_symbols() {
        let src = "def foo():\n    pass\n\ndef bar():\n    pass\n\nclass Baz:\n    pass";
        let syms = extract("python", src, "test.py");
        assert!(syms.len() >= 3);
        let names: Vec<&str> = syms.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"foo"));
        assert!(names.contains(&"bar"));
        assert!(names.contains(&"Baz"));
    }

    #[test]
    fn py_type_annotation_preserved() {
        let src = "def foo(x: int, y: list[str]) -> bool:\n    return True";
        let syms = extract("python", src, "test.py");
        assert_eq!(syms.len(), 1);
        assert!(syms[0].signature.contains("int"), "sig: {}", syms[0].signature);
        assert!(syms[0].signature.contains("bool"), "sig: {}", syms[0].signature);
    }

    // --- Go ---

    #[test]
    fn go_function() {
        let src = "func Calculate(amount int) int {\n\treturn amount * 3\n}";
        let syms = extract("go", src, "test.go");
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "Calculate");
        assert_eq!(syms[0].kind, SymbolKind::Fn);
        assert!(syms[0].signature.contains("func"), "sig: {}", syms[0].signature);
    }

    #[test]
    fn go_method() {
        let src = "func (s *Server) Start() error {\n\treturn nil\n}";
        let syms = extract("go", src, "test.go");
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "Start");
        assert_eq!(syms[0].kind, SymbolKind::Method);
    }

    #[test]
    fn go_type() {
        let src = "type Config struct {\n\tHost string\n}";
        let syms = extract("go", src, "test.go");
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "Config");
        assert_eq!(syms[0].kind, SymbolKind::Type);
    }

    // --- C ---

    #[test]
    fn c_function() {
        let src = "int calculate(int amount) {\n    return amount * 3;\n}";
        let syms = extract("c", src, "test.c");
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "calculate");
        assert_eq!(syms[0].kind, SymbolKind::Fn);
    }

    #[test]
    fn c_pointer_returning_function() {
        let src = "char *strdup(const char *s) {\n    return NULL;\n}";
        let syms = extract("c", src, "test.c");
        let f = syms.iter().find(|s| s.name == "strdup");
        assert!(f.is_some(), "should find pointer-returning fn: {:?}", syms);
        assert_eq!(f.unwrap().kind, SymbolKind::Fn);
    }

    #[test]
    fn c_struct() {
        let src = "struct Config {\n    int rate;\n};";
        let syms = extract("c", src, "test.c");
        let s = syms.iter().find(|s| s.name == "Config");
        assert!(s.is_some(), "should find struct: {:?}", syms);
        assert_eq!(s.unwrap().kind, SymbolKind::Struct);
    }

    // --- C++ ---

    #[test]
    fn cpp_class() {
        let src = "class Server {\npublic:\n    void start();\n};";
        let syms = extract("cpp", src, "test.cpp");
        let class = syms.iter().find(|s| s.name == "Server");
        assert!(class.is_some(), "should find class: {:?}", syms);
        assert_eq!(class.unwrap().kind, SymbolKind::Class);
    }

    // --- Java ---

    #[test]
    fn java_class_and_method() {
        let src = "public class UserService {\n    public String getName() {\n        return \"test\";\n    }\n}";
        let syms = extract("java", src, "Test.java");
        let class = syms.iter().find(|s| s.name == "UserService");
        assert!(class.is_some(), "should find class: {:?}", syms);
        assert_eq!(class.unwrap().kind, SymbolKind::Class);
    }

    // --- Ruby ---

    #[test]
    fn ruby_class_and_method() {
        let src = "class UserService\n  def get_name\n    'test'\n  end\nend";
        let syms = extract("ruby", src, "test.rb");
        let class = syms.iter().find(|s| s.name == "UserService");
        assert!(class.is_some(), "should find class: {:?}", syms);
        assert_eq!(class.unwrap().kind, SymbolKind::Class);
        let method = syms.iter().find(|s| s.name == "get_name");
        assert!(method.is_some(), "should find method: {:?}", syms);
    }

    // --- Lua ---

    #[test]
    fn lua_function() {
        let src = "function greet(name)\n    return 'Hello, ' .. name\nend";
        let syms = extract("lua", src, "test.lua");
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "greet");
        assert_eq!(syms[0].kind, SymbolKind::Fn);
    }

    // --- Zig ---

    #[test]
    fn zig_function() {
        let src = "pub fn calculate(amount: u64) u64 {\n    return amount * 3;\n}";
        let syms = extract("zig", src, "test.zig");
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "calculate");
        assert_eq!(syms[0].kind, SymbolKind::Fn);
    }

    #[test]
    fn zig_struct() {
        let src = "const Point = struct {\n    x: f32,\n    y: f32,\n};";
        let syms = extract("zig", src, "test.zig");
        assert_eq!(syms.len(), 1, "should find struct: {:?}", syms);
        assert_eq!(syms[0].name, "Point");
        assert_eq!(syms[0].kind, SymbolKind::Struct);
    }

    #[test]
    fn zig_enum() {
        let src = "const Color = enum {\n    red,\n    green,\n    blue,\n};";
        let syms = extract("zig", src, "test.zig");
        assert_eq!(syms.len(), 1, "should find enum: {:?}", syms);
        assert_eq!(syms[0].name, "Color");
        assert_eq!(syms[0].kind, SymbolKind::Enum);
    }

    #[test]
    fn zig_union() {
        let src = "const Msg = union {\n    int: i32,\n    float: f64,\n};";
        let syms = extract("zig", src, "test.zig");
        assert_eq!(syms.len(), 1, "should find union: {:?}", syms);
        assert_eq!(syms[0].name, "Msg");
        assert_eq!(syms[0].kind, SymbolKind::Struct);
    }

    #[test]
    fn zig_pub_struct() {
        let src = "pub const Point = struct {\n    x: f32,\n    y: f32,\n};";
        let syms = extract("zig", src, "test.zig");
        assert_eq!(syms.len(), 1, "should find pub struct: {:?}", syms);
        assert_eq!(syms[0].name, "Point");
        assert_eq!(syms[0].kind, SymbolKind::Struct);
    }

    #[test]
    fn zig_error_set() {
        let src = "const MyError = error {\n    OutOfMemory,\n    InvalidInput,\n};";
        let syms = extract("zig", src, "test.zig");
        assert_eq!(syms.len(), 1, "should find error set: {:?}", syms);
        assert_eq!(syms[0].name, "MyError");
        assert_eq!(syms[0].kind, SymbolKind::Enum);
    }

    // --- Bash ---

    #[test]
    fn bash_function() {
        let src = "function greet() {\n    echo \"Hello\"\n}";
        let syms = extract("bash", src, "test.sh");
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "greet");
        assert_eq!(syms[0].kind, SymbolKind::Fn);
    }

    // --- Solidity ---

    #[test]
    fn solidity_contract_and_function() {
        let src = "contract Token {\n    function transfer(address to, uint amount) public {\n    }\n}";
        let syms = extract("solidity", src, "test.sol");
        let contract = syms.iter().find(|s| s.name == "Token");
        assert!(contract.is_some(), "should find contract: {:?}", syms);
        let func = syms.iter().find(|s| s.name == "transfer");
        assert!(func.is_some(), "should find function: {:?}", syms);
    }

    #[test]
    fn solidity_event() {
        let src = "contract Token {\n    event Transfer(address indexed from, address indexed to, uint256 value);\n}";
        let syms = extract("solidity", src, "test.sol");
        let event = syms.iter().find(|s| s.name == "Transfer");
        assert!(event.is_some(), "should find event: {:?}", syms);
        assert_eq!(event.unwrap().kind, SymbolKind::Event);
    }

    // --- Elixir ---

    #[test]
    fn elixir_module_and_function() {
        let src = "defmodule MyApp.Users do\n  def get_user(id) do\n    id\n  end\nend";
        let syms = extract("elixir", src, "test.ex");
        let module = syms.iter().find(|s| s.name == "MyApp.Users");
        assert!(module.is_some(), "should find module: {:?}", syms);
        assert_eq!(module.unwrap().kind, SymbolKind::Module);
        let func = syms.iter().find(|s| s.name == "get_user");
        assert!(func.is_some(), "should find function: {:?}", syms);
        assert_eq!(func.unwrap().kind, SymbolKind::Fn);
    }

    #[test]
    fn elixir_type_definitions() {
        let src = "defmodule MyApp do\n  @type status :: :active | :inactive\n  @typep internal :: map()\n  @opaque token :: binary()\nend";
        let syms = extract("elixir", src, "test.ex");
        let status = syms.iter().find(|s| s.name == "status");
        assert!(status.is_some(), "should find @type: {:?}", syms);
        assert_eq!(status.unwrap().kind, SymbolKind::Type);
        let internal = syms.iter().find(|s| s.name == "internal");
        assert!(internal.is_some(), "should find @typep: {:?}", syms);
        assert_eq!(internal.unwrap().kind, SymbolKind::Type);
        let token = syms.iter().find(|s| s.name == "token");
        assert!(token.is_some(), "should find @opaque: {:?}", syms);
        assert_eq!(token.unwrap().kind, SymbolKind::Type);
    }

    #[test]
    fn elixir_callback() {
        let src = "defmodule MyBehaviour do\n  @callback validate(term()) :: :ok | {:error, term()}\n  @callback format(term()) :: String.t()\nend";
        let syms = extract("elixir", src, "test.ex");
        let validate = syms.iter().find(|s| s.name == "validate");
        assert!(validate.is_some(), "should find @callback validate: {:?}", syms);
        assert_eq!(validate.unwrap().kind, SymbolKind::Method);
        let format = syms.iter().find(|s| s.name == "format");
        assert!(format.is_some(), "should find @callback format: {:?}", syms);
        assert_eq!(format.unwrap().kind, SymbolKind::Method);
    }

    #[test]
    fn elixir_defimpl() {
        let src = "defimpl String.Chars, for: MyApp.User do\n  def to_string(user), do: user.name\nend";
        let syms = extract("elixir", src, "test.ex");
        let impl_sym = syms.iter().find(|s| s.name == "String.Chars");
        assert!(impl_sym.is_some(), "should find defimpl: {:?}", syms);
        assert_eq!(impl_sym.unwrap().kind, SymbolKind::Module);
        let func = syms.iter().find(|s| s.name == "to_string");
        assert!(func.is_some(), "should find function in impl: {:?}", syms);
    }

    #[test]
    fn elixir_protocol() {
        let src = "defprotocol Renderable do\n  @spec render(t()) :: String.t()\n  def render(data)\nend";
        let syms = extract("elixir", src, "test.ex");
        let proto = syms.iter().find(|s| s.name == "Renderable");
        assert!(proto.is_some(), "should find defprotocol: {:?}", syms);
        assert_eq!(proto.unwrap().kind, SymbolKind::Module);
    }

    // --- Swift ---

    #[test]
    fn swift_function() {
        let src = "func greet(name: String) -> String {\n    return \"Hello, \\(name)\"\n}";
        let syms = extract("swift", src, "test.swift");
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "greet");
        assert_eq!(syms[0].kind, SymbolKind::Fn);
        assert!(syms[0].signature.contains("func greet"));
    }

    #[test]
    fn swift_class_and_method() {
        let src = "class Animal {\n    func speak() -> String {\n        return \"...\"\n    }\n}";
        let syms = extract("swift", src, "test.swift");
        let cls = syms.iter().find(|s| s.name == "Animal");
        assert!(cls.is_some(), "should find class: {:?}", syms);
        assert_eq!(cls.unwrap().kind, SymbolKind::Class);
        let method = syms.iter().find(|s| s.name == "speak");
        assert!(method.is_some(), "should find method: {:?}", syms);
        assert_eq!(method.unwrap().kind, SymbolKind::Method);
    }

    #[test]
    fn swift_struct() {
        let src = "struct Point {\n    var x: Double\n    var y: Double\n}";
        let syms = extract("swift", src, "test.swift");
        let s = syms.iter().find(|s| s.name == "Point");
        assert!(s.is_some(), "should find struct: {:?}", syms);
        assert_eq!(s.unwrap().kind, SymbolKind::Struct);
        let x = syms.iter().find(|s| s.name == "x");
        assert!(x.is_some(), "should find property x: {:?}", syms);
        assert_eq!(x.unwrap().kind, SymbolKind::Const);
        let y = syms.iter().find(|s| s.name == "y");
        assert!(y.is_some(), "should find property y: {:?}", syms);
        assert_eq!(y.unwrap().kind, SymbolKind::Const);
    }

    #[test]
    fn swift_enum() {
        let src = "enum Direction {\n    case north, south, east, west\n}";
        let syms = extract("swift", src, "test.swift");
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "Direction");
        assert_eq!(syms[0].kind, SymbolKind::Enum);
    }

    #[test]
    fn swift_enum_methods() {
        let src = "enum Direction {\n    case north, south\n    func opposite() -> Direction {\n        switch self {\n        case .north: return .south\n        default: return .north\n        }\n    }\n    init?(rawValue: String) {\n        switch rawValue {\n        case \"n\": self = .north\n        default: return nil\n        }\n    }\n}";
        let syms = extract("swift", src, "test.swift");
        let e = syms.iter().find(|s| s.name == "Direction");
        assert!(e.is_some(), "should find enum: {:?}", syms);
        assert_eq!(e.unwrap().kind, SymbolKind::Enum);
        let method = syms.iter().find(|s| s.name == "opposite");
        assert!(method.is_some(), "should find method in enum: {:?}", syms);
        assert_eq!(method.unwrap().kind, SymbolKind::Method);
        let init_sym = syms.iter().find(|s| s.name == "init");
        assert!(init_sym.is_some(), "should find init in enum: {:?}", syms);
        assert_eq!(init_sym.unwrap().kind, SymbolKind::Method);
    }

    #[test]
    fn swift_protocol() {
        let src = "protocol Drawable {\n    func draw()\n}";
        let syms = extract("swift", src, "test.swift");
        let proto = syms.iter().find(|s| s.name == "Drawable");
        assert!(proto.is_some(), "should find protocol: {:?}", syms);
        assert_eq!(proto.unwrap().kind, SymbolKind::Interface);
        let draw = syms.iter().find(|s| s.name == "draw");
        assert!(draw.is_some(), "should find protocol method: {:?}", syms);
        assert_eq!(draw.unwrap().kind, SymbolKind::Method);
    }

    #[test]
    fn swift_typealias() {
        let src = "typealias Callback = (Int) -> Void";
        let syms = extract("swift", src, "test.swift");
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "Callback");
        assert_eq!(syms[0].kind, SymbolKind::Type);
    }

    #[test]
    fn swift_init() {
        let src = "class Foo {\n    init(x: Int) {\n        self.x = x\n    }\n}";
        let syms = extract("swift", src, "test.swift");
        let init_sym = syms.iter().find(|s| s.name == "init");
        assert!(init_sym.is_some(), "should find init: {:?}", syms);
        assert_eq!(init_sym.unwrap().kind, SymbolKind::Method);
    }

    #[test]
    fn swift_deinit() {
        let src = "class Foo {\n    deinit {\n        print(\"bye\")\n    }\n}";
        let syms = extract("swift", src, "test.swift");
        let deinit_sym = syms.iter().find(|s| s.name == "deinit");
        assert!(deinit_sym.is_some(), "should find deinit: {:?}", syms);
        assert_eq!(deinit_sym.unwrap().kind, SymbolKind::Method);
    }

    #[test]
    fn swift_actor() {
        let src = "actor BankAccount {\n    var balance: Double\n    func deposit(_ amount: Double) {\n        balance += amount\n    }\n}";
        let syms = extract("swift", src, "test.swift");
        let actor = syms.iter().find(|s| s.name == "BankAccount");
        assert!(actor.is_some(), "should find actor: {:?}", syms);
        assert_eq!(actor.unwrap().kind, SymbolKind::Class);
        let method = syms.iter().find(|s| s.name == "deposit");
        assert!(method.is_some(), "should find actor method: {:?}", syms);
        assert_eq!(method.unwrap().kind, SymbolKind::Method);
        let prop = syms.iter().find(|s| s.name == "balance");
        assert!(prop.is_some(), "should find actor property: {:?}", syms);
        assert_eq!(prop.unwrap().kind, SymbolKind::Const);
    }

    #[test]
    fn swift_extension() {
        let src = "extension String {\n    func reversed() -> String {\n        return String(self.reversed())\n    }\n}";
        let syms = extract("swift", src, "test.swift");
        let ext = syms.iter().find(|s| s.name == "String");
        assert!(ext.is_some(), "should find extension: {:?}", syms);
        assert_eq!(ext.unwrap().kind, SymbolKind::Module);
        let method = syms.iter().find(|s| s.name == "reversed");
        assert!(method.is_some(), "should find extension method: {:?}", syms);
        assert_eq!(method.unwrap().kind, SymbolKind::Method);
    }

    #[test]
    fn swift_extension_constrained() {
        let src = "extension Array where Element: Comparable {\n    func sorted() -> [Element] {\n        return []\n    }\n}";
        let syms = extract("swift", src, "test.swift");
        let ext = syms.iter().find(|s| s.kind == SymbolKind::Module);
        assert!(ext.is_some(), "should find constrained extension: {:?}", syms);
        let method = syms.iter().find(|s| s.name == "sorted");
        assert!(method.is_some(), "should find method in constrained extension: {:?}", syms);
        assert_eq!(method.unwrap().kind, SymbolKind::Method);
    }

    #[test]
    fn swift_subscript() {
        let src = "struct Matrix {\n    subscript(row: Int, col: Int) -> Double {\n        return 0.0\n    }\n}";
        let syms = extract("swift", src, "test.swift");
        let sub = syms.iter().find(|s| s.name == "subscript");
        assert!(sub.is_some(), "should find subscript: {:?}", syms);
        assert_eq!(sub.unwrap().kind, SymbolKind::Method);
    }

    #[test]
    fn swift_protocol_property() {
        let src = "protocol Named {\n    var name: String { get }\n    func greet() -> String\n}";
        let syms = extract("swift", src, "test.swift");
        let proto = syms.iter().find(|s| s.name == "Named");
        assert!(proto.is_some(), "should find protocol: {:?}", syms);
        assert_eq!(proto.unwrap().kind, SymbolKind::Interface);
        let prop = syms.iter().find(|s| s.name == "name");
        assert!(prop.is_some(), "should find protocol property: {:?}", syms);
        assert_eq!(prop.unwrap().kind, SymbolKind::Const);
        let method = syms.iter().find(|s| s.name == "greet");
        assert!(method.is_some(), "should find protocol method: {:?}", syms);
        assert_eq!(method.unwrap().kind, SymbolKind::Method);
    }

    // --- find_references tests ---

    #[test]
    fn refs_rust_finds_all_usages() {
        init_grammar_cache();
        let src = "struct Foo { x: i32 }\nfn bar(f: Foo) -> Foo { f }";
        let refs = find_references("rust", src.as_bytes(), &PathBuf::from("test.rs"), "Foo").unwrap();
        assert_eq!(refs.len(), 3, "should find struct def + 2 usages: {:?}", refs.iter().map(|r| r.line).collect::<Vec<_>>());
    }

    #[test]
    fn refs_rust_no_match() {
        init_grammar_cache();
        let src = "fn main() {}";
        let refs = find_references("rust", src.as_bytes(), &PathBuf::from("test.rs"), "nonexistent").unwrap();
        assert!(refs.is_empty());
    }

    #[test]
    fn refs_line_column_correct() {
        init_grammar_cache();
        let src = "let x = 1;\nlet y = x + x;";
        let refs = find_references("rust", src.as_bytes(), &PathBuf::from("test.rs"), "x").unwrap();
        assert_eq!(refs.len(), 3);
        assert_eq!(refs[0].line, 1);
        assert_eq!(refs[1].line, 2);
        assert_eq!(refs[2].line, 2);
    }

    #[test]
    fn refs_typescript_identifier() {
        init_grammar_cache();
        let src = "const foo = 1;\nconsole.log(foo);";
        let refs = find_references("typescript", src.as_bytes(), &PathBuf::from("test.ts"), "foo").unwrap();
        assert_eq!(refs.len(), 2);
    }

    #[test]
    fn refs_python_identifier() {
        init_grammar_cache();
        let src = "def greet(name):\n    return name";
        let refs = find_references("python", src.as_bytes(), &PathBuf::from("test.py"), "name").unwrap();
        assert_eq!(refs.len(), 2);
    }

    // --- is_test detection tests ---

    #[test]
    fn rust_test_attribute_detected() {
        let syms = extract("rust", "#[test]\nfn test_foo() { assert!(true); }", "test.rs");
        let sym = syms.iter().find(|s| s.name == "test_foo").unwrap();
        assert!(sym.is_test, "function with #[test] should be marked as test");
    }

    #[test]
    fn rust_cfg_test_mod_detected() {
        let src = r#"
fn main() {}

#[cfg(test)]
mod tests {
    fn helper() {}
}
"#;
        let syms = extract("rust", src, "lib.rs");
        let main_sym = syms.iter().find(|s| s.name == "main").unwrap();
        assert!(!main_sym.is_test, "main should not be marked as test");
        let tests_mod = syms.iter().find(|s| s.name == "tests").unwrap();
        assert!(tests_mod.is_test, "#[cfg(test)] mod should be marked as test");
        // helper inside #[cfg(test)] mod should also be marked as test
        if let Some(helper) = syms.iter().find(|s| s.name == "helper") {
            assert!(helper.is_test, "function inside #[cfg(test)] mod should be marked as test");
        }
    }

    #[test]
    fn rust_normal_fn_not_test() {
        let syms = extract("rust", "pub fn add(a: i32, b: i32) -> i32 { a + b }", "lib.rs");
        let sym = syms.iter().find(|s| s.name == "add").unwrap();
        assert!(!sym.is_test, "normal function should not be marked as test");
    }

    #[test]
    fn rust_test_and_normal_mixed() {
        let src = r#"
pub fn real_fn() {}

#[test]
fn test_real_fn() {}

pub struct MyStruct;
"#;
        let syms = extract("rust", src, "lib.rs");
        let real_fn = syms.iter().find(|s| s.name == "real_fn").unwrap();
        assert!(!real_fn.is_test);
        let test_fn = syms.iter().find(|s| s.name == "test_real_fn").unwrap();
        assert!(test_fn.is_test);
        let my_struct = syms.iter().find(|s| s.name == "MyStruct").unwrap();
        assert!(!my_struct.is_test);
    }
}
