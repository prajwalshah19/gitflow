use std::fs;
use std::path::{Path, PathBuf};

pub struct Guardrails {
    pub auto_fetch_every: usize,
    pub sync_reminder_drift: usize,
    pub sync_block_drift: usize,
    pub max_dirty_files: usize,
    pub block_dirty_files: usize,
}

impl Default for Guardrails {
    fn default() -> Self {
        Self {
            auto_fetch_every: 5,
            sync_reminder_drift: 20,
            sync_block_drift: 50,
            max_dirty_files: 10,
            block_dirty_files: 50,
        }
    }
}

pub struct WrapConfig {
    pub pre_end: Option<String>,
}

impl Default for WrapConfig {
    fn default() -> Self {
        Self { pre_end: None }
    }
}

pub struct Config {
    pub trunk: String,
    pub wt_dir: String,
    pub guardrails: Guardrails,
    pub wrap: WrapConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            trunk: "main".to_string(),
            wt_dir: "../{repo}-wt".to_string(),
            guardrails: Guardrails::default(),
            wrap: WrapConfig::default(),
        }
    }
}

impl Config {
    pub fn load(repo_root: &Path) -> Self {
        let path = repo_root.join(".gitflow.toml");
        if !path.exists() {
            return Self::default();
        }
        let contents = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => return Self::default(),
        };
        let mut config = Self::default();
        let mut section = "";

        for line in contents.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if line.starts_with('[') && line.ends_with(']') {
                section = match &line[1..line.len() - 1] {
                    "guardrails" => "guardrails",
                    "wrap" => "wrap",
                    _ => "",
                };
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim().trim_matches('"');
                match section {
                    "guardrails" => match key {
                        "auto_fetch_every" => {
                            config.guardrails.auto_fetch_every =
                                value.parse().unwrap_or(5);
                        }
                        "sync_reminder_drift" => {
                            config.guardrails.sync_reminder_drift =
                                value.parse().unwrap_or(20);
                        }
                        "sync_block_drift" => {
                            config.guardrails.sync_block_drift =
                                value.parse().unwrap_or(50);
                        }
                        "max_dirty_files" => {
                            config.guardrails.max_dirty_files =
                                value.parse().unwrap_or(10);
                        }
                        "block_dirty_files" => {
                            config.guardrails.block_dirty_files =
                                value.parse().unwrap_or(50);
                        }
                        _ => {}
                    },
                    "wrap" => match key {
                        "pre_end" => {
                            config.wrap.pre_end = Some(value.to_string());
                        }
                        _ => {}
                    },
                    _ => match key {
                        "trunk" => config.trunk = value.to_string(),
                        "wt_dir" => config.wt_dir = value.to_string(),
                        _ => {}
                    },
                }
            }
        }
        config
    }

    pub fn save(&self, repo_root: &Path) -> Result<(), String> {
        let path = repo_root.join(".gitflow.toml");
        let mut contents = format!(
            "trunk = \"{}\"\nwt_dir = \"{}\"\n",
            self.trunk, self.wt_dir
        );
        contents.push_str(&format!(
            "\n[guardrails]\n\
             auto_fetch_every = {}\n\
             sync_reminder_drift = {}\n\
             sync_block_drift = {}\n\
             max_dirty_files = {}\n\
             block_dirty_files = {}\n",
            self.guardrails.auto_fetch_every,
            self.guardrails.sync_reminder_drift,
            self.guardrails.sync_block_drift,
            self.guardrails.max_dirty_files,
            self.guardrails.block_dirty_files,
        ));
        if let Some(ref pre_end) = self.wrap.pre_end {
            contents.push_str(&format!("\n[wrap]\npre_end = \"{pre_end}\"\n"));
        }
        fs::write(&path, contents).map_err(|e| format!("failed to write .gitflow.toml: {e}"))
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
