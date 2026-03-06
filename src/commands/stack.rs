use colored::Colorize;

use crate::config::Config;
use crate::git;

use std::fs;
use std::path::Path;

fn stack_file(repo_root: &Path) -> std::path::PathBuf {
    repo_root.join(".gf-stack")
}

fn read_stack(repo_root: &Path) -> Vec<String> {
    let path = stack_file(repo_root);
    if !path.exists() {
        return Vec::new();
    }
    match fs::read_to_string(&path) {
        Ok(contents) => contents
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.trim().to_string())
            .collect(),
        Err(_) => Vec::new(),
    }
}

fn write_stack(repo_root: &Path, stack: &[String]) -> Result<(), String> {
    let path = stack_file(repo_root);
    let contents = stack.join("\n") + "\n";
    fs::write(&path, contents).map_err(|e| format!("failed to write .gf-stack: {e}"))
}

pub fn create(config: &Config, message: &str, repo_root: &Path) -> Result<(), String> {
    let trunk = &config.trunk;

    // Must be on trunk or on the top of the stack
    let current = git::current_branch()?;
    let mut stack = read_stack(repo_root);

    let base = if stack.is_empty() {
        if current != *trunk {
            return Err(format!(
                "must be on {trunk} to start a stack (currently on {current})"
            ));
        }
        trunk.clone()
    } else {
        let top = stack.last().unwrap().clone();
        if current != top {
            return Err(format!(
                "must be on stack top '{top}' to add to stack (currently on {current})"
            ));
        }
        top
    };

    // Check for changes
    if !git::has_uncommitted_changes() {
        return Err("nothing to stack — no uncommitted changes".to_string());
    }

    // Generate branch name
    let slug = git::slugify(message);
    let branch_name = if git::branch_exists(&slug) {
        let hash = git::short_hash();
        format!("{slug}-{hash}")
    } else {
        slug
    };

    // Stage all, create branch, commit
    let add = git::run_git(&["add", "-A"]);
    if !add.success {
        return Err(format!("git add failed: {}", add.stderr));
    }

    let create = git::run_git(&["checkout", "-b", &branch_name]);
    if !create.success {
        return Err(format!("failed to create branch: {}", create.stderr));
    }

    let commit = git::run_git(&["commit", "-m", message]);
    if !commit.success {
        return Err(format!("commit failed: {}", commit.stderr));
    }

    // Add to stack
    stack.push(branch_name.clone());
    write_stack(repo_root, &stack)?;

    println!(
        "{}",
        format!("stacked '{branch_name}' on {base} ({} in stack)", stack.len()).green()
    );
    Ok(())
}

pub fn list(config: &Config, repo_root: &Path) -> Result<(), String> {
    let trunk = &config.trunk;
    let stack = read_stack(repo_root);

    if stack.is_empty() {
        println!("no active stack");
        return Ok(());
    }

    println!("{}", format!("stack ({} branches):", stack.len()).bold());
    let remote_trunk = format!("origin/{trunk}");

    for (i, branch) in stack.iter().enumerate() {
        let is_current = git::current_branch()
            .map(|b| b == *branch)
            .unwrap_or(false);

        let (ahead, _) = git::commits_ahead_behind(branch, &remote_trunk);
        let marker = if is_current { " *" } else { "  " };
        let base = if i == 0 {
            trunk.clone()
        } else {
            stack[i - 1].clone()
        };

        let branch_display = if is_current {
            branch.green().bold().to_string()
        } else {
            branch.cyan().to_string()
        };

        println!(
            "{marker} {} → {} (+{ahead} commits)",
            base.dimmed(),
            branch_display
        );
    }

    Ok(())
}

pub fn ship(config: &Config, all: bool, draft: bool, no_pr: bool, repo_root: &Path) -> Result<(), String> {
    let trunk = &config.trunk;
    let mut stack = read_stack(repo_root);

    if stack.is_empty() {
        return Err("no active stack".to_string());
    }

    if all {
        // Ship each branch independently based on trunk
        let current = git::current_branch()?;

        for branch in &stack {
            // Checkout trunk, cherry-pick the branch's commits, push
            git::run_git(&["checkout", trunk]);
            git::run_git(&["checkout", "-b", &format!("{branch}-pr")]);

            let cherry = git::run_git(&["cherry-pick", &format!("{trunk}..{branch}")]);
            if !cherry.success {
                git::run_git(&["cherry-pick", "--abort"]);
                git::run_git(&["checkout", &current]);
                return Err(format!(
                    "cherry-pick failed for {branch}: {}",
                    cherry.stderr
                ));
            }

            let pr_branch = format!("{branch}-pr");
            let push = git::run_git(&["push", "-u", "origin", &pr_branch]);
            if !push.success {
                eprintln!(
                    "{}",
                    format!("push failed for {pr_branch}: {}", push.stderr).yellow()
                );
                git::run_git(&["checkout", &current]);
                continue;
            }

            if !no_pr {
                open_pr(config, &pr_branch, draft);
            }

            println!("{}", format!("shipped {branch} as {pr_branch}").green());
        }

        // Clean up stack
        git::run_git(&["checkout", trunk]);
        stack.clear();
        write_stack(repo_root, &stack)?;
        println!("{}", "stack cleared — all branches shipped independently".green());
    } else {
        // Ship bottom of stack as PR, rebase rest onto trunk
        let bottom = stack.remove(0);

        // Push the bottom branch
        let push = git::run_git(&["push", "-u", "origin", &bottom]);
        if !push.success {
            stack.insert(0, bottom);
            write_stack(repo_root, &stack)?;
            return Err(format!("push failed: {}", push.stderr));
        }

        if !no_pr {
            open_pr(config, &bottom, draft);
        }

        println!("{}", format!("shipped '{bottom}'").green());

        // Rebase remaining stack onto trunk
        if !stack.is_empty() {
            eprintln!("{}", "rebasing remaining stack onto trunk...".dimmed());

            // Fetch latest trunk
            git::run_git(&["fetch", "origin"]);
            let remote_trunk = format!("origin/{trunk}");

            for branch in &stack {
                git::run_git(&["checkout", branch]);
                let rebase = git::run_git(&["rebase", &remote_trunk]);
                if !rebase.success {
                    git::run_git(&["rebase", "--abort"]);
                    eprintln!(
                        "{}",
                        format!("warning: rebase of {branch} onto {trunk} had conflicts — resolve manually").yellow()
                    );
                }
            }

            // Stay on top of stack
            let top = stack.last().unwrap();
            git::run_git(&["checkout", top]);
            println!(
                "{}",
                format!("{} branches remaining in stack", stack.len()).dimmed()
            );
        } else {
            git::run_git(&["checkout", trunk]);
            println!("{}", "stack empty — back on trunk".green());
        }

        write_stack(repo_root, &stack)?;
    }

    Ok(())
}

fn open_pr(config: &Config, branch: &str, draft: bool) {
    if git::gh_available() {
        let mut pr_args = vec![
            "pr", "create", "--base", &config.trunk, "--fill", "--head", branch,
        ];
        if draft {
            pr_args.push("--draft");
        }
        let pr = git::run_gh(&pr_args);
        if pr.success {
            println!("{}", format!("PR: {}", pr.stdout).green());
        } else {
            eprintln!(
                "{}",
                format!("PR creation failed: {}", pr.stderr).yellow()
            );
        }
    } else {
        println!(
            "{}",
            format!(
                "gh not installed — create PR manually:\n  gh pr create --base {} --fill --head {branch}",
                config.trunk
            )
            .yellow()
        );
    }
}
