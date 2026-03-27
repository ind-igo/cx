use crate::language::{supported_languages, download_names_for};

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

            // Workaround: tree-sitter-language-pack uses c_symbol_for() to build
            // the file path, so get_language("csharp") looks for libtree_sitter_c_sharp.dylib
            // but the downloaded file is libtree_sitter_csharp.dylib. Create symlinks.
            // See KNOWN_ISSUES.md for details.
            if let Ok(libs_dir) = tree_sitter_language_pack::cache_dir() {
                create_compat_symlinks(&libs_dir);
            }

            0
        }
        Err(e) => {
            eprintln!("cx: download failed: {}", e);
            eprintln!("cx: check your network connection and try again");
            1
        }
    }
}

/// Mappings where the download file name differs from what get_language() expects.
/// (download_name, expected_lib_name)
const COMPAT_SYMLINKS: &[(&str, &str)] = &[
    ("csharp", "c_sharp"),
];

/// Create compatibility symlinks so get_language() can find the downloaded grammar.
fn create_compat_symlinks(libs_dir: &std::path::Path) {
    for &(download, c_sym) in COMPAT_SYMLINKS {
        if let Ok(entries) = std::fs::read_dir(libs_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                let prefix = format!("libtree_sitter_{}", download);
                if name_str.starts_with(&prefix) {
                    let ext = &name_str[prefix.len()..];
                    let symlink_name = format!("libtree_sitter_{}{}", c_sym, ext);
                    let symlink_path = libs_dir.join(&symlink_name);
                    if !symlink_path.exists() {
                        #[cfg(unix)]
                        {
                            let _ = std::os::unix::fs::symlink(entry.path().file_name().unwrap(), &symlink_path);
                        }
                    }
                }
            }
        }
    }
}

/// Remove compatibility symlinks created by `create_compat_symlinks`.
fn remove_compat_symlinks(libs_dir: &std::path::Path, download_names: &[&str]) {
    for &(download, c_sym) in COMPAT_SYMLINKS {
        if download_names.contains(&download) {
            let prefix = format!("libtree_sitter_{}", c_sym);
            if let Ok(entries) = std::fs::read_dir(libs_dir) {
                for entry in entries.flatten() {
                    let fname = entry.file_name();
                    let fname_str = fname.to_string_lossy();
                    if fname_str.starts_with(&prefix) {
                        let _ = std::fs::remove_file(entry.path());
                    }
                }
            }
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
        remove_compat_symlinks(&libs_dir, &names);

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
