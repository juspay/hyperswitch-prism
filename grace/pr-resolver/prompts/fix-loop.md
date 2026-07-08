---
name: fix-loop
description: Build-fix retry prompt. Sent after cargo build or clippy fails on the resolver's edits. Keeps the original review-comment intent visible while showing the new errors. Used with `claudeSessionId` + `incremental: true` so Claude resumes the same conversation as the first attempt.
variables:
  - connector
  - loop_iteration
  - threads
  - error_output
---

You were resolving review comments on connector `{{connector}}`, but the build/clippy check failed.

This is fix attempt {{loop_iteration}}. Please fix the errors below while STILL addressing the original review comments.

## Original Review Comments

{{#threads}}
### Comment {{number}} — `{{path}}:{{line}}`

{{#is_outdated}}
> ⚠️ Thread is marked outdated — line anchor may have shifted; locate by context inside `{{path}}`.

{{/is_outdated}}
{{#has_original}}
**Original review comment** by @{{original_author}}:
> {{original_comment}}

{{/has_original}}
**Trigger** by @{{author}}: {{instruction}}

{{#diff_hunk}}
**Code context:**
```diff
{{diff_hunk}}
```
{{/diff_hunk}}

{{/threads}}

## Build/Clippy Errors

```
{{error_output}}
```

## Instructions

1. Read the error messages carefully — they tell you exactly what's wrong.
2. Fix the errors while preserving the intent of the **original review comments** (not the trigger replies).
3. If a fix is incompatible with building, revert that specific fix and leave the code as it was.
4. ONLY modify files under `connectors/{{connector}}/` or `connectors/{{connector}}.rs`.
5. Do NOT run cargo build yourself — the service will verify externally.
6. Use RELATIVE paths for all Read/Edit operations — never use absolute paths starting with /Users/.
