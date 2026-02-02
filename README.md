# ARF: Agent Reasoning Format

A standard format for structured agent reasoning, tracked alongside git.

## The Problem

AI agents modify code, make decisions, take actions. Their reasoning is:
- Buried in chat logs
- Lost after the session ends
- Unstructured and hard to review

**Review the reasoning, not just the diff.**

## The Solution

ARF is a simple TOML schema for capturing agent reasoning:

```toml
what = "Add validation to prevent SQL injection"
why = "Email field passes unsanitized input to query"
how = "Use parameterized queries in register_user()"
backup = "Revert if tests fail"
```

## CLI

```bash
# Initialize ARF tracking (creates orphan branch at .arf/)
arf init

# Record reasoning for current work
arf record --what "Add retry logic" --why "Transient API failures"

# View reasoning history
arf log

# Combined git + reasoning visualization
arf graph

# Show diff with reasoning context
arf diff
```

## Visualization

### `arf graph` - Git history with reasoning

```
Git + ARF History:

├─● 8ae882e Add diff command with ARF reasoning context
│  └─ what: Add diff command
│      why: Combine git diff with ARF reasoning for full context review
│      how: Shows reasoning header then git show output
├─● 5604413 Add graph command for unified git+arf visualization
│  └─ what: Add graph command
│      why: User requested visualization combining git commits with reasoning
│      how: Matches commit SHAs to .arf/records/ directories
├─● 8ec6c98 Add ARF CLI reference implementation
│  └─ what: Implement ARF CLI v0.1
│      why: Need reference implementation for spec
│      how: Rust CLI with init/record/log/sync commands
└─● 3384a83 Initial ARF spec v0.1
```

### `arf diff` - Single commit with reasoning + changes

```
═══════════════════════════════════════════════════════════════
Commit: 8ae882e Add diff command with ARF reasoning context
═══════════════════════════════════════════════════════════════

REASONING:
  what: Add diff command
  why:  Combine git diff with ARF reasoning for full context review
  how:  Shows reasoning header then git show output

───────────────────────────────────────────────────────────────
CHANGES:

 src/main.rs | 118 +++++++++++++++++++++++++++
 1 file changed, 118 insertions(+)
```

### `arf log` - Reasoning records

```
ARF Records (3):

commit 8ae882e
what: Add diff command
why: Combine git diff with ARF reasoning for full context review
how: Shows reasoning header then git show output
time: 2026-02-02T21:18:45+00:00

commit 5604413
what: Add graph command
why: User requested visualization combining git commits with reasoning
time: 2026-02-02T21:15:32+00:00
```

## Storage

ARF uses an orphan git branch mounted as a worktree at `.arf/`:

```
your-repo/
├── .arf/                    # Mounted worktree (arf branch)
│   ├── README.md
│   └── records/
│       ├── 8ae882e6/        # Records by commit SHA
│       │   └── claude-20260202-211845.toml
│       └── 5604413/
│           └── claude-20260202-211532.toml
├── .git/
├── .gitignore               # Contains .arf/
└── src/
```

Benefits:
- Reasoning history separate from code history
- No pollution of main branch commits
- Standard git operations (push, pull, merge)
- Works with existing git workflows

## Use Cases

- **PR Review**: See why an agent made each change, not just the diff
- **Multi-Agent**: Pass structured reasoning between agents
- **Audit Trail**: Keep records of AI decisions for compliance
- **Debugging**: Understand what went wrong when AI-generated code breaks

## Installation

```bash
cargo install --git https://github.com/ducks/arf
```

## Spec

See [SPEC.md](SPEC.md) for the full specification.

## Status

v0.1 - Reference implementation. Feedback welcome.
