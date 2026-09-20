use serde_json::Value;
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

fn run(binary: &Path, project: &Path, cache: &Path, args: &[&str]) -> Output {
    let out = Command::new(binary)
        .current_dir(project)
        .env("CX_CACHE_DIR", cache)
        .env("HTTPS_PROXY", "http://127.0.0.1:1")
        .env("HTTP_PROXY", "http://127.0.0.1:1")
        .env("NO_PROXY", "")
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    out
}

#[test]
fn bundled_languages_work_from_a_standalone_binary_offline() {
    let temp = tempfile::tempdir().unwrap();
    let binary = temp
        .path()
        .join(format!("cx{}", std::env::consts::EXE_SUFFIX));
    fs::copy(env!("CARGO_BIN_EXE_cx"), &binary).unwrap();
    let cache = temp.path().join("cache");
    let project = temp.path().join("project");
    fs::create_dir_all(project.join(".git")).unwrap();
    // Cover both supported Mojo extensions.
    fs::write(project.join("main.mojo"), MOJO).unwrap();
    fs::write(project.join("extra.🔥"), "def flame():\n    pass\n").unwrap();
    fs::write(project.join("main.bend"), BEND).unwrap();
    let cx = |args: &[&str]| run(&binary, &project, &cache, args);
    let json = |args: &[&str]| -> Value {
        let out = cx(args);
        if out.stdout.is_empty() {
            return serde_json::json!([]);
        }
        serde_json::from_slice(&out.stdout).unwrap()
    };
    let assert_status =
        |name: &str, status: &str| {
            let out = cx(&["lang", "list"]);
            assert!(String::from_utf8_lossy(&out.stdout).lines().any(|line| {
                line.split_whitespace().collect::<Vec<_>>() == [name, status, "bundled"]
            }));
        };
    assert_status("mojo", "[missing]");
    assert_status("bend", "[missing]");
    // Seed an empty index so installation must invalidate it without source edits.
    assert_eq!(json(&["--json", "symbols"]), serde_json::json!([]));
    cx(&["lang", "add", "mojo", "bend"]);
    cx(&["lang", "add", "mojo", "bend"]); // Idempotent, still offline.
    assert_status("mojo", "[installed]");
    assert_status("bend", "[installed]");

    let symbols = json(&["--json", "symbols", "--file", "main.mojo"]);
    let mut names: Vec<_> = symbols
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["name"].as_str().unwrap())
        .collect();
    names.sort_unstable();
    assert_eq!(
        names,
        [
            "Countable",
            "Counter",
            "LIMIT",
            "Width",
            "answer",
            "count",
            "count",
            "main"
        ]
    );
    assert!(
        symbols
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["name"] == "Counter" && s["kind"] == "struct")
    );
    assert!(
        symbols
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["name"] == "Countable" && s["kind"] == "trait")
    );
    let body = json(&["--json", "definition", "--name", "Counter"]);
    assert!(body.to_string().contains("comptime Width = 32"));
    let refs = json(&["--json", "references", "--name", "answer", "--context"]);
    assert_eq!(refs.as_array().unwrap().len(), 2);
    assert_eq!(
        json(&["--json", "symbols", "--file", "extra.🔥"])[0]["name"],
        "flame"
    );

    let symbols = json(&["--json", "symbols", "--file", "main.bend"]);
    let mut names: Vec<_> = symbols
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["name"].as_str().unwrap())
        .collect();
    names.sort_unstable();
    assert_eq!(
        names,
        [
            "Circle",
            "Shape",
            "Shape.area",
            "Square",
            "add_zero",
            "add_zero",
            "main"
        ]
    );
    let refs = json(&["--json", "references", "--name", "Shape.area", "--context"]);
    assert_eq!(refs.as_array().unwrap().len(), 2);
    let body = json(&["--json", "definition", "--name", "add_zero", "--all"]);
    assert_eq!(body.as_array().unwrap().len(), 2);
    assert!(body.to_string().contains("law add_zero") && body.to_string().contains("def add_zero"));

    cx(&["lang", "remove", "mojo"]);
    assert_status("mojo", "[missing]");
    assert_eq!(json(&["--json", "symbols"]).as_array().unwrap().len(), 7);
    assert_eq!(
        json(&["--json", "symbols", "--file", "main.bend"])
            .as_array()
            .unwrap()
            .len(),
        7
    );
    cx(&["lang", "remove", "bend"]);
    assert_eq!(json(&["--json", "symbols"]), serde_json::json!([]));
    cx(&["lang", "remove", "bend"]); // Removing an absent language is harmless.
    cx(&["lang", "add", "mojo", "bend"]);
    assert_eq!(json(&["--json", "symbols"]).as_array().unwrap().len(), 16);
    assert!(!cache.join("overrides/mojo.json").exists());
    assert!(!cache.join("overrides/bend.json").exists());
}

const MOJO: &str = r#"comptime LIMIT = 8

trait Countable:
    def count(self) -> Int:
        ...

@fieldwise_init
struct Counter(Countable, Copyable, Movable):
    var value: Int
    comptime Width = 32

    def count(self) -> Int:
        return self.value

def `answer`() -> Int:
    comptime local_only = 42
    return local_only

def main() raises:
    var counter = Counter(7)
    print(counter.count(), `answer`())
"#;

const BEND: &str = r#"import Base

type Shape is Data:
  Circle{r: U32}
  Square{s: U32}

def Shape.area(x: Shape) -> U32:
  match x:
    case Circle{+r}:
      (3 * r * r : U32)
    case Square{+s}:
      (s * s : U32)

law add_zero:
  for x: Nat
  {Nat.add(x, 0n) == x : Nat}

def add_zero(x):
  match x:
    case 0n:
      {==}
    case 1n+p:
      %add_zero(p) : {1n+Nat.add(p, 0n) == 1n+_ : Nat}
      {==}

def main() -> U32:
  Shape.area(Square{5})
"#;
