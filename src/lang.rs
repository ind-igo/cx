use std::path::PathBuf;
use crate::language::{supported_languages, download_names_for};

pub fn cx_cache_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("CX_CACHE_DIR") {
        return PathBuf::from(dir);
    }
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from(".cache"))
        .join("cx")
}

pub fn grammar_cache_dir() -> PathBuf {
    cx_cache_dir().join("grammars")
}

pub fn add(languages: &[String]) -> i32 {
    if languages.is_empty() {
        eprintln!("cx: specify at least one language, e.g.: cx lang add rust typescript");
        return 1;
    }

    let supported = supported_languages();
    for lang in languages {
        if !supported.contains(&lang.as_str()) {
            eprintln!("cx: unknown language '{}' — supported: {}", lang, supported.join(", "));
            return 1;
        }
    }

    // Expand to actual download names (e.g. "typescript" → ["typescript", "tsx"])
    let mut to_download: Vec<&str> = Vec::new();
    for lang in languages {
        for name in download_names_for(lang) {
            if !to_download.contains(&name) {
                to_download.push(name);
            }
        }
    }

    eprintln!("cx: downloading grammars: {}", to_download.join(", "));

    match tree_sitter_language_pack::download(&to_download) {
        Ok(_) => {
            eprintln!("cx: installed {} grammar(s)", to_download.len());
            0
        }
        Err(e) => {
            eprintln!("cx: download failed: {}", e);
            eprintln!("cx: check your network connection and try again");
            1
        }
    }
}

pub fn remove(languages: &[String]) -> i32 {
    if languages.is_empty() {
        eprintln!("cx: specify at least one language, e.g.: cx lang remove rust");
        return 1;
    }

    let libs_dir = match tree_sitter_language_pack::cache_dir() {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!("cx: failed to find cache directory: {}", e);
            return 1;
        }
    };

    for lang in languages {
        let names = download_names_for(lang);
        let names = if names.is_empty() { vec![lang.as_str()] } else { names };
        let mut removed_any = false;
        for name in &names {
            let lib_prefix = format!("libtree_sitter_{}", name);
            if let Ok(entries) = std::fs::read_dir(&libs_dir) {
                for entry in entries.flatten() {
                    let fname = entry.file_name();
                    let fname_str = fname.to_string_lossy();
                    if fname_str.starts_with(&lib_prefix) && (fname_str.ends_with(".so") || fname_str.ends_with(".dylib") || fname_str.ends_with(".dll")) {
                        let _ = std::fs::remove_file(entry.path());
                        removed_any = true;
                    }
                }
            }
        }

        if removed_any {
            eprintln!("cx: removed {} grammar", lang);
        } else {
            eprintln!("cx: {} grammar not found in cache", lang);
        }
    }
    0
}

pub fn list() -> i32 {
    let supported = supported_languages();
    let installed = tree_sitter_language_pack::downloaded_languages();

    for lang in &supported {
        let names = download_names_for(lang);
        let is_installed = names.iter().all(|n| installed.iter().any(|i| i == n));
        let marker = if is_installed { "[installed]" } else { "[missing]" };
        println!("{:<15} {}", lang, marker);
    }
    eprintln!("\nNeed another language? Open an issue: https://github.com/ind-igo/cx/issues/new?template=language-request.yml");
    0
}
