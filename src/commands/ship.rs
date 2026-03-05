use colored::Colorize;

use crate::commands::sync;
use crate::config::Config;
use crate::git;

pub struct ShipOpts {
    pub message: String,
    pub no_sync: bool,
    pub draft: bool,
    pub no_pr: bool,
    pub branch: Option<String>,
    pub staged: bool,
    pub all: bool,
}

pub fn run(config: &Config, opts: ShipOpts) -> Result<(), String> {
    // Auto-sync first unless --no-sync
    if !opts.no_sync {
        if let Err(e) = sync::run(config) {
            return Err(format!("auto-sync failed: {e}\nUse --no-sync to skip."));
        }
    }

    // Check for changes
    if !git::has_uncommitted_changes() {
        return Err("nothing to ship — no uncommitted changes".to_string());
    }

    // Determine what to stage
    let has_staged = git::has_staged_changes();
    if opts.all || !has_staged {
        // Stage everything
        let add = git::run_git(&["add", "-A"]);
        if !add.success {
            return Err(format!("git add failed: {}", add.stderr));
        }
    } else if !opts.staged && has_staged {
        // Partial stage exists — ship only staged (default behavior)
        println!(
            "{}",
            "shipping staged changes only (use --all to include everything)".dimmed()
        );
    }

    // Generate branch name
    let branch_name = if let Some(ref name) = opts.branch {
        name.clone()
    } else {
        let slug = git::slugify(&opts.message);
        if git::branch_exists(&slug) {
            let hash = git::short_hash();
            format!("{slug}-{hash}")
        } else {
            slug
        }
    };

    // Create branch, commit, push
    let create = git::run_git(&["checkout", "-b", &branch_name]);
    if !create.success {
        return Err(format!("failed to create branch: {}", create.stderr));
    }

    let commit = git::run_git(&["commit", "-m", &opts.message]);
    if !commit.success {
        return Err(format!("commit failed: {}", commit.stderr));
    }

    let push = git::run_git(&["push", "-u", "origin", &branch_name]);
    if !push.success {
        return Err(format!("push failed: {}", push.stderr));
    }

    // Open PR
    if !opts.no_pr {
        if git::gh_available() {
            let mut pr_args = vec!["pr", "create", "--base", &config.trunk, "--fill"];
            if opts.draft {
                pr_args.push("--draft");
            }
            let pr = git::run_gh(&pr_args);
            if pr.success {
                println!("{}", format!("PR: {}", pr.stdout).green());
            } else {
                eprintln!("{}", format!("PR creation failed: {}", pr.stderr).yellow());
            }
        } else {
            println!(
                "{}",
                format!(
                    "gh not installed — create PR manually:\n  gh pr create --base {} --fill",
                    config.trunk
                )
                .yellow()
            );
        }
    }

    // Switch back to trunk
    let trunk = &config.trunk;
    let checkout = git::run_git(&["checkout", trunk]);
    if !checkout.success {
        eprintln!(
            "{}",
            format!("warning: couldn't switch back to {trunk}: {}", checkout.stderr).yellow()
        );
    }

    println!(
        "{}",
        format!("shipped {branch_name} — back on {trunk}").green()
    );
    Ok(())
}
