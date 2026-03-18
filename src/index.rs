use ignore::WalkBuilder;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::language::{detect_language, parse_and_extract};

pub const INDEX_VERSION: u32 = 1;
const INDEX_FILE: &str = ".cx-index";
fn index_tmp() -> String {
    format!(".cx-index.tmp.{}", std::process::id())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Index {
    pub version: u32,
    pub root: PathBuf,
    pub files: HashMap<PathBuf, FileEntry>,
    pub exports: HashMap<PathBuf, Vec<Symbol>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileEntry {
    #[serde(with = "system_time_serde")]
    pub mtime: SystemTime,
    pub language: Language,
    pub read_cache: Option<ReadCache>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReadCache {
    pub session_id: String,
    pub content_hash: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub signature: String,
    pub byte_range: (usize, usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
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
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "fn" => Some(Self::Fn),
            "struct" => Some(Self::Struct),
            "enum" => Some(Self::Enum),
            "trait" => Some(Self::Trait),
            "type" => Some(Self::Type),
            "const" => Some(Self::Const),
            "class" => Some(Self::Class),
            "interface" => Some(Self::Interface),
            "method" => Some(Self::Method),
            "module" => Some(Self::Module),
            "event" => Some(Self::Event),
            _ => None,
        }
    }

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
pub enum Language {
    Rust,
    TypeScript,
    Python,
    Go,
    C,
    Cpp,
    Java,
    Ruby,
    CSharp,
    Lua,
    Zig,
    Bash,
    Solidity,
    Elixir,
}

impl Index {
    pub fn new(root: PathBuf) -> Self {
        Self {
            version: INDEX_VERSION,
            root,
            files: HashMap::new(),
            exports: HashMap::new(),
        }
    }

    /// Load or build the index for the given project root.
    /// Performs incremental update if the index exists and version matches.
    pub fn load_or_build(root: &Path) -> Self {
        let index_path = root.join(INDEX_FILE);

        // Try loading existing index
        if let Ok(data) = fs::read(&index_path) {
            if let Ok(mut index) = serde_json::from_slice::<Index>(&data) {
                if index.version == INDEX_VERSION && index.root == root {
                    index.incremental_update();
                    return index;
                }
            }
            // Version mismatch or corrupt — rebuild
        }

        let mut index = Index::new(root.to_path_buf());
        index.full_crawl();
        index.save();
        index
    }

    /// Crawl from project root, collecting all supported files.
    /// Respects .gitignore and always skips the index file.
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

            self.files.insert(
                rel_path.clone(),
                FileEntry {
                    mtime,
                    language: lang,
                    read_cache: None,
                },
            );

            // Parse and extract symbols
            let symbols = match fs::read(path) {
                Ok(source) => parse_and_extract(lang, &source, path),
                Err(e) => {
                    eprintln!("cx: warning: failed to read {}: {}", path.display(), e);
                    Vec::new()
                }
            };
            self.exports.insert(rel_path, symbols);
        }
    }

    /// Check for changed/new/deleted files and update the index.
    fn incremental_update(&mut self) {
        // Collect current files on disk
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
        let indexed_paths: Vec<PathBuf> = self.files.keys().cloned().collect();
        for path in indexed_paths {
            if !on_disk.contains_key(&path) {
                self.files.remove(&path);
                self.exports.remove(&path);
            }
        }

        // Add new or update changed files
        let mut changed = false;
        for (path, (mtime, lang)) in &on_disk {
            match self.files.get(path) {
                Some(entry) if entry.mtime == *mtime => {
                    // Unchanged — skip
                }
                _ => {
                    // New or changed — update entry, preserve read_cache
                    let read_cache = self
                        .files
                        .get(path)
                        .and_then(|e| e.read_cache.as_ref())
                        .map(|rc| ReadCache {
                            session_id: rc.session_id.clone(),
                            content_hash: rc.content_hash,
                        });

                    self.files.insert(
                        path.clone(),
                        FileEntry {
                            mtime: *mtime,
                            language: *lang,
                            read_cache,
                        },
                    );
                    // Re-parse and extract symbols
                    let abs_path = self.root.join(path);
                    let symbols = match fs::read(&abs_path) {
                        Ok(source) => parse_and_extract(*lang, &source, &abs_path),
                        Err(_) => Vec::new(),
                    };
                    self.exports.insert(path.clone(), symbols);
                    changed = true;
                }
            }
        }

        if changed {
            self.save();
        }
    }

    /// Atomically write the index to disk.
    pub fn save(&self) {
        let index_path = self.root.join(INDEX_FILE);
        let tmp_path = self.root.join(index_tmp());

        let data = match serde_json::to_vec(self) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("cx: failed to serialize index: {}", e);
                return;
            }
        };

        if let Err(e) = fs::write(&tmp_path, &data) {
            eprintln!("cx: failed to write index tmp: {}", e);
            return;
        }

        if let Err(e) = fs::rename(&tmp_path, &index_path) {
            eprintln!("cx: failed to rename index: {}", e);
        }
    }
}

/// Walk the project tree, respecting .gitignore and skipping the index file.
fn walk(root: &Path) -> impl Iterator<Item = ignore::DirEntry> {
    WalkBuilder::new(root)
        .hidden(false)       // don't skip dotfiles — gitignore handles what to skip
        .git_ignore(true)    // respect .gitignore
        .git_global(true)    // respect global gitignore
        .git_exclude(true)   // respect .git/info/exclude
        .filter_entry(|e| {
            let name = e.file_name().to_str().unwrap_or("");
            // Always skip .git, the index file, and .cx-ignore sentinel dirs
            if name == ".git" || name == INDEX_FILE {
                return false;
            }
            if name.starts_with(".cx-index.tmp") {
                return false;
            }
            if e.file_type().map_or(false, |ft| ft.is_dir()) && e.path().join(".cx-ignore").exists() {
                return false;
            }
            true
        })
        .build()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map_or(false, |ft| ft.is_file()))
}

mod system_time_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    pub fn serialize<S: Serializer>(time: &SystemTime, s: S) -> Result<S::Ok, S::Error> {
        let dur = time.duration_since(UNIX_EPOCH).unwrap_or(Duration::ZERO);
        (dur.as_secs(), dur.subsec_nanos()).serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<SystemTime, D::Error> {
        let (secs, nanos): (u64, u32) = Deserialize::deserialize(d)?;
        Ok(UNIX_EPOCH + Duration::new(secs, nanos))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_index_new() {
        let idx = Index::new(PathBuf::from("/tmp/test"));
        assert_eq!(idx.version, INDEX_VERSION);
        assert!(idx.files.is_empty());
        assert!(idx.exports.is_empty());
    }

    #[test]
    fn test_index_serialize_roundtrip() {
        let mut idx = Index::new(PathBuf::from("/tmp/test"));
        idx.files.insert(
            PathBuf::from("src/main.rs"),
            FileEntry {
                mtime: SystemTime::UNIX_EPOCH,
                language: Language::Rust,
                read_cache: None,
            },
        );

        let data = serde_json::to_vec(&idx).unwrap();
        let idx2: Index = serde_json::from_slice(&data).unwrap();
        assert_eq!(idx2.version, INDEX_VERSION);
        assert!(idx2.files.contains_key(&PathBuf::from("src/main.rs")));
    }

    #[test]
    fn test_full_crawl_finds_rust_files() {
        let cwd = env::current_dir().unwrap();
        let mut idx = Index::new(cwd);
        idx.full_crawl();

        // Should find at least src/main.rs
        assert!(idx.files.contains_key(&PathBuf::from("src/main.rs")));

        // Should not contain target/ files
        for path in idx.files.keys() {
            assert!(
                !path.starts_with("target/"),
                "found target/ file in index: {:?}",
                path
            );
        }
    }

    #[test]
    fn test_walk_respects_gitignore() {
        let cwd = env::current_dir().unwrap();

        let entries: Vec<_> = walk(&cwd).collect();

        for entry in &entries {
            let path = entry.path();
            let rel = path.strip_prefix(&cwd).unwrap_or(path);
            assert!(
                !rel.starts_with(".git/"),
                "found .git file: {:?}",
                rel
            );
            assert!(
                !rel.starts_with("target/"),
                "found target file: {:?}",
                rel
            );
        }
    }

    #[test]
    fn test_incremental_detects_no_changes() {
        let cwd = env::current_dir().unwrap();
        let mut idx = Index::new(cwd);
        idx.full_crawl();

        let file_count_before = idx.files.len();
        idx.incremental_update();
        assert_eq!(idx.files.len(), file_count_before);
    }
}
