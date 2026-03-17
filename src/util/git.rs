use std::path::{Path, PathBuf};

/// Walk up from `start` looking for a directory containing `.git`.
/// Returns the directory that contains `.git`, or `start` if none found.
pub fn find_project_root(start: &Path) -> PathBuf {
    let mut current = if start.is_file() {
        start.parent().unwrap_or(start).to_path_buf()
    } else {
        start.to_path_buf()
    };

    loop {
        if current.join(".git").exists() {
            return current;
        }
        match current.parent() {
            Some(parent) => current = parent.to_path_buf(),
            None => return start.to_path_buf(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_find_project_root_from_cwd() {
        // This test runs inside the cx project, so it should find the .git root
        let cwd = env::current_dir().unwrap();
        let root = find_project_root(&cwd);
        assert!(root.join(".git").exists());
    }

    #[test]
    fn test_find_project_root_from_subdirectory() {
        let cwd = env::current_dir().unwrap();
        let subdir = cwd.join("src");
        if subdir.exists() {
            let root = find_project_root(&subdir);
            assert!(root.join(".git").exists());
        }
    }
}
