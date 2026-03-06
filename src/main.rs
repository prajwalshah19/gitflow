mod commands;
mod config;
mod git;
mod guardrails;

use clap::{Parser, Subcommand};
use colored::Colorize;
use config::Config;
use std::path::Path;
use std::process;

#[derive(Parser)]
#[command(
    name = "gf",
    about = "Opinionated trunk-based development CLI built for agentic workflows",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Rebase current branch on latest trunk
    Sync,

    /// Package current work into a branch + PR, return to trunk
    Ship {
        /// Commit message
        message: String,

        /// Skip auto-sync before branching
        #[arg(long)]
        no_sync: bool,

        /// Open PR as draft
        #[arg(long)]
        draft: bool,

        /// Just create branch + commit + push, don't open PR
        #[arg(long)]
        no_pr: bool,

        /// Override the auto-generated branch name
        #[arg(long)]
        branch: Option<String>,

        /// Force shipping only staged changes
        #[arg(long)]
        staged: bool,

        /// Force shipping everything even with a partial stage
        #[arg(long)]
        all: bool,
    },

    /// Worktree management
    #[command(subcommand, name = "wt")]
    Wt(WtCommands),

    /// Stacked branch management for quick fixes
    #[command(subcommand)]
    Stack(StackCommands),

    /// Agentic session boundary — begin/end isolated work
    #[command(subcommand)]
    Wrap(WrapCommands),

    /// Single-screen overview of everything in flight
    Status,

    /// Repo health check — drift, cleanliness, guardrails
    Doctor {
        /// Only print warnings and errors
        #[arg(long)]
        quiet: bool,
    },

    /// Set up config for the current repo
    Init {
        /// Trunk branch name
        #[arg(long, default_value = "main")]
        trunk: Option<String>,

        /// Worktree directory
        #[arg(long)]
        wt_dir: Option<String>,

        /// Install post-commit hook for automatic guardrails
        #[arg(long)]
        hooks: bool,
    },
}

#[derive(Subcommand)]
enum WtCommands {
    /// Create a new worktree branching from latest trunk
    Create {
        /// Worktree/branch name
        name: String,
    },

    /// Rebase a worktree on latest trunk
    Sync {
        /// Worktree name (optional if inside a worktree)
        name: Option<String>,
    },

    /// Ship a worktree's work as a PR
    Ship {
        /// Worktree name
        name: String,

        /// Open PR as draft
        #[arg(long)]
        draft: bool,

        /// Remove the worktree after shipping
        #[arg(long)]
        remove: bool,

        /// Just push, don't open PR
        #[arg(long)]
        no_pr: bool,
    },

    /// Clean up a worktree
    Remove {
        /// Worktree name
        name: String,

        /// Remove even with uncommitted changes
        #[arg(long)]
        force: bool,

        /// Also delete the remote branch
        #[arg(long)]
        prune: bool,
    },

    /// Checkout a PR or remote branch into a worktree
    Checkout {
        /// PR number or branch name
        pr_or_branch: String,
    },

    /// Show all active worktrees with context
    List,
}

#[derive(Subcommand)]
enum StackCommands {
    /// Create a new branch on top of the stack
    Create {
        /// Commit message (also used as branch name slug)
        message: String,
    },

    /// Show the current stack
    List,

    /// Ship the bottom of the stack (or all with --all)
    Ship {
        /// Ship all branches independently
        #[arg(long)]
        all: bool,

        /// Open PRs as draft
        #[arg(long)]
        draft: bool,

        /// Just push, don't open PRs
        #[arg(long)]
        no_pr: bool,
    },
}

#[derive(Subcommand)]
enum WrapCommands {
    /// Begin an isolated work session in a new worktree
    Begin {
        /// Description of the task
        description: String,
    },

    /// End session — commit, ship PR, clean up worktree
    End,
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Init {
            trunk,
            wt_dir,
            hooks,
        } => commands::init::run(trunk.as_deref(), wt_dir.as_deref(), hooks),
        _ => {
            let root = match git::repo_root() {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("{}", format!("error: {e}").red());
                    process::exit(1);
                }
            };
            let root = Path::new(&root).to_path_buf();
            let config = Config::load(&root);

            // Run guardrails on mutating commands
            let needs_guardrails = matches!(
                cli.command,
                Commands::Ship { .. }
                    | Commands::Stack(StackCommands::Create { .. })
                    | Commands::Stack(StackCommands::Ship { .. })
                    | Commands::Wrap(WrapCommands::End)
            );

            if needs_guardrails {
                if let Err(e) = guardrails::enforce(&config) {
                    eprintln!("{}", format!("blocked: {e}").red());
                    process::exit(1);
                }
            }

            match cli.command {
                Commands::Sync => commands::sync::run(&config),
                Commands::Ship {
                    message,
                    no_sync,
                    draft,
                    no_pr,
                    branch,
                    staged,
                    all,
                } => commands::ship::run(
                    &config,
                    commands::ship::ShipOpts {
                        message,
                        no_sync,
                        draft,
                        no_pr,
                        branch,
                        staged,
                        all,
                    },
                ),
                Commands::Wt(wt_cmd) => match wt_cmd {
                    WtCommands::Create { name } => {
                        commands::worktree::create(&config, &name, &root)
                    }
                    WtCommands::Sync { name } => {
                        commands::worktree::sync_wt(&config, name.as_deref(), &root)
                    }
                    WtCommands::Ship {
                        name,
                        draft,
                        remove,
                        no_pr,
                    } => commands::worktree::ship_wt(&config, &name, draft, remove, no_pr, &root),
                    WtCommands::Remove {
                        name,
                        force,
                        prune,
                    } => commands::worktree::remove_wt(&config, &name, force, prune, &root),
                    WtCommands::Checkout { pr_or_branch } => {
                        commands::worktree::checkout_wt(&config, &pr_or_branch, &root)
                    }
                    WtCommands::List => commands::worktree::list_wt(&config, &root),
                },
                Commands::Stack(stack_cmd) => match stack_cmd {
                    StackCommands::Create { message } => {
                        commands::stack::create(&config, &message, &root)
                    }
                    StackCommands::List => commands::stack::list(&config, &root),
                    StackCommands::Ship {
                        all,
                        draft,
                        no_pr,
                    } => commands::stack::ship(&config, all, draft, no_pr, &root),
                },
                Commands::Wrap(wrap_cmd) => match wrap_cmd {
                    WrapCommands::Begin { description } => {
                        commands::wrap::begin(&config, &description, &root)
                    }
                    WrapCommands::End => commands::wrap::end(&config, &root),
                },
                Commands::Status => commands::status::run(&config, &root),
                Commands::Doctor { quiet } => commands::doctor::run(&config, &root, quiet),
                Commands::Init { .. } => unreachable!(),
            }
        }
    };

    if let Err(e) = result {
        eprintln!("{}", format!("error: {e}").red());
        process::exit(1);
    }
}
