---
name: grpc-test-gen
description: Generate grpcurl test commands for a PR that has no Testing section in its body. Uses the connector name, the cumulative diff, and a hint about available creds so Claude writes commands that actually exercise the modified flow against the local gRPC server.
variables:
  - connector
  - pr_title
  - pr_body
  - diff
  - grpc_port
  - creds_hint
---

You are generating `grpcurl` test commands for the local gRPC server (`localhost:{{grpc_port}}`) so the **{{connector}}** connector changes in this PR can be exercised end-to-end. There is no `## Testing` section in the PR body, so the harness is asking you to write one.

## PR

**Title:** {{pr_title}}

**Body (may be empty):**

{{pr_body}}

## Cumulative diff since the baseline (truncated tail)

```diff
{{diff}}
```

## Credentials hint

{{creds_hint}}

## How to reply

Reply with a **single fenced bash code block** containing one `grpcurl` invocation per line. No prose, no explanation, no other code blocks. Example shape:

```bash
grpcurl -plaintext -d '{"amount": 1000, ...}' localhost:{{grpc_port}} ucs.PaymentService/Authorize
grpcurl -plaintext -d '{"transaction_id": "..."}' localhost:{{grpc_port}} ucs.PaymentService/PSync
```

Rules:

- Use only `grpcurl -plaintext` (the server is local, no TLS).
- Cover the **modified flow(s)** of `{{connector}}` only — don't write tests for unrelated paths.
- Prefer realistic payloads (use values from the creds hint when relevant).
- Keep the list short: 1–5 commands is plenty.
- Do NOT chain commands with `&&`; one command per line.
- Do NOT include `cd`, `cargo`, or any non-grpcurl lines — the harness runs each line independently.
