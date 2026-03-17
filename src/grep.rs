use std::process::Command;

pub fn run(args: &[String]) -> i32 {
    // Try rg first, fall back to grep
    let program = if which_exists("rg") { "rg" } else { "grep" };

    match Command::new(program).args(args).status() {
        Ok(status) => status.code().unwrap_or(1),
        Err(e) => {
            eprintln!("cx grep: failed to execute {}: {}", program, e);
            1
        }
    }
}

fn which_exists(name: &str) -> bool {
    Command::new("which")
        .arg(name)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}
