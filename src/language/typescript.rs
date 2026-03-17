use std::collections::HashMap;
use tree_sitter::{Node, Query, QueryCursor, Tree};

use crate::index::{Language, Symbol, SymbolKind};
use super::LanguageModule;

pub struct TypeScriptModule;

// Custom query that covers common TypeScript/JS declarations.
// The bundled tags.scm only captures signatures and interfaces.
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
    value: (arrow_function)) @definition.function)

(variable_declaration
  (variable_declarator
    name: (identifier) @name
    value: (arrow_function)) @definition.function)
"#;

impl LanguageModule for TypeScriptModule {
    fn language(&self) -> Language {
        Language::TypeScript
    }

    fn extensions(&self) -> &[&str] {
        &["ts", "tsx", "js", "jsx"]
    }

    fn extract_symbols(&self, tree: &Tree, source: &[u8]) -> Vec<Symbol> {
        let ts_lang = tree.language();
        let query = match Query::new(&ts_lang, TS_QUERY) {
            Ok(q) => q,
            Err(e) => {
                eprintln!("cx: failed to compile TypeScript tags query: {}", e);
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
                "definition.method" => SymbolKind::Method,
                "definition.class" => SymbolKind::Class,
                "definition.interface" => SymbolKind::Interface,
                "definition.type" => SymbolKind::Type,
                "definition.enum" => SymbolKind::Enum,
                "definition.module" => SymbolKind::Module,
                _ => continue,
            };

            let byte_range = (def_n.start_byte(), def_n.end_byte());
            let signature = build_ts_signature(def_n, source);
            let is_exported = is_ts_exported(def_n, source);

            symbols.push(Symbol {
                name,
                kind,
                signature,
                byte_range,
                is_exported,
            });
        }

        // Deduplicate by byte_range (arrow functions can match both lexical and variable)
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

fn build_ts_signature(node: Node, source: &[u8]) -> String {
    let start = node.start_byte();
    let end = node.end_byte();
    let text = &source[start..end];

    // For functions/methods, slice to opening '{'
    if let Some(pos) = text.iter().position(|&b| b == b'{') {
        let sig = String::from_utf8_lossy(&text[..pos]).trim().to_string();
        if !sig.is_empty() {
            return sig;
        }
    }

    // For interfaces/types, take first line
    let first_line = text
        .iter()
        .position(|&b| b == b'\n')
        .map(|p| &text[..p])
        .unwrap_or(text);

    String::from_utf8_lossy(first_line)
        .trim()
        .trim_end_matches('{')
        .trim()
        .to_string()
}

/// Check if the node is exported: preceded by `export` keyword or inside export_statement.
fn is_ts_exported(node: Node, source: &[u8]) -> bool {
    // Check if parent is an export_statement
    if let Some(parent) = node.parent() {
        if parent.kind() == "export_statement" {
            return true;
        }
    }

    // Check if the text before this node on the same line starts with "export"
    let start = node.start_byte();
    if start >= 7 {
        let prefix = &source[start.saturating_sub(20)..start];
        if prefix.ends_with(b"export ")
            || prefix.ends_with(b"export default ")
            || prefix.ends_with(b"export declare ")
        {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::language::LanguageModule;
    use tree_sitter::Parser;

    fn parse_ts(source: &str) -> Tree {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into())
            .unwrap();
        parser.parse(source, None).unwrap()
    }

    fn parse_tsx(source: &str) -> Tree {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_typescript::LANGUAGE_TSX.into())
            .unwrap();
        parser.parse(source, None).unwrap()
    }

    #[test]
    fn test_extract_function() {
        let src = "function greet(name: string): string { return name; }";
        let tree = parse_ts(src);
        let module = TypeScriptModule;
        let symbols = module.extract_symbols(&tree, src.as_bytes());

        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "greet");
        assert_eq!(symbols[0].kind, SymbolKind::Fn);
        assert!(!symbols[0].is_exported);
    }

    #[test]
    fn test_extract_exported_function() {
        let src = "export function greet(name: string): string { return name; }";
        let tree = parse_ts(src);
        let module = TypeScriptModule;
        let symbols = module.extract_symbols(&tree, src.as_bytes());

        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "greet");
        assert!(symbols[0].is_exported);
    }

    #[test]
    fn test_extract_class() {
        let src = "export class UserService {\n  getName() { return 'test'; }\n}";
        let tree = parse_ts(src);
        let module = TypeScriptModule;
        let symbols = module.extract_symbols(&tree, src.as_bytes());

        let class = symbols.iter().find(|s| s.name == "UserService").unwrap();
        assert_eq!(class.kind, SymbolKind::Class);
        assert!(class.is_exported);

        let method = symbols.iter().find(|s| s.name == "getName").unwrap();
        assert_eq!(method.kind, SymbolKind::Method);
    }

    #[test]
    fn test_extract_interface() {
        let src = "export interface Config {\n  host: string;\n  port: number;\n}";
        let tree = parse_ts(src);
        let module = TypeScriptModule;
        let symbols = module.extract_symbols(&tree, src.as_bytes());

        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "Config");
        assert_eq!(symbols[0].kind, SymbolKind::Interface);
        assert!(symbols[0].is_exported);
    }

    #[test]
    fn test_extract_arrow_function() {
        let src = "const add = (a: number, b: number) => a + b;";
        let tree = parse_ts(src);
        let module = TypeScriptModule;
        let symbols = module.extract_symbols(&tree, src.as_bytes());

        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "add");
        assert_eq!(symbols[0].kind, SymbolKind::Fn);
    }

    #[test]
    fn test_non_exported_private() {
        let src = "function helper() { return 1; }";
        let tree = parse_ts(src);
        let module = TypeScriptModule;
        let symbols = module.extract_symbols(&tree, src.as_bytes());

        assert!(!symbols[0].is_exported);
    }

    #[test]
    fn test_tsx_works() {
        let src = "export function App() { return <div />; }";
        let tree = parse_tsx(src);
        let module = TypeScriptModule;
        let symbols = module.extract_symbols(&tree, src.as_bytes());

        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "App");
        assert!(symbols[0].is_exported);
    }

    #[test]
    fn test_type_alias() {
        let src = "export type UserId = string;";
        let tree = parse_ts(src);
        let module = TypeScriptModule;
        let symbols = module.extract_symbols(&tree, src.as_bytes());

        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "UserId");
        assert_eq!(symbols[0].kind, SymbolKind::Type);
    }
}
