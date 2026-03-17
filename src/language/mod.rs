pub mod rust;
pub mod typescript;
pub mod python;

use crate::index::{Language, Symbol};
use std::path::Path;
use tree_sitter::{Parser, Tree};

pub trait LanguageModule: Send + Sync {
    fn language(&self) -> Language;
    fn extensions(&self) -> &[&str];
    fn extract_symbols(&self, tree: &Tree, source: &[u8]) -> Vec<Symbol>;
}

pub fn detect_language(path: &Path) -> Language {
    match path.extension().and_then(|e| e.to_str()) {
        Some("rs") => Language::Rust,
        Some("ts" | "tsx" | "js" | "jsx") => Language::TypeScript,
        Some("py") => Language::Python,
        _ => Language::Unknown,
    }
}

/// Parse a file and extract symbols for the given language.
/// `path` is used to distinguish .tsx from .ts for grammar selection.
pub fn parse_and_extract(lang: Language, source: &[u8], path: &Path) -> Vec<Symbol> {
    let ts_lang = match lang {
        Language::Rust => tree_sitter_rust::LANGUAGE.into(),
        Language::TypeScript => {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            match ext {
                "tsx" | "jsx" => tree_sitter_typescript::LANGUAGE_TSX.into(),
                _ => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            }
        }
        Language::Python => tree_sitter_python::LANGUAGE.into(),
        Language::Unknown => {
            return Vec::new();
        }
    };

    let mut parser = Parser::new();
    if parser.set_language(&ts_lang).is_err() {
        return Vec::new();
    }

    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return Vec::new(),
    };

    let module: Box<dyn LanguageModule> = match lang {
        Language::Rust => Box::new(rust::RustModule),
        Language::TypeScript => Box::new(typescript::TypeScriptModule),
        Language::Python => Box::new(python::PythonModule),
        _ => return Vec::new(),
    };

    module.extract_symbols(&tree, source)
}
