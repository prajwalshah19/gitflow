use colored::Colorize;

use crate::config::Config;
use crate::git;

pub enum CheckResult {
    Ok,
    Warning(String),
    Block(String),
}

pub fn check(config: &Config) -> CheckResult {
    let g = &config.guardrails;
    let trunk = &config.trunk;
    let remote_trunk = format!("origin/{trunk}");

    // Check drift behind trunk
    let (_, behind) = git::commits_ahead_behind("HEAD", &remote_trunk);

    if behind >= g.sync_block_drift {
        return CheckResult::Block(format!(
            "branch is {behind} commits behind {trunk} (limit: {}). Run `gf sync` first.",
            g.sync_block_drift
        ));
    }

    if behind >= g.sync_reminder_drift {
        return CheckResult::Warning(format!(
            "branch is {behind} commits behind {trunk}. Consider running `gf sync`.",
        ));
    }

    // Check dirty file count
    let status = git::run_git(&["status", "--porcelain"]);
    if status.success && !status.stdout.is_empty() {
        let dirty_count = status.stdout.lines().count();

        if dirty_count >= g.block_dirty_files {
            return CheckResult::Block(format!(
                "{dirty_count} uncommitted files (limit: {}). Commit or clean up before proceeding.",
                g.block_dirty_files
            ));
        }

        if dirty_count >= g.max_dirty_files {
            return CheckResult::Warning(format!(
                "{dirty_count} uncommitted files. Consider committing or cleaning up.",
            ));
        }
    }

    // Auto-fetch check: fetch if commits ahead exceed threshold
    let (ahead, _) = git::commits_ahead_behind("HEAD", &remote_trunk);
    if ahead > 0 && ahead % g.auto_fetch_every == 0 {
        eprintln!(
            "{}",
            format!("auto-fetching origin ({ahead} commits since last fetch)...").dimmed()
        );
        git::run_git(&["fetch", "origin"]);
    }

    CheckResult::Ok
}

pub fn enforce(config: &Config) -> Result<(), String> {
    match check(config) {
        CheckResult::Ok => Ok(()),
        CheckResult::Warning(msg) => {
            eprintln!("{}", format!("warning: {msg}").yellow());
            Ok(())
        }
        CheckResult::Block(msg) => Err(msg),
    }
}
