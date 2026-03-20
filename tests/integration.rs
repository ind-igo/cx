use std::process::Command;
use std::io::Write;

fn cx() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_cx"));
    cmd.current_dir(env!("CARGO_MANIFEST_DIR"));
    cmd
}

/// Create a temporary directory with a fake git repo for isolated tests.
/// Returns the temp dir (dropped = cleaned up).
fn temp_project(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    // Create .git so cx finds project root
    std::fs::create_dir(dir.path().join(".git")).unwrap();
    for (path, content) in files {
        let full = dir.path().join(path);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        let mut f = std::fs::File::create(&full).unwrap();
        f.write_all(content.as_bytes()).unwrap();
    }
    dir
}

fn cx_in(dir: &std::path::Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_cx"));
    cmd.current_dir(dir);
    cmd
}

#[test]
fn overview_main_rs() {
    let out = cx().args(["overview", "src/main.rs"]).output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert!(stdout.contains("{name,kind,signature}:"), "should have TOON header: {stdout}");
    assert!(stdout.contains("main,fn,"));
    assert!(stdout.contains("resolve_root,fn,"));
}

#[test]
fn symbols_kind_fn() {
    let out = cx().args(["symbols", "--kind", "fn"]).output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success());
    assert!(stdout.contains("main,fn,"));
    assert!(stdout.contains("print_toon,fn,"));
}

#[test]
fn definition_main() {
    let out = cx().args(["definition", "--name", "main"]).output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success());
    assert!(stdout.contains("file: src/main.rs"), "{stdout}");
    assert!(stdout.contains("signature: fn main()"), "{stdout}");
    assert!(stdout.contains("Cli::parse()"), "{stdout}");
}

#[test]
fn overview_nonexistent_exits_1() {
    let out = cx().args(["overview", "nonexistent.rs"]).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("nonexistent.rs"), "stderr should mention the file: {stderr}");
}

#[test]
fn symbols_no_match_exits_2() {
    let out = cx().args(["symbols", "--name", "zzz_no_match"]).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn json_overview() {
    let out = cx().args(["--json", "overview", "src/main.rs"]).output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success());
    let parsed: serde_json::Value = serde_json::from_str(&stdout)
        .expect("should be valid JSON");
    assert!(parsed.is_array());
}

#[test]
fn json_definition_always_array() {
    let out = cx().args(["--json", "definition", "--name", "main"]).output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success());
    let parsed: serde_json::Value = serde_json::from_str(&stdout)
        .expect("should be valid JSON");
    // Always an array, even for single results (audit fix)
    assert!(parsed.is_array(), "definition JSON should always be an array: {stdout}");
    assert_eq!(parsed.as_array().unwrap().len(), 1);
}

// --- Definition --from and --max-lines tests ---

#[test]
fn definition_from_disambiguates() {
    let dir = temp_project(&[
        ("src/a.rs", "pub fn helper() { 1 }\n"),
        ("src/b.rs", "pub fn helper() { 2 }\n"),
    ]);

    // Without --from: should find both
    let out = cx_in(dir.path()).args(["--json", "definition", "--name", "helper"]).output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(parsed.as_array().unwrap().len(), 2, "should find both: {stdout}");

    // With --from: should find only one
    let out2 = cx_in(dir.path())
        .args(["--json", "definition", "--name", "helper", "--from", "src/a.rs"])
        .output()
        .unwrap();
    let stdout2 = String::from_utf8_lossy(&out2.stdout);
    let parsed2: serde_json::Value = serde_json::from_str(&stdout2).unwrap();
    let arr = parsed2.as_array().unwrap();
    assert_eq!(arr.len(), 1, "should find one: {stdout2}");
    assert_eq!(arr[0]["file"].as_str().unwrap(), "src/a.rs");
}

#[test]
fn definition_max_lines_truncates() {
    // Create a file with a long function
    let mut body = String::from("pub fn big() {\n");
    for i in 0..250 {
        body.push_str(&format!("    let x{i} = {i};\n"));
    }
    body.push_str("}\n");

    let dir = temp_project(&[("src/big.rs", &body)]);

    // Default max-lines (200) should truncate
    let out = cx_in(dir.path())
        .args(["--json", "definition", "--name", "big"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let item = &parsed.as_array().unwrap()[0];
    assert_eq!(item["truncated"].as_bool(), Some(true), "should be truncated: {stdout}");
    assert!(item["lines"].as_u64().unwrap() > 200, "should report total lines: {stdout}");
}

// --- JSON output for definition ---

#[test]
fn json_definition_has_expected_fields() {
    let out = cx().args(["--json", "definition", "--name", "main"]).output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let item = &parsed.as_array().unwrap()[0];
    assert!(item["file"].is_string());
    assert!(item["signature"].is_string());
    assert!(item["range"].is_array());
    assert!(item["body"].is_string());
}
