use colored::Colorize;

use crate::config::Config;
use crate::git;

use std::path::Path;

pub fn run(trunk: Option<&str>, wt_dir: Option<&str>, hooks: bool) -> Result<(), String> {
    let root = git::repo_root()?;
    let root = Path::new(&root);

    let trunk = trunk.unwrap_or("main");

    // Validate trunk exists
    if !git::branch_exists(trunk) && !git::branch_exists(&format!("origin/{trunk}")) {
        return Err(format!(
            "branch '{trunk}' doesn't exist locally or on origin"
        ));
    }

    let repo_name = root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("repo");
    let default_wt_dir = format!("../{repo_name}-wt");
    let wt_dir = wt_dir.unwrap_or(&default_wt_dir);

    let config = Config {
        trunk: trunk.to_string(),
        wt_dir: wt_dir.to_string(),
        ..Config::default()
    };
    config.save(root)?;

    println!("{}", "initialized .gitflow.toml".green());
    println!("  trunk = \"{trunk}\"");
    println!("  wt_dir = \"{wt_dir}\"");

    // Install post-commit hook if requested
    if hooks {
        install_hook(root)?;
    }

    // Add tracking files to .gitignore
    let gitignore = root.join(".gitignore");
    let entries = [".gf-stack", ".gf-session"];
    let existing = std::fs::read_to_string(&gitignore).unwrap_or_default();
    let mut to_add = Vec::new();
    for entry in &entries {
        if !existing.lines().any(|l| l.trim() == *entry) {
            to_add.push(*entry);
        }
    }
    if !to_add.is_empty() {
        let mut content = existing;
        if !content.is_empty() && !content.ends_with('\n') {
            content.push('\n');
        }
        for entry in &to_add {
            content.push_str(entry);
            content.push('\n');
        }
        std::fs::write(&gitignore, content)
            .map_err(|e| format!("failed to update .gitignore: {e}"))?;
        println!(
            "{}",
            format!("added {} to .gitignore", to_add.join(", ")).dimmed()
        );
    }

    Ok(())
}

fn install_hook(repo_root: &Path) -> Result<(), String> {
    let hooks_dir = repo_root.join(".git/hooks");
    if !hooks_dir.exists() {
        std::fs::create_dir_all(&hooks_dir)
            .map_err(|e| format!("failed to create hooks dir: {e}"))?;
    }

    let hook_path = hooks_dir.join("post-commit");
    let hook_content = "#!/bin/sh\ngf doctor --quiet 2>/dev/null || true\n";

    // Don't overwrite existing hook, append instead
    if hook_path.exists() {
        let existing = std::fs::read_to_string(&hook_path)
            .map_err(|e| format!("failed to read existing hook: {e}"))?;
        if existing.contains("gf doctor") {
            println!("{}", "post-commit hook already installed".dimmed());
            return Ok(());
        }
        let updated = format!("{existing}\n# gitflow guardrails\ngf doctor --quiet 2>/dev/null || true\n");
        std::fs::write(&hook_path, updated)
            .map_err(|e| format!("failed to update hook: {e}"))?;
    } else {
        std::fs::write(&hook_path, hook_content)
            .map_err(|e| format!("failed to write hook: {e}"))?;
    }

    // Make executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o755);
        std::fs::set_permissions(&hook_path, perms)
            .map_err(|e| format!("failed to set hook permissions: {e}"))?;
    }

    println!("{}", "installed post-commit hook".green());
    Ok(())
}
