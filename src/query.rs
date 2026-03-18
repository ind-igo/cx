use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::index::{Index, ReadCache, Symbol, SymbolKind};
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

#[derive(Serialize)]
struct ReadUnchanged {
    status: &'static str,
    file: String,
    hash: String,
}

#[derive(Serialize)]
struct ReadChanged {
    status: &'static str,
    file: String,
    content: String,
}

#[derive(Serialize)]
struct ReadFull {
    file: String,
    content: String,
}

// --- Query implementations ---

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

    // Sort by file then name for stable output
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

    // Always return an array for consistent JSON schema
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

/// Execute the read command with session-scoped cache.
pub fn read(index: &mut Index, file: &Path, fresh: bool, json: bool) -> i32 {
    let rel = make_relative(file, &index.root);
    let abs = index.root.join(&rel);

    // Read current file content
    let content = match fs::read(&abs) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("cx: cannot read {}: {}", file.display(), e);
            return 1;
        }
    };

    let content_hash = hash_bytes(&content);
    let content_str = String::from_utf8_lossy(&content);

    if fresh {
        update_cache(index, &rel, content_hash);
        print_read_content(&rel, &content_str, json);
        return 0;
    }

    let session_id = get_session_id();

    // Check cache
    if let Some(entry) = index.files.get(&rel)
        && let Some(ref cache) = entry.read_cache
            && cache.session_id == session_id {
                if cache.content_hash == content_hash {
                    // Unchanged
                    let out = ReadUnchanged {
                        status: "unchanged",
                        file: rel.display().to_string(),
                        hash: format!("{:08x}", content_hash),
                    };
                    if json { print_json(&out) } else { print_toon(&out) }
                    return 0;
                } else {
                    // Changed — return full new content
                    update_cache(index, &rel, content_hash);
                    if json {
                        print_json(&ReadChanged {
                            status: "changed",
                            file: rel.display().to_string(),
                            content: content_str.to_string(),
                        });
                    } else {
                        print_toon(&ReadChanged {
                            status: "changed",
                            file: rel.display().to_string(),
                            content: content_str.to_string(),
                        });
                    }
                    return 0;
                }
            }

    // Cache miss — first read in session
    update_cache_with_session(index, &rel, content_hash, &session_id);
    print_read_content(&rel, &content_str, json);
    0
}

fn print_read_content(rel: &Path, content: &str, json: bool) {
    if json {
        print_json(&ReadFull {
            file: rel.display().to_string(),
            content: content.to_string(),
        });
    } else {
        println!("{}", content);
    }
}

fn update_cache(index: &mut Index, rel: &Path, content_hash: u64) {
    let session_id = get_session_id();
    update_cache_with_session(index, rel, content_hash, &session_id);
}

fn update_cache_with_session(index: &mut Index, rel: &Path, content_hash: u64, session_id: &str) {
    if let Some(entry) = index.files.get_mut(rel) {
        entry.read_cache = Some(ReadCache {
            session_id: session_id.to_string(),
            content_hash,
        });
    }
    index.save();
}

fn hash_bytes(data: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    data.hash(&mut hasher);
    hasher.finish()
}

/// Get the parent process ID, falling back to own PID on non-unix.
fn parent_pid() -> u32 {
    #[cfg(unix)]
    { std::os::unix::process::parent_id() }
    #[cfg(not(unix))]
    { std::process::id() }
}

/// Get the TTY device name for the current process, if any.
fn tty_suffix() -> String {
    #[cfg(unix)]
    {
        use std::os::unix::io::AsRawFd;
        let fd = std::io::stdin().as_raw_fd();
        // SAFETY: ttyname_r would be safer but libc::ttyname is fine for a short-lived read
        let ptr = unsafe { libc::ttyname(fd) };
        if !ptr.is_null()
            && let Ok(s) = unsafe { std::ffi::CStr::from_ptr(ptr) }.to_str() {
                // Sanitize path: /dev/ttys003 -> ttys003
                return s.trim_start_matches("/dev/").replace('/', "-");
            }
    }
    String::new()
}

/// Get or create session ID based on parent PID + TTY.
/// TTY distinguishes agents in different terminals sharing a parent process.
/// Falls back to PPID-only when no TTY is available (pipes, CI).
fn get_session_id() -> String {
    let ppid = parent_pid();
    let user = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| format!("u{}", std::process::id()));
    let tty = tty_suffix();
    let session_file = if tty.is_empty() {
        PathBuf::from(format!("/tmp/cx-session-{}-{}", user, ppid))
    } else {
        PathBuf::from(format!("/tmp/cx-session-{}-{}-{}", user, ppid, tty))
    };

    if let Ok(id) = fs::read_to_string(&session_file) {
        let trimmed = id.trim().to_string();
        if !trimmed.is_empty() {
            return trimmed;
        }
    }

    // Generate new session ID
    let id = format!(
        "{:016x}{:016x}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64,
        std::process::id() as u64
    );

    let _ = fs::write(&session_file, &id);
    id
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
