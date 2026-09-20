mod extract;
mod markdown;
mod queries;
pub mod overrides;
pub mod bundled;

use crate::index::{Symbol, SymbolKind};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::{LazyLock, RwLock};
use tree_sitter::{Parser, Query};

/// Cache compiled queries keyed by resolved grammar name (e.g. "rust", "tsx").
static QUERY_CACHE: LazyLock<RwLock<HashMap<&'static str, Query>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

// --- Language registry ---

#[derive(Hash)]
pub struct LanguageConfig {
    pub name: String,
    pub extensions: Vec<String>,
    /// Map certain file extensions to a different grammar name (e.g. tsx → "tsx").
    pub grammar_override: Vec<(String, String)>,
    /// Names to pass to `tree_sitter_language_pack::download()`. Empty = use name.
    pub download_names: Vec<String>,
    pub query: String,
    /// Find this child node kind or field name; signature = text before the body.
    pub sig_body_child: Option<String>,
    /// Scan for this byte to split signature from body (e.g. b'{').
    pub sig_delimiter: Option<u8>,
    /// (`capture_name`, `node_kind`, `SymbolKind`) — checked before defaults.
    /// Empty `node_kind` matches any node.
    pub kind_overrides: Vec<(String, String, SymbolKind)>,
    /// Node kinds that represent identifier references (for find-references).
    pub ref_node_types: Vec<String>,
}

static LANGUAGES: LazyLock<Vec<LanguageConfig>> = LazyLock::new(|| vec![
    LanguageConfig {
        name: "mojo".into(),
        extensions: vec!["mojo".into(), "🔥".into()],
        grammar_override: vec![],
        download_names: vec![],
        query: include_str!("../../languages/mojo/symbols.scm").into(),
        sig_body_child: Some("body".into()),
        sig_delimiter: None,
        kind_overrides: vec![],
        ref_node_types: vec!["identifier".into(), "escaped_identifier_content".into()],
    },
    LanguageConfig {
        name: "bend".into(),
        extensions: vec!["bend".into()],
        grammar_override: vec![],
        download_names: vec![],
        query: include_str!("../../languages/bend/symbols.scm").into(),
        sig_body_child: Some("body".into()),
        sig_delimiter: None,
        kind_overrides: vec![],
        ref_node_types: vec!["identifier".into(), "scoped_identifier".into()],
    },
    LanguageConfig {
        name: "markdown".into(),
        extensions: vec!["md".into(), "markdown".into(), "mdown".into()],
        grammar_override: vec![],
        download_names: vec![],
        query: "".into(),
        sig_body_child: None,
        sig_delimiter: None,
        kind_overrides: vec![],
        ref_node_types: vec![],
    },
    LanguageConfig {
        name: "rust".into(),
        extensions: vec!["rs".into()],
        grammar_override: vec![],
        download_names: vec![],
        query: queries::RUST.into(),
        sig_body_child: None,
        sig_delimiter: Some(b'{'),
        kind_overrides: vec![
            ("definition.class".into(), "struct_item".into(), SymbolKind::Struct),
            ("definition.class".into(), "enum_item".into(), SymbolKind::Enum),
            ("definition.class".into(), "union_item".into(), SymbolKind::Struct),
            ("definition.class".into(), "type_item".into(), SymbolKind::Type),
            ("definition.class".into(), "".into(), SymbolKind::Struct),
            ("definition.interface".into(), "".into(), SymbolKind::Trait),
            ("definition.macro".into(), "".into(), SymbolKind::Fn),
        ],
        ref_node_types: vec!["identifier".into(), "type_identifier".into(), "field_identifier".into()],
    },
    LanguageConfig {
        name: "typescript".into(),
        extensions: vec!["ts".into(), "tsx".into(), "js".into(), "jsx".into()],
        grammar_override: vec![("tsx".into(), "tsx".into()), ("jsx".into(), "tsx".into())],
        download_names: vec!["typescript".into(), "tsx".into()],
        query: queries::TYPESCRIPT.into(),
        sig_body_child: None,
        sig_delimiter: Some(b'{'),
        kind_overrides: vec![],
        ref_node_types: vec!["identifier".into(), "type_identifier".into(), "property_identifier".into(), "shorthand_property_identifier".into(), "shorthand_property_identifier_pattern".into()],
    },
    LanguageConfig {
        name: "python".into(),
        extensions: vec!["py".into()],
        grammar_override: vec![],
        download_names: vec![],
        query: queries::PYTHON.into(),
        sig_body_child: Some("block".into()),
        sig_delimiter: None,
        kind_overrides: vec![],
        ref_node_types: vec!["identifier".into()],
    },
    LanguageConfig {
        name: "go".into(),
        extensions: vec!["go".into()],
        grammar_override: vec![],
        download_names: vec![],
        query: queries::GO.into(),
        sig_body_child: None,
        sig_delimiter: Some(b'{'),
        kind_overrides: vec![],
        ref_node_types: vec!["identifier".into(), "type_identifier".into(), "field_identifier".into()],
    },
    LanguageConfig {
        name: "c".into(),
        extensions: vec!["c".into()],
        grammar_override: vec![],
        download_names: vec![],
        query: queries::C.into(),
        sig_body_child: None,
        sig_delimiter: Some(b'{'),
        kind_overrides: vec![
            ("definition.class".into(), "".into(), SymbolKind::Struct),
        ],
        ref_node_types: vec!["identifier".into(), "type_identifier".into(), "field_identifier".into()],
    },
    LanguageConfig {
        name: "objc".into(),
        extensions: vec!["m".into(), "mm".into()],
        grammar_override: vec![],
        download_names: vec![],
        query: queries::OBJC.into(),
        sig_body_child: Some("compound_statement".into()),
        sig_delimiter: None,
        kind_overrides: vec![],
        ref_node_types: vec![
            "identifier".into(),
            "type_identifier".into(),
            "field_identifier".into(),
            "method_identifier".into(),
        ],
    },
    LanguageConfig {
        name: "cpp".into(),
        extensions: vec!["cpp".into(), "cc".into(), "cxx".into(), "h".into(), "hpp".into(), "hxx".into(), "hh".into()],
        grammar_override: vec![],
        download_names: vec![],
        query: queries::CPP.into(),
        sig_body_child: None,
        sig_delimiter: Some(b'{'),
        kind_overrides: vec![],
        ref_node_types: vec!["identifier".into(), "type_identifier".into(), "field_identifier".into()],
    },
    LanguageConfig {
        name: "java".into(),
        extensions: vec!["java".into()],
        grammar_override: vec![],
        download_names: vec![],
        query: queries::JAVA.into(),
        sig_body_child: None,
        sig_delimiter: Some(b'{'),
        kind_overrides: vec![],
        ref_node_types: vec!["identifier".into(), "type_identifier".into()],
    },
    LanguageConfig {
        name: "ruby".into(),
        extensions: vec!["rb".into()],
        grammar_override: vec![],
        download_names: vec![],
        query: queries::RUBY.into(),
        sig_body_child: None,
        sig_delimiter: None,
        kind_overrides: vec![],
        ref_node_types: vec!["identifier".into(), "constant".into()],
    },
    LanguageConfig {
        name: "lua".into(),
        extensions: vec!["lua".into()],
        grammar_override: vec![],
        download_names: vec![],
        query: queries::LUA.into(),
        sig_body_child: None,
        sig_delimiter: None,
        kind_overrides: vec![],
        ref_node_types: vec!["identifier".into()],
    },
    LanguageConfig {
        name: "zig".into(),
        extensions: vec!["zig".into()],
        grammar_override: vec![],
        download_names: vec![],
        query: queries::ZIG.into(),
        sig_body_child: None,
        sig_delimiter: Some(b'{'),
        kind_overrides: vec![
            ("definition.class".into(), "Decl".into(), SymbolKind::Struct),
        ],
        ref_node_types: vec!["IDENTIFIER".into()],
    },
    LanguageConfig {
        name: "bash".into(),
        extensions: vec!["sh".into(), "bash".into()],
        grammar_override: vec![],
        download_names: vec![],
        query: queries::BASH.into(),
        sig_body_child: None,
        sig_delimiter: Some(b'{'),
        kind_overrides: vec![],
        ref_node_types: vec!["word".into()],
    },
    LanguageConfig {
        name: "solidity".into(),
        extensions: vec!["sol".into()],
        grammar_override: vec![],
        download_names: vec![],
        query: queries::SOLIDITY.into(),
        sig_body_child: None,
        sig_delimiter: Some(b'{'),
        kind_overrides: vec![],
        ref_node_types: vec!["identifier".into()],
    },
    LanguageConfig {
        name: "dart".into(),
        extensions: vec!["dart".into()],
        grammar_override: vec![],
        download_names: vec![],
        query: queries::DART.into(),
        sig_body_child: None,
        sig_delimiter: Some(b'{'),
        kind_overrides: vec![],
        ref_node_types: vec!["identifier".into(), "type_identifier".into()],
    },
    LanguageConfig {
        name: "elixir".into(),
        extensions: vec!["ex".into(), "exs".into()],
        grammar_override: vec![],
        download_names: vec![],
        query: queries::ELIXIR.into(),
        sig_body_child: None,
        sig_delimiter: None,
        kind_overrides: vec![],
        ref_node_types: vec!["identifier".into(), "alias".into()],
    },
    LanguageConfig {
        name: "swift".into(),
        extensions: vec!["swift".into()],
        grammar_override: vec![],
        download_names: vec![],
        query: queries::SWIFT.into(),
        sig_body_child: None,
        sig_delimiter: Some(b'{'),
        kind_overrides: vec![
            ("definition.struct".into(), "".into(), SymbolKind::Struct),
            ("definition.enum".into(), "".into(), SymbolKind::Enum),
        ],
        ref_node_types: vec!["simple_identifier".into(), "type_identifier".into()],
    },
    LanguageConfig {
        name: "php".into(),
        extensions: vec!["php".into()],
        grammar_override: vec![],
        download_names: vec![],
        query: queries::PHP.into(),
        sig_body_child: Some("body".into()),
        sig_delimiter: None,
        kind_overrides: vec![
            ("definition.class".into(), "trait_declaration".into(), SymbolKind::Trait),
        ],
        ref_node_types: vec!["name".into(), "variable_name".into(), "qualified_name".into(), "namespace_name".into()],
    },
]);

// --- Errors ---

#[derive(Debug)]
pub enum LangError {
    NotInstalled(String),
    ParseFailed,
}

impl std::fmt::Display for LangError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotInstalled(name) => write!(f, "{name} grammar not installed — run: cx lang add {name}"),
            Self::ParseFailed => write!(f, "parse failed"),
        }
    }
}

// --- Public API ---

/// Parser installation state and definitions used by persistent indexes.
pub fn fingerprint() -> u64 {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    overrides::fingerprint().hash(&mut hash);
    LANGUAGES.hash(&mut hash);
    bundled::installed().hash(&mut hash);
    env!("CX_BUNDLED_GRAMMAR_HASH").hash(&mut hash);
    hash.finish()
}

/// Detect language config name from file extension.
pub fn detect_language(path: &Path) -> Option<&'static str> {
    let ext = path.extension().and_then(|e| e.to_str())?;
    if let Some(language) = overrides::installed().iter().find(|o| o.config.extensions.iter().any(|e| e == ext)) {
        return Some(&language.config.name);
    }
    LANGUAGES
        .iter()
        .find(|c| overrides::get(&c.name).is_none() && c.extensions.iter().any(|e| e == ext))
        .map(|c| c.name.as_str())
}

/// Return all supported language config names.
pub fn builtin_languages() -> Vec<&'static str> {
    LANGUAGES.iter().map(|c| c.name.as_str()).collect()
}

pub fn supported_languages() -> Vec<&'static str> {
    let mut names: Vec<_> = LANGUAGES.iter().map(|c| c.name.as_str()).chain(overrides::installed().iter().map(|o| o.config.name.as_str())).collect();
    names.sort_unstable();
    names.dedup();
    names
}

/// Return the primary file extension for a language config name.
pub fn primary_extension(lang: &str) -> &str {
    if let Some(language) = overrides::get(lang) {
        return language.config.extensions.first().map(String::as_str).unwrap_or(lang);
    }
    LANGUAGES.iter()
        .find(|c| c.name == lang)
        .and_then(|c| c.extensions.first().map(String::as_str))
        .unwrap_or(lang)
}

/// Return the download names for a language (for `cx lang add`).
pub fn download_names_for(lang: &str) -> Vec<&'static str> {
    LANGUAGES.iter()
        .find(|c| c.name == lang)
        .map(|c| {
            if c.download_names.is_empty() {
                vec![c.name.as_str()]
            } else {
                c.download_names.iter().map(String::as_str).collect()
            }
        })
        .unwrap_or_default()
}

/// Resolve the grammar name for a given config + file extension.
fn resolve_grammar_name<'a>(config: &'a LanguageConfig, ext: &str) -> &'a str {
    for (e, grammar) in &config.grammar_override {
        if e == ext {
            return grammar;
        }
    }
    &config.name
}

/// Look up config, create parser, and parse source into a tree.
fn parse_source(lang: &str, source: &[u8], path: &Path) -> Result<(&'static LanguageConfig, tree_sitter::Tree, &'static str), LangError> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let (config, grammar_name, ts_lang) = if let Some(language) = overrides::get(lang) {
        (&language.config, language.config.name.as_str(), language.language.clone())
    } else if let Some(language) = bundled::language(lang) {
        if !bundled::installed().contains(&lang) {
            return Err(LangError::NotInstalled(lang.to_string()));
        }
        let config = LANGUAGES.iter().find(|c| c.name == lang).expect("bundled language config");
        (config, config.name.as_str(), language)
    } else {
        let config = LANGUAGES.iter().find(|c| c.name == lang).ok_or_else(|| LangError::NotInstalled(lang.to_string()))?;
        let grammar_name = resolve_grammar_name(config, ext);
        // get_language auto-downloads missing grammars; leave downloads to `cx lang add`.
        static INSTALLED: LazyLock<Vec<String>> = LazyLock::new(tree_sitter_language_pack::downloaded_languages);
        if !INSTALLED.iter().any(|name| name == grammar_name) {
            return Err(LangError::NotInstalled(config.name.to_string()));
        }
        let ts_lang = tree_sitter_language_pack::get_language(grammar_name)
            .map_err(|_| LangError::NotInstalled(config.name.to_string()))?;
        (config, grammar_name, ts_lang)
    };

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
        if config.ref_node_types.iter().any(|kind| kind == node.kind())
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
    if let Some(language) = overrides::get(lang) {
        let (config, tree, _) = parse_source(lang, source, path)?;
        return Ok(extract::extract_symbols(config, &language.query, &tree, source));
    }
    if lang == "markdown" {
        parse_source(lang, source, path)?;
        return Ok(markdown::extract_headings(source));
    }

    let (config, tree, grammar_name) = parse_source(lang, source, path)?;

    // Fast path: read lock for cache hits (concurrent reads don't block each other)
    {
        let cache = QUERY_CACHE.read().unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(query) = cache.get(grammar_name) {
            return Ok(extract::extract_symbols(config, query, &tree, source));
        }
    }

    // Slow path: write lock for cache miss
    let mut cache = QUERY_CACHE.write().unwrap_or_else(std::sync::PoisonError::into_inner);
    let query = cache.entry(grammar_name).or_insert_with(|| {
        Query::new(&tree.language(), &config.query).expect("query compilation failed")
    });

    Ok(extract::extract_symbols(config, query, &tree, source))
}

#[cfg(test)]
mod tests;
