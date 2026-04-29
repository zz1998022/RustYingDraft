use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/refs/heads");
    println!("cargo:rerun-if-changed=../../.git/refs/tags");

    let version = git_version()
        .filter(|version| !version.is_empty())
        .unwrap_or_else(|| format!("v{}", env!("CARGO_PKG_VERSION")));

    println!("cargo:rustc-env=YINGDRAFT_VERSION={version}");
}

fn git_version() -> Option<String> {
    run_git(&["describe", "--tags", "--exact-match", "HEAD"])
        .or_else(|| run_git(&["describe", "--tags", "--always", "--dirty"]))
}

fn run_git(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }

    String::from_utf8(output.stdout)
        .ok()
        .map(|version| version.trim().to_string())
        .filter(|version| !version.is_empty())
}
