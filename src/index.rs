use ignore::WalkBuilder;
use redb::{Database, ReadOnlyDatabase, ReadableDatabase, ReadableTable, TableDefinition};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::language::{detect_language, parse_and_extract};

pub const INDEX_VERSION: u32 = 3;
const DB_FILE: &str = ".cx-index.db";

const META_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("meta");
const FILES_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("files");
const SYMBOLS_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("symbols");

pub struct Index {
    pub root: PathBuf,
    db: Option<Database>,
    /// In-memory mirror for fast query access.
    pub entries: HashMap<PathBuf, FileData>,
}

#[derive(Debug, Clone)]
pub struct FileData {
    pub meta: FileEntry,
    pub symbols: Vec<Symbol>,
}

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub mtime: SystemTime,
    pub language: Language,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub signature: String,
    pub byte_range: (usize, usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
#[clap(rename_all = "lowercase")]
pub enum SymbolKind {
    Fn,
    Struct,
    Enum,
    Trait,
    Type,
    Const,
    Class,
    Interface,
    Method,
    Module,
    Event,
}

impl SymbolKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Fn => "fn",
            Self::Struct => "struct",
            Self::Enum => "enum",
            Self::Trait => "trait",
            Self::Type => "type",
            Self::Const => "const",
            Self::Class => "class",
            Self::Interface => "interface",
            Self::Method => "method",
            Self::Module => "module",
            Self::Event => "event",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[repr(u8)]
pub enum Language {
    Rust = 0,
    TypeScript = 1,
    Python = 2,
    Go = 3,
    C = 4,
    Cpp = 5,
    Java = 6,
    Ruby = 7,
    CSharp = 8,
    Lua = 9,
    Zig = 10,
    Bash = 11,
    Solidity = 12,
    Elixir = 13,
}

// --- Byte encoding for FileEntry (13 bytes: u64 secs + u32 nanos + u8 language) ---

fn encode_file_entry(entry: &FileEntry) -> [u8; 13] {
    let dur = entry.mtime.duration_since(UNIX_EPOCH).unwrap_or(Duration::ZERO);
    let mut buf = [0u8; 13];
    buf[0..8].copy_from_slice(&dur.as_secs().to_le_bytes());
    buf[8..12].copy_from_slice(&dur.subsec_nanos().to_le_bytes());
    buf[12] = language_to_u8(entry.language);
    buf
}

/// Decode a FileEntry from bytes. Returns None if data is truncated or language is unknown.
fn decode_file_entry(bytes: &[u8]) -> Option<FileEntry> {
    if bytes.len() < 13 {
        return None;
    }
    let secs = u64::from_le_bytes(bytes[0..8].try_into().unwrap());
    let nanos = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
    let language = u8_to_language(bytes[12])?;
    Some(FileEntry {
        mtime: UNIX_EPOCH + Duration::new(secs, nanos),
        language,
    })
}

fn language_to_u8(lang: Language) -> u8 {
    lang as u8
}

/// Returns None for unknown language discriminants, triggering a re-index.
fn u8_to_language(b: u8) -> Option<Language> {
    use Language::*;
    [Rust, TypeScript, Python, Go, C, Cpp, Java, Ruby, CSharp, Lua, Zig, Bash, Solidity, Elixir]
        .get(b as usize)
        .copied()
}

/// Open the database exclusively, retrying on lock contention.
fn open_db_exclusive(path: &Path) -> Result<Database, redb::DatabaseError> {
    let mut attempts = 0;
    loop {
        match Database::create(path) {
            Ok(db) => return Ok(db),
            Err(redb::DatabaseError::DatabaseAlreadyOpen) if attempts < 20 => {
                attempts += 1;
                if attempts == 1 {
                    eprintln!("cx: database locked, waiting...");
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            Err(e) => return Err(e),
        }
    }
}

/// Load entries from a readable database into memory.
fn load_entries(db: &impl ReadableDatabase) -> Option<HashMap<PathBuf, FileData>> {
    let read_txn = db.begin_read().ok()?;

    // Check version
    let version_ok = (|| -> Option<bool> {
        let table = read_txn.open_table(META_TABLE).ok()?;
        let val = table.get("version").ok()??;
        let bytes = val.value();
        if bytes.len() == 4 {
            Some(u32::from_le_bytes(bytes.try_into().unwrap()) == INDEX_VERSION)
        } else {
            None
        }
    })().unwrap_or(false);

    if !version_ok {
        return None;
    }

    let mut entries: HashMap<PathBuf, FileData> = HashMap::new();

    if let Ok(table) = read_txn.open_table(FILES_TABLE) {
        for item in table.iter().into_iter().flatten() {
            let Ok((key, val)) = item else { continue };
            let path = PathBuf::from(key.value());
            if let Some(meta) = decode_file_entry(val.value()) {
                entries.insert(path, FileData { meta, symbols: Vec::new() });
            }
        }
    }
    if let Ok(table) = read_txn.open_table(SYMBOLS_TABLE) {
        for item in table.iter().into_iter().flatten() {
            let Ok((key, val)) = item else { continue };
            let path = PathBuf::from(key.value());
            let syms: Vec<Symbol> = bincode::deserialize(val.value()).unwrap_or_default();
            if let Some(data) = entries.get_mut(&path) {
                data.symbols = syms;
            }
        }
    }

    Some(entries)
}

/// Check if any files on disk have changed compared to indexed entries.
fn needs_update(root: &Path, entries: &HashMap<PathBuf, FileData>) -> bool {
    let mut on_disk_count = 0usize;
    for entry in walk(root) {
        let path = entry.path();
        if detect_language(path).is_none() {
            continue;
        }
        let rel_path = match path.strip_prefix(root) {
            Ok(p) => p.to_path_buf(),
            Err(_) => continue,
        };
        on_disk_count += 1;
        let mtime = entry.metadata().ok()
            .and_then(|m| m.modified().ok())
            .unwrap_or(SystemTime::UNIX_EPOCH);
        match entries.get(&rel_path) {
            Some(data) if data.meta.mtime == mtime => {}
            _ => return true,
        }
    }
    // Check for deleted files
    on_disk_count != entries.len()
}

impl Index {
    /// Load or build the index for the given project root.
    ///
    /// Tries a shared (read-only) open first so multiple cx processes can
    /// run concurrently.  Falls back to an exclusive open only when the
    /// index needs to be created or updated.
    pub fn load_or_build(root: &Path) -> Self {
        let db_path = root.join(DB_FILE);

        // Fast path: open read-only (shared lock) and check if index is fresh
        if db_path.exists() {
            match ReadOnlyDatabase::open(&db_path) {
                Ok(ro_db) => {
                    if let Some(entries) = load_entries(&ro_db)
                        && !needs_update(root, &entries) {
                        return Index { root: root.to_path_buf(), db: None, entries };
                    }
                }
                Err(redb::DatabaseError::UpgradeRequired(_)) => {
                    // Old redb format; delete so exclusive path recreates it
                    let _ = fs::remove_file(&db_path);
                }
                Err(_) => {}
            }
        }

        // Slow path: need exclusive access to create or update the index
        let db = match open_db_exclusive(&db_path) {
            Ok(db) => db,
            Err(redb::DatabaseError::UpgradeRequired(_)) => {
                // Old redb format (e.g. v2 → v3 upgrade); delete and recreate
                let _ = fs::remove_file(&db_path);
                match open_db_exclusive(&db_path) {
                    Ok(db) => db,
                    Err(e) => {
                        eprintln!("cx: failed to open database: {}", e);
                        std::process::exit(1);
                    }
                }
            }
            Err(e) => {
                eprintln!("cx: failed to open database: {}", e);
                std::process::exit(1);
            }
        };

        match load_entries(&db) {
            Some(entries) => {
                let mut idx = Index { root: root.to_path_buf(), db: Some(db), entries };
                idx.incremental_update();
                idx
            }
            None => {
                let mut idx = Index {
                    root: root.to_path_buf(),
                    db: Some(db),
                    entries: HashMap::new(),
                };
                idx.full_crawl();
                idx.save_all();
                warn_if_not_gitignored(root);
                idx
            }
        }
    }

    /// Crawl from project root, collecting all supported files.
    fn full_crawl(&mut self) {
        for entry in walk(&self.root) {
            let path = entry.path();
            let Some(lang) = detect_language(path) else {
                continue;
            };

            let rel_path = match path.strip_prefix(&self.root) {
                Ok(p) => p.to_path_buf(),
                Err(_) => continue,
            };

            let mtime = entry
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .unwrap_or(SystemTime::UNIX_EPOCH);

            let symbols = match fs::read(path) {
                Ok(source) => parse_and_extract(lang, &source, path),
                Err(e) => {
                    eprintln!("cx: warning: failed to read {}: {}", path.display(), e);
                    Vec::new()
                }
            };
            self.entries.insert(rel_path, FileData {
                meta: FileEntry { mtime, language: lang },
                symbols,
            });
        }
    }

    /// Check for changed/new/deleted files and update the index.
    fn incremental_update(&mut self) {
        let mut on_disk: HashMap<PathBuf, (SystemTime, Language)> = HashMap::new();

        for entry in walk(&self.root) {
            let path = entry.path();
            let Some(lang) = detect_language(path) else {
                continue;
            };

            let rel_path = match path.strip_prefix(&self.root) {
                Ok(p) => p.to_path_buf(),
                Err(_) => continue,
            };

            let mtime = entry
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .unwrap_or(SystemTime::UNIX_EPOCH);

            on_disk.insert(rel_path, (mtime, lang));
        }

        // Remove deleted files
        let indexed_paths: Vec<PathBuf> = self.entries.keys().cloned().collect();
        let mut deleted = Vec::new();
        for path in indexed_paths {
            if !on_disk.contains_key(&path) {
                self.entries.remove(&path);
                deleted.push(path);
            }
        }

        // Add new or update changed files
        let mut changed: Vec<(PathBuf, FileEntry, Vec<Symbol>)> = Vec::new();
        for (path, (mtime, lang)) in &on_disk {
            let needs_update = !matches!(self.entries.get(path), Some(data) if data.meta.mtime == *mtime);
            if needs_update {
                let file_entry = FileEntry { mtime: *mtime, language: *lang };
                let abs_path = self.root.join(path);
                let symbols = match fs::read(&abs_path) {
                    Ok(source) => parse_and_extract(*lang, &source, &abs_path),
                    Err(_) => Vec::new(),
                };
                self.entries.insert(path.clone(), FileData {
                    meta: file_entry.clone(),
                    symbols: symbols.clone(),
                });
                changed.push((path.clone(), file_entry, symbols));
            }
        }

        if !deleted.is_empty() || !changed.is_empty() {
            let Some(ref db) = self.db else { return };
            let write_txn = match db.begin_write() {
                Ok(txn) => txn,
                Err(e) => {
                    eprintln!("cx: failed to begin write for incremental update: {}", e);
                    return;
                }
            };
            {
                let Ok(mut files_table) = write_txn.open_table(FILES_TABLE) else {
                    eprintln!("cx: failed to open files table — rebuild with: rm .cx-index.db");
                    return;
                };
                let Ok(mut syms_table) = write_txn.open_table(SYMBOLS_TABLE) else {
                    eprintln!("cx: failed to open symbols table — rebuild with: rm .cx-index.db");
                    return;
                };
                for path in &deleted {
                    let key = path.to_string_lossy();
                    let _ = files_table.remove(key.as_ref());
                    let _ = syms_table.remove(key.as_ref());
                }
                for (path, entry, symbols) in &changed {
                    let key = path.to_string_lossy();
                    match bincode::serialize(symbols) {
                        Ok(sym_bytes) => {
                            let _ = files_table.insert(key.as_ref(), encode_file_entry(entry).as_slice());
                            let _ = syms_table.insert(key.as_ref(), sym_bytes.as_slice());
                        }
                        Err(e) => eprintln!("cx: failed to serialize symbols for {}: {}", key, e),
                    }
                }
            }
            if let Err(e) = write_txn.commit() {
                eprintln!("cx: failed to commit incremental update: {}", e);
            }
        }
    }

    /// Write the entire index to the database (used after full_crawl).
    /// Clears all existing data first to avoid stale entries.
    fn save_all(&self) {
        let Some(ref db) = self.db else { return };
        let write_txn = match db.begin_write() {
            Ok(txn) => txn,
            Err(e) => {
                eprintln!("cx: failed to begin write: {}", e);
                return;
            }
        };

        // Delete and recreate tables to clear stale entries
        let _ = write_txn.delete_table(FILES_TABLE);
        let _ = write_txn.delete_table(SYMBOLS_TABLE);

        // Write version
        {
            let Ok(mut table) = write_txn.open_table(META_TABLE) else {
                eprintln!("cx: failed to open meta table — rebuild with: rm .cx-index.db");
                return;
            };
            let _ = table.insert("version", INDEX_VERSION.to_le_bytes().as_slice());
        }

        // Write files and symbols
        {
            let Ok(mut files_table) = write_txn.open_table(FILES_TABLE) else {
                eprintln!("cx: failed to open files table — rebuild with: rm .cx-index.db");
                return;
            };
            let Ok(mut syms_table) = write_txn.open_table(SYMBOLS_TABLE) else {
                eprintln!("cx: failed to open symbols table — rebuild with: rm .cx-index.db");
                return;
            };
            for (path, data) in &self.entries {
                let key = path.to_string_lossy();
                let _ = files_table.insert(key.as_ref(), encode_file_entry(&data.meta).as_slice());
                match bincode::serialize(&data.symbols) {
                    Ok(sym_bytes) => { let _ = syms_table.insert(key.as_ref(), sym_bytes.as_slice()); }
                    Err(e) => eprintln!("cx: failed to serialize symbols for {}: {}", key, e),
                }
            }
        }

        if let Err(e) = write_txn.commit() {
            eprintln!("cx: failed to commit: {}", e);
        }
    }

}

/// Warn once if .cx-index.db is not in .gitignore.
fn warn_if_not_gitignored(root: &Path) {
    use std::process::Command;
    let ok = Command::new("git")
        .args(["check-ignore", "-q", DB_FILE])
        .current_dir(root)
        .status()
        .is_ok_and(|s| s.success());
    if !ok {
        eprintln!("cx: created .cx-index.db — consider adding it to .gitignore");
    }
}

/// Walk the project tree, respecting .gitignore and skipping the index/db files.
fn walk(root: &Path) -> impl Iterator<Item = ignore::DirEntry> {
    WalkBuilder::new(root)
        .hidden(false)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .filter_entry(|e| {
            let name = e.file_name().to_str().unwrap_or("");
            if name == ".git" || name.starts_with(".cx-index") {
                return false;
            }
            if e.file_type().is_some_and(|ft| ft.is_dir()) && e.path().join(".cx-ignore").exists() {
                return false;
            }
            true
        })
        .build()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_some_and(|ft| ft.is_file()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_file_entry_encode_roundtrip() {
        let entry = FileEntry {
            mtime: UNIX_EPOCH + Duration::new(1234567890, 42),
            language: Language::Rust,
        };
        let bytes = encode_file_entry(&entry);
        let decoded = decode_file_entry(&bytes).expect("should decode");
        assert_eq!(
            entry.mtime.duration_since(UNIX_EPOCH).unwrap(),
            decoded.mtime.duration_since(UNIX_EPOCH).unwrap()
        );
        assert_eq!(entry.language, decoded.language);
    }

    #[test]
    fn test_file_entry_decode_truncated_returns_none() {
        assert!(decode_file_entry(&[0u8; 5]).is_none());
        assert!(decode_file_entry(&[]).is_none());
    }

    #[test]
    fn test_file_entry_decode_unknown_language_returns_none() {
        let entry = FileEntry {
            mtime: UNIX_EPOCH + Duration::new(100, 0),
            language: Language::Rust,
        };
        let mut bytes = encode_file_entry(&entry);
        bytes[12] = 255; // unknown language
        assert!(decode_file_entry(&bytes).is_none());
    }

    #[test]
    fn test_language_roundtrip() {
        for lang in [
            Language::Rust, Language::TypeScript, Language::Python, Language::Go,
            Language::C, Language::Cpp, Language::Java, Language::Ruby,
            Language::CSharp, Language::Lua, Language::Zig, Language::Bash,
            Language::Solidity, Language::Elixir,
        ] {
            assert_eq!(u8_to_language(language_to_u8(lang)), Some(lang));
        }
    }

    #[test]
    fn test_u8_to_language_unknown_returns_none() {
        assert!(u8_to_language(200).is_none());
        assert!(u8_to_language(14).is_none());
        assert!(u8_to_language(255).is_none());
    }

    #[test]
    fn test_symbol_bincode_roundtrip() {
        let symbols = vec![
            Symbol {
                name: "foo".to_string(),
                kind: SymbolKind::Fn,
                signature: "pub fn foo(x: i32) -> bool".to_string(),
                byte_range: (100, 500),
            },
            Symbol {
                name: "Bar".to_string(),
                kind: SymbolKind::Struct,
                signature: "pub struct Bar".to_string(),
                byte_range: (600, 800),
            },
        ];
        let bytes = bincode::serialize(&symbols).unwrap();
        let decoded: Vec<Symbol> = bincode::deserialize(&bytes).unwrap();
        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded[0].name, "foo");
        assert_eq!(decoded[1].kind, SymbolKind::Struct);
        assert_eq!(decoded[0].byte_range, (100, 500));
    }

    #[test]
    fn test_full_crawl_finds_rust_files() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test-crawl.db");
        let db = Database::create(&db_path).unwrap();
        // Use the real project root for crawling, but store db in tempdir
        let cwd = env::current_dir().unwrap();
        let mut idx = Index {
            root: cwd,
            db: Some(db),
            entries: HashMap::new(),
        };
        idx.full_crawl();

        assert!(idx.entries.contains_key(&PathBuf::from("src/main.rs")));
        for path in idx.entries.keys() {
            assert!(!path.starts_with("target/"), "found target/ file: {:?}", path);
        }
    }

    #[test]
    fn test_walk_respects_gitignore() {
        let cwd = env::current_dir().unwrap();
        let entries: Vec<_> = walk(&cwd).collect();
        for entry in &entries {
            let path = entry.path();
            let rel = path.strip_prefix(&cwd).unwrap_or(path);
            assert!(!rel.starts_with(".git/"), "found .git file: {:?}", rel);
            assert!(!rel.starts_with("target/"), "found target file: {:?}", rel);
        }
    }

    /// Helper: create a temp project with .git dir and source files, return (tempdir, Index).
    fn build_temp_index(files: &[(&str, &str)]) -> (tempfile::TempDir, Index) {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join(".git")).unwrap();
        for (path, content) in files {
            let full = dir.path().join(path);
            if let Some(parent) = full.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(&full, content).unwrap();
        }
        let idx = Index::load_or_build(dir.path());
        (dir, idx)
    }

    #[test]
    fn test_load_or_build_fresh_db() {
        let (dir, idx) = build_temp_index(&[
            ("src/main.rs", "fn main() {}\n"),
            ("src/lib.rs", "pub fn hello() {}\n"),
        ]);

        assert!(idx.entries.contains_key(&PathBuf::from("src/main.rs")));
        assert!(idx.entries.contains_key(&PathBuf::from("src/lib.rs")));
        assert_eq!(idx.entries.get(&PathBuf::from("src/main.rs")).unwrap().symbols.len(), 1);
        assert_eq!(idx.entries.get(&PathBuf::from("src/lib.rs")).unwrap().symbols.len(), 1);

        // DB file should exist
        assert!(dir.path().join(DB_FILE).exists());
    }

    #[test]
    fn test_load_or_build_reloads_from_existing_db() {
        let (dir, idx) = build_temp_index(&[
            ("src/main.rs", "fn main() {}\nfn helper() {}\n"),
        ]);

        let file_count = idx.entries.len();
        let sym_count = idx.entries.get(&PathBuf::from("src/main.rs")).unwrap().symbols.len();
        assert!(sym_count >= 2, "should have at least 2 symbols: {sym_count}");

        // Drop and reload — should get same data from redb
        drop(idx);
        let idx2 = Index::load_or_build(dir.path());
        assert_eq!(idx2.entries.len(), file_count);
        assert_eq!(
            idx2.entries.get(&PathBuf::from("src/main.rs")).unwrap().symbols.len(),
            sym_count,
        );
    }

    #[test]
    fn test_save_all_clears_stale_entries() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join(".git")).unwrap();
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::write(dir.path().join("src/a.rs"), "fn a() {}\n").unwrap();
        fs::write(dir.path().join("src/b.rs"), "fn b() {}\n").unwrap();

        // Build index with both files
        let idx = Index::load_or_build(dir.path());
        assert!(idx.entries.contains_key(&PathBuf::from("src/a.rs")));
        assert!(idx.entries.contains_key(&PathBuf::from("src/b.rs")));
        drop(idx);

        // Remove b.rs, rebuild
        fs::remove_file(dir.path().join("src/b.rs")).unwrap();
        let idx2 = Index::load_or_build(dir.path());
        assert!(idx2.entries.contains_key(&PathBuf::from("src/a.rs")));
        assert!(!idx2.entries.contains_key(&PathBuf::from("src/b.rs")));

        // Reload again — b.rs should still be gone from redb
        drop(idx2);
        let idx3 = Index::load_or_build(dir.path());
        assert!(!idx3.entries.contains_key(&PathBuf::from("src/b.rs")));
    }

    #[test]
    fn test_incremental_update_detects_new_file() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join(".git")).unwrap();
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::write(dir.path().join("src/a.rs"), "fn a() {}\n").unwrap();

        let idx = Index::load_or_build(dir.path());
        assert_eq!(idx.entries.len(), 1);
        drop(idx);

        // Add a new file
        fs::write(dir.path().join("src/b.rs"), "fn b() {}\n").unwrap();

        let idx2 = Index::load_or_build(dir.path());
        assert_eq!(idx2.entries.len(), 2);
        assert!(idx2.entries.contains_key(&PathBuf::from("src/b.rs")));
        assert_eq!(idx2.entries.get(&PathBuf::from("src/b.rs")).unwrap().symbols.len(), 1);
    }

    #[test]
    fn test_incremental_update_detects_modified_file() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join(".git")).unwrap();
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::write(dir.path().join("src/a.rs"), "fn a() {}\n").unwrap();

        let idx = Index::load_or_build(dir.path());
        assert_eq!(idx.entries.get(&PathBuf::from("src/a.rs")).unwrap().symbols.len(), 1);
        drop(idx);

        // Modify the file — add a second function
        // Sleep briefly to ensure mtime changes
        std::thread::sleep(std::time::Duration::from_millis(50));
        fs::write(dir.path().join("src/a.rs"), "fn a() {}\nfn b() {}\n").unwrap();

        let idx2 = Index::load_or_build(dir.path());
        assert_eq!(
            idx2.entries.get(&PathBuf::from("src/a.rs")).unwrap().symbols.len(),
            2,
            "should detect modified file and re-parse symbols"
        );
    }

    #[test]
    fn test_incremental_update_detects_deleted_file() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join(".git")).unwrap();
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::write(dir.path().join("src/a.rs"), "fn a() {}\n").unwrap();
        fs::write(dir.path().join("src/b.rs"), "fn b() {}\n").unwrap();

        let idx = Index::load_or_build(dir.path());
        assert_eq!(idx.entries.len(), 2);
        drop(idx);

        // Delete one file
        fs::remove_file(dir.path().join("src/b.rs")).unwrap();

        let idx2 = Index::load_or_build(dir.path());
        assert_eq!(idx2.entries.len(), 1);
        assert!(idx2.entries.contains_key(&PathBuf::from("src/a.rs")));
        assert!(!idx2.entries.contains_key(&PathBuf::from("src/b.rs")));
    }

    #[test]
    fn test_version_mismatch_triggers_rebuild() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join(".git")).unwrap();
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::write(dir.path().join("src/a.rs"), "fn a() {}\n").unwrap();

        // Build normally
        let idx = Index::load_or_build(dir.path());
        assert!(idx.entries.contains_key(&PathBuf::from("src/a.rs")));
        drop(idx);

        // Corrupt the version in the db
        let db = Database::create(dir.path().join(DB_FILE)).unwrap();
        {
            let write_txn = db.begin_write().unwrap();
            {
                let mut table = write_txn.open_table(META_TABLE).unwrap();
                let _ = table.insert("version", 999u32.to_le_bytes().as_slice());
            }
            write_txn.commit().unwrap();
        }
        drop(db);

        // Reload — should detect version mismatch and rebuild
        let idx2 = Index::load_or_build(dir.path());
        assert!(idx2.entries.contains_key(&PathBuf::from("src/a.rs")));
    }

    #[test]
    fn test_symbols_persisted_to_redb() {
        let (dir, idx) = build_temp_index(&[
            ("src/main.rs", "pub fn foo(x: i32) -> bool { true }\nstruct Bar;\n"),
        ]);

        let syms = &idx.entries.get(&PathBuf::from("src/main.rs")).unwrap().symbols;
        assert!(syms.iter().any(|s| s.name == "foo" && s.kind == SymbolKind::Fn));
        assert!(syms.iter().any(|s| s.name == "Bar" && s.kind == SymbolKind::Struct));
        drop(idx);

        // Reload and verify symbols survive the roundtrip through redb + bincode
        let idx2 = Index::load_or_build(dir.path());
        let syms2 = &idx2.entries.get(&PathBuf::from("src/main.rs")).unwrap().symbols;
        assert!(syms2.iter().any(|s| s.name == "foo" && s.kind == SymbolKind::Fn));
        assert!(syms2.iter().any(|s| s.name == "Bar" && s.kind == SymbolKind::Struct));
    }
}
