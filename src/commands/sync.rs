use colored::Colorize;

use crate::config::Config;
use crate::git;

pub fn run(config: &Config) -> Result<(), String> {
    let branch = git::current_branch()?;
    let trunk = &config.trunk;

    // Fetch latest
    let fetch = git::run_git(&["fetch", "origin"]);
    if !fetch.success {
        return Err(format!("fetch failed: {}", fetch.stderr));
    }

    // Stash if dirty
    let dirty = git::has_uncommitted_changes();
    if dirty {
        let stash = git::run_git(&["stash"]);
        if !stash.success {
            return Err(format!("stash failed: {}", stash.stderr));
        }
    }

    // Rebase on trunk
    let remote_trunk = format!("origin/{trunk}");
    let rebase = git::run_git(&["rebase", &remote_trunk]);
    if !rebase.success {
        // Abort and restore
        git::run_git(&["rebase", "--abort"]);
        if dirty {
            git::run_git(&["stash", "pop"]);
        }
        return Err(format!(
            "rebase conflict on {branch} against {remote_trunk}.\n\
             Resolve manually:\n  \
             git rebase {remote_trunk}\n  \
             # fix conflicts, then: git rebase --continue"
        ));
    }

    // Restore stash
    if dirty {
        let pop = git::run_git(&["stash", "pop"]);
        if !pop.success {
            eprintln!(
                "{}",
                "warning: stash pop had conflicts, resolve manually".yellow()
            );
        }
    }

    // Report
    let (ahead, behind) = git::commits_ahead_behind(&branch, &remote_trunk);
    if branch == *trunk {
        println!(
            "{}",
            format!("synced {trunk} — {ahead} commits ahead, {behind} behind origin").green()
        );
    } else {
        println!(
            "{}",
            format!("{branch} rebased on {trunk} — {ahead} commits ahead").green()
        );
    }

    Ok(())
}
