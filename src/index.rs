use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::SystemTime;

pub const INDEX_VERSION: u32 = 1;

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
    pub is_exported: bool,
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
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    Rust,
    TypeScript,
    Python,
    Unknown,
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
