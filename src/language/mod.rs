use crate::index::{Language, Symbol, SymbolKind};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{LazyLock, Mutex};
use tree_sitter::{Node, Parser, Query, QueryCursor, StreamingIterator};

/// Cache compiled queries keyed by (language discriminant, grammar variant).
/// Variant distinguishes TS vs TSX grammars; 0 for all other languages.
static QUERY_CACHE: LazyLock<Mutex<HashMap<(u8, u8), Query>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

// --- Language registry ---

struct LanguageConfig {
    language: Language,
    extensions: &'static [&'static str],
    grammar: fn(&str) -> tree_sitter::Language,
    query: fn() -> &'static str,
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

// --- Grammar functions ---

fn rust_grammar(_ext: &str) -> tree_sitter::Language { tree_sitter_rust::LANGUAGE.into() }
fn ts_grammar(ext: &str) -> tree_sitter::Language {
    match ext {
        "tsx" | "jsx" => tree_sitter_typescript::LANGUAGE_TSX.into(),
        _ => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
    }
}
fn py_grammar(_ext: &str) -> tree_sitter::Language { tree_sitter_python::LANGUAGE.into() }
fn go_grammar(_ext: &str) -> tree_sitter::Language { tree_sitter_go::LANGUAGE.into() }
fn c_grammar(_ext: &str) -> tree_sitter::Language { tree_sitter_c::LANGUAGE.into() }
fn cpp_grammar(_ext: &str) -> tree_sitter::Language { tree_sitter_cpp::LANGUAGE.into() }
fn java_grammar(_ext: &str) -> tree_sitter::Language { tree_sitter_java::LANGUAGE.into() }
fn ruby_grammar(_ext: &str) -> tree_sitter::Language { tree_sitter_ruby::LANGUAGE.into() }
fn csharp_grammar(_ext: &str) -> tree_sitter::Language { tree_sitter_c_sharp::LANGUAGE.into() }
fn lua_grammar(_ext: &str) -> tree_sitter::Language { tree_sitter_lua::LANGUAGE.into() }
fn zig_grammar(_ext: &str) -> tree_sitter::Language { tree_sitter_zig::LANGUAGE.into() }
fn bash_grammar(_ext: &str) -> tree_sitter::Language { tree_sitter_bash::LANGUAGE.into() }
fn sol_grammar(_ext: &str) -> tree_sitter::Language { tree_sitter_solidity::LANGUAGE.into() }
fn elixir_grammar(_ext: &str) -> tree_sitter::Language { tree_sitter_elixir::LANGUAGE.into() }

// --- Query functions (custom queries for reliability across tree-sitter versions) ---

fn rust_query() -> &'static str { tree_sitter_rust::TAGS_QUERY }
fn py_query() -> &'static str { tree_sitter_python::TAGS_QUERY }

fn ts_query() -> &'static str { TS_QUERY }
fn go_query() -> &'static str { GO_QUERY }
fn c_query() -> &'static str { C_QUERY }
fn cpp_query() -> &'static str { CPP_QUERY }
fn java_query() -> &'static str { JAVA_QUERY }
fn ruby_query() -> &'static str { RUBY_QUERY }
fn csharp_query() -> &'static str { CSHARP_QUERY }
fn lua_query() -> &'static str { LUA_QUERY }
fn zig_query() -> &'static str { ZIG_QUERY }
fn bash_query() -> &'static str { BASH_QUERY }
fn sol_query() -> &'static str { SOL_QUERY }
fn elixir_query() -> &'static str { ELIXIR_QUERY }

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

const CSHARP_QUERY: &str = r#"
(class_declaration
  name: (identifier) @name) @definition.class

(struct_declaration
  name: (identifier) @name) @definition.class

(interface_declaration
  name: (identifier) @name) @definition.interface

(enum_declaration
  name: (identifier) @name) @definition.enum

(method_declaration
  name: (identifier) @name) @definition.method

(namespace_declaration
  name: (identifier) @name) @definition.module
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
(function_declaration
  name: (identifier) @name) @definition.function

(variable_declaration
  (identifier) @name
  (struct_declaration)) @definition.class

(variable_declaration
  (identifier) @name
  (enum_declaration)) @definition.enum

(variable_declaration
  (identifier) @name
  (union_declaration)) @definition.class

(variable_declaration
  (identifier) @name
  (error_set_declaration)) @definition.enum
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
  (#any-of? @_keyword "defmodule" "defprotocol")) @definition.module

(call
  target: (identifier) @_keyword
  (arguments
    [(identifier) @name
     (call target: (identifier) @name)
     (binary_operator left: (call target: (identifier) @name))])
  (#any-of? @_keyword "def" "defp" "defmacro" "defmacrop" "defguard" "defguardp" "defdelegate")) @definition.function
"#;

// --- Registry ---

static LANGUAGES: &[LanguageConfig] = &[
    LanguageConfig {
        language: Language::Rust,
        extensions: &["rs"],
        grammar: rust_grammar,
        query: rust_query,
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
        language: Language::TypeScript,
        extensions: &["ts", "tsx", "js", "jsx"],
        grammar: ts_grammar,
        query: ts_query,
        sig_body_child: None,
        sig_delimiter: Some(b'{'),
        kind_overrides: &[],
        ref_node_types: &["identifier", "type_identifier", "property_identifier", "shorthand_property_identifier", "shorthand_property_identifier_pattern"],
    },
    LanguageConfig {
        language: Language::Python,
        extensions: &["py"],
        grammar: py_grammar,
        query: py_query,
        sig_body_child: Some("block"),
        sig_delimiter: None,
        kind_overrides: &[],
        ref_node_types: &["identifier"],
    },
    LanguageConfig {
        language: Language::Go,
        extensions: &["go"],
        grammar: go_grammar,
        query: go_query,
        sig_body_child: None,
        sig_delimiter: Some(b'{'),
        kind_overrides: &[],
        ref_node_types: &["identifier", "type_identifier", "field_identifier"],
    },
    LanguageConfig {
        language: Language::C,
        extensions: &["c"],
        grammar: c_grammar,
        query: c_query,
        sig_body_child: None,
        sig_delimiter: Some(b'{'),
        kind_overrides: &[
            ("definition.class", "", SymbolKind::Struct),
        ],
        ref_node_types: &["identifier", "type_identifier", "field_identifier"],
    },
    LanguageConfig {
        language: Language::Cpp,
        extensions: &["cpp", "cc", "cxx", "h", "hpp", "hxx", "hh"],
        grammar: cpp_grammar,
        query: cpp_query,
        sig_body_child: None,
        sig_delimiter: Some(b'{'),
        kind_overrides: &[],
        ref_node_types: &["identifier", "type_identifier", "field_identifier"],
    },
    LanguageConfig {
        language: Language::Java,
        extensions: &["java"],
        grammar: java_grammar,
        query: java_query,
        sig_body_child: None,
        sig_delimiter: Some(b'{'),
        kind_overrides: &[],
        ref_node_types: &["identifier"],
    },
    LanguageConfig {
        language: Language::Ruby,
        extensions: &["rb"],
        grammar: ruby_grammar,
        query: ruby_query,
        sig_body_child: None,
        sig_delimiter: None,
        kind_overrides: &[],
        ref_node_types: &["identifier", "constant"],
    },
    LanguageConfig {
        language: Language::CSharp,
        extensions: &["cs"],
        grammar: csharp_grammar,
        query: csharp_query,
        sig_body_child: None,
        sig_delimiter: Some(b'{'),
        kind_overrides: &[
            ("definition.class", "struct_declaration", SymbolKind::Struct),
        ],
        ref_node_types: &["identifier"],
    },
    LanguageConfig {
        language: Language::Lua,
        extensions: &["lua"],
        grammar: lua_grammar,
        query: lua_query,
        sig_body_child: None,
        sig_delimiter: None,
        kind_overrides: &[],
        ref_node_types: &["identifier"],
    },
    LanguageConfig {
        language: Language::Zig,
        extensions: &["zig"],
        grammar: zig_grammar,
        query: zig_query,
        sig_body_child: None,
        sig_delimiter: Some(b'{'),
        kind_overrides: &[
            ("definition.class", "variable_declaration", SymbolKind::Struct),
        ],
        ref_node_types: &["identifier"],
    },
    LanguageConfig {
        language: Language::Bash,
        extensions: &["sh", "bash"],
        grammar: bash_grammar,
        query: bash_query,
        sig_body_child: None,
        sig_delimiter: Some(b'{'),
        kind_overrides: &[],
        ref_node_types: &["word"],
    },
    LanguageConfig {
        language: Language::Solidity,
        extensions: &["sol"],
        grammar: sol_grammar,
        query: sol_query,
        sig_body_child: None,
        sig_delimiter: Some(b'{'),
        kind_overrides: &[],
        ref_node_types: &["identifier"],
    },
    LanguageConfig {
        language: Language::Elixir,
        extensions: &["ex", "exs"],
        grammar: elixir_grammar,
        query: elixir_query,
        sig_body_child: None,
        sig_delimiter: None,
        kind_overrides: &[],
        ref_node_types: &["identifier", "alias"],
    },
];

// --- Public API ---

pub fn detect_language(path: &Path) -> Option<Language> {
    let ext = path.extension().and_then(|e| e.to_str())?;
    LANGUAGES
        .iter()
        .find(|c| c.extensions.contains(&ext))
        .map(|c| c.language)
}

pub struct Reference {
    pub line: usize, // 1-based
}

/// Parse source and find all identifier nodes whose text matches `name`.
pub fn find_references(lang: Language, source: &[u8], path: &Path, name: &str) -> Vec<Reference> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let config = match LANGUAGES.iter().find(|c| c.language == lang) {
        Some(c) => c,
        None => return Vec::new(),
    };

    let ts_lang = (config.grammar)(ext);
    let mut parser = Parser::new();
    if parser.set_language(&ts_lang).is_err() {
        return Vec::new();
    }

    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return Vec::new(),
    };

    let mut refs = Vec::new();
    let mut stack = vec![tree.root_node()];
    while let Some(node) = stack.pop() {
        if node.child_count() == 0
            && config.ref_node_types.contains(&node.kind())
            && node.utf8_text(source).ok() == Some(name)
        {
            refs.push(Reference {
                line: node.start_position().row + 1,
            });
        }
        for i in (0..node.child_count()).rev() {
            if let Some(child) = node.child(i) {
                stack.push(child);
            }
        }
    }

    refs
}

/// Parse a file and extract symbols for the given language.
/// `path` is used to distinguish .tsx from .ts for grammar selection.
pub fn parse_and_extract(lang: Language, source: &[u8], path: &Path) -> Vec<Symbol> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let config = match LANGUAGES.iter().find(|c| c.language == lang) {
        Some(c) => c,
        None => return Vec::new(),
    };

    let ts_lang = (config.grammar)(ext);
    let mut parser = Parser::new();
    if parser.set_language(&ts_lang).is_err() {
        return Vec::new();
    }

    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return Vec::new(),
    };

    let variant = match (config.language, ext) {
        (Language::TypeScript, "tsx" | "jsx") => 1u8,
        _ => 0u8,
    };

    let mut cache = QUERY_CACHE.lock().unwrap();
    let query = cache.entry((config.language as u8, variant)).or_insert_with(|| {
        Query::new(&ts_lang, (config.query)()).expect("query compilation failed")
    });

    extract_symbols(config, query, &tree, source)
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

        symbols.push(Symbol {
            name,
            kind,
            signature,
            byte_range,
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

    // --- Rust ---

    #[test]
    fn rust_function() {
        let src = "pub fn calculate_fee(amount: u64) -> u64 {\n    amount * 3 / 1000\n}";
        let syms = parse_and_extract(Language::Rust, src.as_bytes(), &PathBuf::from("test.rs"));
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "calculate_fee");
        assert_eq!(syms[0].kind, SymbolKind::Fn);
        assert!(!syms[0].signature.contains('{'));
        assert!(syms[0].signature.contains("pub fn"));
    }

    #[test]
    fn rust_struct() {
        let src = "pub struct FeeConfig {\n    pub rate: u64,\n}";
        let syms = parse_and_extract(Language::Rust, src.as_bytes(), &PathBuf::from("test.rs"));
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "FeeConfig");
        assert_eq!(syms[0].kind, SymbolKind::Struct);
    }

    #[test]
    fn rust_enum() {
        let src = "pub enum FeeTier {\n    Low,\n    High,\n}";
        let syms = parse_and_extract(Language::Rust, src.as_bytes(), &PathBuf::from("test.rs"));
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "FeeTier");
        assert_eq!(syms[0].kind, SymbolKind::Enum);
    }

    #[test]
    fn rust_trait() {
        let src = "pub trait Configurable {\n    fn configure(&self);\n}";
        let syms = parse_and_extract(Language::Rust, src.as_bytes(), &PathBuf::from("test.rs"));
        let trait_sym = syms.iter().find(|s| s.name == "Configurable").unwrap();
        assert_eq!(trait_sym.kind, SymbolKind::Trait);
    }

    #[test]
    fn rust_multiple_symbols() {
        let src = "pub fn foo() {}\nfn bar() {}\npub struct Baz;";
        let syms = parse_and_extract(Language::Rust, src.as_bytes(), &PathBuf::from("test.rs"));
        assert!(syms.len() >= 3);
        let names: Vec<&str> = syms.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"foo"));
        assert!(names.contains(&"bar"));
        assert!(names.contains(&"Baz"));
    }

    #[test]
    fn rust_byte_range() {
        let src = "pub fn test_func() -> u32 { 42 }";
        let syms = parse_and_extract(Language::Rust, src.as_bytes(), &PathBuf::from("test.rs"));
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
        let syms = parse_and_extract(Language::TypeScript, src.as_bytes(), &PathBuf::from("test.ts"));
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "greet");
        assert_eq!(syms[0].kind, SymbolKind::Fn);
    }

    #[test]
    fn ts_class() {
        let src = "export class UserService {\n  getName() { return 'test'; }\n}";
        let syms = parse_and_extract(Language::TypeScript, src.as_bytes(), &PathBuf::from("test.ts"));
        let class = syms.iter().find(|s| s.name == "UserService").unwrap();
        assert_eq!(class.kind, SymbolKind::Class);
        let method = syms.iter().find(|s| s.name == "getName").unwrap();
        assert_eq!(method.kind, SymbolKind::Method);
    }

    #[test]
    fn ts_interface() {
        let src = "export interface Config {\n  host: string;\n  port: number;\n}";
        let syms = parse_and_extract(Language::TypeScript, src.as_bytes(), &PathBuf::from("test.ts"));
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "Config");
        assert_eq!(syms[0].kind, SymbolKind::Interface);
    }

    #[test]
    fn ts_arrow_function() {
        let src = "const add = (a: number, b: number) => a + b;";
        let syms = parse_and_extract(Language::TypeScript, src.as_bytes(), &PathBuf::from("test.ts"));
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "add");
        assert_eq!(syms[0].kind, SymbolKind::Fn);
        assert!(syms[0].signature.contains("const add"), "should include const: {}", syms[0].signature);
        assert!(!syms[0].signature.contains("a + b"), "should not include body: {}", syms[0].signature);
    }

    #[test]
    fn ts_tsx() {
        let src = "export function App() { return <div />; }";
        let syms = parse_and_extract(Language::TypeScript, src.as_bytes(), &PathBuf::from("test.tsx"));
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "App");
    }

    // --- Python ---

    #[test]
    fn py_function() {
        let src = "def greet(name: str) -> str:\n    return f'Hello, {name}'";
        let syms = parse_and_extract(Language::Python, src.as_bytes(), &PathBuf::from("test.py"));
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "greet");
        assert_eq!(syms[0].kind, SymbolKind::Fn);
        assert!(syms[0].signature.contains("-> str"), "should preserve return type: {}", syms[0].signature);
        assert!(syms[0].signature.contains("name: str"), "should preserve param types: {}", syms[0].signature);
    }

    #[test]
    fn py_class() {
        let src = "class UserService:\n    def get_name(self):\n        return 'test'";
        let syms = parse_and_extract(Language::Python, src.as_bytes(), &PathBuf::from("test.py"));
        let class = syms.iter().find(|s| s.name == "UserService").unwrap();
        assert_eq!(class.kind, SymbolKind::Class);
    }

    #[test]
    fn py_constant() {
        let src = "MAX_SIZE = 100";
        let syms = parse_and_extract(Language::Python, src.as_bytes(), &PathBuf::from("test.py"));
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "MAX_SIZE");
        assert_eq!(syms[0].kind, SymbolKind::Const);
    }

    #[test]
    fn py_multiple_symbols() {
        let src = "def foo():\n    pass\n\ndef bar():\n    pass\n\nclass Baz:\n    pass";
        let syms = parse_and_extract(Language::Python, src.as_bytes(), &PathBuf::from("test.py"));
        assert!(syms.len() >= 3);
        let names: Vec<&str> = syms.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"foo"));
        assert!(names.contains(&"bar"));
        assert!(names.contains(&"Baz"));
    }

    #[test]
    fn py_type_annotation_preserved() {
        let src = "def foo(x: int, y: list[str]) -> bool:\n    return True";
        let syms = parse_and_extract(Language::Python, src.as_bytes(), &PathBuf::from("test.py"));
        assert_eq!(syms.len(), 1);
        assert!(syms[0].signature.contains("int"), "sig: {}", syms[0].signature);
        assert!(syms[0].signature.contains("bool"), "sig: {}", syms[0].signature);
    }

    // --- Go ---

    #[test]
    fn go_function() {
        let src = "func Calculate(amount int) int {\n\treturn amount * 3\n}";
        let syms = parse_and_extract(Language::Go, src.as_bytes(), &PathBuf::from("test.go"));
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "Calculate");
        assert_eq!(syms[0].kind, SymbolKind::Fn);
        assert!(syms[0].signature.contains("func"), "sig: {}", syms[0].signature);
    }

    #[test]
    fn go_method() {
        let src = "func (s *Server) Start() error {\n\treturn nil\n}";
        let syms = parse_and_extract(Language::Go, src.as_bytes(), &PathBuf::from("test.go"));
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "Start");
        assert_eq!(syms[0].kind, SymbolKind::Method);
    }

    #[test]
    fn go_type() {
        let src = "type Config struct {\n\tHost string\n}";
        let syms = parse_and_extract(Language::Go, src.as_bytes(), &PathBuf::from("test.go"));
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "Config");
        assert_eq!(syms[0].kind, SymbolKind::Type);
    }

    // --- C ---

    #[test]
    fn c_function() {
        let src = "int calculate(int amount) {\n    return amount * 3;\n}";
        let syms = parse_and_extract(Language::C, src.as_bytes(), &PathBuf::from("test.c"));
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "calculate");
        assert_eq!(syms[0].kind, SymbolKind::Fn);
    }

    #[test]
    fn c_pointer_returning_function() {
        let src = "char *strdup(const char *s) {\n    return NULL;\n}";
        let syms = parse_and_extract(Language::C, src.as_bytes(), &PathBuf::from("test.c"));
        let f = syms.iter().find(|s| s.name == "strdup");
        assert!(f.is_some(), "should find pointer-returning fn: {:?}", syms);
        assert_eq!(f.unwrap().kind, SymbolKind::Fn);
    }

    #[test]
    fn c_struct() {
        let src = "struct Config {\n    int rate;\n};";
        let syms = parse_and_extract(Language::C, src.as_bytes(), &PathBuf::from("test.c"));
        let s = syms.iter().find(|s| s.name == "Config");
        assert!(s.is_some(), "should find struct: {:?}", syms);
        assert_eq!(s.unwrap().kind, SymbolKind::Struct);
    }

    // --- C++ ---

    #[test]
    fn cpp_class() {
        let src = "class Server {\npublic:\n    void start();\n};";
        let syms = parse_and_extract(Language::Cpp, src.as_bytes(), &PathBuf::from("test.cpp"));
        let class = syms.iter().find(|s| s.name == "Server");
        assert!(class.is_some(), "should find class: {:?}", syms);
        assert_eq!(class.unwrap().kind, SymbolKind::Class);
    }

    // --- Java ---

    #[test]
    fn java_class_and_method() {
        let src = "public class UserService {\n    public String getName() {\n        return \"test\";\n    }\n}";
        let syms = parse_and_extract(Language::Java, src.as_bytes(), &PathBuf::from("Test.java"));
        let class = syms.iter().find(|s| s.name == "UserService");
        assert!(class.is_some(), "should find class: {:?}", syms);
        assert_eq!(class.unwrap().kind, SymbolKind::Class);
    }

    // --- Ruby ---

    #[test]
    fn ruby_class_and_method() {
        let src = "class UserService\n  def get_name\n    'test'\n  end\nend";
        let syms = parse_and_extract(Language::Ruby, src.as_bytes(), &PathBuf::from("test.rb"));
        let class = syms.iter().find(|s| s.name == "UserService");
        assert!(class.is_some(), "should find class: {:?}", syms);
        assert_eq!(class.unwrap().kind, SymbolKind::Class);
        let method = syms.iter().find(|s| s.name == "get_name");
        assert!(method.is_some(), "should find method: {:?}", syms);
    }

    // --- C# ---

    #[test]
    fn csharp_class_and_method() {
        let src = "public class UserService {\n    public string GetName() {\n        return \"test\";\n    }\n}";
        let syms = parse_and_extract(Language::CSharp, src.as_bytes(), &PathBuf::from("Test.cs"));
        let class = syms.iter().find(|s| s.name == "UserService");
        assert!(class.is_some(), "should find class: {:?}", syms);
        assert_eq!(class.unwrap().kind, SymbolKind::Class);
        let method = syms.iter().find(|s| s.name == "GetName");
        assert!(method.is_some(), "should find method: {:?}", syms);
    }

    #[test]
    fn csharp_struct() {
        let src = "public struct Point {\n    public int X;\n    public int Y;\n}";
        let syms = parse_and_extract(Language::CSharp, src.as_bytes(), &PathBuf::from("Test.cs"));
        let s = syms.iter().find(|s| s.name == "Point");
        assert!(s.is_some(), "should find struct: {:?}", syms);
        assert_eq!(s.unwrap().kind, SymbolKind::Struct);
    }

    // --- Lua ---

    #[test]
    fn lua_function() {
        let src = "function greet(name)\n    return 'Hello, ' .. name\nend";
        let syms = parse_and_extract(Language::Lua, src.as_bytes(), &PathBuf::from("test.lua"));
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "greet");
        assert_eq!(syms[0].kind, SymbolKind::Fn);
    }

    // --- Zig ---

    #[test]
    fn zig_function() {
        let src = "pub fn calculate(amount: u64) u64 {\n    return amount * 3;\n}";
        let syms = parse_and_extract(Language::Zig, src.as_bytes(), &PathBuf::from("test.zig"));
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "calculate");
        assert_eq!(syms[0].kind, SymbolKind::Fn);
    }

    #[test]
    fn zig_struct() {
        let src = "const Point = struct {\n    x: f32,\n    y: f32,\n};";
        let syms = parse_and_extract(Language::Zig, src.as_bytes(), &PathBuf::from("test.zig"));
        assert_eq!(syms.len(), 1, "should find struct: {:?}", syms);
        assert_eq!(syms[0].name, "Point");
        assert_eq!(syms[0].kind, SymbolKind::Struct);
    }

    #[test]
    fn zig_enum() {
        let src = "const Color = enum {\n    red,\n    green,\n    blue,\n};";
        let syms = parse_and_extract(Language::Zig, src.as_bytes(), &PathBuf::from("test.zig"));
        assert_eq!(syms.len(), 1, "should find enum: {:?}", syms);
        assert_eq!(syms[0].name, "Color");
        assert_eq!(syms[0].kind, SymbolKind::Enum);
    }

    #[test]
    fn zig_union() {
        let src = "const Msg = union {\n    int: i32,\n    float: f64,\n};";
        let syms = parse_and_extract(Language::Zig, src.as_bytes(), &PathBuf::from("test.zig"));
        assert_eq!(syms.len(), 1, "should find union: {:?}", syms);
        assert_eq!(syms[0].name, "Msg");
        assert_eq!(syms[0].kind, SymbolKind::Struct);
    }

    #[test]
    fn zig_pub_struct() {
        let src = "pub const Point = struct {\n    x: f32,\n    y: f32,\n};";
        let syms = parse_and_extract(Language::Zig, src.as_bytes(), &PathBuf::from("test.zig"));
        assert_eq!(syms.len(), 1, "should find pub struct: {:?}", syms);
        assert_eq!(syms[0].name, "Point");
        assert_eq!(syms[0].kind, SymbolKind::Struct);
    }

    #[test]
    fn zig_error_set() {
        let src = "const MyError = error {\n    OutOfMemory,\n    InvalidInput,\n};";
        let syms = parse_and_extract(Language::Zig, src.as_bytes(), &PathBuf::from("test.zig"));
        assert_eq!(syms.len(), 1, "should find error set: {:?}", syms);
        assert_eq!(syms[0].name, "MyError");
        assert_eq!(syms[0].kind, SymbolKind::Enum);
    }

    // --- Bash ---

    #[test]
    fn bash_function() {
        let src = "function greet() {\n    echo \"Hello\"\n}";
        let syms = parse_and_extract(Language::Bash, src.as_bytes(), &PathBuf::from("test.sh"));
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "greet");
        assert_eq!(syms[0].kind, SymbolKind::Fn);
    }

    // --- Solidity ---

    #[test]
    fn solidity_contract_and_function() {
        let src = "contract Token {\n    function transfer(address to, uint amount) public {\n    }\n}";
        let syms = parse_and_extract(Language::Solidity, src.as_bytes(), &PathBuf::from("test.sol"));
        let contract = syms.iter().find(|s| s.name == "Token");
        assert!(contract.is_some(), "should find contract: {:?}", syms);
        let func = syms.iter().find(|s| s.name == "transfer");
        assert!(func.is_some(), "should find function: {:?}", syms);
    }

    #[test]
    fn solidity_event() {
        let src = "contract Token {\n    event Transfer(address indexed from, address indexed to, uint256 value);\n}";
        let syms = parse_and_extract(Language::Solidity, src.as_bytes(), &PathBuf::from("test.sol"));
        let event = syms.iter().find(|s| s.name == "Transfer");
        assert!(event.is_some(), "should find event: {:?}", syms);
        assert_eq!(event.unwrap().kind, SymbolKind::Event);
    }

    // --- Elixir ---

    #[test]
    fn elixir_module_and_function() {
        let src = "defmodule MyApp.Users do\n  def get_user(id) do\n    id\n  end\nend";
        let syms = parse_and_extract(Language::Elixir, src.as_bytes(), &PathBuf::from("test.ex"));
        let module = syms.iter().find(|s| s.name == "MyApp.Users");
        assert!(module.is_some(), "should find module: {:?}", syms);
        assert_eq!(module.unwrap().kind, SymbolKind::Module);
        let func = syms.iter().find(|s| s.name == "get_user");
        assert!(func.is_some(), "should find function: {:?}", syms);
        assert_eq!(func.unwrap().kind, SymbolKind::Fn);
    }

    // --- find_references tests ---

    #[test]
    fn refs_rust_finds_all_usages() {
        let src = "struct Foo { x: i32 }\nfn bar(f: Foo) -> Foo { f }";
        let refs = find_references(Language::Rust, src.as_bytes(), &PathBuf::from("test.rs"), "Foo");
        assert_eq!(refs.len(), 3, "should find struct def + 2 usages: {:?}", refs.iter().map(|r| r.line).collect::<Vec<_>>());
    }

    #[test]
    fn refs_rust_no_match() {
        let src = "fn main() {}";
        let refs = find_references(Language::Rust, src.as_bytes(), &PathBuf::from("test.rs"), "nonexistent");
        assert!(refs.is_empty());
    }

    #[test]
    fn refs_line_column_correct() {
        let src = "let x = 1;\nlet y = x + x;";
        let refs = find_references(Language::Rust, src.as_bytes(), &PathBuf::from("test.rs"), "x");
        assert_eq!(refs.len(), 3);
        assert_eq!(refs[0].line, 1);
        assert_eq!(refs[1].line, 2);
        assert_eq!(refs[2].line, 2);
    }

    #[test]
    fn refs_typescript_identifier() {
        let src = "const foo = 1;\nconsole.log(foo);";
        let refs = find_references(Language::TypeScript, src.as_bytes(), &PathBuf::from("test.ts"), "foo");
        assert_eq!(refs.len(), 2);
    }

    #[test]
    fn refs_python_identifier() {
        let src = "def greet(name):\n    return name";
        let refs = find_references(Language::Python, src.as_bytes(), &PathBuf::from("test.py"), "name");
        assert_eq!(refs.len(), 2);
    }
}
