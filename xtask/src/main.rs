//! Repository automation. `cargo xtask ci` runs every check that CI runs.
//!
//! Subcommands:
//! - `ci`: format check, clippy, tests, release build, layering check, docs link check.
//! - `layering`: fail when a crate depends on a workspace crate its layer may not use.
//! - `docs`: fail when a Markdown file links to a local path that does not exist.

use std::ffi::OsString;
use std::path::Path;
use std::process::{Command, ExitCode};

/// Allowed normal dependencies on other workspace packages, per package.
/// Keep in sync with ARCHITECTURE.md; the 1.0 layout is in
/// plans/livecraft-1.0/reference/TARGET-ARCHITECTURE.md.
const LAYERS: &[(&str, &[&str])] = &[
    ("livecraft-model", &["livecraft-time"]),
    ("livecraft-formats", &["livecraft-model"]),
    ("livecraft-dsp", &[]),
    (
        "livecraft-devices",
        &["livecraft-model", "livecraft-dsp", "livecraft-time"],
    ),
    (
        "livecraft-engine",
        &[
            "livecraft-model",
            "livecraft-dsp",
            "livecraft-devices",
            "livecraft-formats",
            "livecraft-time",
        ],
    ),
    (
        "livecraft-session",
        &[
            "livecraft-time",
            "livecraft-model",
            "livecraft-formats",
            "livecraft-engine",
        ],
    ),
    (
        "livecraft-ui",
        &["livecraft-model", "livecraft-session", "livecraft-time"],
    ),
    (
        "livecraft",
        &[
            "livecraft-model",
            "livecraft-formats",
            "livecraft-engine",
            "livecraft-devices",
            "livecraft-session",
            "livecraft-ui",
        ],
    ),
    (
        "livecraft-testkit",
        &["livecraft-model", "livecraft-engine"],
    ),
    ("livecraft-time", &[]),
    ("xtask", &[]),
];

type Result<T = ()> = std::result::Result<T, String>;

fn main() -> ExitCode {
    let command = std::env::args().nth(1);
    let result = match command.as_deref() {
        Some("ci") => ci(std::env::args().any(|arg| arg == "--no-deny")),
        Some("layering") => layering(),
        Some("docs") => docs(),
        _ => Err("usage: cargo xtask <ci [--no-deny]|layering|docs>".to_string()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("xtask: {message}");
            ExitCode::FAILURE
        }
    }
}

fn ci(skip_deny: bool) -> Result {
    cargo(&["fmt", "--all", "--", "--check"])?;
    cargo(&[
        "clippy",
        "--workspace",
        "--all-targets",
        "--",
        "-D",
        "warnings",
    ])?;
    cargo(&["test", "--workspace"])?;
    cargo(&["build", "--release", "--workspace"])?;
    layering()?;
    docs()?;
    if skip_deny {
        println!("xtask: skipping cargo-deny (--no-deny)");
    } else {
        deny()?;
    }
    println!("xtask: all checks passed");
    Ok(())
}

fn cargo_program() -> OsString {
    std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"))
}

fn cargo(args: &[&str]) -> Result {
    println!("xtask: cargo {}", args.join(" "));
    let status = Command::new(cargo_program())
        .args(args)
        .status()
        .map_err(|e| format!("could not run cargo: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("`cargo {}` failed", args.join(" ")))
    }
}

fn layering() -> Result {
    let output = Command::new(cargo_program())
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .output()
        .map_err(|e| format!("could not run cargo metadata: {e}"))?;
    if !output.status.success() {
        return Err("cargo metadata failed".to_string());
    }
    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("cargo metadata output is not JSON: {e}"))?;
    let packages = metadata["packages"]
        .as_array()
        .ok_or("cargo metadata has no packages array")?;
    let workspace: Vec<&str> = packages.iter().filter_map(|p| p["name"].as_str()).collect();
    let mut errors = Vec::new();
    for package in packages {
        let name = package["name"].as_str().unwrap_or_default();
        let internal: Vec<&str> = package["dependencies"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or_default()
            .iter()
            .filter(|dep| dep["kind"].is_null())
            .filter_map(|dep| dep["name"].as_str())
            .filter(|dep| workspace.contains(dep))
            .collect();
        errors.extend(violations(name, &internal));
    }
    report("layering", errors)
}

/// Layering problems for one package given its normal workspace dependencies.
fn violations(package: &str, internal_deps: &[&str]) -> Vec<String> {
    let Some((_, allowed)) = LAYERS.iter().find(|(name, _)| *name == package) else {
        return vec![format!(
            "package `{package}` has no entry in LAYERS in xtask/src/main.rs"
        )];
    };
    internal_deps
        .iter()
        .filter(|dep| !allowed.contains(dep))
        .map(|dep| format!("`{package}` must not depend on `{dep}`"))
        .collect()
}

fn docs() -> Result {
    let output = Command::new("git")
        .args([
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "*.md",
        ])
        .output()
        .map_err(|e| format!("could not run git: {e}"))?;
    if !output.status.success() {
        return Err("git ls-files failed".to_string());
    }
    let mut errors = Vec::new();
    for file in String::from_utf8_lossy(&output.stdout).lines() {
        let path = Path::new(file);
        if !path.exists() {
            continue;
        }
        let text = std::fs::read_to_string(path).map_err(|e| format!("{file}: {e}"))?;
        let base = path.parent().unwrap_or_else(|| Path::new(""));
        for link in markdown_links(&text) {
            if let Some(target) = local_target(&link)
                && !base.join(target).exists()
            {
                errors.push(format!("{file}: broken link `{link}`"));
            }
        }
    }
    report("docs", errors)
}

fn report(check: &str, errors: Vec<String>) -> Result {
    if errors.is_empty() {
        println!("xtask: {check} check passed");
        return Ok(());
    }
    for error in &errors {
        eprintln!("  {error}");
    }
    Err(format!("{check} check found {} problem(s)", errors.len()))
}

/// Targets of inline Markdown links `[text](target)`, skipping fenced code blocks.
fn markdown_links(text: &str) -> Vec<String> {
    let mut links = Vec::new();
    let mut fence: Option<usize> = None;
    for line in text.lines() {
        let trimmed = line.trim_start();
        let ticks = trimmed.chars().take_while(|c| *c == '`').count();
        if ticks >= 3 {
            match fence {
                None => fence = Some(ticks),
                Some(open) if ticks >= open && trimmed[ticks..].trim().is_empty() => fence = None,
                Some(_) => {}
            }
            continue;
        }
        if fence.is_some() {
            continue;
        }
        let mut rest = line;
        while let Some(start) = rest.find("](") {
            let after = &rest[start + 2..];
            let Some(end) = after.find(')') else {
                break;
            };
            links.push(after[..end].to_string());
            rest = &after[end + 1..];
        }
    }
    links
}

/// The local file part of a link, or `None` for URLs, mail links, and same-file anchors.
fn local_target(link: &str) -> Option<&str> {
    if link.contains("://") || link.starts_with("mailto:") || link.starts_with('#') {
        return None;
    }
    let path = link.split('#').next().unwrap_or_default();
    (!path.is_empty()).then_some(path)
}

/// Runs `cargo-deny check` with the repo-local binary in `./.tools/bin` and `deny.toml`.
fn deny() -> Result {
    let exe = std::env::current_dir()
        .map_err(|e| format!("no current directory: {e}"))?
        .join(".tools")
        .join("bin")
        .join(if cfg!(windows) {
            "cargo-deny.exe"
        } else {
            "cargo-deny"
        });
    if !exe.exists() {
        return Err(format!(
            "{} is missing; install it with `cargo install cargo-deny@0.20.2 --locked --root .tools`, or run `cargo xtask ci --no-deny`",
            exe.display()
        ));
    }
    println!("xtask: cargo-deny check");
    let status = Command::new(&exe)
        .arg("check")
        .status()
        .map_err(|e| format!("could not run cargo-deny: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("cargo-deny check failed".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layering_accepts_allowed_edges() {
        assert!(
            violations(
                "livecraft-ui",
                &["livecraft-model", "livecraft-session", "livecraft-time"]
            )
            .is_empty()
        );
    }

    #[test]
    fn layering_rejects_the_ui_reaching_the_engine() {
        assert!(!violations("livecraft-ui", &["livecraft-engine"]).is_empty());
    }

    #[test]
    fn layering_rejects_reverse_edge() {
        assert_eq!(
            violations("livecraft-model", &["livecraft-ui"]),
            vec!["`livecraft-model` must not depend on `livecraft-ui`".to_string()]
        );
    }

    #[test]
    fn layering_rejects_unknown_package() {
        assert_eq!(violations("livecraft-new", &[]).len(), 1);
    }

    #[test]
    fn links_skip_fenced_code_urls_and_anchors() {
        let text = "[a](x.md) [b](https://e.com) [c](#top)\n````md\n```rust\n[d](y.md)\n```\n````\n[e](z.md#part)";
        let links = markdown_links(text);
        assert_eq!(links, vec!["x.md", "https://e.com", "#top", "z.md#part"]);
        let local: Vec<&str> = links.iter().filter_map(|l| local_target(l)).collect();
        assert_eq!(local, vec!["x.md", "z.md"]);
    }
}
