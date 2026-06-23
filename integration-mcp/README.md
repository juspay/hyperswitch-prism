# hyperswitch-prism-integration-mcp

An [MCP](https://modelcontextprotocol.io) server that takes a merchant developer from **zero to a working `hyperswitch-prism` payment integration** inside their own codebase — with minimal friction.

Install it into your AI coding agent (Claude Code, Cursor, Windsurf) and say:

> "Add Hyperswitch Prism to my Express app so I can charge cards through Stripe."

The agent then uses this server to detect your stack, fetch the connector's **exact** credential fields, scaffold the client + flow routes with correct types, generate `.env` + config, and run a sandbox test charge to confirm.

This is a **developer-experience / integration assistant**, not a runtime payments gateway. Every tool returns copy-pasteable output; the agent writes the files into your repo.

## Why it exists

Integrating a payments SDK by hand means reading proto files to learn each connector's credential fields, memorizing numeric status codes, and guessing request shapes. This server removes that: **every connector field name and status code is sourced from the SDK's own `payment.proto`** at build time (96 connectors), so there are no hallucinated symbols.

## Tools

| Tool | What it does |
|---|---|
| `prism_integration_guide` | **Start here.** Ordered, end-to-end plan for a framework + flows, each step mapped to the next tool. |
| `prism_list_connectors` | List supported processors (96, from proto), with optional search. |
| `prism_connector_requirements` | A connector's exact credential fields, secret wrapping, config template, sandbox cards. |
| `prism_scaffold_integration` | Generate `{ path, contents }` code files (client + flow handlers + routes + `.env.example`). |
| `prism_generate_config` | `PRISM_CONNECTOR_CONFIG` JSON + `.env` / `.env.example` (secrets stay in env). |
| `prism_validate_config` | Validate a candidate config; precise list of what's missing/malformed + fixes. |
| `prism_explain_error` | Plain-language cause + fix for error classes, codes, and FAILURE-status declines. |
| `prism_status_reference` | Meaning of the numeric `PaymentStatus` / `RefundStatus` codes. |
| `prism_doctor` | Environment check: Node, platform/arch vs the shipped native binary, SDK load. |
| `prism_test_charge` | Runs a **real sandbox** `authorize` to prove the setup. Reads creds from env; enforces `test_mode`. |

It also exposes three MCP **resources**: `prism://guide`, `prism://status-table`, and `prism://llm-txt` (cached fetch of the official reference).

## Install / run

Requires Node 18+.

```bash
npx -y hyperswitch-prism-integration-mcp
```

### Claude Code / Claude Desktop

```json
{
  "mcpServers": {
    "hyperswitch-prism": {
      "command": "npx",
      "args": ["-y", "hyperswitch-prism-integration-mcp"],
      "env": {
        "PRISM_CONNECTOR_CONFIG": "{\"connectorConfig\":{\"stripe\":{\"apiKey\":{\"value\":\"sk_test_...\"}}}}"
      }
    }
  }
}
```

`PRISM_CONNECTOR_CONFIG` is only needed for `prism_test_charge`; all other tools work without it.

### Cursor / Windsurf

Add to `.cursor/mcp.json` (or the Windsurf equivalent):

```json
{
  "mcpServers": {
    "hyperswitch-prism": { "command": "npx", "args": ["-y", "hyperswitch-prism-integration-mcp"] }
  }
}
```

### HTTP transport (optional)

```bash
npx hyperswitch-prism-integration-mcp --http --port 3000
```

## Status codes (the #1 gotcha)

`response.status` is a **number**, not a string. Compare against `types.PaymentStatus.*` / `types.RefundStatus.*`:

| code | meaning |
|---|---|
| 6 | `AUTHORIZED` (funds held; capture required for MANUAL) |
| 8 | `CHARGED` (success) |
| 11 | `VOIDED` |
| 20 | `PENDING` (async; poll with `get`) |
| 21 | `FAILURE` — **soft decline returned in the body, not thrown** |

Refunds use `RefundStatus` (success = `REFUND_SUCCESS` = 4). Hard failures throw `IntegrationError` / `ConnectorError` / `NetworkError`.

## Platform support

The SDK ships a native FFI library for **linux-x64** and **macOS (arm64/x64)** only. There is **no linux-arm64 binary**, so it cannot run on ARM Linux (e.g. AWS Graviton, some CI runners).

The linux `.so` is also built against **glibc ≥ 2.38**, so older distros (Ubuntu 22.04 / glibc 2.35, Debian 11, etc.) cannot load it even on x64 — you'll get `version 'GLIBC_2.38' not found`. Use a newer base image (Ubuntu 24.04, Debian 12).

`prism_doctor` doesn't just check the arch — it actually constructs a `PaymentClient` (no network) to **really load the native library**, so it catches arch *and* glibc/loader failures and tells you the fix. `prism_test_charge` performs the same check before attempting a charge.

## Develop

```bash
npm install
npm run gen:connectors   # regenerate connector data from payment.proto
npm run build            # tsc + copy data
npm run smoke            # JSON-RPC initialize + tools/list + per-tool round-trip
npm run inspect          # @modelcontextprotocol/inspector
```

Connector data is generated from `payment.proto` into `src/data/connectors.generated.json` (committed). CI re-runs the codegen and fails on drift.

## License

Apache-2.0
