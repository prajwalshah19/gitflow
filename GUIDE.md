# gitflow (`gf`) — Guide

## Thesis

**Your trunk should always be clean. Your branches should be small. Your agent should never get lost.**

`gf` is built on three convictions:

1. **Trunk-based development is the only sane model for agentic coding.** When AI agents create branches, rebase, and ship PRs autonomously, long-lived feature branches become a liability. You need a workflow where trunk is always deployable, branches live for hours (not weeks), and every PR is a small, reviewable diff.

2. **Isolation prevents disasters.** The #1 failure mode in agent-assisted development is editing the wrong files on the wrong branch. Worktrees give you physical directory isolation — not just git branch isolation — so an agent literally cannot touch your trunk code while working on a feature.

3. **Guardrails beat discipline.** Developers forget to sync. Agents drift from trunk. Uncommitted files pile up. Rather than relying on memory, `gf` enforces hygiene automatically — blocking operations when drift gets too large, warning when worktrees get dirty, and validating state before every mutation.

---

## Quick Start

```sh
# Install
cargo install --path .

# Initialize in any git repo
cd your-repo
gf init --trunk main --hooks

# You're ready. Your workflow is now:
gf sync                              # stay current with trunk
gf ship "fix auth redirect"          # ship a change as a PR in one command
```

---

## The Two-Track Model

`gf` gives you two ways to work. Pick the right one for the size of the task.

### Track 1: Worktrees — for features and agent tasks

Each feature gets its own directory. Files never mix. You `cd` between tasks.

```sh
# Start a feature
gf wt create add-rate-limiting
cd ../your-repo-wt/add-rate-limiting

# Work on it...
# ...

# Ship it
gf wt ship add-rate-limiting --remove
```

**When to use:** Any task that takes more than a few minutes. Any task an agent is doing autonomously. Any task where you want zero risk of polluting trunk.

### Track 2: Stacks — for quick fixes

Stay on trunk. Batch small changes into a stack of branches. Ship them as individual PRs.

```sh
# Fix a typo
gf stack create "fix typo in login page"

# Fix another thing
gf stack create "update error message for auth timeout"

# See the stack
gf stack list

# Ship them all as independent PRs
gf stack ship --all
```

**When to use:** Small fixes that take minutes. Things you notice while reviewing code. Batch mode for cleaning up tech debt.

---

## Commands in Detail

### `gf sync`

Rebase your current branch on latest trunk. The most-used command.

```sh
gf sync
```

Handles everything: fetches, stashes dirty work, rebases, pops stash, reports result. If there's a conflict, it aborts and tells you exactly what to do.

### `gf ship "<message>"`

The one-command PR workflow. Does everything: syncs trunk, creates a branch, commits, pushes, opens a PR, switches you back to trunk.

```sh
gf ship "fix auth redirect on mobile"
gf ship "add rate limiter" --draft         # draft PR
gf ship "quick fix" --no-pr                # just push, no PR
gf ship "fix bug" --branch hotfix-auth     # custom branch name
```

**Staging behavior:** If you've staged specific files, `gf ship` ships only those. If nothing is staged, it ships everything. Use `--all` to force shipping everything even with a partial stage.

### `gf wt` — Worktree management

```sh
gf wt create <name>                        # new worktree from trunk
gf wt checkout <pr-number-or-branch>       # pull existing PR into worktree
gf wt sync [name]                          # rebase worktree on trunk
gf wt ship <name> [--draft] [--remove]     # push + PR
gf wt remove <name> [--force] [--prune]    # clean up
gf wt list                                 # show all worktrees with status
```

`gf wt checkout` is how you jump into PR review or address feedback — it creates a worktree from an existing remote branch or PR number:

```sh
gf wt checkout 42           # checkout PR #42 into a worktree
cd ../your-repo-wt/feature-branch-name
# make changes, commit, push
```

### `gf stack` — Stacked branches

```sh
gf stack create "<message>"    # new branch on top of stack, stages + commits all
gf stack list                  # show the stack
gf stack ship                  # ship bottom of stack as PR, rebase rest
gf stack ship --all            # ship each branch as independent PR
```

The stack is tracked in `.gf-stack` (gitignored). `gf stack ship` (without `--all`) peels the bottom branch off, pushes it as a PR, and rebases the remaining branches onto trunk. `--all` treats each branch as independent — cherry-picks each onto trunk and ships separate PRs.

### `gf wrap` — Agentic session lifecycle

Designed for AI agents. Clean start/end boundaries for autonomous tasks.

```sh
gf wrap begin "add rate limiting"    # sync trunk, create worktree, start session
gf wrap save                        # checkpoint WIP (commit without shipping)
gf wrap status                      # what session am I in?
gf wrap end                         # commit, run checks, ship PR, clean up
gf wrap abort                       # throw away everything, clean up
```

**`begin`** syncs trunk, creates a worktree, writes a `.gf-session` marker with the task description and timestamp. **`save`** commits all changes as a WIP checkpoint — if the agent crashes, work isn't lost. **`end`** commits remaining changes, runs the configured `pre_end` check (e.g., `cargo test`), ships a PR, removes the worktree, and returns to trunk. **`abort`** discards everything.

### `gf guard` — Pre-flight state check

The wrong-branch prevention system. Call this before any file mutation.

```sh
gf guard                                  # basic check
gf guard --expect-branch add-feature      # assert specific branch
```

Output is machine-readable:

```
OK branch=add-feature worktree=/path/to/wt session=active dirty=3 ahead=2 behind=0
```

Or:

```
BLOCK branch=main worktree=none session=none dirty=0 ahead=0 behind=0
  BLOCK: SESSION MISMATCH: session expects branch 'add-feature', on 'main'
```

**How agents should use it:** Call `gf guard` before every batch of edits. If it returns `BLOCK`, stop and investigate. If it returns `WARN`, proceed with caution. Parse the output to confirm you're in the right worktree.

### `gf review` — PR review workflow

```sh
gf review start 42          # checkout PR into worktree, fetch comments, run checks
# ... address feedback ...
gf review ship 42           # push updates, post "addressed feedback" comment
gf review done 42           # remove worktree, clean up
```

`start` pulls the PR into a worktree, displays review comments, runs configured checks (lint, test, typecheck), and shows diff stats. All in one command.

### `gf context` — Codebase map

```sh
gf context generate          # scan repo, write .gf-context.md
gf context update            # refresh
```

Produces a structured context file that agents can read to skip the exploration phase:

```markdown
# Project: myapp
## Structure
- `src/` — source code (42 files)
- `tests/` — tests
- `helm/` — infrastructure/deployment
## Key Files
- `Cargo.toml` — Rust project manifest
- `.github/workflows` — GitHub Actions CI/CD
## Conventions
- Language: Rust
- Tests: in dedicated test directory
- CI: GitHub Actions
```

### `gf doctor` — Health check

```sh
gf doctor                   # full report
gf doctor --quiet           # only warnings/errors (for hooks)
```

Shows trunk drift, dirty file count, worktree status, stack state, and guardrail thresholds. The `--quiet` flag is designed for the post-commit hook — it only prints when something needs attention.

### `gf status` — Overview

```sh
gf status
```

One-screen summary: current branch, working tree status, ahead/behind trunk, all worktrees, open PRs.

---

## Configuration

### `.gitflow.toml`

```toml
trunk = "main"
wt_dir = "../{repo}-wt"

[guardrails]
auto_fetch_every = 5          # fetch origin every N commits ahead
sync_reminder_drift = 20      # warn when N commits behind trunk
sync_block_drift = 50         # block operations when N commits behind
max_dirty_files = 10          # warn at N uncommitted files
block_dirty_files = 50        # block at N uncommitted files

[wrap]
pre_end = "cargo test"        # command to run before gf wrap end ships

[review]
checks = "cargo clippy, cargo test"   # commands to run during gf review start
```

**`{repo}`** in `wt_dir` is replaced with the repo directory name at runtime.

**Zero config required.** If `.gitflow.toml` doesn't exist, defaults apply. `gf init` creates it for you.

### Tracking files

| File | Purpose | Gitignored |
|---|---|---|
| `.gitflow.toml` | Repo config | No (shared) |
| `.gf-stack` | Current stack order | Yes |
| `.gf-session` | Active session marker | Yes |
| `.gf-context.md` | Codebase map | Yes |

`gf init` adds the gitignored files to `.gitignore` automatically.

---

## The Agentic Loop

This is the workflow `gf` is designed around. Every agent interaction follows this pattern:

```
gf doctor                          # 1. check repo health
gf wrap begin "implement X"        # 2. isolated start
gf guard                           # 3. verify state

# ... agent works ...

gf wrap save                       # 4. checkpoint
gf guard                           # 5. still correct?

# ... more work ...

gf wrap end                        # 6. test, ship, clean
```

**Steps 3 and 5 are the key insight.** Agents lose track of state. `gf guard` catches it before damage is done. The machine-readable output means hooks can parse it, CI can validate it, and agents can branch on it.

### Integrating with Claude Code

Add to your `CLAUDE.md`:

```markdown
## Development workflow
- Before starting any task: `gf wrap begin "<task description>"`
- Before editing files: `gf guard --expect-branch <branch>`
- To save progress: `gf wrap save`
- When done: `gf wrap end`
- To check repo health: `gf doctor`
```

Set up a Claude Code hook that runs `gf guard --expect-branch $(git branch --show-current)` before every edit tool call. This catches wrong-branch mistakes before they happen.

---

## Design Principles

1. **Print every command.** Every `git` and `gh` invocation is printed as `→ git ...` in dimmed text. No hidden magic. You can always see what happened and reproduce it manually.

2. **No state beyond what's visible.** `.gitflow.toml` is readable TOML. `.gf-stack` is a plain text list of branch names. `.gf-session` is key-value pairs. Everything else is derived from git at runtime.

3. **Fail loudly, fail early.** Every command checks preconditions before mutating anything. If a rebase conflicts, it aborts and restores your stash. If a branch name collides, it appends a hash. No half-done states.

4. **`gh` is optional.** Every command works without GitHub CLI. PR features degrade gracefully with a message showing the manual command.

5. **Auto-sync by default, opt out explicitly.** `gf ship` syncs before branching. `--no-sync` is the escape hatch. The default keeps you close to trunk.

6. **Guardrails are automatic.** Mutating commands (`ship`, `stack create`, `stack ship`, `wrap end`) run guardrail checks before executing. You don't have to remember to sync — the tool blocks you if you've drifted too far.

---

## What `gf` Is Not

- Not a merge/review manager. It creates PRs, it doesn't manage them.
- Not a CI tool. It runs local checks (`pre_end`, review checks), not pipelines.
- Not a multi-remote tool. It assumes `origin` is your only remote.
- Not interactive. Everything is flags. No prompts, no TUI.
- Not a replacement for git. It shells out to git for everything and prints what it runs. You can always fall back to raw git.
