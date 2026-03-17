use std::process::Command;

fn cx() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_cx"));
    cmd.current_dir(env!("CARGO_MANIFEST_DIR"));
    cmd
}

#[test]
fn overview_main_rs() {
    let out = cx().args(["overview", "src/main.rs"]).output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert!(stdout.contains("symbols["));
    assert!(stdout.contains("main,fn,"));
    assert!(stdout.contains("resolve_root,fn,"));
}

#[test]
fn symbols_kind_fn() {
    let out = cx().args(["symbols", "--kind", "fn"]).output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success());
    assert!(stdout.contains("main,fn,"));
    assert!(stdout.contains("run,fn,"));
    assert!(stdout.contains("toon_table,fn,"));
}

#[test]
fn definition_main() {
    let out = cx().args(["definition", "--name", "main"]).output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success());
    assert!(stdout.contains("file: src/main.rs"));
    assert!(stdout.contains("signature: fn main()"));
    assert!(stdout.contains("body: |"));
    assert!(stdout.contains("Cli::parse()"));
}

#[test]
fn read_returns_content() {
    let out = cx().args(["read", "src/main.rs", "--fresh"]).output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success());
    assert!(stdout.contains("mod index;"));
    assert!(stdout.contains("fn main()"));
}

#[test]
fn grep_finds_matches() {
    let out = cx().args(["grep", "-rn", "fn main", "src/"]).output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success());
    assert!(stdout.contains("src/main.rs"));
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
