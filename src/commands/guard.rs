use colored::Colorize;

use crate::config::Config;
use crate::git;

use std::fs;
use std::path::Path;

pub fn run(config: &Config, expect_branch: Option<&str>) -> Result<(), String> {
    let trunk = &config.trunk;
    let branch = git::current_branch().unwrap_or_else(|_| "DETACHED".to_string());
    let root_str = git::repo_root()?;
    let root = Path::new(&root_str);
    let cwd = std::env::current_dir().map_err(|e| format!("can't get cwd: {e}"))?;

    // Detect session context
    let session_file = root.join(".gf-session");
    let in_session = session_file.exists();
    let session_branch = if in_session {
        read_session_branch(&session_file)
    } else {
        None
    };

    // Detect worktree vs main repo
    let common_dir_result = git::run_git(&["rev-parse", "--git-common-dir"]);
    let in_worktree = if common_dir_result.success {
        let common = Path::new(&common_dir_result.stdout);
        let common_parent = common.parent().unwrap_or(common);
        // If common dir parent != toplevel, we're in a worktree
        common_parent != root
    } else {
        false
    };

    // Dirty file count
    let status = git::run_git(&["status", "--porcelain"]);
    let dirty_count = if status.success && !status.stdout.is_empty() {
        status.stdout.lines().count()
    } else {
        0
    };

    // Drift
    let remote_trunk = format!("origin/{trunk}");
    let (ahead, behind) = git::commits_ahead_behind("HEAD", &remote_trunk);

    // Branch lock check
    let mut blocked = false;
    let mut warnings = Vec::new();

    if let Some(expected) = expect_branch {
        if branch != expected {
            blocked = true;
            warnings.push(format!(
                "WRONG BRANCH: expected '{expected}', on '{branch}'"
            ));
        }
    }

    if in_session {
        if let Some(ref sb) = session_branch {
            if branch != *sb {
                blocked = true;
                warnings.push(format!(
                    "SESSION MISMATCH: session expects branch '{sb}', on '{branch}'"
                ));
            }
        }
    }

    // On trunk when should be on feature branch (only warn, don't block)
    if branch == *trunk && !in_session && expect_branch.is_none() {
        warnings.push("on trunk — no active session".to_string());
    }

    // Dirty file guardrails
    let g = &config.guardrails;
    if dirty_count >= g.block_dirty_files {
        blocked = true;
        warnings.push(format!(
            "{dirty_count} uncommitted files (block limit: {})",
            g.block_dirty_files
        ));
    } else if dirty_count >= g.max_dirty_files {
        warnings.push(format!(
            "{dirty_count} uncommitted files (warn limit: {})",
            g.max_dirty_files
        ));
    }

    // Drift guardrails
    if behind >= g.sync_block_drift {
        blocked = true;
        warnings.push(format!(
            "{behind} commits behind {trunk} (block limit: {})",
            g.sync_block_drift
        ));
    } else if behind >= g.sync_reminder_drift {
        warnings.push(format!(
            "{behind} commits behind {trunk} (warn limit: {})",
            g.sync_reminder_drift
        ));
    }

    // Machine-readable output
    let status_word = if blocked {
        "BLOCK"
    } else if !warnings.is_empty() {
        "WARN"
    } else {
        "OK"
    };

    let wt_label = if in_worktree { root_str.as_str() } else { "none" };
    let session_label = if in_session { "active" } else { "none" };

    println!(
        "{} branch={} worktree={} session={} dirty={} ahead={} behind={}",
        status_word, branch, wt_label, session_label, dirty_count, ahead, behind
    );

    // Human-readable warnings
    for w in &warnings {
        if blocked {
            eprintln!("{}", format!("  BLOCK: {w}").red());
        } else {
            eprintln!("{}", format!("  WARN: {w}").yellow());
        }
    }

    if blocked {
        // Print cwd for debugging
        eprintln!("{}", format!("  cwd: {}", cwd.display()).dimmed());
        Err("guard check failed — see warnings above".to_string())
    } else {
        Ok(())
    }
}

fn read_session_branch(session_file: &Path) -> Option<String> {
    let data = fs::read_to_string(session_file).ok()?;
    for line in data.lines() {
        if let Some((key, value)) = line.split_once('=') {
            if key.trim() == "branch" {
                return Some(value.trim().trim_matches('"').to_string());
            }
        }
    }
    None
}
