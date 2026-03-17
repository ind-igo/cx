use std::collections::HashMap;
use tree_sitter::{Node, Query, QueryCursor, Tree};

use crate::index::{Language, Symbol, SymbolKind};
use super::LanguageModule;

pub struct RustModule;

impl LanguageModule for RustModule {
    fn language(&self) -> Language {
        Language::Rust
    }

    fn extensions(&self) -> &[&str] {
        &["rs"]
    }

    fn extract_symbols(&self, tree: &Tree, source: &[u8]) -> Vec<Symbol> {
        let ts_lang = tree.language();
        let query = match Query::new(&ts_lang, tree_sitter_rust::TAGS_QUERY) {
            Ok(q) => q,
            Err(e) => {
                eprintln!("cx: failed to compile Rust tags query: {}", e);
                return Vec::new();
            }
        };

        let mut cursor = QueryCursor::new();
        let matches = cursor.matches(&query, tree.root_node(), source);

        let mut symbols = Vec::new();
        let capture_names = query.capture_names();

        for m in matches {
            let mut name_node: Option<Node> = None;
            let mut def_node: Option<Node> = None;
            let mut def_kind: Option<&str> = None;

            for capture in m.captures {
                let cname = &*capture_names[capture.index as usize];
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

            let kind = match kind_str {
                "definition.function" => SymbolKind::Fn,
                "definition.method" => SymbolKind::Method,
                "definition.class" => {
                    // Disambiguate struct/enum/type based on node kind
                    match def_n.kind() {
                        "struct_item" => SymbolKind::Struct,
                        "enum_item" => SymbolKind::Enum,
                        "union_item" => SymbolKind::Struct,
                        "type_item" => SymbolKind::Type,
                        _ => SymbolKind::Struct,
                    }
                }
                "definition.interface" => SymbolKind::Trait,
                "definition.module" => SymbolKind::Module,
                "definition.macro" => SymbolKind::Fn, // treat macros as fn for now
                _ => continue,
            };

            let byte_range = (def_n.start_byte(), def_n.end_byte());
            let signature = build_signature(def_n, source);
            let is_exported = has_pub_visibility(def_n, source);

            symbols.push(Symbol {
                name,
                kind,
                signature,
                byte_range,
                is_exported,
            });
        }

        // Deduplicate: methods in impl blocks match both definition.method and
        // definition.function patterns. Keep the method version (more specific).
        let mut seen_ranges: HashMap<(usize, usize), usize> = HashMap::new();
        let mut deduped: Vec<Symbol> = Vec::new();

        for sym in symbols {
            if let Some(&existing_idx) = seen_ranges.get(&sym.byte_range) {
                // If existing is Fn and new is Method, replace with Method
                if deduped[existing_idx].kind == SymbolKind::Fn && sym.kind == SymbolKind::Method {
                    deduped[existing_idx] = sym;
                }
                // Otherwise keep existing (Method beats Fn)
            } else {
                seen_ranges.insert(sym.byte_range, deduped.len());
                deduped.push(sym);
            }
        }

        deduped
    }
}

/// Build signature: for functions, slice from start to opening '{'.
/// For structs/enums/traits, take the declaration line.
fn build_signature(node: Node, source: &[u8]) -> String {
    let start = node.start_byte();
    let end = node.end_byte();
    let text = &source[start..end];

    // For function items, find the opening '{'
    if node.kind() == "function_item" {
        if let Some(pos) = text.iter().position(|&b| b == b'{') {
            let sig = String::from_utf8_lossy(&text[..pos]).trim().to_string();
            return sig;
        }
    }

    // For other items, take first line
    let first_line = text
        .iter()
        .position(|&b| b == b'\n')
        .map(|p| &text[..p])
        .unwrap_or(text);

    let sig = String::from_utf8_lossy(first_line).trim().to_string();
    // Strip trailing '{' if present
    sig.trim_end_matches('{').trim().to_string()
}

/// Check if a node or its parent has a `pub` visibility modifier.
fn has_pub_visibility(node: Node, source: &[u8]) -> bool {
    // Check direct children for visibility_modifier
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "visibility_modifier" {
            return true;
        }
    }

    // Check if parent is a declaration_list inside a pub impl
    // (methods inside pub impl are accessible but not individually pub-marked)
    if let Some(parent) = node.parent() {
        if parent.kind() == "declaration_list" {
            // Check the node text for `pub` keyword at start
            let text = &source[node.start_byte()..node.end_byte()];
            if text.starts_with(b"pub ") || text.starts_with(b"pub(") {
                return true;
            }
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::language::LanguageModule;
    use tree_sitter::Parser;

    fn parse_rust(source: &str) -> Tree {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .unwrap();
        parser.parse(source, None).unwrap()
    }

    #[test]
    fn test_extract_pub_function() {
        let src = r#"pub fn calculate_fee(amount: u64) -> u64 {
    amount * 3 / 1000
}"#;
        let tree = parse_rust(src);
        let module = RustModule;
        let symbols = module.extract_symbols(&tree, src.as_bytes());

        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "calculate_fee");
        assert_eq!(symbols[0].kind, SymbolKind::Fn);
        assert!(symbols[0].is_exported);
        assert!(
            !symbols[0].signature.contains('{'),
            "signature should not contain '{{': {}",
            symbols[0].signature
        );
    }

    #[test]
    fn test_extract_private_function() {
        let src = "fn helper() -> bool { true }";
        let tree = parse_rust(src);
        let module = RustModule;
        let symbols = module.extract_symbols(&tree, src.as_bytes());

        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "helper");
        assert!(!symbols[0].is_exported);
    }

    #[test]
    fn test_extract_struct() {
        let src = "pub struct FeeConfig {\n    pub rate: u64,\n}";
        let tree = parse_rust(src);
        let module = RustModule;
        let symbols = module.extract_symbols(&tree, src.as_bytes());

        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "FeeConfig");
        assert_eq!(symbols[0].kind, SymbolKind::Struct);
        assert!(symbols[0].is_exported);
    }

    #[test]
    fn test_extract_enum() {
        let src = "pub enum FeeTier {\n    Low,\n    High,\n}";
        let tree = parse_rust(src);
        let module = RustModule;
        let symbols = module.extract_symbols(&tree, src.as_bytes());

        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "FeeTier");
        assert_eq!(symbols[0].kind, SymbolKind::Enum);
    }

    #[test]
    fn test_extract_trait() {
        let src = "pub trait Configurable {\n    fn configure(&self);\n}";
        let tree = parse_rust(src);
        let module = RustModule;
        let symbols = module.extract_symbols(&tree, src.as_bytes());

        // Should have trait + method
        let trait_sym = symbols.iter().find(|s| s.name == "Configurable").unwrap();
        assert_eq!(trait_sym.kind, SymbolKind::Trait);
    }

    #[test]
    fn test_extract_multiple_symbols() {
        let src = r#"
pub fn foo() {}
fn bar() {}
pub struct Baz;
"#;
        let tree = parse_rust(src);
        let module = RustModule;
        let symbols = module.extract_symbols(&tree, src.as_bytes());

        assert!(symbols.len() >= 3);
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"foo"));
        assert!(names.contains(&"bar"));
        assert!(names.contains(&"Baz"));
    }

    #[test]
    fn test_byte_range_valid() {
        let src = "pub fn test_func() -> u32 { 42 }";
        let tree = parse_rust(src);
        let module = RustModule;
        let symbols = module.extract_symbols(&tree, src.as_bytes());

        assert_eq!(symbols.len(), 1);
        let (start, end) = symbols[0].byte_range;
        assert!(start < end);
        assert!(end <= src.len());
        let body = &src[start..end];
        assert!(body.contains("test_func"));
    }
}
