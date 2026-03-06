use colored::Colorize;

use crate::commands::stack;
use crate::commands::worktree;
use crate::config::Config;
use crate::git;

use std::path::Path;

pub fn run(config: &Config, repo_root: &Path, quiet: bool) -> Result<(), String> {
    let trunk = &config.trunk;
    let remote_trunk = format!("origin/{trunk}");
    let g = &config.guardrails;

    let mut issues = 0;

    // Trunk drift
    git::run_git(&["fetch", "origin"]);
    let (ahead, behind) = git::commits_ahead_behind("HEAD", &remote_trunk);

    if !quiet {
        let drift_status = if behind == 0 {
            "up to date".green().to_string()
        } else if behind >= g.sync_block_drift {
            issues += 1;
            format!("{behind} behind").red().to_string()
        } else if behind >= g.sync_reminder_drift {
            issues += 1;
            format!("{behind} behind").yellow().to_string()
        } else {
            format!("{behind} behind").to_string()
        };
        println!(
            "trunk:      {} ({}, {ahead} ahead, {drift_status})",
            trunk.bold(),
            git::current_branch().unwrap_or_else(|_| "detached".to_string())
        );
    } else if behind >= g.sync_reminder_drift {
        issues += 1;
        eprintln!(
            "{}",
            format!("warning: {behind} commits behind {trunk} — run `gf sync`").yellow()
        );
    }

    // Dirty files
    let status = git::run_git(&["status", "--porcelain"]);
    let dirty_count = if status.success && !status.stdout.is_empty() {
        status.stdout.lines().count()
    } else {
        0
    };

    if !quiet {
        let dirty_status = if dirty_count == 0 {
            "clean".green().to_string()
        } else if dirty_count >= g.block_dirty_files {
            issues += 1;
            format!("{dirty_count} files").red().to_string()
        } else if dirty_count >= g.max_dirty_files {
            issues += 1;
            format!("{dirty_count} files").yellow().to_string()
        } else {
            format!("{dirty_count} files").to_string()
        };
        println!("working dir: {dirty_status}");
    } else if dirty_count >= g.max_dirty_files {
        issues += 1;
        eprintln!(
            "{}",
            format!("warning: {dirty_count} uncommitted files").yellow()
        );
    }

    // Worktrees
    if !quiet {
        println!("\n{}", "worktrees:".bold());
        if let Err(e) = worktree::list_wt(config, repo_root) {
            eprintln!("  {}", format!("error: {e}").yellow());
        }
    }

    // Stack
    if !quiet {
        println!("\n{}", "stack:".bold());
        if let Err(e) = stack::list(config, repo_root) {
            eprintln!("  {}", format!("error: {e}").yellow());
        }
    }

    // Guardrails config
    if !quiet {
        println!("\n{}", "guardrails:".bold());
        print_guardrail("auto-fetch", &format!("every {} commits", g.auto_fetch_every), true);
        print_guardrail(
            "drift",
            &if behind >= g.sync_reminder_drift {
                format!("{behind} behind trunk (warn: {}, block: {})", g.sync_reminder_drift, g.sync_block_drift)
            } else {
                format!("ok (warn: {}, block: {})", g.sync_reminder_drift, g.sync_block_drift)
            },
            behind < g.sync_reminder_drift,
        );
        print_guardrail(
            "cleanliness",
            &if dirty_count >= g.max_dirty_files {
                format!("{dirty_count} dirty (warn: {}, block: {})", g.max_dirty_files, g.block_dirty_files)
            } else {
                format!("ok (warn: {}, block: {})", g.max_dirty_files, g.block_dirty_files)
            },
            dirty_count < g.max_dirty_files,
        );
    }

    // Summary
    if !quiet {
        println!();
        if issues == 0 {
            println!("{}", "all clear".green().bold());
        } else {
            println!(
                "{}",
                format!("{issues} issue(s) need attention").yellow().bold()
            );
        }
    }

    Ok(())
}

fn print_guardrail(name: &str, detail: &str, ok: bool) {
    let check = if ok {
        "✓".green().to_string()
    } else {
        "✗".red().to_string()
    };
    println!("  {check} {name}: {detail}");
}
