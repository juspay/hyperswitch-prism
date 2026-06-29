---
name: review-summary
description: After cargo + grpc pass, ask Claude to write a reviewer-facing summary of what changed and why. Runs as a fresh session with the final diff + review comments, so the reviewer can approve or reject in ~30 seconds without re-reading the whole diff.
variables:
  - connector
  - pr_number
  - threads
  - diff
---

A reviewer needs to approve or reject a set of code changes you (a separate Claude session) just produced on connector `{{connector}}` (PR #{{pr_number}}).

Your job is to write a **short, plain-language summary** so the reviewer can decide quickly. The reviewer will read your summary first, then spot-check the diff below.

## Review comments that drove these changes

{{#threads}}
**Comment {{number}} — `{{path}}:{{line}}`** by @{{author}}:
> {{instruction}}
{{#has_original}}
> (original by @{{original_author}}: {{original_comment}})
{{/has_original}}

{{/threads}}

## Final diff

```diff
{{diff}}
```

## What to write

Output a markdown document with these exact sections. No preamble, no closing remarks. Keep the whole thing under 300 words.

## TL;DR
One or two sentences. What did the reviewer ask for, and what did you do? Be concrete.

## What changed
- Bullet per logical change, not per file. Name the enum variant, struct field, trait impl, or function — concrete Rust identifiers, not paraphrases.
- If you changed an error path to a success path (or vice versa), say so explicitly.

## Why
- Plain English. What constraint or convention drove the choice?
- If you consulted other files in the repo, similar connectors, or trait definitions, cite the path so the reviewer can spot-check.

## Files touched
- `path/to/file.rs` (Lx–Ly) — one-line role of the change

## Risks
- Anything the reviewer should look at twice. "None obvious" is fine if true.

**Hard rules** (meta-instructions for you, not output):
- Do NOT include `## Summary`, "The change is in place and correct," or any other preamble.
- Do NOT restate the review comment verbatim — paraphrase what they wanted in one phrase.
- Do NOT include code blocks for the changes themselves; the diff is already shown above.
- Identifiers go in backticks; file paths go in backticks.
- Under 300 words total.
