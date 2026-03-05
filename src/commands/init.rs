use colored::Colorize;

use crate::config::Config;
use crate::git;

use std::path::Path;

pub fn run(trunk: Option<&str>, wt_dir: Option<&str>) -> Result<(), String> {
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
    };
    config.save(root)?;

    println!("{}", format!("initialized .flow.toml").green());
    println!("  trunk = \"{trunk}\"");
    println!("  wt_dir = \"{wt_dir}\"");

    Ok(())
}
