use colored::Colorize;

use crate::git;

use std::fs;
use std::path::Path;

pub fn generate(repo_root: &Path) -> Result<(), String> {
    let repo_name = repo_root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("project");

    let mut sections = Vec::new();

    // Header
    sections.push(format!("# Project: {repo_name}\n"));

    // Detect project type and key files
    sections.push("## Structure\n".to_string());
    let structure = scan_structure(repo_root);
    for line in &structure {
        sections.push(format!("{line}\n"));
    }

    // Key files
    sections.push("\n## Key Files\n".to_string());
    let key_files = find_key_files(repo_root);
    for (path, desc) in &key_files {
        sections.push(format!("- `{path}` — {desc}\n"));
    }

    // Conventions
    sections.push("\n## Conventions\n".to_string());
    let conventions = detect_conventions(repo_root);
    for conv in &conventions {
        sections.push(format!("- {conv}\n"));
    }

    // Git info
    sections.push("\n## Git\n".to_string());
    let branch = git::current_branch().unwrap_or_else(|_| "unknown".to_string());
    sections.push(format!("- Current branch: `{branch}`\n"));

    let remote = git::run_git(&["remote", "get-url", "origin"]);
    if remote.success {
        sections.push(format!("- Remote: `{}`\n", remote.stdout));
    }

    let content: String = sections.join("");
    let output_path = repo_root.join(".gf-context.md");
    fs::write(&output_path, &content)
        .map_err(|e| format!("failed to write .gf-context.md: {e}"))?;

    println!("{}", "generated .gf-context.md".green());
    println!("{}", content.dimmed());
    Ok(())
}

pub fn update(repo_root: &Path) -> Result<(), String> {
    let context_path = repo_root.join(".gf-context.md");
    if !context_path.exists() {
        return generate(repo_root);
    }
    // Re-generate (full refresh)
    generate(repo_root)
}

fn scan_structure(root: &Path) -> Vec<String> {
    let mut dirs: Vec<String> = Vec::new();

    // Walk top-level directories
    let entries = match fs::read_dir(root) {
        Ok(e) => e,
        Err(_) => return dirs,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        // Skip hidden dirs and common non-essential dirs
        if name.starts_with('.') || name == "node_modules" || name == "target" || name == "dist" || name == "build" || name == "__pycache__" {
            continue;
        }

        if path.is_dir() {
            let desc = describe_dir(&path, &name);
            dirs.push(format!("- `{name}/` — {desc}"));
        }
    }

    if dirs.is_empty() {
        dirs.push("- (flat structure)".to_string());
    }

    dirs
}

fn describe_dir(path: &Path, name: &str) -> String {
    // Try to infer purpose from directory name and contents
    match name {
        "src" => {
            let count = count_files_recursive(path);
            format!("source code ({count} files)")
        }
        "tests" | "test" | "__tests__" | "spec" => "tests".to_string(),
        "docs" | "doc" | "documentation" => "documentation".to_string(),
        "scripts" | "bin" => "scripts/tooling".to_string(),
        "helm" | "k8s" | "kubernetes" | "deploy" | "infra" => "infrastructure/deployment".to_string(),
        "ci" | ".github" => "CI/CD".to_string(),
        "lib" | "libs" | "packages" => "libraries/packages".to_string(),
        "cmd" => "CLI entrypoints".to_string(),
        "internal" | "pkg" => "internal packages".to_string(),
        "api" => "API definitions".to_string(),
        "proto" | "protos" => "protocol buffers".to_string(),
        "migrations" => "database migrations".to_string(),
        "config" | "configs" | "conf" => "configuration".to_string(),
        "public" | "static" | "assets" => "static assets".to_string(),
        "components" => "UI components".to_string(),
        "pages" | "views" | "routes" => "page/route definitions".to_string(),
        "models" => "data models".to_string(),
        "services" => "service layer".to_string(),
        "utils" | "helpers" | "common" => "utilities".to_string(),
        _ => {
            let count = count_files_recursive(path);
            format!("{count} files")
        }
    }
}

fn count_files_recursive(path: &Path) -> usize {
    let mut count = 0;
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() {
                count += 1;
            } else if p.is_dir() {
                let name = entry.file_name().to_string_lossy().to_string();
                if !name.starts_with('.') && name != "node_modules" && name != "target" {
                    count += count_files_recursive(&p);
                }
            }
        }
    }
    count
}

fn find_key_files(root: &Path) -> Vec<(String, String)> {
    let mut files = Vec::new();

    let candidates: Vec<(&str, &str)> = vec![
        ("Cargo.toml", "Rust project manifest"),
        ("package.json", "Node.js project manifest"),
        ("go.mod", "Go module definition"),
        ("pyproject.toml", "Python project config"),
        ("requirements.txt", "Python dependencies"),
        ("Makefile", "build automation"),
        ("Dockerfile", "container definition"),
        ("docker-compose.yml", "container orchestration"),
        ("docker-compose.yaml", "container orchestration"),
        (".github/workflows", "GitHub Actions CI/CD"),
        ("CLAUDE.md", "Claude Code context"),
        (".gitflow.toml", "gitflow config"),
        ("tsconfig.json", "TypeScript config"),
        ("webpack.config.js", "webpack bundler config"),
        ("vite.config.ts", "Vite config"),
        ("tailwind.config.js", "Tailwind CSS config"),
        (".eslintrc.json", "ESLint config"),
        (".prettierrc", "Prettier config"),
    ];

    for (file, desc) in candidates {
        if root.join(file).exists() {
            files.push((file.to_string(), desc.to_string()));
        }
    }

    files
}

fn detect_conventions(root: &Path) -> Vec<String> {
    let mut conventions = Vec::new();

    // Detect language
    if root.join("Cargo.toml").exists() {
        conventions.push("Language: Rust".to_string());
    }
    if root.join("package.json").exists() {
        conventions.push("Language: JavaScript/TypeScript".to_string());
    }
    if root.join("go.mod").exists() {
        conventions.push("Language: Go".to_string());
    }
    if root.join("pyproject.toml").exists() || root.join("requirements.txt").exists() {
        conventions.push("Language: Python".to_string());
    }

    // Detect test framework
    if root.join("tests").is_dir() || root.join("test").is_dir() {
        conventions.push("Tests: in dedicated test directory".to_string());
    }

    // Detect CI
    if root.join(".github/workflows").is_dir() {
        conventions.push("CI: GitHub Actions".to_string());
    }

    // Detect monorepo indicators
    if root.join("lerna.json").exists()
        || root.join("pnpm-workspace.yaml").exists()
        || root.join("nx.json").exists()
    {
        conventions.push("Monorepo: yes".to_string());
    }

    // Check for CLAUDE.md
    if root.join("CLAUDE.md").exists() {
        conventions.push("Has CLAUDE.md for AI context".to_string());
    }

    conventions
}
