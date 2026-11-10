use std::path::Path;
use std::process::Command;

fn main() {
    let head = Path::new("../../.git/HEAD");
    println!("cargo:rerun-if-changed={}", head.display());
    if let Ok(contents) = std::fs::read_to_string(head)
        && let Some(rest) = contents.trim().strip_prefix("ref: ")
    {
        let ref_path = Path::new("../../.git").join(rest.trim());
        if ref_path.exists() {
            println!("cargo:rerun-if-changed={}", ref_path.display());
        }
    }

    let commit = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|out| out.status.success())
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string());

    println!("cargo:rustc-env=LIVECRAFT_GIT_COMMIT={commit}");
}
