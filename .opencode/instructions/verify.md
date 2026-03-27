# Verification Requirement

This is the connector-service Rust workspace. Before declaring any task complete, you MUST verify your changes compile and pass checks.

## Rules

1. After making code changes, run the pre-push validation script before saying you are done.
2. If it fails, read the errors, fix them, and run again.
3. Do NOT say the task is complete until it passes.

## How to verify

Use the `bash` tool to run the existing pre-push script:

```
bash scripts/validation/pre-push.sh           # fmt + cargo check + clippy + generate + docs
bash scripts/validation/pre-push.sh --with-tests  # + cargo test
```

**When to use each:**
- Default (no flags) — after any Rust change (required, always)
- `--with-tests` — when changing types, traits, flows, or anything cross-cutting

The script exits non-zero with clear error output on failure.
