# Tech Spec Agent

You generate the tech spec for **{CONNECTOR}** using `grace techspec`. This must complete before any code work begins.

---

## Inputs

| Parameter | Description | Example |
|-----------|-------------|---------|
| `{CONNECTOR}` / `{Connector_Name}` | Connector name (exact casing from connector list) | `Adyen` |
| `{FLOW}` | Payment flow being implemented | `BankDebit`, `MIT`, `Wallet` |

Use `{Connector_Name}` (exact casing) for `grace techspec` commands. Use `{connector}` (lowercase) for file names.

---

## Phase 1a: Extract URLs for the connector

The Links Agent has already gathered documentation URLs and written them to `data/integration-source-links.json`. Extract the URLs for this connector:

```bash
# From connector-service root:
cat data/integration-source-links.json | jq -r '."{Connector_Name}" // ."{connector}" // empty | .[]' 2>/dev/null
```

If `data/integration-source-links.json` does not exist or has no entry for this connector, return FAILED with reason "No documentation links found for {CONNECTOR}. Run the Links Agent (2.1_links.md) first."

---

## Phase 1b: Create the URL file

Create a file `{connector_name}.txt` (lowercase) containing the URLs, one per line:

```bash
# From connector-service root:
cat data/integration-source-links.json | jq -r '."{Connector_Name}" // ."{connector}" // empty | .[]' > {connector_name}.txt
```

Verify the file is non-empty:
```bash
wc -l {connector_name}.txt
```

If the file is empty (0 lines), return FAILED with reason "No URLs extracted for {CONNECTOR}."

---

## Phase 1c: Run grace techspec

```bash
# IMPORTANT: Switch to grace/ directory and activate virtualenv
# workdir: connector-service/grace
source .venv/bin/activate
cat ../{connector_name}.txt | grace techspec {Connector_Name} -e
```

**Wait for the `grace techspec` command to complete before proceeding.** This can take approximately 20 minutes. Do NOT interrupt or proceed until the techspec is fully generated.

**Critical**:
- The working directory MUST be `grace/` when running this command.
- The virtual environment MUST be activated first.
- The `-e` flag is required.

---

## Phase 1d: Verify tech spec output

After `grace techspec` finishes, verify the spec was created:

```bash
# From connector-service root:
find grace/rulesbook/codegen/references -iname "*{connector}*" | head -20
```

If no spec was generated, return FAILED.

---

## Output

```
CONNECTOR: {CONNECTOR}
FLOW: {FLOW}
STATUS: SUCCESS | FAILED
TECHSPEC_PATH: <path to generated tech spec, if successful>
REASON: <details, if failed>
```

---

## Rules

1. **No code generation** — this agent only produces the tech spec, nothing else.
2. **Verify before reporting success** — always confirm the file exists on disk.
3. **Clean up on failure** — if the grace command fails, capture the error output and include it in the `REASON` field.
4. **Idempotent** — if a tech spec already exists for this connector, report SUCCESS with the existing path (do not regenerate).
5. **URL source** — URLs come from `data/integration-source-links.json`, written by the Links Agent. Do NOT expect or require an integration details file.
