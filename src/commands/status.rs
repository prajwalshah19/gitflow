use colored::Colorize;

use crate::config::Config;
use crate::git;

use std::path::Path;

pub fn run(config: &Config, repo_root: &Path) -> Result<(), String> {
    let trunk = &config.trunk;

    // Current branch
    let branch = git::current_branch().unwrap_or_else(|_| "detached HEAD".to_string());
    if branch == *trunk {
        println!("on {} (trunk)", branch.green().bold());
    } else {
        println!("on {}", branch.cyan().bold());
    }

    // Working tree status
    if git::has_uncommitted_changes() {
        println!("working tree: {}", "dirty".yellow());
    } else {
        println!("working tree: {}", "clean".green());
    }

    // Ahead/behind trunk
    let remote_trunk = format!("origin/{trunk}");
    let (ahead, behind) = git::commits_ahead_behind("HEAD", &remote_trunk);
    println!("{ahead} ahead, {behind} behind {remote_trunk}");

    // Worktrees
    println!("\n{}", "worktrees:".bold());
    if let Err(e) = crate::commands::worktree::list_wt(config, repo_root) {
        eprintln!("  {}", format!("error listing worktrees: {e}").yellow());
    }

    // Open PRs
    println!("\n{}", "open PRs:".bold());
    if git::gh_available() {
        let pr = git::run_gh(&["pr", "list", "--author", "@me", "--state", "open"]);
        if pr.success && !pr.stdout.is_empty() {
            for line in pr.stdout.lines() {
                println!("  {line}");
            }
        } else if pr.success {
            println!("  none");
        } else {
            println!("  {}", format!("error: {}", pr.stderr).yellow());
        }
    } else {
        println!("  {}", "gh not installed — PR info unavailable".dimmed());
    }

    Ok(())
}
