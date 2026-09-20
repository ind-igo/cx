use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::LazyLock;
use tree_sitter::Language;

pub const NAMES: &[&str] = &["mojo", "bend"];

pub fn installed() -> &'static [&'static str] {
    // Keep parser availability consistent throughout one indexing process.
    static INSTALLED: LazyLock<Vec<&str>> = LazyLock::new(|| {
        NAMES
            .iter()
            .copied()
            .filter(|name| marker(name).is_file())
            .collect()
    });
    &INSTALLED
}

fn marker(name: &str) -> PathBuf {
    crate::lang::cx_cache_dir().join("bundled").join(name)
}

pub fn install(name: &str) -> io::Result<()> {
    let path = marker(name);
    fs::create_dir_all(path.parent().unwrap())?;
    fs::write(path, [])
}

pub fn remove(name: &str) -> io::Result<bool> {
    match fs::remove_file(marker(name)) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e),
    }
}

pub fn language(name: &str) -> Option<Language> {
    match name {
        "mojo" => {
            unsafe extern "C" {
                fn tree_sitter_mojo() -> *const tree_sitter::ffi::TSLanguage;
            }
            // The generated parser is statically linked and lives for the whole process.
            Some(unsafe { Language::from_raw(tree_sitter_mojo()) })
        }
        "bend" => Some(tree_sitter_bend2::LANGUAGE.into()),
        _ => None,
    }
}
