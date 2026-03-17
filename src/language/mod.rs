pub mod rust;
pub mod typescript;
pub mod python;

use crate::index::{Language, Symbol};
use tree_sitter::Tree;

pub trait LanguageModule: Send + Sync {
    fn language(&self) -> Language;
    fn extensions(&self) -> &[&str];
    fn extract_symbols(&self, tree: &Tree, source: &[u8]) -> Vec<Symbol>;
}

pub fn detect_language(path: &std::path::Path) -> Language {
    match path.extension().and_then(|e| e.to_str()) {
        Some("rs") => Language::Rust,
        Some("ts" | "tsx" | "js" | "jsx") => Language::TypeScript,
        Some("py") => Language::Python,
        _ => Language::Unknown,
    }
}
