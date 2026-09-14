# PR Resolver Prompts

Markdown prompts consumed by `grace/grace-workspace/packages/core/src/pr-resolver/prompts.ts`. The loader anchors lookup at `cfg.projectRoot + 'grace/pr-resolver/prompts/'`. Each file has YAML frontmatter declaring `name`, `description`, and `variables`. The body is rendered with Mustache; HTML escaping is disabled in the loader so variable values pass through verbatim (cargo output, diff hunks, etc., are not mangled).

## Files

| File | Used when | Required variables |
| --- | --- | --- |
| `resolve-comment.md` | First Claude call per sub-task (all triggered comments on one connector). | `connector`, `thread_count`, `threads`, `revision_feedback` (optional — Mustache section, body wrapped) |
| `fix-loop.md` | After `cargo build` / `cargo clippy` fails. Resumes the same Claude session via `claudeSessionId` + `incremental: true`. | `connector`, `loop_iteration`, `threads`, `error_output` |
| `grpc-test-plan.md` | Generating a structured grpcurl test plan from a PR's connector changes. | `connector`, `pr_title`, `pr_body`, `pr_comments`, `diff`, `grpc_port`, `creds_block`, `service_hint` |
| `review-summary.md` | Composing the per-thread reply posted back to GitHub after the resolver runs. | `connector`, `pr_number`, `threads`, `diff` |

## `threads` shape

Each element of the `threads` array:

| Field | Type | Notes |
| --- | --- | --- |
| `number` | string | 1-based position in this sub-task. |
| `path` | string | File path relative to repo root. |
| `line` | string | `'?'` when the GitHub thread has no line anchor. |
| `author` | string | GitHub login of the reviewer who left the trigger comment. |
| `instruction` | string | Comment body with the trigger tag (`@HS-prism-bot`) stripped. |
| `diff_hunk` | string \| undefined | Optional; the `Code context` block in the template is wrapped in a Mustache section so it's omitted when this is falsy. |
| `is_outdated` | bool \| undefined | Optional; Mustache section in `resolve-comment.md` / `fix-loop.md` surfaces an "outdated" warning when truthy. |
| `has_original` | bool \| undefined | Optional; gates the "Original review comment" block in `resolve-comment.md` / `fix-loop.md` / `review-summary.md`. |
| `original_author` | string \| undefined | Optional; reviewer login of the original (pre-revision) comment. Rendered inside the `has_original` section. |
| `original_comment` | string \| undefined | Optional; body of the original (pre-revision) comment. Rendered inside the `has_original` section. |
| `thread_transcript` | string \| undefined | Optional; full transcript of the thread (chronological). Rendered by `resolve-comment.md` / `fix-loop.md` so the model sees the full back-and-forth. |

## Editing prompts

Prompt edits do not require a TypeScript rebuild — the loader reads from disk on every call. Keep variable names in the body in sync with the `variables:` list in the frontmatter; the loader logs a warning if a referenced variable is missing or if a declared variable is unused.

## Not in this directory

- Connector-codegen skills (L1–L4) live under `grace/grace-workspace/packages/core/skills/` and are loaded by the 10xgrace pipeline, not the PR resolver. Don't mix the two.
