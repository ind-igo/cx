use crate::index::{Symbol, SymbolKind};
use std::collections::HashMap;
use tree_sitter::{Node, Query, QueryCursor, StreamingIterator};

use super::LanguageConfig;

pub struct Reference {
    pub line: usize, // 1-based
    pub parent_kind: String,
}

// --- Test detection ---

/// Check if a symbol node represents a test (Rust `#[test]`/`#[cfg(test)]`, Zig `test` decl).
pub(super) fn detect_test_symbol(lang: &str, node: Node, source: &[u8]) -> bool {
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
            if let Ok(text) = sib.utf8_text(source) && is_test_attribute(text) {
                return true;
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
                    if let Ok(text) = s.utf8_text(source) && text.contains("cfg(test)") {
                        return true;
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

pub(super) fn extract_symbols(
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
