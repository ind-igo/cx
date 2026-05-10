use std::fs;
use std::path::{Path, PathBuf};

use memchr::memmem;
use serde::Serialize;

use crate::index::{FileData, Index, Symbol, SymbolKind};
use crate::language::{self, detect_language};
use crate::output::{print_toon, print_json};
use crate::util::glob::glob_match;

// --- Pagination ---

/// Pagination parameters resolved from CLI flags.
pub struct Pagination {
    /// Max results to return (None = unlimited).
    pub limit: Option<usize>,
    /// Number of results to skip.
    pub offset: usize,
}

/// Result of applying pagination to a result set.
struct Paginated<T> {
    /// The visible slice after offset + limit.
    items: Vec<T>,
    /// Total number of results before pagination.
    total: usize,
    /// The offset that was applied.
    offset: usize,
    /// The limit that was applied (None = unlimited).
    limit: Option<usize>,
}

impl<T> Paginated<T> {
    /// True when results were cut off (more items exist after this page).
    const fn was_truncated(&self) -> bool {
        self.offset + self.items.len() < self.total
    }

    /// True when JSON output should use the paginated envelope
    /// (either truncated or mid-pagination via offset).
    const fn needs_envelope(&self) -> bool {
        self.was_truncated() || self.offset > 0
    }
}

fn paginate<T>(items: Vec<T>, pg: &Pagination) -> Paginated<T> {
    let total = items.len();
    let visible = items.into_iter()
        .skip(pg.offset)
        .take(pg.limit.unwrap_or(usize::MAX))
        .collect();
    Paginated { items: visible, total, offset: pg.offset, limit: pg.limit }
}

/// Wraps results with pagination metadata for JSON output.
#[derive(Serialize)]
struct PaginatedJson<'a, T: Serialize> {
    total: usize,
    offset: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    limit: Option<usize>,
    results: &'a [T],
}

/// Emit a compact pagination hint on stderr.
fn emit_pagination_hint(total: usize, offset: usize, shown: usize, subject: &str, narrow_hint: &str) {
    let next_offset = offset + shown;
    eprintln!(
        "cx: {shown}/{total} {subject} | {narrow_hint} to narrow | --offset {next_offset} for more | --all"
    );
}

fn print_paginated_json<T: Serialize>(pg: &Paginated<T>) {
    let wrapper = PaginatedJson {
        total: pg.total,
        offset: pg.offset,
        limit: pg.limit,
        results: &pg.items,
    };
    print_json(&wrapper);
}

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
struct SymbolRowWithRangeOut {
    #[serde(skip_serializing_if = "Option::is_none")]
    file: Option<String>,
    name: String,
    kind: String,
    range: String,
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
    ranges: bool,
    json: bool,
    pg: &Pagination,
) -> i32 {
    let mut rows: Vec<SymbolRow<'_>> = Vec::new();

    let rel_path = file.map(|f| make_relative(f, &index.root));

    let files_to_search: Vec<(&PathBuf, &FileData)> = match rel_path {
        Some(ref rel) => match resolve_file_filter(rel, index) {
            Ok(v) => v,
            Err(code) => return code,
        },
        None => index.entries.iter().collect(),
    };
    let is_single_file = file.is_some() && files_to_search.len() == 1;

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

    let single_file = is_single_file;
    let mut line_cache = std::collections::HashMap::new();
    if ranges {
        let out: Vec<SymbolRowWithRangeOut> = rows
            .into_iter()
            .map(|r| SymbolRowWithRangeOut {
                file: if single_file { None } else { Some(display_path(r.file)) },
                name: r.symbol.name.clone(),
                kind: r.symbol.kind.as_str().to_string(),
                range: line_range(index, &mut line_cache, r.file, r.symbol.byte_range)
                    .unwrap_or_default(),
                signature: r.symbol.signature.clone(),
            })
            .collect();
        let paged = paginate(out, pg);
        if json {
            if paged.needs_envelope() {
                print_paginated_json(&paged);
            } else {
                print_json(&paged.items);
            }
        } else {
            print_toon(&paged.items);
        }
        if paged.was_truncated() {
            emit_pagination_hint(paged.total, paged.offset, paged.items.len(), "symbols", "--file PATH | --kind KIND");
        }
    } else {
        let out: Vec<SymbolRowOut> = rows
            .into_iter()
            .map(|r| SymbolRowOut {
                file: if single_file { None } else { Some(display_path(r.file)) },
                name: r.symbol.name.clone(),
                kind: r.symbol.kind.as_str().to_string(),
                signature: r.symbol.signature.clone(),
            })
            .collect();
        let paged = paginate(out, pg);
        if json {
            if paged.needs_envelope() {
                print_paginated_json(&paged);
            } else {
                print_json(&paged.items);
            }
        } else {
            print_toon(&paged.items);
        }
        if paged.was_truncated() {
            emit_pagination_hint(paged.total, paged.offset, paged.items.len(), "symbols", "--file PATH | --kind KIND");
        }
    }

    0
}

/// Serializable row for `kind_counts` output.
#[derive(Serialize)]
struct KindCountRow {
    kind: String,
    count: usize,
}

/// List distinct symbol kinds with their counts, optionally scoped to a file.
pub fn kind_counts(
    index: &Index,
    file: Option<&Path>,
    json: bool,
) -> i32 {
    let rel_path = file.map(|f| make_relative(f, &index.root));

    let files_to_search: Vec<(&PathBuf, &FileData)> = match rel_path {
        Some(ref rel) => match resolve_file_filter(rel, index) {
            Ok(v) => v,
            Err(code) => return code,
        },
        None => index.entries.iter().collect(),
    };

    let mut counts: std::collections::BTreeMap<&'static str, usize> = std::collections::BTreeMap::new();
    for (_path, data) in files_to_search {
        for sym in &data.symbols {
            *counts.entry(sym.kind.as_str()).or_insert(0) += 1;
        }
    }

    if counts.is_empty() {
        eprintln!("cx: no symbols in index");
        return 0;
    }

    let mut rows: Vec<KindCountRow> = counts
        .into_iter()
        .map(|(kind, count)| KindCountRow { kind: kind.to_string(), count })
        .collect();
    rows.sort_by_key(|r| std::cmp::Reverse(r.count));

    if json {
        print_json(&rows);
    } else {
        print_toon(&rows);
    }

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
    pg: &Pagination,
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
        let is_dir = index.root.join(from_path).is_dir();
        let from_matches: Vec<_> = matches
            .iter()
            .filter(|(path, _)| {
                if is_dir { path.starts_with(from_path) } else { *path == from_path }
            })
            .copied()
            .collect();
        if !from_matches.is_empty() {
            matches = from_matches;
        }
    }

    if matches.is_empty() {
        eprintln!("cx: no matches");
        return 0;
    }

    // Sort by symbol priority (types first) then by file path
    matches.sort_by(|a, b| {
        symbol_priority(a.1.kind).cmp(&symbol_priority(b.1.kind))
            .then(a.0.cmp(b.0))
    });

    // Paginate matches BEFORE reading bodies to avoid pointless disk I/O
    let paged_matches = paginate(matches, pg);

    let results: Vec<DefinitionResult> = paged_matches.items
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
        if paged_matches.needs_envelope() {
            let wrapper = PaginatedJson {
                total: paged_matches.total,
                offset: paged_matches.offset,
                limit: paged_matches.limit,
                results: &results,
            };
            print_json(&wrapper);
        } else {
            print_json(&results);
        }
    } else {
        for (i, r) in results.iter().enumerate() {
            if i > 0 {
                println!();
            }
            print!("file: {}\nline: {}", r.file, r.line);
            if let Some(total) = r.lines {
                print!("\ntruncated: {total} lines total");
            }
            println!("\n---\n{}", r.body);
        }
    }

    if paged_matches.was_truncated() {
        let subject = format!("definitions for \"{name}\"");
        emit_pagination_hint(paged_matches.total, paged_matches.offset, results.len(), &subject, "--from PATH");
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

/// File-level reference summary for default references output.
#[derive(Serialize)]
struct ReferenceSummaryRow {
    file: String,
    lines: String,
    refs: usize,
    callers: String,
}

/// Find all usages of a symbol name across project files.
pub fn references(
    index: &Index,
    name: &str,
    file: Option<&Path>,
    context: bool,
    json: bool,
    pg: &Pagination,
) -> i32 {
    let rel_path = file.map(|f| make_relative(f, &index.root));

    let files_to_search: Vec<(&PathBuf, &FileData)> = match rel_path {
        Some(ref rel) => match resolve_file_filter(rel, index) {
            Ok(v) => v,
            Err(code) => return code,
        },
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
                eprintln!("cx: {lang} grammar not installed — run: cx lang add {lang}");
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
                .map(std::string::ToString::to_string);
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

    let narrow_hint = "--file PATH";

    if !context {
        let mut by_file: std::collections::BTreeMap<String, (usize, std::collections::BTreeSet<String>, Vec<usize>)> =
            std::collections::BTreeMap::new();
        for row in rows {
            let entry = by_file
                .entry(row.file)
                .or_insert_with(|| (0, std::collections::BTreeSet::new(), Vec::new()));
            entry.0 += 1;
            if let Some(caller) = row.caller {
                entry.1.insert(caller);
            }
            entry.2.push(row.line);
        }
        let summary_rows: Vec<ReferenceSummaryRow> = by_file
            .into_iter()
            .map(|(file, (refs, callers, lines))| ReferenceSummaryRow {
                file,
                lines: lines
                    .into_iter()
                    .map(|line| line.to_string())
                    .collect::<Vec<_>>()
                    .join(", "),
                refs,
                callers: callers.into_iter().collect::<Vec<_>>().join(", "),
            })
            .collect();
        let paged = paginate(summary_rows, pg);
        if json {
            if paged.needs_envelope() { print_paginated_json(&paged); } else { print_json(&paged.items); }
        } else {
            print_toon(&paged.items);
        }
        if paged.was_truncated() {
            let subject = format!("references for \"{name}\"");
            emit_pagination_hint(paged.total, paged.offset, paged.items.len(), &subject, narrow_hint);
        }
    } else {
        let paged = paginate(rows, pg);
        if json {
            if paged.needs_envelope() { print_paginated_json(&paged); } else { print_json(&paged.items); }
        } else {
            print_toon(&paged.items);
        }
        if paged.was_truncated() {
            let subject = format!("references for \"{name}\"");
            emit_pagination_hint(paged.total, paged.offset, paged.items.len(), &subject, narrow_hint);
        }
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
    range: String,
    signature: String,
}

/// Priority for symbol kinds in directory overview: lower = shown first.
const fn symbol_priority(kind: SymbolKind) -> u8 {
    match kind {
        SymbolKind::Struct | SymbolKind::Enum | SymbolKind::Trait
        | SymbolKind::Interface | SymbolKind::Class => 0,
        SymbolKind::Fn | SymbolKind::Const | SymbolKind::Type
        | SymbolKind::Module | SymbolKind::Event | SymbolKind::Heading => 1,
        SymbolKind::Field => 2,
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
    no_tests: bool,
    json: bool,
    pg: &Pagination,
) -> i32 {
    let rel_dir = make_relative(dir, &index.root);
    let rel_dir = if rel_dir == Path::new(".") { PathBuf::new() } else { rel_dir };

    let all_entries: Vec<(&PathBuf, &FileData)> = index
        .entries
        .iter()
        .filter(|(path, _)| rel_dir.as_os_str().is_empty() || path.starts_with(&rel_dir))
        .filter(|(path, _)| !no_tests || !is_test_file(path))
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
        let sym_count = if no_tests {
            data.symbols.iter().filter(|s| !s.is_test).count()
        } else {
            data.symbols.len()
        };
        if child.components().count() == 1 && child.extension().is_some() {
            direct_files.push((path, data));
        } else {
            let dir_name = child.to_string_lossy().to_string();
            let entry = subdirs.entry(dir_name).or_insert((0, 0));
            entry.0 += 1;
            entry.1 += sym_count;
        }
    }

    direct_files.sort_by_key(|(path, _)| *path);

    // Shared: format subdir display path
    let format_subdir = |dir_name: &str| -> String {
        if rel_dir.as_os_str().is_empty() {
            format!("{dir_name}/")
        } else {
            format!("{}/{}/", display_path(&rel_dir), dir_name)
        }
    };

    fn prepare_symbols(data: &FileData, no_tests: bool) -> Vec<&Symbol> {
        let mut syms: Vec<&Symbol> = data.symbols.iter()
            .filter(|s| !no_tests || !s.is_test)
            .collect();
        syms.sort_by(|a, b| symbol_priority(a.kind).cmp(&symbol_priority(b.kind))
            .then(a.name.cmp(&b.name)));
        syms
    }

    if full {
        let mut rows: Vec<DirOverviewFullRow> = Vec::new();
        let mut line_cache = std::collections::HashMap::new();
        for (dir_name, (file_count, sym_count)) in &subdirs {
            rows.push(DirOverviewFullRow {
                file: format_subdir(dir_name),
                name: format!("({file_count} files, {sym_count} symbols)"),
                kind: String::new(),
                range: String::new(),
                signature: String::new(),
            });
        }
        for (path, data) in &direct_files {
            let syms = prepare_symbols(data, no_tests);
            if syms.is_empty() { continue; }
            let total = syms.len();
            for sym in syms.iter().take(DIR_OVERVIEW_MAX_SYMBOLS) {
                rows.push(DirOverviewFullRow {
                    file: display_path(path),
                    name: sym.name.clone(),
                    kind: sym.kind.as_str().to_string(),
                    range: line_range(index, &mut line_cache, path, sym.byte_range).unwrap_or_default(),
                    signature: sym.signature.clone(),
                });
            }
            if total > DIR_OVERVIEW_MAX_SYMBOLS {
                rows.push(DirOverviewFullRow {
                    file: display_path(path),
                    name: format!("... (+{} more)", total - DIR_OVERVIEW_MAX_SYMBOLS),
                    kind: String::new(),
                    range: String::new(),
                    signature: String::new(),
                });
            }
        }
        let paged = paginate(rows, pg);
        if json {
            if paged.needs_envelope() { print_paginated_json(&paged); } else { print_json(&paged.items); }
        } else {
            print_toon(&paged.items);
        }
        if paged.was_truncated() {
            emit_pagination_hint(paged.total, paged.offset, paged.items.len(), "entries", "cx overview <subdir>");
        }
    } else {
        let mut rows: Vec<DirOverviewRow> = Vec::new();
        for (dir_name, (file_count, sym_count)) in &subdirs {
            rows.push(DirOverviewRow {
                file: format_subdir(dir_name),
                symbols: format!("({file_count} files, {sym_count} symbols)"),
            });
        }
        for (path, data) in &direct_files {
            let syms = prepare_symbols(data, no_tests);
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
        let paged = paginate(rows, pg);
        if json {
            if paged.needs_envelope() { print_paginated_json(&paged); } else { print_json(&paged.items); }
        } else {
            print_toon(&paged.items);
        }
        if paged.was_truncated() {
            emit_pagination_hint(paged.total, paged.offset, paged.items.len(), "entries", "cx overview <subdir>");
        }
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

fn line_range(
    index: &Index,
    cache: &mut std::collections::HashMap<PathBuf, Vec<usize>>,
    file: &Path,
    byte_range: (usize, usize),
) -> Option<String> {
    let line_starts = match cache.entry(file.to_path_buf()) {
        std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
        std::collections::hash_map::Entry::Vacant(entry) => {
            let source = fs::read(index.root.join(file)).ok()?;
            let mut starts = vec![0];
            starts.extend(source.iter().enumerate().filter_map(|(i, &byte)| {
                (byte == b'\n').then_some(i + 1)
            }));
            entry.insert(starts)
        }
    };

    let (start, end) = byte_range;
    if start > end {
        return None;
    }

    let end_offset = end.saturating_sub(1).max(start);
    let start_line = line_starts.partition_point(|&line_start| line_start <= start);
    let end_line = line_starts.partition_point(|&line_start| line_start <= end_offset);
    Some(if start_line == end_line {
        start_line.to_string()
    } else {
        format!("{start_line}-{end_line}")
    })
}

/// Display a path using forward slashes (consistent across platforms).
fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn resolve_file_filter<'a>(
    rel: &Path,
    index: &'a Index,
) -> Result<Vec<(&'a PathBuf, &'a FileData)>, i32> {
    if let Some(kv) = index.entries.get_key_value(rel) {
        return Ok(vec![kv]);
    }
    let abs = index.root.join(rel);
    if abs.is_dir() {
        let matches: Vec<_> = index
            .entries
            .iter()
            .filter(|(path, _)| path.starts_with(rel))
            .collect();
        if matches.is_empty() {
            eprintln!("cx: no indexed files under {}", display_path(rel));
            return Err(1);
        }
        return Ok(matches);
    }
    if abs.exists() && detect_language(&abs).is_none() {
        let ext = abs.extension().and_then(|e| e.to_str()).unwrap_or("(none)");
        eprintln!("cx: unsupported file type: .{ext}");
    } else {
        eprintln!("cx: file not in index: {}", display_path(rel));
    }
    Err(1)
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
        assert!(symbol_priority(SymbolKind::Fn) < symbol_priority(SymbolKind::Field));
    }
}
