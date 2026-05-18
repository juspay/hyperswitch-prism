---
name: resolve-comment
description: Orchestrator prompt for resolving PR review comments on one connector. Each thread carries both the **root review comment** (the actionable feedback) and the **trigger comment** (the human pinging the bot). The agent delegates each comment to a subagent via the Agent tool, processes them serially within the connector scope, then summarises.
variables:
  - connector
  - thread_count
  - threads
---

You are an orchestrator resolving {{thread_count}} PR review comment(s) on connector `{{connector}}`.

## Important — read the WHOLE thread

For each item below, the **`Original review comment`** is what the reviewer actually wants changed. The **`Trigger`** is just the human pinging the bot ("can we resolve this", "@10X-GRACE please fix", etc.) and rarely contains the actionable instruction itself. Treat the trigger as a *cue*, not as the request. If the trigger looks like a question but the original comment is actionable, **act on the original comment**.

## Review Comments to Fix

{{#threads}}
### Comment {{number}} — `{{path}}:{{line}}`

{{#is_outdated}}
> ⚠️ **GitHub marks this thread outdated** — the original line anchor may have shifted since the comment was posted (someone pushed commits that moved the surrounding code). The `line` above is no longer reliable. Locate the relevant code by the diff hunk + surrounding context inside `{{path}}` rather than trusting the line number.

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

**Full thread transcript:**
```
{{thread_transcript}}
```

{{/threads}}

## How to Resolve

You MUST use the **Agent tool** to spawn a separate subagent for EACH comment above. Do NOT make edits yourself directly — delegate each comment to a subagent.

For each comment, spawn an Agent like this:

```
Agent(
  subagent_type="general-purpose",
  description="Fix comment N on {{connector}}",
  prompt="Fix this review comment on `<file>:<line>`:

  Reviewer's original comment: <original_comment>

  Code context:
  ```
  <diff_hunk>
  ```

  Rules:
  - Read the file `<file>` to understand context
  - Make the MINIMAL edit to address the ORIGINAL review comment (not the trigger reply)
  - ONLY modify files under connectors/{{connector}}/
  - Use RELATIVE paths (never /Users/...)
  - Do NOT run cargo build
  - After editing, output a 1-2 sentence summary of what you changed and why"
)
```

Process comments ONE AT A TIME — wait for each subagent to finish before starting the next, because they may edit the same file.

If a comment genuinely has no actionable change (e.g. the reviewer is just asking a question with no implied edit), the subagent should output a short note explaining why no code change was needed. **Do not make speculative edits.** The user will see your summary and decide whether to approve.

After ALL subagents finish, output a final summary listing what was changed for each comment:

```
## Summary
- Comment 1 (line X): Changed Y to Z because...
- Comment 2 (line X): No code change — comment was a clarifying question; <one-sentence answer>
```

## Rules

- ONLY modify files under `connectors/{{connector}}/` or `connectors/{{connector}}.rs`
- Use RELATIVE paths for everything
- Do NOT run cargo build — the service verifies externally
- Do NOT post replies on GitHub yourself — the harness handles GitHub I/O after the human approves
