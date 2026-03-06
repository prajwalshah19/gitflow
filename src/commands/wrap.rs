use colored::Colorize;

use crate::commands::worktree;
use crate::config::Config;
use crate::git;

use std::fs;
use std::path::Path;
use std::process::Command;

pub fn begin(config: &Config, description: &str, repo_root: &Path) -> Result<(), String> {
    let trunk = &config.trunk;

    // Must be on trunk
    let current = git::current_branch()?;
    if current != *trunk {
        return Err(format!(
            "must be on {trunk} to begin a session (currently on {current})"
        ));
    }

    // Auto-sync trunk first
    eprintln!("{}", "syncing trunk before session...".dimmed());
    let fetch = git::run_git(&["fetch", "origin"]);
    if !fetch.success {
        eprintln!(
            "{}",
            format!("warning: fetch failed: {}", fetch.stderr).yellow()
        );
    }
    let remote_trunk = format!("origin/{trunk}");
    let rebase = git::run_git(&["rebase", &remote_trunk]);
    if !rebase.success {
        git::run_git(&["rebase", "--abort"]);
        return Err(format!(
            "trunk sync failed — resolve conflicts first"
        ));
    }

    // Create worktree
    let slug = git::slugify(description);
    let name = if git::branch_exists(&slug) {
        let hash = git::short_hash();
        format!("{slug}-{hash}")
    } else {
        slug
    };

    worktree::create(config, &name, repo_root)?;

    // Write session marker
    let wt_dir = config.resolve_wt_dir(repo_root);
    let wt_path = wt_dir.join(&name);
    let session_file = wt_path.join(".gf-session");
    let session_data = format!(
        "description = \"{}\"\nstarted = \"{}\"\nbranch = \"{}\"\n",
        description,
        chrono_now(),
        name,
    );
    fs::write(&session_file, &session_data)
        .map_err(|e| format!("failed to write session file: {e}"))?;

    println!("\n{}", format!("session started: {description}").green().bold());
    println!("  cd {}", wt_path.display());
    Ok(())
}

pub fn end(config: &Config, repo_root: &Path) -> Result<(), String> {
    // Detect current worktree
    let root_str = git::repo_root()?;
    let current_root = Path::new(&root_str);

    // Read session file
    let session_file = current_root.join(".gf-session");
    if !session_file.exists() {
        return Err(
            "no active session — run `gf wrap begin` first, or you're not in a session worktree"
                .to_string(),
        );
    }

    let session_data = fs::read_to_string(&session_file)
        .map_err(|e| format!("failed to read session file: {e}"))?;

    let mut description = String::new();
    let mut branch = String::new();
    for line in session_data.lines() {
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let value = value.trim().trim_matches('"');
            match key {
                "description" => description = value.to_string(),
                "branch" => branch = value.to_string(),
                _ => {}
            }
        }
    }

    if branch.is_empty() {
        return Err("corrupt session file — missing branch name".to_string());
    }

    // Run pre-end check if configured
    if let Some(ref pre_end) = config.wrap.pre_end {
        eprintln!("{}", format!("running pre-end check: {pre_end}").dimmed());
        let output = Command::new("sh")
            .arg("-c")
            .arg(pre_end)
            .current_dir(current_root)
            .output()
            .map_err(|e| format!("failed to run pre-end command: {e}"))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!(
                "pre-end check failed:\n{stderr}\nFix the issues before ending the session."
            ));
        }
        println!("{}", "pre-end check passed".green());
    }

    // Check for changes to commit
    if git::has_uncommitted_changes() {
        let add = git::run_git(&["add", "-A"]);
        if !add.success {
            return Err(format!("git add failed: {}", add.stderr));
        }

        let commit_msg = if description.is_empty() {
            "session work".to_string()
        } else {
            description.clone()
        };

        let commit = git::run_git(&["commit", "-m", &commit_msg]);
        if !commit.success {
            return Err(format!("commit failed: {}", commit.stderr));
        }
    }

    // Remove session file
    let _ = fs::remove_file(&session_file);

    // Ship the worktree
    worktree::ship_wt(config, &branch, false, false, false, repo_root)?;

    // Remove the worktree
    worktree::remove_wt(config, &branch, true, false, repo_root)?;

    println!(
        "{}",
        format!("session complete: {description}").green().bold()
    );
    Ok(())
}

fn chrono_now() -> String {
    // Simple timestamp without adding chrono dependency
    let output = std::process::Command::new("date")
        .arg("+%Y-%m-%dT%H:%M:%S")
        .output();
    match output {
        Ok(o) if o.status.success() => {
            String::from_utf8_lossy(&o.stdout).trim().to_string()
        }
        _ => "unknown".to_string(),
    }
}
