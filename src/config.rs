use std::fs;
use std::path::{Path, PathBuf};

pub struct Config {
    pub trunk: String,
    pub wt_dir: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            trunk: "main".to_string(),
            wt_dir: "../{repo}-wt".to_string(),
        }
    }
}

impl Config {
    pub fn load(repo_root: &Path) -> Self {
        let path = repo_root.join(".flow.toml");
        if !path.exists() {
            return Self::default();
        }
        let contents = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => return Self::default(),
        };
        let mut config = Self::default();
        for line in contents.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim().trim_matches('"');
                match key {
                    "trunk" => config.trunk = value.to_string(),
                    "wt_dir" => config.wt_dir = value.to_string(),
                    _ => {}
                }
            }
        }
        config
    }

    pub fn save(&self, repo_root: &Path) -> Result<(), String> {
        let path = repo_root.join(".flow.toml");
        let contents = format!(
            "trunk = \"{}\"\nwt_dir = \"{}\"\n",
            self.trunk, self.wt_dir
        );
        fs::write(&path, contents).map_err(|e| format!("failed to write .flow.toml: {e}"))
    }

    pub fn resolve_wt_dir(&self, repo_root: &Path) -> PathBuf {
        let repo_name = repo_root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("repo");
        let resolved = self.wt_dir.replace("{repo}", repo_name);
        let wt_path = Path::new(&resolved);
        if wt_path.is_absolute() {
            wt_path.to_path_buf()
        } else {
            repo_root.join(&resolved)
        }
    }
}
