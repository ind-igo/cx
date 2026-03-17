use std::fs;
use std::path::{Path, PathBuf};

use crate::index::{Index, Symbol, SymbolKind};
use crate::output::{toon_table, toon_scalar};
use crate::util::glob::glob_match;

/// Result row for symbols query — includes file path context.
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

    // Resolve file filter path up front so it lives long enough
    let rel_path = file.map(|f| make_relative(f, &index.root));

    if let Some(ref rel) = rel_path {
        if !index.exports.contains_key(rel) {
            eprintln!("cx: file not in index: {}", rel.display());
            return 1;
        }
    }

    let files_to_search: Vec<(&PathBuf, &Vec<Symbol>)> = match rel_path {
        Some(ref rel) => {
            index.exports.get_key_value(rel).into_iter().collect()
        }
        None => index.exports.iter().collect(),
    };

    for (path, syms) in files_to_search {
        for sym in syms {
            // Apply name glob filter
            if let Some(pattern) = name_glob {
                if !glob_match(pattern, &sym.name) {
                    continue;
                }
            }

            // Apply kind filter
            if let Some(kind) = kind_filter {
                if sym.kind != kind {
                    continue;
                }
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

    // Sort by file then name for stable output
    rows.sort_by(|a, b| a.file.cmp(&b.file).then(a.symbol.name.cmp(&b.symbol.name)));

    if json {
        print_symbols_json(&rows, single_file);
    } else {
        print_symbols_toon(&rows, single_file);
    }

    0
}

fn print_symbols_toon(rows: &[SymbolRow], single_file: bool) {
    if single_file {
        let fields = &["name", "kind", "signature"];
        let table_rows: Vec<Vec<String>> = rows
            .iter()
            .map(|r| {
                vec![
                    r.symbol.name.clone(),
                    r.symbol.kind.as_str().to_string(),
                    r.symbol.signature.clone(),
                ]
            })
            .collect();
        print!("{}", toon_table("symbols", fields, &table_rows));
    } else {
        let fields = &["file", "name", "kind", "signature"];
        let table_rows: Vec<Vec<String>> = rows
            .iter()
            .map(|r| {
                vec![
                    r.file.display().to_string(),
                    r.symbol.name.clone(),
                    r.symbol.kind.as_str().to_string(),
                    r.symbol.signature.clone(),
                ]
            })
            .collect();
        print!("{}", toon_table("symbols", fields, &table_rows));
    }
}

fn print_symbols_json(rows: &[SymbolRow], single_file: bool) {
    let json_rows: Vec<serde_json::Value> = rows
        .iter()
        .map(|r| {
            let mut map = serde_json::Map::new();
            if !single_file {
                map.insert(
                    "file".into(),
                    serde_json::Value::String(r.file.display().to_string()),
                );
            }
            map.insert(
                "name".into(),
                serde_json::Value::String(r.symbol.name.clone()),
            );
            map.insert(
                "kind".into(),
                serde_json::Value::String(r.symbol.kind.as_str().to_string()),
            );
            map.insert(
                "signature".into(),
                serde_json::Value::String(r.symbol.signature.clone()),
            );
            serde_json::Value::Object(map)
        })
        .collect();

    println!(
        "{}",
        serde_json::to_string_pretty(&json_rows).unwrap_or_default()
    );
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

    // Collect all matching symbols
    let mut matches: Vec<(&PathBuf, &Symbol)> = Vec::new();
    for (path, syms) in &index.exports {
        for sym in syms {
            if sym.name == name {
                matches.push((path, sym));
            }
        }
    }

    // If --from given, prefer symbols from that file
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

    if json {
        print_definitions_json(&matches, &index.root, max_lines);
    } else {
        print_definitions_toon(&matches, &index.root, max_lines);
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

fn print_definitions_toon(matches: &[(&PathBuf, &Symbol)], root: &Path, max_lines: usize) {
    for (path, sym) in matches {
        let body = read_body(root, path, sym.byte_range).unwrap_or_default();
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

        let range_str = format!("[{},{}]", sym.byte_range.0, sym.byte_range.1);

        let mut fields: Vec<(&str, String)> = vec![
            ("file", path.display().to_string()),
            ("signature", sym.signature.clone()),
            ("range", range_str),
        ];

        if truncated {
            fields.push(("truncated", "true".to_string()));
            fields.push(("lines", line_count.to_string()));
        }

        fields.push(("body", display_body));

        let field_refs: Vec<(&str, &str)> = fields.iter().map(|(k, v)| (*k, &**v)).collect();
        print!("{}", toon_scalar(&field_refs));

        // Separator between multiple results
        if matches.len() > 1 {
            println!("---");
        }
    }
}

fn print_definitions_json(matches: &[(&PathBuf, &Symbol)], root: &Path, max_lines: usize) {
    let json_results: Vec<serde_json::Value> = matches
        .iter()
        .map(|(path, sym)| {
            let body = read_body(root, path, sym.byte_range).unwrap_or_default();
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

            let mut map = serde_json::Map::new();
            map.insert("file".into(), serde_json::Value::String(path.display().to_string()));
            map.insert("signature".into(), serde_json::Value::String(sym.signature.clone()));
            map.insert("range".into(), serde_json::json!([sym.byte_range.0, sym.byte_range.1]));
            if truncated {
                map.insert("truncated".into(), serde_json::Value::Bool(true));
                map.insert("lines".into(), serde_json::json!(line_count));
            }
            map.insert("body".into(), serde_json::Value::String(display_body));
            serde_json::Value::Object(map)
        })
        .collect();

    if json_results.len() == 1 {
        println!("{}", serde_json::to_string_pretty(&json_results[0]).unwrap_or_default());
    } else {
        println!("{}", serde_json::to_string_pretty(&json_results).unwrap_or_default());
    }
}

/// Make a path relative to the project root if it's absolute,
/// or resolve it from cwd if relative.
fn make_relative(path: &Path, root: &Path) -> PathBuf {
    if path.is_absolute() {
        path.strip_prefix(root).unwrap_or(path).to_path_buf()
    } else {
        // Path is relative to cwd — resolve against root
        let cwd = std::env::current_dir().unwrap_or_default();
        let abs = cwd.join(path);
        abs.strip_prefix(root).unwrap_or(path).to_path_buf()
    }
}
