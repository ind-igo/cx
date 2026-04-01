mod extract;
mod queries;

use crate::index::{Symbol, SymbolKind};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{LazyLock, RwLock};
use tree_sitter::{Parser, Query};

/// Cache compiled queries keyed by resolved grammar name (e.g. "rust", "tsx").
static QUERY_CACHE: LazyLock<RwLock<HashMap<&'static str, Query>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

// --- Language registry ---

pub(crate) struct LanguageConfig {
    pub name: &'static str,
    pub extensions: &'static [&'static str],
    /// Map certain file extensions to a different grammar name (e.g. tsx → "tsx").
    pub grammar_override: &'static [(&'static str, &'static str)],
    /// Names to pass to `tree_sitter_language_pack::download()`. Empty = use name.
    pub download_names: &'static [&'static str],
    pub query: &'static str,
    /// Find this child node kind to determine where the body starts; signature = text before it.
    pub sig_body_child: Option<&'static str>,
    /// Scan for this byte to split signature from body (e.g. b'{').
    pub sig_delimiter: Option<u8>,
    /// (capture_name, node_kind, SymbolKind) — checked before defaults.
    /// Empty node_kind matches any node.
    pub kind_overrides: &'static [(&'static str, &'static str, SymbolKind)],
    /// Node kinds that represent identifier references (for find-references).
    pub ref_node_types: &'static [&'static str],
}

static LANGUAGES: &[LanguageConfig] = &[
    LanguageConfig {
        name: "rust",
        extensions: &["rs"],
        grammar_override: &[],
        download_names: &[],
        query: queries::RUST,
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
        query: queries::TYPESCRIPT,
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
        query: queries::PYTHON,
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
        query: queries::GO,
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
        query: queries::C,
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
        query: queries::CPP,
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
        query: queries::JAVA,
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
        query: queries::RUBY,
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
        query: queries::LUA,
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
        query: queries::ZIG,
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
        query: queries::BASH,
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
        query: queries::SOLIDITY,
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
        query: queries::ELIXIR,
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
        query: queries::SWIFT,
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

    thread_local! {
        static PARSER: std::cell::RefCell<Parser> = std::cell::RefCell::new(Parser::new());
    }

    let tree = PARSER.with_borrow_mut(|parser| {
        parser.set_language(&ts_lang).map_err(|_| LangError::ParseFailed)?;
        parser.parse(source, None).ok_or(LangError::ParseFailed)
    })?;
    Ok((config, tree, grammar_name))
}

/// Parse source and find all identifier nodes whose text matches `name`.
pub fn find_references(lang: &str, source: &[u8], path: &Path, name: &str) -> Result<Vec<extract::Reference>, LangError> {
    let (config, tree, _) = parse_source(lang, source, path)?;

    let mut refs = Vec::new();
    let mut stack = vec![tree.root_node()];
    while let Some(node) = stack.pop() {
        if node.child_count() == 0
            && config.ref_node_types.contains(&node.kind())
            && node.utf8_text(source).ok() == Some(name)
        {
            refs.push(extract::Reference {
                line: node.start_position().row + 1,
                byte_offset: node.start_byte(),
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

    // Fast path: read lock for cache hits (concurrent reads don't block each other)
    {
        let cache = QUERY_CACHE.read().unwrap_or_else(|e| e.into_inner());
        if let Some(query) = cache.get(grammar_name) {
            return Ok(extract::extract_symbols(config, query, &tree, source));
        }
    }

    // Slow path: write lock for cache miss
    let mut cache = QUERY_CACHE.write().unwrap_or_else(|e| e.into_inner());
    let query = cache.entry(grammar_name).or_insert_with(|| {
        Query::new(&tree.language(), config.query).expect("query compilation failed")
    });

    Ok(extract::extract_symbols(config, query, &tree, source))
}

#[cfg(test)]
mod tests;
