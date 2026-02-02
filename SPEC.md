# ARF: Agent Reasoning Format

A standard format for structured agent reasoning.

## Why

AI agents modify code, make decisions, and take actions. Today, their reasoning
is buried in chat logs or lost entirely. ARF provides a standard format for
agents to declare intent and capture reasoning in a way that:

- Humans can review (in PRs, audits, postmortems)
- Other agents can consume (multi-agent workflows)
- Tools can validate and process

## Core Schema

An ARF record has these fields:

```json
{
  "what": "string (required)",
  "why": "string (required)",
  "how": "string (optional)",
  "backup": "string (optional)",
  "outcome": "success | failure | partial (optional)",
  "context": {}
}
```

### Required Fields

**what** - Concrete action being taken. Not "I will analyze" but "Add
validation to prevent negative values in calculate_total()".

**why** - Reasoning behind this approach. Why this solution over alternatives?
What problem does it solve?

### Optional Fields

**how** - Implementation details. Code snippets, file paths, specific changes.

**backup** - Rollback plan if it fails. What to do if this breaks something.

**outcome** - Result after execution: `success`, `failure`, or `partial`.
Can include details: `{"outcome": "failure", "reason": "tests failed"}`.

**context** - Arbitrary metadata. Timestamps, commit SHAs, session IDs, etc.

## Usage Patterns

### 1. Pre-Action Declaration

Agent declares intent BEFORE acting:

```json
{
  "what": "Add input validation to user registration",
  "why": "Current code allows SQL injection via email field",
  "how": "Add parameterized queries in register_user()",
  "backup": "Revert commit if integration tests fail"
}
```

### 2. Post-Action Record

Agent records what happened AFTER acting:

```json
{
  "what": "Add input validation to user registration",
  "why": "Current code allows SQL injection via email field",
  "how": "Added parameterized queries in register_user()",
  "outcome": "success",
  "context": {
    "commit": "abc123",
    "files_changed": ["src/auth.rs"]
  }
}
```

### 3. Multi-Agent Handoff

Agent A passes structured reasoning to Agent B:

```json
{
  "what": "Identified N+1 query in user_posts()",
  "why": "Each post triggers separate author lookup",
  "how": "Recommend eager loading with includes(:author)",
  "context": {
    "source_agent": "analyzer",
    "confidence": 0.95
  }
}
```

## Prompt Template

To get ARF-formatted output from an LLM:

```
Before acting, output your reasoning in this format:

### What
Concrete action you're taking (not "I will analyze" - be specific)

### Why
Reasoning behind this approach

### How
Implementation details (optional)

### Backup
Rollback plan if it fails (optional)

DO NOT narrate your investigation. Output ONLY the structured reasoning.
```

## Examples

### Bug Fix

```json
{
  "what": "Fix off-by-one error in pagination",
  "why": "Users report missing last item on each page. Loop uses < instead of <=",
  "how": "Change line 42: `i < count` to `i <= count`",
  "backup": "Revert if pagination tests fail",
  "outcome": "success",
  "context": {
    "issue": "#123",
    "commit": "def456"
  }
}
```

### Architecture Decision

```json
{
  "what": "Use Redis for session storage instead of PostgreSQL",
  "why": "Session lookups are 100x more frequent than writes. Redis reduces p99 latency from 50ms to 2ms",
  "how": "Add redis-rs dependency, implement SessionStore trait",
  "backup": "Keep PostgreSQL adapter as fallback if Redis unavailable"
}
```

### Code Review Finding

```json
{
  "what": "Flag potential race condition in cache invalidation",
  "why": "delete() and set() not atomic - stale data possible under load",
  "how": "Recommend using WATCH/MULTI/EXEC or lua script",
  "context": {
    "file": "src/cache.rs",
    "line": 89,
    "severity": "medium"
  }
}
```

## File Format

ARF records can be stored as:

- **JSON** - Single record or array of records
- **JSONL** - One record per line (for logs/streams)
- **Embedded** - In commit messages, PR descriptions, comments

## Versioning

This is ARF v0.1. The format is intentionally minimal. Extensions should use
the `context` field rather than adding new top-level fields.
