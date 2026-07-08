---
name: grpc-test-plan
description: Generate a structured grpcurl test plan that exercises a PR's connector changes against the local gRPC server. Reads PR description / diff / connector creds and emits a single JSON object with named, dependency-ordered test steps. Replaces the older bash-block-of-grpcurls flow — the runner now substitutes values from prior step responses, so chained sequences (Authorize → Capture → Reverse) actually work.
variables:
  - connector
  - pr_title
  - pr_body
  - pr_comments
  - diff
  - grpc_port
  - creds_block
  - service_hint
---

You are building a **gRPC test plan** that exercises the changes in this PR against a local server at `localhost:{{grpc_port}}`. The harness will run each step via `grpcurl`, capture responses, and chain values between steps. Output is a single JSON object — no prose, no bash, no markdown around the JSON.

## Connector

`{{connector}}`

## PR

**Title:** {{pr_title}}

**Body:**

{{pr_body}}

**Conversation comments** (chronological, includes ad-hoc test snippets reviewers may have dropped here):

{{pr_comments}}

## Cumulative diff (truncated)

```diff
{{diff}}
```

## Credentials for this connector

These come from `creds.json`. Use the **actual values** verbatim in headers / payloads — never `<api_key>` or `REDACTED`.

```json
{{creds_block}}
```

## Service surface hint

{{service_hint}}

## Output schema

Reply with exactly one JSON object, no surrounding text. The runner will `JSON.parse` it directly.

```json
{
  "tests": [
    {
      "name": "string (unique within the plan, snake_case, e.g. 'authorize_card_manual_capture')",
      "method": "string (full gRPC method, e.g. 'types.PaymentService/Authorize')",
      "depends_on": "string (optional — name of an earlier step this one needs)",
      "headers": {
        "x-connector": "{{connector}}",
        "x-auth": "header-key | body-key | signature-key (match the creds.auth_type)",
        "x-api-key": "<real value from creds_block>"
      },
      "body": {
        "...": "...",
        "connector_transaction_id": "{{=<% %>=}}{{ prior_step_name.connector_transaction_id }}<%={{ }}=%>"
      },
      "captures": {
        "connector_transaction_id": "$.connectorTransactionId",
        "status": "$.status"
      },
      "expect": {
        "exit_code": 0,
        "status_in": ["AUTHORIZED", "PENDING"]
      }
    }
  ]
}
```

### Field semantics

- **`name`** — unique snake_case label. Used by `depends_on` and `{{=<% %>=}}{{ … }}<%={{ }}=%>` templating.
- **`method`** — the full proto method. The runner sends `grpcurl ... localhost:{{grpc_port}} <method>`.
- **`depends_on`** — when set, this step is skipped if the parent failed or was skipped.
- **`headers`** — sent as `-H "k: v"`. Fill in real credentials from the `creds_block` above.
- **`body`** — JSON object; the runner stringifies and sends via `-d`. You can interpolate prior captures with Mustache: `{{=<% %>=}}{{ step_name.capture_key }}<%={{ }}=%>`. The runner resolves these before execution.
- **`captures`** — JSONPath subset (`$.field.nested`) extracted from the grpcurl stdout (the gRPC response, JSON-decoded). Captures are scoped under the step's `name`.
- **`expect`** — optional rules the runner enforces:
  - `exit_code: 0` — grpcurl must exit 0 (default if omitted).
  - `status_in: ["X", "Y"]` — match against the response's status. The runner reads `captures.status` first, then falls back to `$.status`, `$.response.status`, `$.payments_response.status` on the parsed response — so even if your capture path is wrong, the check still has a chance to pass. Hyperswitch-prism's responses use camelCase fields at the root (e.g. `status`, `connectorTransactionId`, `merchantTransactionId`); only use a `response.*` prefix if the connector you're targeting actually wraps its payload.
  - `response_contains: "text"` — substring match in stdout.

### Sequences

Many connector flows require setup. Typical patterns:

- **Authorize → Capture → Reverse** (manual capture flow): Authorize captures `connector_transaction_id`; Capture uses it via `{{=<% %>=}}{{ authorize.connector_transaction_id }}<%={{ }}=%>` and may itself capture a `capture_id`; Reverse uses whichever id the connector expects.
- **Authorize → PSync**: PSync verifies the authorization status.
- **Authorize → Refund → RSync**: same shape with refunds.

Use `depends_on` and captures rather than guessing values — placeholder strings like `"<connector_transaction_id_from_authorize>"` will be sent literally and the call will fail.

## Rules

1. **Reply with one JSON object.** No prose, no triple-backtick wrapper, no "Here is the plan" header. The very first character must be `{`, the very last `}`.
2. **Use real credential values** from the `creds_block`. Never invent or leave placeholders.
3. **Cover the modified flow** specifically — read the diff above to decide which methods the PR touches, and design a plan that exercises that surface.
4. **Sequence appropriately**: if the modified flow needs setup (e.g. you can't Reverse without Authorize + Capture first), include the setup steps and chain via `depends_on` + captures.
5. **Keep it tight** — 1 to 6 steps. The runner caps at 10 and rejects more.
6. **Don't include cleanup** unless it's part of the test surface (defer teardown to a future iteration).
