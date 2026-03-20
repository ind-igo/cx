use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::index::{Index, Symbol, SymbolKind};
use crate::output::{print_toon, print_json};
use crate::util::glob::glob_match;

// --- Serializable output types ---

#[derive(Serialize)]
struct SymbolRowSingle {
    name: String,
    kind: String,
    signature: String,
}

#[derive(Serialize)]
struct SymbolRowFull {
    file: String,
    name: String,
    kind: String,
    signature: String,
}

#[derive(Serialize)]
struct DefinitionResult {
    file: String,
    signature: String,
    range: (usize, usize),
    #[serde(skip_serializing_if = "Option::is_none")]
    truncated: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lines: Option<usize>,
    body: String,
}

// --- Query implementations ---

struct SymbolRow {
    file: PathBuf,
    symbol: Symbol,
}

/// Execute the symbols query with optional file, name glob, and kind filters.
/// If `single_file` is true, omits the file column from output (used by overview).
pub fn symbols(
    index: &Index,
    file: Option<&Path>,
    name_glob: Option<&str>,
    kind_filter: Option<SymbolKind>,
    single_file: bool,
    json: bool,
) -> i32 {
    let mut rows: Vec<SymbolRow> = Vec::new();

    let rel_path = file.map(|f| make_relative(f, &index.root));

    if let Some(ref rel) = rel_path
        && !index.exports.contains_key(rel) {
            eprintln!("cx: file not in index: {}", rel.display());
            return 1;
        }

    let files_to_search: Vec<(&PathBuf, &Vec<Symbol>)> = match rel_path {
        Some(ref rel) => {
            index.exports.get_key_value(rel).into_iter().collect()
        }
        None => index.exports.iter().collect(),
    };

    for (path, syms) in files_to_search {
        for sym in syms {
            if let Some(pattern) = name_glob
                && !glob_match(pattern, &sym.name) {
                    continue;
                }

            if let Some(kind) = kind_filter
                && sym.kind != kind {
                    continue;
                }

            rows.push(SymbolRow {
                file: path.clone(),
                symbol: sym.clone(),
            });
        }
    }

    if rows.is_empty() {
        return 2;
    }

    rows.sort_by(|a, b| a.file.cmp(&b.file).then(a.symbol.name.cmp(&b.symbol.name)));

    if single_file {
        let out: Vec<SymbolRowSingle> = rows
            .iter()
            .map(|r| SymbolRowSingle {
                name: r.symbol.name.clone(),
                kind: r.symbol.kind.as_str().to_string(),
                signature: r.symbol.signature.clone(),
            })
            .collect();
        if json { print_json(&out) } else { print_toon(&out) }
    } else {
        let out: Vec<SymbolRowFull> = rows
            .iter()
            .map(|r| SymbolRowFull {
                file: r.file.display().to_string(),
                name: r.symbol.name.clone(),
                kind: r.symbol.kind.as_str().to_string(),
                signature: r.symbol.signature.clone(),
            })
            .collect();
        if json { print_json(&out) } else { print_toon(&out) }
    }

    0
}

/// Execute the definition query: find symbol by exact name, return its body.
pub fn definition(
    index: &Index,
    name: &str,
    from: Option<&Path>,
    max_lines: usize,
    json: bool,
) -> i32 {
    let from_rel = from.map(|f| make_relative(f, &index.root));

    let mut matches: Vec<(&PathBuf, &Symbol)> = Vec::new();
    for (path, syms) in &index.exports {
        for sym in syms {
            if sym.name == name {
                matches.push((path, sym));
            }
        }
    }

    if let Some(ref from_path) = from_rel {
        let from_matches: Vec<_> = matches
            .iter()
            .filter(|(path, _)| *path == from_path)
            .cloned()
            .collect();
        if !from_matches.is_empty() {
            matches = from_matches;
        }
    }

    if matches.is_empty() {
        return 2;
    }

    let results: Vec<DefinitionResult> = matches
        .iter()
        .map(|(path, sym)| {
            let body = read_body(&index.root, path, sym.byte_range).unwrap_or_default();
            let line_count = body.lines().count();
            let truncated = line_count > max_lines;

            let display_body = if truncated {
                body.lines()
                    .take(max_lines)
                    .collect::<Vec<_>>()
                    .join("\n")
            } else {
                body
            };

            DefinitionResult {
                file: path.display().to_string(),
                signature: sym.signature.clone(),
                range: sym.byte_range,
                truncated: if truncated { Some(true) } else { None },
                lines: if truncated { Some(line_count) } else { None },
                body: display_body,
            }
        })
        .collect();

    if json {
        print_json(&results);
    } else {
        print_toon(&results);
    }

    0
}

fn read_body(root: &Path, file: &Path, byte_range: (usize, usize)) -> Option<String> {
    let abs_path = root.join(file);
    let source = fs::read(&abs_path).ok()?;
    let (start, end) = byte_range;
    if end > source.len() {
        return None;
    }
    Some(String::from_utf8_lossy(&source[start..end]).to_string())
}

/// Make a path relative to the project root if it's absolute,
/// or resolve it from cwd if relative.
fn make_relative(path: &Path, root: &Path) -> PathBuf {
    if path.is_absolute() {
        path.strip_prefix(root).unwrap_or(path).to_path_buf()
    } else {
        let cwd = std::env::current_dir().unwrap_or_default();
        let abs = cwd.join(path);
        abs.strip_prefix(root).unwrap_or(path).to_path_buf()
    }
}
