use std::collections::HashMap;
use tree_sitter::{Node, Query, QueryCursor, Tree};

use crate::index::{Language, Symbol, SymbolKind};
use super::LanguageModule;

pub struct PythonModule;

impl LanguageModule for PythonModule {
    fn language(&self) -> Language {
        Language::Python
    }

    fn extensions(&self) -> &[&str] {
        &["py"]
    }

    fn extract_symbols(&self, tree: &Tree, source: &[u8]) -> Vec<Symbol> {
        let ts_lang = tree.language();
        let query = match Query::new(&ts_lang, tree_sitter_python::TAGS_QUERY) {
            Ok(q) => q,
            Err(e) => {
                eprintln!("cx: failed to compile Python tags query: {}", e);
                return Vec::new();
            }
        };

        let mut cursor = QueryCursor::new();
        let matches = cursor.matches(&query, tree.root_node(), source);
        let capture_names = query.capture_names();

        let mut symbols = Vec::new();

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
                "definition.class" => SymbolKind::Class,
                "definition.constant" => SymbolKind::Const,
                _ => continue,
            };

            let byte_range = (def_n.start_byte(), def_n.end_byte());
            let signature = build_py_signature(def_n, source);

            // Exported: top-level (parent is module) and name doesn't start with _
            let is_exported = is_top_level(def_n) && !name.starts_with('_');

            symbols.push(Symbol {
                name,
                kind,
                signature,
                byte_range,
                is_exported,
            });
        }

        // Deduplicate by byte_range
        let mut seen: HashMap<(usize, usize), usize> = HashMap::new();
        let mut deduped: Vec<Symbol> = Vec::new();
        for sym in symbols {
            if !seen.contains_key(&sym.byte_range) {
                seen.insert(sym.byte_range, deduped.len());
                deduped.push(sym);
            }
        }

        deduped
    }
}

fn build_py_signature(node: Node, source: &[u8]) -> String {
    let start = node.start_byte();
    let end = node.end_byte();
    let text = &source[start..end];

    // For functions/classes, take up to the colon
    if let Some(pos) = text.iter().position(|&b| b == b':') {
        let sig = String::from_utf8_lossy(&text[..pos]).trim().to_string();
        if !sig.is_empty() {
            return sig;
        }
    }

    // Fallback: first line
    let first_line = text
        .iter()
        .position(|&b| b == b'\n')
        .map(|p| &text[..p])
        .unwrap_or(text);

    String::from_utf8_lossy(first_line).trim().to_string()
}

fn is_top_level(node: Node) -> bool {
    let mut current = node;
    while let Some(parent) = current.parent() {
        match parent.kind() {
            "module" => return true,
            // If we hit a function or class body, we're nested
            "function_definition" | "class_definition" => return false,
            // Keep walking through expression_statement, block, etc.
            _ => current = parent,
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::language::LanguageModule;
    use tree_sitter::Parser;

    fn parse_py(source: &str) -> Tree {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_python::LANGUAGE.into())
            .unwrap();
        parser.parse(source, None).unwrap()
    }

    #[test]
    fn test_extract_function() {
        let src = "def greet(name: str) -> str:\n    return f'Hello, {name}'";
        let tree = parse_py(src);
        let module = PythonModule;
        let symbols = module.extract_symbols(&tree, src.as_bytes());

        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "greet");
        assert_eq!(symbols[0].kind, SymbolKind::Fn);
        assert!(symbols[0].is_exported);
        assert!(symbols[0].signature.contains("greet"));
        assert!(!symbols[0].signature.contains(':'));
    }

    #[test]
    fn test_extract_class() {
        let src = "class UserService:\n    def get_name(self):\n        return 'test'";
        let tree = parse_py(src);
        let module = PythonModule;
        let symbols = module.extract_symbols(&tree, src.as_bytes());

        let class = symbols.iter().find(|s| s.name == "UserService").unwrap();
        assert_eq!(class.kind, SymbolKind::Class);
        assert!(class.is_exported);

        // Nested method should also be captured but not exported
        let method = symbols.iter().find(|s| s.name == "get_name");
        assert!(method.is_some());
        assert!(!method.unwrap().is_exported); // nested, not top-level
    }

    #[test]
    fn test_private_function() {
        let src = "def _helper():\n    pass";
        let tree = parse_py(src);
        let module = PythonModule;
        let symbols = module.extract_symbols(&tree, src.as_bytes());

        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "_helper");
        assert!(!symbols[0].is_exported);
    }

    #[test]
    fn test_top_level_assignment() {
        let src = "MAX_SIZE = 100";
        let tree = parse_py(src);
        let module = PythonModule;
        let symbols = module.extract_symbols(&tree, src.as_bytes());

        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "MAX_SIZE");
        assert_eq!(symbols[0].kind, SymbolKind::Const);
        assert!(symbols[0].is_exported);
    }

    #[test]
    fn test_private_constant() {
        let src = "_internal = 42";
        let tree = parse_py(src);
        let module = PythonModule;
        let symbols = module.extract_symbols(&tree, src.as_bytes());

        assert_eq!(symbols.len(), 1);
        assert!(!symbols[0].is_exported);
    }

    #[test]
    fn test_multiple_symbols() {
        let src = "def foo():\n    pass\n\ndef bar():\n    pass\n\nclass Baz:\n    pass";
        let tree = parse_py(src);
        let module = PythonModule;
        let symbols = module.extract_symbols(&tree, src.as_bytes());

        assert!(symbols.len() >= 3);
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"foo"));
        assert!(names.contains(&"bar"));
        assert!(names.contains(&"Baz"));
    }

    #[test]
    fn test_nested_function_not_exported() {
        let src = "def outer():\n    def inner():\n        pass";
        let tree = parse_py(src);
        let module = PythonModule;
        let symbols = module.extract_symbols(&tree, src.as_bytes());

        let outer = symbols.iter().find(|s| s.name == "outer").unwrap();
        assert!(outer.is_exported);

        let inner = symbols.iter().find(|s| s.name == "inner").unwrap();
        assert!(!inner.is_exported);
    }
}
