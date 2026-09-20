use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::LazyLock;

static RUST_PARSER: LazyLock<(PathBuf, PathBuf)> = LazyLock::new(|| {
    let base = std::env::var_os("CX_CACHE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| dirs::cache_dir().unwrap().join("cx"));
    let grammars = base.join("grammars");
    tree_sitter_language_pack::configure(&tree_sitter_language_pack::PackConfig {
        cache_dir: Some(grammars.clone()),
        ..Default::default()
    })
    .unwrap();
    let file = tree_sitter_language_pack::cache_dir()
        .unwrap()
        .join(format!(
            "{}tree_sitter_rust{}",
            std::env::consts::DLL_PREFIX,
            std::env::consts::DLL_SUFFIX
        ));
    assert!(
        file.is_file(),
        "install the Rust grammar before tests: cargo run -- lang add rust"
    );
    let relative = file.strip_prefix(grammars).unwrap().to_owned();
    (file, relative)
});

const FUNCTIONS: &str = "(function_item name: (identifier) @name) @definition.function";
const TYPES: &str = "(struct_item name: (type_identifier) @name) @definition.struct";

struct Fixture {
    _temp: tempfile::TempDir,
    cache: PathBuf,
    bundle: PathBuf,
    project: PathBuf,
}

impl Fixture {
    fn new(name: &str, extension: &str) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let cache = temp.path().join("cache");
        let bundle = temp.path().join("bundle");
        let project = temp.path().join("project");
        fs::create_dir_all(&bundle).unwrap();
        fs::create_dir_all(project.join(".git")).unwrap();
        fs::copy(&RUST_PARSER.0, bundle.join("parser")).unwrap();
        fs::write(bundle.join("symbols.scm"), FUNCTIONS).unwrap();
        fs::write(
            bundle.join("language.json"),
            json!({
                "name": name, "grammar": "rust", "extensions": [extension],
                "library": "parser", "query": "symbols.scm",
                "ref_node_types": ["identifier", "type_identifier"], "sig_body_child": "body"
            })
            .to_string(),
        )
        .unwrap();
        fs::write(
            project.join(format!("main.{extension}")),
            "fn hello() {}\nfn main() { hello(); }\nstruct Thing;\n",
        )
        .unwrap();
        Self {
            _temp: temp,
            cache,
            bundle,
            project,
        }
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_cx"))
            .current_dir(&self.project)
            .env("CX_CACHE_DIR", &self.cache)
            .env("HTTPS_PROXY", "http://127.0.0.1:1")
            .env("NO_PROXY", "")
            .args(args)
            .output()
            .unwrap()
    }

    fn ok(&self, args: &[&str]) -> Output {
        let out = self.run(args);
        assert!(
            out.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        out
    }

    fn install(&self, name: &str) -> Output {
        self.ok(&["lang", "add", name, "--from", self.bundle.to_str().unwrap()])
    }

    fn names(&self) -> Vec<String> {
        let out = self.ok(&["--json", "symbols"]);
        if out.stdout.is_empty() {
            return Vec::new();
        }
        let data: Value = serde_json::from_slice(&out.stdout).unwrap();
        data.as_array()
            .unwrap()
            .iter()
            .map(|s| s["name"].as_str().unwrap().to_owned())
            .collect()
    }

    fn installed_dir(&self, name: &str) -> PathBuf {
        let record: Value = serde_json::from_slice(
            &fs::read(self.cache.join(format!("overrides/{name}.json"))).unwrap(),
        )
        .unwrap();
        Path::new(record["directory"].as_str().unwrap()).to_owned()
    }
}

#[test]
fn local_language_is_copied_and_query_changes_reindex_unchanged_files() {
    let f = Fixture::new("example", "example");
    // Build an index before the language exists; installing it must invalidate that index.
    assert!(f.names().is_empty());
    f.install("example");
    assert_eq!(f.names(), ["hello", "main"]);
    let list = f.ok(&["lang", "list"]);
    assert!(String::from_utf8_lossy(&list.stdout).contains("example         [override]"));
    assert!(String::from_utf8_lossy(&list.stdout).contains(f.bundle.to_str().unwrap()));
    let definition = f.ok(&["definition", "--name", "hello"]);
    assert!(String::from_utf8_lossy(&definition.stdout).contains("fn hello() {}"));
    let refs = f.ok(&["--json", "references", "--name", "hello", "--context"]);
    let refs: Value = serde_json::from_slice(&refs.stdout).unwrap();
    assert_eq!(refs.as_array().unwrap().len(), 2);
    f.ok(&["lang", "add", "example"]); // No attempt to download from the pack.

    fs::remove_dir_all(&f.bundle).unwrap();
    fs::write(f.installed_dir("example").join("symbols.scm"), TYPES).unwrap();
    assert_eq!(f.names(), ["Thing"]);
    f.ok(&["lang", "remove", "example", "--override"]);
    assert!(f.names().is_empty());
}

#[test]
fn replacing_and_removing_builtin_override_preserves_default_parser() {
    let f = Fixture::new("rust", "rs");
    let default = f.cache.join("grammars").join(&RUST_PARSER.1);
    fs::create_dir_all(default.parent().unwrap()).unwrap();
    fs::copy(&RUST_PARSER.0, &default).unwrap();
    assert_eq!(f.names(), ["Thing", "hello", "main"]);
    f.install("rust");
    assert_eq!(f.names(), ["hello", "main"]);
    assert!(!f.run(&["lang", "remove", "rust"]).status.success());

    fs::write(f.bundle.join("symbols.scm"), TYPES).unwrap();
    f.install("rust");
    assert_eq!(f.names(), ["Thing"]);
    fs::write(
        f.bundle.join("symbols.scm"),
        "(not_a_real_node) @name @definition.function",
    )
    .unwrap();
    let failed = f.run(&["lang", "add", "rust", "--from", f.bundle.to_str().unwrap()]);
    assert!(!failed.status.success());
    assert!(String::from_utf8_lossy(&failed.stderr).contains("override installation failed"));
    assert_eq!(f.names(), ["Thing"]); // Failed replacement left the working override intact.

    fs::write(
        f.installed_dir("rust").join("symbols.scm"),
        "(not_a_real_node)",
    )
    .unwrap();
    let broken = f.run(&["symbols"]);
    assert!(!broken.status.success());
    assert!(String::from_utf8_lossy(&broken.stderr).contains("invalid language override: rust"));
    assert!(broken.stdout.is_empty()); // No cached output or silent fallback.
    f.ok(&["lang", "remove", "rust", "--override"]);
    assert_eq!(f.names(), ["Thing", "hello", "main"]);
    assert!(default.is_file());
}

#[test]
fn overrides_index_new_files_without_default_parsers() {
    for (name, extension) in [("mojo", "mojo"), ("rust", "rs")] {
        let f = Fixture::new(name, extension);
        let source = f.project.join(format!("main.{extension}"));
        fs::remove_file(&source).unwrap();
        f.install(name);
        assert!(f.names().is_empty());
        fs::write(source, "fn first() {}\n").unwrap();
        assert_eq!(f.names(), ["first"]);
    }
}

#[test]
fn removing_an_override_restores_the_bundled_parser() {
    let f = Fixture::new("mojo", "mojo");
    fs::write(
        f.project.join("main.mojo"),
        "def answer():\n    return 42\n",
    )
    .unwrap();
    f.ok(&["lang", "add", "mojo"]);
    assert_eq!(f.names(), ["answer"]);
    // Override Mojo with the Rust fixture parser, which has no functions in this source.
    f.install("mojo");
    assert!(f.names().is_empty());
    f.ok(&["lang", "add", "mojo"]); // Preserve the explicit override.
    assert!(f.names().is_empty());
    f.ok(&["lang", "remove", "mojo", "--override"]);
    assert_eq!(f.names(), ["answer"]);
}

#[test]
fn invalid_local_parsers_and_metadata_are_rejected_before_activation() {
    let f = Fixture::new("example", "rs");
    let args = [
        "lang",
        "add",
        "example",
        "--from",
        f.bundle.to_str().unwrap(),
    ];
    let conflict = f.run(&args);
    assert!(!conflict.status.success());
    assert!(String::from_utf8_lossy(&conflict.stderr).contains("conflict with rust"));
    let path = f.bundle.join("language.json");
    let mut manifest: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    manifest["extensions"] = json!(["example"]);
    manifest["grammar"] = json!("missing_export");
    fs::write(&path, manifest.to_string()).unwrap();
    assert!(!f.run(&args).status.success());
    manifest["grammar"] = json!("shell");
    fs::write(&path, manifest.to_string()).unwrap();
    let literal_export = f.run(&args);
    assert!(!literal_export.status.success());
    assert!(String::from_utf8_lossy(&literal_export.stderr).contains("tree_sitter_shell"));
    manifest["grammar"] = json!("rust");
    manifest["ref_node_types"] = json!(["not_a_node"]);
    fs::write(&path, manifest.to_string()).unwrap();
    assert!(!f.run(&args).status.success());
    manifest["ref_node_types"] = json!(["identifier"]);
    fs::write(&path, manifest.to_string()).unwrap();
    fs::write(f.bundle.join("parser"), "not a shared library").unwrap();
    assert!(!f.run(&args).status.success());
    assert!(!f.cache.join("overrides/example.json").exists());
    assert!(
        !f.run(&[
            "lang",
            "add",
            "../escape",
            "--from",
            f.bundle.to_str().unwrap()
        ])
        .status
        .success()
    );
    assert!(
        !f.run(&["lang", "remove", "../escape", "--override"])
            .status
            .success()
    );
}

#[test]
fn concurrent_installs_cannot_activate_conflicting_extensions() {
    let f = Fixture::new("first", "custom");
    let second = f._temp.path().join("second");
    fs::create_dir(&second).unwrap();
    for file in ["parser", "symbols.scm"] {
        fs::copy(f.bundle.join(file), second.join(file)).unwrap();
    }
    let mut manifest: Value =
        serde_json::from_slice(&fs::read(f.bundle.join("language.json")).unwrap()).unwrap();
    manifest["name"] = json!("second");
    fs::write(second.join("language.json"), manifest.to_string()).unwrap();
    let spawn = |name: &str, bundle: &Path| {
        Command::new(env!("CARGO_BIN_EXE_cx"))
            .current_dir(&f.project)
            .env("CX_CACHE_DIR", &f.cache)
            .args(["lang", "add", name, "--from", bundle.to_str().unwrap()])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap()
    };
    let first = spawn("first", &f.bundle);
    let second = spawn("second", &second);
    let first = first.wait_with_output().unwrap();
    let second = second.wait_with_output().unwrap();
    assert_ne!(first.status.success(), second.status.success());
    let failed = if first.status.success() {
        &second
    } else {
        &first
    };
    assert!(String::from_utf8_lossy(&failed.stderr).contains("conflict with"));
    assert_eq!(f.names(), ["hello", "main"]);
}
