use std::fs;
use std::path::{Path, PathBuf};

use memchr::memmem;
use serde::Serialize;

use crate::index::{FileData, Index, Symbol, SymbolKind};
use crate::language;
use crate::output::{print_toon, print_json};
use crate::util::glob::glob_match;

// --- Serializable output types ---

#[derive(Serialize)]
struct SymbolRowOut {
    #[serde(skip_serializing_if = "Option::is_none")]
    file: Option<String>,
    name: String,
    kind: String,
    signature: String,
}

#[derive(Serialize)]
struct DefinitionResult {
    file: String,
    line: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    truncated: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lines: Option<usize>,
    body: String,
}

// --- Query implementations ---

struct SymbolRow<'a> {
    file: &'a Path,
    symbol: &'a Symbol,
}

/// Execute the symbols query with optional file, name glob, and kind filters.
/// When scoped to a single file, omits the file column from output.
pub fn symbols(
    index: &Index,
    file: Option<&Path>,
    name_glob: Option<&str>,
    kind_filter: Option<SymbolKind>,
    json: bool,
) -> i32 {
    let mut rows: Vec<SymbolRow<'_>> = Vec::new();

    let rel_path = file.map(|f| make_relative(f, &index.root));

    if let Some(ref rel) = rel_path
        && !index.entries.contains_key(rel) {
            eprintln!("cx: file not in index: {}", rel.display());
            return 1;
        }

    let files_to_search: Vec<(&PathBuf, &FileData)> = match rel_path {
        Some(ref rel) => {
            index.entries.get_key_value(rel).into_iter().collect()
        }
        None => index.entries.iter().collect(),
    };

    for (path, data) in files_to_search {
        for sym in &data.symbols {
            if let Some(pattern) = name_glob
                && !glob_match(pattern, &sym.name) {
                    continue;
                }

            if let Some(kind) = kind_filter
                && sym.kind != kind {
                    continue;
                }

            rows.push(SymbolRow {
                file: path,
                symbol: sym,
            });
        }
    }

    if rows.is_empty() {
        eprintln!("cx: no matches");
        return 2;
    }

    rows.sort_by(|a, b| a.file.cmp(b.file).then(a.symbol.name.cmp(&b.symbol.name)));

    let single_file = file.is_some();
    let out: Vec<SymbolRowOut> = rows
        .into_iter()
        .map(|r| SymbolRowOut {
            file: if single_file { None } else { Some(r.file.display().to_string()) },
            name: r.symbol.name.clone(),
            kind: r.symbol.kind.as_str().to_string(),
            signature: r.symbol.signature.clone(),
        })
        .collect();
    if json { print_json(&out) } else { print_toon(&out) }

    0
}

/// Execute the definition query: find symbol by exact name, return its body.
pub fn definition(
    index: &Index,
    name: &str,
    from: Option<&Path>,
    kind_filter: Option<SymbolKind>,
    max_lines: usize,
    json: bool,
) -> i32 {
    let from_rel = from.map(|f| make_relative(f, &index.root));

    let mut matches: Vec<(&PathBuf, &Symbol)> = Vec::new();
    for (path, data) in &index.entries {
        for sym in &data.symbols {
            if sym.name == name {
                if let Some(kind) = kind_filter
                    && sym.kind != kind {
                        continue;
                    }
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
        eprintln!("cx: no matches");
        return 2;
    }

    let results: Vec<DefinitionResult> = matches
        .iter()
        .map(|(path, sym)| {
            let (body, start_line) = read_body(&index.root, path, sym.byte_range)
                .unwrap_or((String::new(), 0));
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
                line: start_line,
                truncated: if truncated { Some(true) } else { None },
                lines: if truncated { Some(line_count) } else { None },
                body: display_body,
            }
        })
        .collect();

    if json {
        print_json(&results);
    } else {
        for (i, r) in results.iter().enumerate() {
            if i > 0 {
                println!();
            }
            print!("file: {}\nline: {}", r.file, r.line);
            if let Some(total) = r.lines {
                print!("\ntruncated: {} lines total", total);
            }
            println!("\n---\n{}", r.body);
        }
    }

    0
}

#[derive(Serialize)]
struct ReferenceRow {
    file: String,
    line: usize,
    kind: String,
    context: String,
}

/// Find all usages of a symbol name across project files.
pub fn references(
    index: &Index,
    name: &str,
    file: Option<&Path>,
    json: bool,
) -> i32 {
    let rel_path = file.map(|f| make_relative(f, &index.root));

    let files_to_search: Vec<(&PathBuf, &FileData)> = match rel_path {
        Some(ref rel) => {
            match index.entries.get_key_value(rel) {
                Some(kv) => vec![kv],
                None => {
                    eprintln!("cx: file not in index: {}", rel.display());
                    return 1;
                }
            }
        }
        None => index.entries.iter().collect(),
    };

    let mut rows: Vec<ReferenceRow> = Vec::new();
    let name_bytes = name.as_bytes();

    for (path, data) in files_to_search {
        let abs_path = index.root.join(path);
        let source = match fs::read(&abs_path) {
            Ok(s) => s,
            Err(_) => continue,
        };

        // Skip files that can't possibly contain the name
        if memmem::find(&source, name_bytes).is_none() {
            continue;
        }

        let refs = language::find_references(data.meta.language, &source, &abs_path, name);
        if refs.is_empty() {
            continue;
        }

        // Convert to str once per file for context extraction
        let text = std::str::from_utf8(&source).ok();
        let lines: Vec<&str> = text.map(|t| t.lines().collect()).unwrap_or_default();

        for r in refs {
            let context = lines
                .get(r.line.wrapping_sub(1))
                .map(|l| l.trim().to_string())
                .unwrap_or_default();
            rows.push(ReferenceRow {
                file: path.display().to_string(),
                line: r.line,
                kind: r.parent_kind,
                context,
            });
        }
    }

    if rows.is_empty() {
        eprintln!("cx: no matches");
        return 2;
    }

    rows.sort_by(|a, b| a.file.cmp(&b.file).then(a.line.cmp(&b.line)));
    rows.dedup_by(|a, b| a.file == b.file && a.line == b.line);

    if json { print_json(&rows) } else { print_toon(&rows) }

    0
}

fn read_body(root: &Path, file: &Path, byte_range: (usize, usize)) -> Option<(String, usize)> {
    let abs_path = root.join(file);
    let source = fs::read(&abs_path).ok()?;
    let (start, end) = byte_range;
    if end > source.len() {
        return None;
    }
    let line = source[..start].iter().filter(|&&b| b == b'\n').count() + 1;
    let body = String::from_utf8_lossy(&source[start..end]).to_string();
    Some((body, line))
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
