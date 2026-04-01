use std::fs;
use std::path::{Path, PathBuf};

use memchr::memmem;
use serde::Serialize;

use crate::index::{FileData, Index, Symbol, SymbolKind};
use crate::language::{self, detect_language};
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
            let abs = index.root.join(rel);
            if abs.exists() && detect_language(&abs).is_none() {
                let ext = abs.extension().and_then(|e| e.to_str()).unwrap_or("(none)");
                eprintln!("cx: unsupported file type: .{}", ext);
            } else {
                eprintln!("cx: file not in index: {}", display_path(rel));
            }
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
        return 0;
    }

    rows.sort_by(|a, b| a.file.cmp(b.file).then(a.symbol.name.cmp(&b.symbol.name)));

    let single_file = file.is_some();
    let out: Vec<SymbolRowOut> = rows
        .into_iter()
        .map(|r| SymbolRowOut {
            file: if single_file { None } else { Some(display_path(r.file)) },
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
        return 0;
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
                file: display_path(path),
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
    #[serde(skip_serializing_if = "Option::is_none")]
    caller: Option<String>,
    context: String,
}

/// Find the enclosing symbol for a byte offset in a file's symbol list.
fn find_enclosing_symbol(symbols: &[Symbol], byte_offset: usize) -> Option<&str> {
    symbols
        .iter()
        .filter(|s| s.byte_range.0 <= byte_offset && byte_offset < s.byte_range.1)
        // Pick the tightest enclosing symbol (smallest range)
        .min_by_key(|s| s.byte_range.1 - s.byte_range.0)
        .map(|s| s.name.as_str())
}

/// Unique caller row for --unique mode.
#[derive(Serialize)]
struct UniqueCallerRow {
    file: String,
    caller: String,
    line: usize,
}

/// Find all usages of a symbol name across project files.
pub fn references(
    index: &Index,
    name: &str,
    file: Option<&Path>,
    unique: bool,
    json: bool,
) -> i32 {
    let rel_path = file.map(|f| make_relative(f, &index.root));

    let files_to_search: Vec<(&PathBuf, &FileData)> = match rel_path {
        Some(ref rel) => {
            match index.entries.get_key_value(rel) {
                Some(kv) => vec![kv],
                None => {
                    let abs = index.root.join(rel);
                    if abs.exists() && detect_language(&abs).is_none() {
                        let ext = abs.extension().and_then(|e| e.to_str()).unwrap_or("(none)");
                        eprintln!("cx: unsupported file type: .{}", ext);
                    } else {
                        eprintln!("cx: file not in index: {}", display_path(rel));
                    }
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

        let refs = match language::find_references(&data.meta.language, &source, &abs_path, name) {
            Ok(r) => r,
            Err(language::LangError::NotInstalled(lang)) => {
                eprintln!("cx: {} grammar not installed — run: cx lang add {}", lang, lang);
                return 1;
            }
            Err(_) => continue,
        };
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
            let caller = find_enclosing_symbol(&data.symbols, r.byte_offset)
                .map(|s| s.to_string());
            rows.push(ReferenceRow {
                file: display_path(path),
                line: r.line,
                caller,
                context,
            });
        }
    }

    if rows.is_empty() {
        eprintln!("cx: no matches");
        return 0;
    }

    rows.sort_by(|a, b| a.file.cmp(&b.file).then(a.line.cmp(&b.line)));
    rows.dedup_by(|a, b| a.file == b.file && a.line == b.line);

    if unique {
        // Deduplicate to one row per (file, caller) pair
        let mut seen = std::collections::HashSet::new();
        let unique_rows: Vec<UniqueCallerRow> = rows
            .into_iter()
            .filter_map(|r| {
                let caller = r.caller?;
                if seen.insert((r.file.clone(), caller.clone())) {
                    Some(UniqueCallerRow { file: r.file, caller, line: r.line })
                } else {
                    None
                }
            })
            .collect();
        if unique_rows.is_empty() {
            eprintln!("cx: no callers found");
            return 0;
        }
        if json { print_json(&unique_rows) } else { print_toon(&unique_rows) }
    } else {
        if json { print_json(&rows) } else { print_toon(&rows) }
    }

    0
}

// --- Directory overview ---

const DIR_OVERVIEW_MAX_SYMBOLS: usize = 10;

#[derive(Serialize)]
struct DirOverviewRow {
    file: String,
    symbols: String,
}

#[derive(Serialize)]
struct DirOverviewFullRow {
    file: String,
    name: String,
    kind: String,
    signature: String,
}

/// Priority for symbol kinds in directory overview: lower = shown first.
fn symbol_priority(kind: SymbolKind) -> u8 {
    match kind {
        SymbolKind::Struct | SymbolKind::Enum | SymbolKind::Trait
        | SymbolKind::Interface | SymbolKind::Class => 0,
        SymbolKind::Fn | SymbolKind::Const | SymbolKind::Type
        | SymbolKind::Module | SymbolKind::Event => 1,
        SymbolKind::Method => 2,
    }
}

/// Check if a file path looks like a test file based on naming conventions.
fn is_test_file(path: &Path) -> bool {
    for component in path.components() {
        if let std::path::Component::Normal(s) = component {
            let s = s.to_str().unwrap_or("");
            if s == "tests" || s == "test" || s == "__tests__" {
                return true;
            }
        }
    }
    let name = match path.file_name().and_then(|n| n.to_str()) {
        Some(n) => n,
        None => return false,
    };
    // Go: *_test.go
    if name.ends_with("_test.go") { return true; }
    // JS/TS: *.test.* or *.spec.*
    for ext in &[".test.ts", ".test.tsx", ".test.js", ".test.jsx",
                 ".spec.ts", ".spec.tsx", ".spec.js", ".spec.jsx"] {
        if name.ends_with(ext) { return true; }
    }
    // Python: test_*.py
    if name.starts_with("test_") && name.ends_with(".py") { return true; }
    // Ruby: *_spec.rb
    if name.ends_with("_spec.rb") { return true; }
    false
}

/// Extract the immediate child component of `path` relative to `dir`.
/// Returns `None` if the path is not under `dir`.
/// For a direct child file, returns the full relative path.
/// For a nested file, returns just the first subdirectory component (with trailing /).
fn child_component(path: &Path, dir: &Path) -> Option<PathBuf> {
    let relative = if dir.as_os_str().is_empty() {
        path.to_path_buf()
    } else {
        path.strip_prefix(dir).ok()?.to_path_buf()
    };
    let mut components = relative.components();
    let first = components.next()?;
    if components.next().is_some() {
        // Nested — return just the subdir name
        Some(PathBuf::from(first.as_os_str()))
    } else {
        // Direct child file
        Some(relative)
    }
}

/// Show a single-level overview of files and subdirectories.
pub fn dir_overview(
    index: &Index,
    dir: &Path,
    full: bool,
    json: bool,
) -> i32 {
    let rel_dir = make_relative(dir, &index.root);
    // Normalize "." to empty path so starts_with matches all entries
    let rel_dir = if rel_dir == Path::new(".") { PathBuf::new() } else { rel_dir };

    let all_entries: Vec<(&PathBuf, &FileData)> = index
        .entries
        .iter()
        .filter(|(path, _)| rel_dir.as_os_str().is_empty() || path.starts_with(&rel_dir))
        .filter(|(path, _)| !is_test_file(path))
        .collect();

    if all_entries.is_empty() {
        eprintln!("cx: no indexed files under {}", display_path(&rel_dir));
        return 1;
    }

    // Partition into direct files and subdirectory aggregates
    let mut direct_files: Vec<(&PathBuf, &FileData)> = Vec::new();
    let mut subdirs: std::collections::BTreeMap<String, (usize, usize)> = std::collections::BTreeMap::new();

    for (path, data) in &all_entries {
        let child = match child_component(path, &rel_dir) {
            Some(c) => c,
            None => continue,
        };
        let non_test_count = data.symbols.iter().filter(|s| !s.is_test).count();
        if child.components().count() == 1 && child.extension().is_some() {
            // Direct child file
            direct_files.push((path, data));
        } else {
            // Subdirectory — aggregate
            let dir_name = child.to_string_lossy().to_string();
            let entry = subdirs.entry(dir_name).or_insert((0, 0));
            entry.0 += 1;
            entry.1 += non_test_count;
        }
    }

    direct_files.sort_by_key(|(path, _)| *path);

    // Shared: format subdir display path
    let format_subdir = |dir_name: &str| -> String {
        if rel_dir.as_os_str().is_empty() {
            format!("{}/", dir_name)
        } else {
            format!("{}/{}/", display_path(&rel_dir), dir_name)
        }
    };

    fn prepare_symbols(data: &FileData) -> Vec<&Symbol> {
        let mut syms: Vec<&Symbol> = data.symbols.iter()
            .filter(|s| !s.is_test)
            .collect();
        syms.sort_by(|a, b| symbol_priority(a.kind).cmp(&symbol_priority(b.kind))
            .then(a.name.cmp(&b.name)));
        syms
    }

    if full {
        let mut rows: Vec<DirOverviewFullRow> = Vec::new();
        for (dir_name, (file_count, sym_count)) in &subdirs {
            rows.push(DirOverviewFullRow {
                file: format_subdir(dir_name),
                name: format!("({} files, {} symbols)", file_count, sym_count),
                kind: String::new(),
                signature: String::new(),
            });
        }
        for (path, data) in &direct_files {
            let syms = prepare_symbols(data);
            if syms.is_empty() { continue; }
            let total = syms.len();
            for sym in syms.iter().take(DIR_OVERVIEW_MAX_SYMBOLS) {
                rows.push(DirOverviewFullRow {
                    file: display_path(path),
                    name: sym.name.clone(),
                    kind: sym.kind.as_str().to_string(),
                    signature: sym.signature.clone(),
                });
            }
            if total > DIR_OVERVIEW_MAX_SYMBOLS {
                rows.push(DirOverviewFullRow {
                    file: display_path(path),
                    name: format!("... (+{} more)", total - DIR_OVERVIEW_MAX_SYMBOLS),
                    kind: String::new(),
                    signature: String::new(),
                });
            }
        }
        if json { print_json(&rows) } else { print_toon(&rows) }
    } else {
        let mut rows: Vec<DirOverviewRow> = Vec::new();
        for (dir_name, (file_count, sym_count)) in &subdirs {
            rows.push(DirOverviewRow {
                file: format_subdir(dir_name),
                symbols: format!("({} files, {} symbols)", file_count, sym_count),
            });
        }
        for (path, data) in &direct_files {
            let syms = prepare_symbols(data);
            if syms.is_empty() { continue; }
            let total = syms.len();
            // Deduplicate names (e.g. overloaded type params)
            let mut seen = std::collections::HashSet::new();
            let names: Vec<&str> = syms.iter()
                .take(DIR_OVERVIEW_MAX_SYMBOLS)
                .map(|s| s.name.as_str())
                .filter(|n| seen.insert(*n))
                .collect();
            let shown = names.len();
            let suffix = if total > shown {
                format!(", ... (+{} more)", total - shown)
            } else {
                String::new()
            };
            rows.push(DirOverviewRow {
                file: display_path(path),
                symbols: format!("{}{}", names.join(", "), suffix),
            });
        }
        if json { print_json(&rows) } else { print_toon(&rows) }
    }

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

/// Display a path using forward slashes (consistent across platforms).
fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_path_normalizes_backslashes() {
        assert_eq!(display_path(Path::new("src/main.rs")), "src/main.rs");
        assert_eq!(display_path(Path::new("src\\main.rs")), "src/main.rs");
        assert_eq!(display_path(Path::new("src\\sub\\file.rs")), "src/sub/file.rs");
    }

    // --- is_test_file tests ---

    #[test]
    fn test_file_go() {
        assert!(is_test_file(Path::new("pkg/handler_test.go")));
        assert!(!is_test_file(Path::new("pkg/handler.go")));
    }

    #[test]
    fn test_file_ts_js() {
        assert!(is_test_file(Path::new("src/app.test.ts")));
        assert!(is_test_file(Path::new("src/app.test.tsx")));
        assert!(is_test_file(Path::new("src/app.spec.js")));
        assert!(is_test_file(Path::new("src/app.spec.jsx")));
        assert!(!is_test_file(Path::new("src/app.ts")));
    }

    #[test]
    fn test_file_python() {
        assert!(is_test_file(Path::new("test_utils.py")));
        assert!(!is_test_file(Path::new("utils_test.py"))); // Python convention is test_ prefix
        assert!(!is_test_file(Path::new("test_utils.rs"))); // wrong extension
    }

    #[test]
    fn test_file_ruby() {
        assert!(is_test_file(Path::new("models/user_spec.rb")));
        assert!(!is_test_file(Path::new("models/user.rb")));
    }

    #[test]
    fn test_file_directory() {
        assert!(is_test_file(Path::new("tests/unit/foo.rs")));
        assert!(is_test_file(Path::new("test/foo.js")));
        assert!(is_test_file(Path::new("src/__tests__/app.tsx")));
        assert!(!is_test_file(Path::new("src/foo.rs")));
    }

    #[test]
    fn test_file_normal_files() {
        assert!(!is_test_file(Path::new("src/main.rs")));
        assert!(!is_test_file(Path::new("lib/utils.ts")));
        assert!(!is_test_file(Path::new("index.js")));
    }

    // --- symbol_priority tests ---

    #[test]
    fn symbol_priority_ordering() {
        // Types should come before functions, which come before methods
        assert!(symbol_priority(SymbolKind::Struct) < symbol_priority(SymbolKind::Fn));
        assert!(symbol_priority(SymbolKind::Enum) < symbol_priority(SymbolKind::Fn));
        assert!(symbol_priority(SymbolKind::Trait) < symbol_priority(SymbolKind::Fn));
        assert!(symbol_priority(SymbolKind::Interface) < symbol_priority(SymbolKind::Fn));
        assert!(symbol_priority(SymbolKind::Class) < symbol_priority(SymbolKind::Fn));
        assert!(symbol_priority(SymbolKind::Fn) < symbol_priority(SymbolKind::Method));
    }
}
