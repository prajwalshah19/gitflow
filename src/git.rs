use colored::Colorize;
use std::path::Path;
use std::process::Command;

pub struct CmdResult {
    pub stdout: String,
    pub stderr: String,
    pub success: bool,
}

pub fn run_git(args: &[&str]) -> CmdResult {
    run_git_in(args, None)
}

pub fn run_git_in(args: &[&str], dir: Option<&Path>) -> CmdResult {
    let display_args: Vec<&str> = args.to_vec();
    eprintln!("{}", format!("→ git {}", display_args.join(" ")).dimmed());

    let mut cmd = Command::new("git");
    cmd.args(args);
    if let Some(d) = dir {
        cmd.current_dir(d);
    }
    match cmd.output() {
        Ok(output) => CmdResult {
            stdout: String::from_utf8_lossy(&output.stdout).trim().to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            success: output.status.success(),
        },
        Err(e) => CmdResult {
            stdout: String::new(),
            stderr: format!("failed to run git: {e}"),
            success: false,
        },
    }
}

pub fn run_gh(args: &[&str]) -> CmdResult {
    eprintln!("{}", format!("→ gh {}", args.join(" ")).dimmed());

    match Command::new("gh").args(args).output() {
        Ok(output) => CmdResult {
            stdout: String::from_utf8_lossy(&output.stdout).trim().to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            success: output.status.success(),
        },
        Err(_) => CmdResult {
            stdout: String::new(),
            stderr: "gh is not installed".to_string(),
            success: false,
        },
    }
}

pub fn gh_available() -> bool {
    Command::new("gh")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn repo_root() -> Result<String, String> {
    let result = run_git(&["rev-parse", "--show-toplevel"]);
    if result.success {
        Ok(result.stdout)
    } else {
        Err("not inside a git repository".to_string())
    }
}

pub fn current_branch() -> Result<String, String> {
    let result = run_git(&["symbolic-ref", "--short", "HEAD"]);
    if result.success {
        Ok(result.stdout)
    } else {
        Err("detached HEAD state — checkout a branch first".to_string())
    }
}

pub fn has_uncommitted_changes() -> bool {
    let result = run_git(&["status", "--porcelain"]);
    result.success && !result.stdout.is_empty()
}

pub fn has_staged_changes() -> bool {
    let result = run_git(&["diff", "--cached", "--quiet"]);
    !result.success
}

pub fn commits_ahead_behind(branch: &str, trunk: &str) -> (usize, usize) {
    let result = run_git(&[
        "rev-list",
        "--left-right",
        "--count",
        &format!("{branch}...{trunk}"),
    ]);
    if !result.success {
        return (0, 0);
    }
    let parts: Vec<&str> = result.stdout.split_whitespace().collect();
    if parts.len() == 2 {
        let ahead = parts[0].parse().unwrap_or(0);
        let behind = parts[1].parse().unwrap_or(0);
        (ahead, behind)
    } else {
        (0, 0)
    }
}

pub fn branch_exists(name: &str) -> bool {
    run_git(&["rev-parse", "--verify", name]).success
}

pub fn slugify(message: &str) -> String {
    let slug: String = message
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();
    // collapse multiple dashes and trim
    let mut result = String::new();
    let mut prev_dash = false;
    for c in slug.chars() {
        if c == '-' {
            if !prev_dash && !result.is_empty() {
                result.push('-');
            }
            prev_dash = true;
        } else {
            result.push(c);
            prev_dash = false;
        }
    }
    result.trim_end_matches('-').to_string()
}

pub fn short_hash() -> String {
    let result = run_git(&["rev-parse", "--short", "HEAD"]);
    if result.success {
        result.stdout
    } else {
        "0000".to_string()
    }
}
