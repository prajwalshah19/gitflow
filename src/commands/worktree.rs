use colored::Colorize;

use crate::config::Config;
use crate::git;

use std::path::Path;

pub fn create(config: &Config, name: &str, repo_root: &Path) -> Result<(), String> {
    let trunk = &config.trunk;
    let wt_dir = config.resolve_wt_dir(repo_root);
    let wt_path = wt_dir.join(name);

    // Fetch latest
    let fetch = git::run_git(&["fetch", "origin"]);
    if !fetch.success {
        return Err(format!("fetch failed: {}", fetch.stderr));
    }

    // Check branch doesn't already exist
    if git::branch_exists(name) {
        return Err(format!("branch '{name}' already exists"));
    }

    // Check worktree path doesn't exist
    if wt_path.exists() {
        return Err(format!("worktree directory already exists: {}", wt_path.display()));
    }

    // Create parent directory
    if let Some(parent) = wt_path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("failed to create directory: {e}"))?;
        }
    }

    let wt_path_str = wt_path.to_string_lossy();
    let remote_trunk = format!("origin/{trunk}");
    let result = git::run_git(&["worktree", "add", &wt_path_str, "-b", name, &remote_trunk]);
    if !result.success {
        return Err(format!("worktree creation failed: {}", result.stderr));
    }

    println!("{}", format!("created worktree at {wt_path_str}").green());
    println!("  cd {wt_path_str}");
    Ok(())
}

pub fn sync_wt(config: &Config, name: Option<&str>, repo_root: &Path) -> Result<(), String> {
    let trunk = &config.trunk;
    let remote_trunk = format!("origin/{trunk}");

    let wt_path = if let Some(n) = name {
        let wt_dir = config.resolve_wt_dir(repo_root);
        let p = wt_dir.join(n);
        if !p.exists() {
            return Err(format!(
                "worktree '{n}' not found — try `flow wt list`"
            ));
        }
        p
    } else {
        // Try to detect if we're inside a worktree
        let result = git::run_git(&["rev-parse", "--show-toplevel"]);
        if !result.success {
            return Err("not inside a git worktree — specify a name or cd into one".to_string());
        }
        let toplevel = Path::new(&result.stdout).to_path_buf();
        // Check this is actually a worktree (not the main repo)
        let common_dir = git::run_git(&["rev-parse", "--git-common-dir"]).stdout;
        let main_root = Path::new(&common_dir);
        let main_toplevel = main_root.parent().unwrap_or(main_root);
        if toplevel == main_toplevel {
            return Err("you're in the main repo — specify a worktree name or cd into one".to_string());
        }
        toplevel
    };

    let fetch = git::run_git_in(&["fetch", "origin"], Some(&wt_path));
    if !fetch.success {
        return Err(format!("fetch failed: {}", fetch.stderr));
    }

    let rebase = git::run_git_in(&["rebase", &remote_trunk], Some(&wt_path));
    if !rebase.success {
        git::run_git_in(&["rebase", "--abort"], Some(&wt_path));
        return Err(format!(
            "rebase conflict in worktree at {}.\n\
             Resolve manually:\n  \
             cd {} && git rebase {remote_trunk}",
            wt_path.display(),
            wt_path.display()
        ));
    }

    println!(
        "{}",
        format!("worktree at {} synced with {trunk}", wt_path.display()).green()
    );
    Ok(())
}

pub fn ship_wt(
    config: &Config,
    name: &str,
    draft: bool,
    remove: bool,
    no_pr: bool,
    repo_root: &Path,
) -> Result<(), String> {
    let trunk = &config.trunk;
    let wt_dir = config.resolve_wt_dir(repo_root);
    let wt_path = wt_dir.join(name);

    if !wt_path.exists() {
        return Err(format!("worktree '{name}' not found — try `flow wt list`"));
    }

    // Check for uncommitted changes
    let status = git::run_git_in(&["status", "--porcelain"], Some(&wt_path));
    if status.success && !status.stdout.is_empty() {
        return Err(format!(
            "worktree '{name}' has uncommitted changes — commit them first"
        ));
    }

    // Check commits ahead
    let remote_trunk = format!("origin/{trunk}");
    let (ahead, _) = {
        let result = git::run_git_in(
            &["rev-list", "--left-right", "--count", &format!("HEAD...{remote_trunk}")],
            Some(&wt_path),
        );
        if !result.success {
            (0usize, 0usize)
        } else {
            let parts: Vec<&str> = result.stdout.split_whitespace().collect();
            if parts.len() == 2 {
                (
                    parts[0].parse().unwrap_or(0),
                    parts[1].parse().unwrap_or(0),
                )
            } else {
                (0, 0)
            }
        }
    };

    if ahead == 0 {
        return Err("nothing to ship — no commits ahead of trunk".to_string());
    }

    // Push
    let push = git::run_git_in(&["push", "-u", "origin", name], Some(&wt_path));
    if !push.success {
        return Err(format!("push failed: {}", push.stderr));
    }

    // Open PR
    if !no_pr {
        if git::gh_available() {
            let mut pr_args = vec!["pr", "create", "--base", trunk, "--fill", "--head", name];
            if draft {
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
                    "gh not installed — create PR manually:\n  gh pr create --base {trunk} --fill --head {name}"
                )
                .yellow()
            );
        }
    }

    println!("{}", format!("shipped worktree '{name}'").green());

    if remove {
        remove_wt(config, name, false, false, repo_root)?;
    }

    Ok(())
}

pub fn remove_wt(
    config: &Config,
    name: &str,
    force: bool,
    prune: bool,
    repo_root: &Path,
) -> Result<(), String> {
    let wt_dir = config.resolve_wt_dir(repo_root);
    let wt_path = wt_dir.join(name);

    if !wt_path.exists() {
        return Err(format!("worktree '{name}' not found"));
    }

    // Check if we're inside the worktree being removed
    let cwd = std::env::current_dir().map_err(|e| format!("can't get cwd: {e}"))?;
    if cwd.starts_with(&wt_path) {
        return Err("you're inside this worktree — cd out first".to_string());
    }

    // Check for uncommitted changes
    if !force {
        let status = git::run_git_in(&["status", "--porcelain"], Some(&wt_path));
        if status.success && !status.stdout.is_empty() {
            return Err(format!(
                "worktree '{name}' has uncommitted changes — use --force to remove anyway"
            ));
        }

        // Check for unshipped commits
        let trunk = &config.trunk;
        let remote_trunk = format!("origin/{trunk}");
        let log = git::run_git_in(
            &["log", "--oneline", &format!("{remote_trunk}..HEAD")],
            Some(&wt_path),
        );
        if log.success && !log.stdout.is_empty() {
            let count = log.stdout.lines().count();
            eprintln!(
                "{}",
                format!("warning: {count} unshipped commit(s) in '{name}'").yellow()
            );
            if !force {
                return Err("use --force to remove with unshipped work".to_string());
            }
        }
    }

    let mut remove_args = vec!["worktree", "remove"];
    if force {
        remove_args.push("--force");
    }
    let wt_path_str = wt_path.to_string_lossy().to_string();
    remove_args.push(&wt_path_str);
    let result = git::run_git(&remove_args);
    if !result.success {
        return Err(format!("worktree remove failed: {}", result.stderr));
    }

    // Delete local branch
    let branch_del = git::run_git(&["branch", "-D", name]);
    if !branch_del.success {
        eprintln!(
            "{}",
            format!("warning: couldn't delete branch '{name}': {}", branch_del.stderr).yellow()
        );
    }

    // Prune remote branch
    if prune {
        let remote_del = git::run_git(&["push", "origin", "--delete", name]);
        if !remote_del.success {
            eprintln!(
                "{}",
                format!("warning: couldn't delete remote branch '{name}': {}", remote_del.stderr)
                    .yellow()
            );
        }
    }

    println!("{}", format!("removed worktree '{name}'").green());
    Ok(())
}

pub fn list_wt(config: &Config, repo_root: &Path) -> Result<(), String> {
    let trunk = &config.trunk;
    let result = git::run_git(&["worktree", "list", "--porcelain"]);
    if !result.success {
        return Err(format!("failed to list worktrees: {}", result.stderr));
    }

    if result.stdout.is_empty() {
        println!("no worktrees");
        return Ok(());
    }

    let wt_dir = config.resolve_wt_dir(repo_root);
    let mut found = false;

    // Parse porcelain output
    let mut current_path = String::new();
    let mut current_branch = String::new();

    for line in result.stdout.lines() {
        if let Some(path) = line.strip_prefix("worktree ") {
            current_path = path.to_string();
        } else if let Some(branch) = line.strip_prefix("branch refs/heads/") {
            current_branch = branch.to_string();
        } else if line.is_empty() && !current_path.is_empty() {
            // End of entry — only show worktrees in our wt_dir
            let p = Path::new(&current_path);
            if p.starts_with(&wt_dir) {
                found = true;
                print_worktree_info(&current_path, &current_branch, trunk);
            }
            current_path.clear();
            current_branch.clear();
        }
    }
    // Handle last entry (porcelain output may not end with blank line)
    if !current_path.is_empty() {
        let p = Path::new(&current_path);
        if p.starts_with(&wt_dir) {
            found = true;
            print_worktree_info(&current_path, &current_branch, trunk);
        }
    }

    if !found {
        println!("no flow worktrees (use `flow wt create <name>`)");
    }

    Ok(())
}

fn print_worktree_info(path: &str, branch: &str, trunk: &str) {
    let wt_path = Path::new(path);
    let remote_trunk = format!("origin/{trunk}");

    // Commits ahead/behind
    let (ahead, behind) = {
        let result = git::run_git_in(
            &[
                "rev-list",
                "--left-right",
                "--count",
                &format!("HEAD...{remote_trunk}"),
            ],
            Some(wt_path),
        );
        if result.success {
            let parts: Vec<&str> = result.stdout.split_whitespace().collect();
            if parts.len() == 2 {
                (
                    parts[0].parse::<usize>().unwrap_or(0),
                    parts[1].parse::<usize>().unwrap_or(0),
                )
            } else {
                (0, 0)
            }
        } else {
            (0, 0)
        }
    };

    // Clean/dirty
    let status = git::run_git_in(&["status", "--porcelain"], Some(wt_path));
    let clean = status.success && status.stdout.is_empty();

    // Last commit age
    let age = git::run_git_in(&["log", "-1", "--format=%cr"], Some(wt_path));
    let age_str = if age.success { &age.stdout } else { "unknown" };

    let status_str = if clean {
        "clean".green().to_string()
    } else {
        "dirty".yellow().to_string()
    };

    println!(
        "  {} ({}) — +{ahead}/-{behind} {} last commit {}",
        path.bold(),
        branch.cyan(),
        status_str,
        age_str.dimmed()
    );
}
