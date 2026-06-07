# ARF: Agent Reasoning Format

`git blame` tells you who wrote this line. `arf why` tells you what they were thinking.

ARF is a TOML schema and CLI for capturing agent reasoning alongside
git commits. Records live on an orphan branch keyed to commit SHAs;
optionally anchored to specific files and line ranges. Reviewers
(human or agent) can see what the agent thought, not just what it
changed.

## The problem

AI agents modify code, make decisions, take actions. Their
reasoning is:

- Buried in chat logs
- Lost after the session ends
- Unstructured and hard to review

Review the reasoning, not just the diff.

## The format

A record is a TOML file with required `what` and `why`, optional
`how`, `backup`, and `files`, plus auto-populated `timestamp`,
`commit`, and `agent` fields:

```toml
what = "Add validation to prevent SQL injection"
why = "Email field passes unsanitized input to query"
how = "Use parameterized queries in register_user()"
backup = "Revert if tests fail"

[[files]]
path = "src/auth.rs"
lines = [42, 76]
```

See [SPEC.md](SPEC.md) for the full schema.

## Install

```bash
cargo install arf-cli
```

The crate publishes as `arf-cli` (the bare `arf` name was already
taken on crates.io); the installed binary is still called `arf`.

## CLI

```bash
# Initialize ARF tracking (creates orphan branch at .arf/).
# Detects an existing local or remote arf branch and attaches a
# worktree to it instead of erroring.
arf init

# Record reasoning. --file is repeatable; each value is
# `path[:start[-end]]`.
arf record \
  --what "Add retry logic" \
  --why "Transient API failures cause cascading 503s under load" \
  --how "3-attempt loop with 100/200/400ms backoff" \
  --file src/api.rs:142-180

# Ask "why does this line exist?" - resolves file:line to a commit
# via git blame, then prints reasoning records for that commit.
arf why src/api.rs:155

# Human-readable history.
arf log

# Combined git + reasoning tree.
arf graph

# A single commit with reasoning + diff.
arf diff

# Machine-shaped output for downstream tooling.
arf export --format json
arf export --format jsonl --since 2026-06-01
arf export --format toml --commit abc123

# Interactive TUI browser.
arf browse

# Push/pull the orphan branch from origin.
arf sync
```

## Visualization

### `arf why` - the headline command

```
$ arf why src/api.rs:155

Reasoning for src/api.rs:155 (last touched in commit a3f9c012):

what: Add retry logic
why:  Transient API failures cause cascading 503s under load
how:  3-attempt loop with 100/200/400ms backoff
time: 2026-06-08T14:23:11+00:00
```

### `arf graph` - git history with reasoning

```
Git + ARF History:

|-* 8ae882e Add diff command with ARF reasoning context
|  +-- what: Add diff command
|      why: Combine git diff with ARF reasoning for full context review
|      how: Shows reasoning header then git show output
|-* 5604413 Add graph command for unified git+arf visualization
|  +-- what: Add graph command
|      why: User requested visualization combining git commits with reasoning
|      how: Matches commit SHAs to .arf/records/ directories
+-* 8ec6c98 Add ARF CLI reference implementation
   +-- what: Implement ARF CLI v0.1
       why: Need reference implementation for spec
       how: Rust CLI with init/record/log/sync commands
```

### `arf diff` - single commit with reasoning + changes

```
===============================================================
Commit: 8ae882e Add diff command with ARF reasoning context
===============================================================

REASONING:
  what: Add diff command
  why:  Combine git diff with ARF reasoning for full context review
  how:  Shows reasoning header then git show output

---------------------------------------------------------------
CHANGES:

 src/main.rs | 118 +++++++++++++++++++++++++++
 1 file changed, 118 insertions(+)
```

## Storage

ARF uses an orphan git branch mounted as a worktree at `.arf/`:

```
your-repo/
+-- .arf/                    # mounted worktree (arf branch)
|   +-- README.md
|   +-- records/
|       +-- 8ae882e6/        # records by short commit SHA
|       |   +-- claude-20260202-211845.toml
|       +-- 5604413/
|           +-- claude-20260202-211532.toml
+-- .git/
+-- .gitignore               # contains .arf/
+-- src/
```

The orphan branch keeps reasoning history separate from code
history. Standard git operations work (push, pull, merge). A fresh
clone of a repo that already has an `arf` branch gets attached to
that branch via `arf init` rather than needing manual setup.

## Use cases

- **PR review**: see why an agent made each change, not just the
  diff.
- **Multi-agent handoff**: pass structured reasoning between agents
  via the `agent` field.
- **Audit trail**: keep records of AI decisions for compliance.
- **Debugging**: when AI-generated code breaks, find the reasoning
  that produced it.
- **`arf why <file>:<line>`**: the per-line lookup that closes the
  loop between code and decision.

## Using with Claude Code

The companion skill at <https://github.com/ducks/arf-skill> teaches
Claude Code when to emit ARF records during a session. Install via
[skillz](https://github.com/ducks/skillz) or drop `SKILL.md` into
your Claude Code skills directory:

```bash
skillz install github:ducks/arf-skill
```

After install, Claude will emit `arf record` calls automatically at
significant decision points - before non-trivial changes, before
commits, when changing strategy, when recovering from failures.

## Status

The CLI is on crates.io as `arf-cli`. The format spec is at
[SPEC.md](SPEC.md). Feedback welcome via GitHub issues.
