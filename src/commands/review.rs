use colored::Colorize;

use crate::commands::worktree;
use crate::config::Config;
use crate::git;

use std::path::Path;
use std::process::Command;

pub fn start(config: &Config, pr: &str, repo_root: &Path) -> Result<(), String> {
    // Checkout PR into worktree
    worktree::checkout_wt(config, pr, repo_root)?;

    // Resolve branch name for display
    let branch_name = resolve_pr_branch(pr)?;
    let wt_dir = config.resolve_wt_dir(repo_root);
    let wt_path = wt_dir.join(&branch_name);

    println!("\n{}", format!("reviewing PR #{pr}").bold());

    // Fetch review comments
    if git::gh_available() {
        println!("\n{}", "review comments:".bold());
        let comments = git::run_gh(&[
            "pr", "view", pr, "--json",
            "reviews,comments,title,state,additions,deletions,changedFiles",
            "--jq",
            r#"
"  title: \(.title)",
"  state: \(.state)",
"  +\(.additions) -\(.deletions) in \(.changedFiles) files",
"",
(.reviews[]? | "  [\(.state)] \(.author.login): \(.body // "(no comment)")"),
(.comments[]? | "  \(.author.login): \(.body)")"#,
        ]);
        if comments.success && !comments.stdout.is_empty() {
            println!("{}", comments.stdout);
        } else if comments.success {
            println!("  no comments yet");
        } else {
            eprintln!("{}", format!("  error fetching comments: {}", comments.stderr).yellow());
        }
    }

    // Run configured checks
    if !config.review_checks.is_empty() {
        println!("\n{}", "checks:".bold());
        for check_cmd in &config.review_checks {
            eprint!("  {} ... ", check_cmd.dimmed());
            let output = Command::new("sh")
                .arg("-c")
                .arg(check_cmd)
                .current_dir(&wt_path)
                .output();
            match output {
                Ok(o) if o.status.success() => {
                    eprintln!("{}", "passed".green());
                }
                Ok(o) => {
                    eprintln!("{}", "failed".red());
                    let stderr = String::from_utf8_lossy(&o.stderr);
                    if !stderr.is_empty() {
                        for line in stderr.lines().take(5) {
                            eprintln!("    {line}");
                        }
                    }
                }
                Err(e) => {
                    eprintln!("{}", format!("error: {e}").red());
                }
            }
        }
    }

    // Diff stats
    println!("\n{}", "diff stats:".bold());
    let stats = git::run_git_in(
        &["diff", "--stat", &format!("origin/{}..HEAD", config.trunk)],
        Some(&wt_path),
    );
    if stats.success && !stats.stdout.is_empty() {
        for line in stats.stdout.lines() {
            println!("  {line}");
        }
    }

    println!("\n  cd {}", wt_path.display());
    Ok(())
}

pub fn ship_review(config: &Config, pr: &str, repo_root: &Path) -> Result<(), String> {
    let branch_name = resolve_pr_branch(pr)?;
    let wt_dir = config.resolve_wt_dir(repo_root);
    let wt_path = wt_dir.join(&branch_name);

    if !wt_path.exists() {
        return Err(format!(
            "no worktree for PR #{pr} — run `gf review {pr}` first"
        ));
    }

    // Check for uncommitted changes, commit if any
    let status = git::run_git_in(&["status", "--porcelain"], Some(&wt_path));
    if status.success && !status.stdout.is_empty() {
        git::run_git_in(&["add", "-A"], Some(&wt_path));
        git::run_git_in(
            &["commit", "-m", &format!("address review feedback (PR #{pr})")],
            Some(&wt_path),
        );
    }

    // Push updates
    let push = git::run_git_in(&["push"], Some(&wt_path));
    if !push.success {
        return Err(format!("push failed: {}", push.stderr));
    }

    // Post comment
    if git::gh_available() {
        let comment = git::run_gh(&[
            "pr", "comment", pr, "--body", "Addressed review feedback.",
        ]);
        if comment.success {
            println!("{}", "posted review update comment".green());
        } else {
            eprintln!(
                "{}",
                format!("couldn't post comment: {}", comment.stderr).yellow()
            );
        }
    }

    println!("{}", format!("pushed updates for PR #{pr}").green());
    Ok(())
}

pub fn done(config: &Config, pr: &str, repo_root: &Path) -> Result<(), String> {
    let branch_name = resolve_pr_branch(pr)?;
    let wt_dir = config.resolve_wt_dir(repo_root);
    let wt_path = wt_dir.join(&branch_name);

    if !wt_path.exists() {
        return Err(format!("no worktree for PR #{pr}"));
    }

    // Check for uncommitted changes
    let status = git::run_git_in(&["status", "--porcelain"], Some(&wt_path));
    if status.success && !status.stdout.is_empty() {
        return Err(format!(
            "worktree for PR #{pr} has uncommitted changes — run `gf review ship {pr}` first or discard"
        ));
    }

    // Remove worktree
    worktree::remove_wt(config, &branch_name, true, false, repo_root)?;
    println!("{}", format!("cleaned up review worktree for PR #{pr}").green());
    Ok(())
}

fn resolve_pr_branch(pr: &str) -> Result<String, String> {
    if pr.chars().all(|c| c.is_ascii_digit()) {
        if !git::gh_available() {
            return Err("gh is required to resolve PR numbers".to_string());
        }
        let pr_info = git::run_gh(&[
            "pr", "view", pr, "--json", "headRefName", "--jq", ".headRefName",
        ]);
        if !pr_info.success {
            return Err(format!("failed to get PR #{pr}: {}", pr_info.stderr));
        }
        Ok(pr_info.stdout)
    } else {
        Ok(pr.to_string())
    }
}
