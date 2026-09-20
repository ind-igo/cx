use std::path::{Path, PathBuf};
use crate::language::{supported_languages, download_names_for, overrides, bundled};

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

pub fn add(languages: &[String], from: Option<&Path>) -> i32 {
    if let Some(source) = from {
        if languages.len() != 1 {
            eprintln!("cx: --from requires exactly one language name");
            return 1;
        }
        return match overrides::install(&languages[0], source) {
            Ok(()) => { eprintln!("cx: installed {} override from {}", languages[0], source.display()); 0 }
            Err(e) => { eprintln!("cx: override installation failed: {e}"); 1 }
        };
    }
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
        if overrides::get(lang).is_some() {
            eprintln!("cx: {lang} uses a local override; reinstall with --from DIR to update it");
            continue;
        }
        if bundled::NAMES.contains(&lang.as_str()) {
            if let Err(e) = bundled::install(lang) {
                eprintln!("cx: failed to install {lang}: {e}");
                return 1;
            }
            eprintln!("cx: installed {lang} (bundled)");
            continue;
        }
        for name in download_names_for(lang) {
            if !to_download.contains(&name) {
                to_download.push(name);
            }
        }
    }

    if to_download.is_empty() { return 0; }

    eprintln!("cx: downloading grammars: {}", to_download.join(", "));

    match tree_sitter_language_pack::download(&to_download) {
        Ok(_) => {
            eprintln!("cx: installed {} grammar(s)", to_download.len());
            0
        }
        Err(e) => {
            eprintln!("cx: download failed: {e}");
            eprintln!("cx: check your network connection and try again");
            1
        }
    }
}

pub fn remove(languages: &[String], local_override: bool) -> i32 {
    if local_override {
        if languages.is_empty() { eprintln!("cx: specify a language to remove"); return 1; }
        for name in languages {
            match overrides::remove(name) {
                Ok(true) => eprintln!("cx: removed {name} override"),
                Ok(false) => eprintln!("cx: {name} override not installed"),
                Err(e) => { eprintln!("cx: failed to remove {name} override: {e}"); return 1; }
            }
        }
        return 0;
    }
    let records = match overrides::registrations() {
        Ok(records) => records,
        Err(e) => { eprintln!("cx: failed to read overrides: {e}"); return 1; }
    };
    for name in languages {
        if records.iter().any(|(n, _)| n == name) {
            eprintln!("cx: {name} uses a local override — remove with: cx lang remove {name} --override");
            return 1;
        }
    }
    if languages.is_empty() {
        eprintln!("cx: specify at least one language, e.g.: cx lang remove rust");
        return 1;
    }

    let libs_dir = match tree_sitter_language_pack::cache_dir() {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!("cx: failed to find cache directory: {e}");
            return 1;
        }
    };

    for lang in languages {
        if bundled::NAMES.contains(&lang.as_str()) {
            match bundled::remove(lang) {
                Ok(true) => eprintln!("cx: removed {lang} grammar"),
                Ok(false) => eprintln!("cx: {lang} grammar not installed"),
                Err(e) => { eprintln!("cx: failed to remove {lang}: {e}"); return 1; }
            }
            continue;
        }
        let names = download_names_for(lang);
        let names = if names.is_empty() { vec![lang.as_str()] } else { names };
        let mut removed_any = false;
        for name in &names {
            let lib_prefix = format!("libtree_sitter_{name}");
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
            eprintln!("cx: removed {lang} grammar");
        } else {
            eprintln!("cx: {lang} grammar not found in cache");
        }
    }
    0
}

pub fn list() -> i32 {
    let records = match overrides::registrations() {
        Ok(records) => records,
        Err(e) => { eprintln!("cx: failed to read overrides: {e}"); return 1; }
    };
    let mut supported: Vec<_> = crate::language::builtin_languages().into_iter()
        .chain(records.iter().map(|(name, _)| name.as_str())).collect();
    supported.sort_unstable();
    supported.dedup();
    let installed = tree_sitter_language_pack::downloaded_languages();

    for lang in &supported {
        if let Some((_, record)) = records.iter().find(|(name, _)| name == lang) {
            println!("{lang:<15} [override] {}", record.source.display());
            continue;
        }
        let is_bundled = bundled::NAMES.contains(lang);
        let is_installed = if is_bundled {
            bundled::installed().contains(lang)
        } else {
            download_names_for(lang).iter().all(|n| installed.iter().any(|i| i == n))
        };
        let marker = if is_installed { "[installed]" } else { "[missing]" };
        let source = if is_bundled { "bundled" } else { "language-pack" };
        println!("{lang:<15} {marker} {source}");
    }
    eprintln!("\nNeed another language? Open an issue: https://github.com/ind-igo/cx/issues/new?template=language-request.yml");
    0
}
