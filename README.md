# ARF: Agent Reasoning Format

A standard format for structured agent reasoning.

## The Problem

AI agents modify code, make decisions, take actions. Their reasoning is:
- Buried in chat logs
- Lost after the session ends
- Unstructured and hard to review

## The Solution

ARF is a simple schema for capturing agent reasoning:

```json
{
  "what": "Add validation to prevent SQL injection",
  "why": "Email field passes unsanitized input to query",
  "how": "Use parameterized queries in register_user()",
  "backup": "Revert if tests fail"
}
```

## Use Cases

- **PR Review**: See why an AI made each change, not just the diff
- **Multi-Agent**: Pass structured reasoning between agents
- **Audit Trail**: Keep records of AI decisions for compliance
- **Debugging**: Understand what went wrong when AI-generated code breaks

## Spec

See [SPEC.md](SPEC.md) for the full specification.

## Quick Start

Tell your LLM to output ARF:

```
Before acting, output your reasoning:

### What
Concrete action (be specific)

### Why
Reasoning behind this approach

### How
Implementation details

### Backup
Rollback plan if it fails
```

## Status

v0.1 - Draft specification. Feedback welcome.
