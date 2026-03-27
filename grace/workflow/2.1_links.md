# Backend Integration Links — Subagent Prompt

This is the **subagent prompt** invoked once per connector by the orchestrator. Each subagent runs independently, researches and verifies documentation links for a specific payment method, and writes verified links to the shared tracking file.

Replace `{{CONNECTOR_NAME}}` and `{{PAYMENT_METHOD}}` before invoking.

---

## THE PROMPT

````
You are a backend payment integration researcher. Your task is to **find, verify, and store official API documentation links** for integrating **{{PAYMENT_METHOD}}** via the **{{CONNECTOR_NAME}}** payment connector (BACKEND ONLY — no frontend SDK or hosted UI links).

You have access to the repository at the current working directory (`/Users/tushar.shukla/Downloads/Work/connectors-status`) which tracks connector implementation status across 62 payment connectors.

---

## CONTEXT LOADING (minimal — for file-write keys only)

Read these files ONLY to resolve write-target keys. Do NOT use any existing links or doc URLs as search starting points — all URL discovery must be from scratch.

1. **`data/connectors.json`** — Verify `{{CONNECTOR_NAME}}` exists in the canonical list (use exact casing as listed)

2. **`data/features.json`** — Look up the feature entry matching `{{PAYMENT_METHOD}}` to get the feature ID (e.g., `f2` for Apple Pay)

3. **`src/App.tsx` lines 87-159** — Look up `FEATURE_DOC_KEYS` map to get the `connector_info_key` for the feature ID (e.g., `applepay`, `bank_debit`). This key is needed for writing to `connector-info.json` and `integration-source-links.json`.

If `{{CONNECTOR_NAME}}` is not in `data/connectors.json`, report the error and stop.

**IMPORTANT: FRESH SEARCH ONLY**
- Do NOT read or reference any existing `integration_doc_url` values from `data/connector-info.json`
- Do NOT use any previously stored links as starting points
- Do NOT skip this connector because it was "already processed" in `data/integration-source-links.json`
- Every URL must be discovered fresh through the search process in Phase 1

---

## PHASE 1: DOCUMENTATION DISCOVERY

### 1A. Gather Candidate URLs

Discover documentation URLs **from scratch** using these strategies (do NOT use any existing stored links):

1. **Find the connector's developer docs site** — Try fetching common patterns to locate the official docs:
   - `https://developer.{{connector_domain}}/`
   - `https://docs.{{connector_domain}}/`
   - `https://developers.{{connector_domain}}/`
   - `https://{{connector_domain}}.readme.io/`
   - `https://api.{{connector_domain}}/docs/`
   - The connector may use a parent company domain (e.g., Braintree uses `developer.paypal.com/braintree/`)
   - The connector may refer to their API as "CNP API" (Card Not Present) or use other branding

2. **Navigate from the docs home to find {{PAYMENT_METHOD}} pages** — Once you find the docs site, look for:
   - `{{docs_base}}/payment-methods/{{payment_method_slug}}`
   - `{{docs_base}}/guides/{{payment_method_slug}}`
   - `{{docs_base}}/{{payment_method_slug}}/api-only`
   - `{{docs_base}}/{{payment_method_slug}}/server-side`
   - `{{docs_base}}/api/reference` (general API reference)

3. **Try alternative naming** — The payment method may be called differently by this connector:
   - Apple Pay: "apple-pay", "applepay", "digital-wallets/apple-pay"
   - Google Pay: "google-pay", "googlepay", "digital-wallets/google-pay"
   - Bank Debit: "ach", "sepa-direct-debit", "direct-debit", "echeck", "e-check", "bank-debit"
   - 3DS: "3d-secure", "3ds", "three-d-secure", "3dsecure"
   - MIT: "recurring", "merchant-initiated", "stored-credentials", "card-on-file"

4. **Look for API reference pages** — Many connectors have a dedicated API explorer or reference:
   - `{{docs_base}}/api-explorer/`
   - `{{docs_base}}/api/reference/`
   - `{{docs_base}}/rest-api/`

### 1B. Categorize Found URLs

For each URL found, categorize it as one of:
- **api_reference** — Full API reference with endpoint details (POST URLs, request/response schemas)
- **payment_method_guide** — {{PAYMENT_METHOD}}-specific integration guide (backend flow, required params)
- **authentication_guide** — How to authenticate API requests (API keys, headers, HMAC)
- **webhooks_guide** — Webhook/notification setup, event types, payload format, signature verification
- **testing_guide** — Sandbox/test credentials, test card numbers, test scenarios
- **error_reference** — Error codes, decline codes, troubleshooting guide
- **getting_started** — General getting started / onboarding / account setup guide

### 1C. Filter for Backend Only

**EXCLUDE** any URL that is primarily about:
- Frontend SDKs (Drop-in UI, Web Components, React/iOS/Android SDKs)
- Hosted payment pages
- Client-side JavaScript integration
- Mobile app integration

**INCLUDE** only URLs about:
- Server-to-server / API-only / backend-to-backend integration
- REST API endpoints (POST /payments, etc.)
- Authentication headers and credential setup
- Webhook handling (server-side)
- API error codes and response handling

If a page covers both frontend and backend, include it but note "mixed frontend/backend content" in the notes.

---

## PHASE 2: DOCUMENTATION VERIFICATION

For each candidate URL, fetch the page using the WebFetch tool and verify:

### 2A. Accessibility Check
- Returns HTTP 200 (not 404, 403, login-wall, or redirect to homepage)
- Contains actual content (not an empty page, "coming soon", or paywall)
- Is API/technical documentation (not marketing, blog, case study, or press release)

Mark inaccessible URLs with `"verified": false` and note the reason.

### 2B. Content Verification — 10-Point Backend Spec Checklist

For each accessible URL, check which of these backend spec elements the documentation covers:

| # | Spec Element | What to Look For |
|---|-------------|-----------------|
| 1 | **API Endpoint** | A POST URL for creating/processing {{PAYMENT_METHOD}} payments (e.g., `POST /v1/payments`, `POST /secure_payments`) |
| 2 | **Authentication** | Method + required headers (API-Key, Basic Auth, X-Login, X-Trans-Key, Bearer, HMAC, X-Date, etc.) |
| 3 | **Request Schema** | JSON request body with {{PAYMENT_METHOD}}-specific fields documented |
| 4 | **Response Schema (Success)** | Success/pending/declined response structure with HTTP status codes |
| 5 | **Response Schema (Error)** | Error response structure with error codes |
| 6 | **{{PAYMENT_METHOD}} Parameters** | Parameters unique to this payment method (payment_method_type, token, encrypted_data, mandate refs, wallet tokens, bank routing, 3DS data, etc.) |
| 7 | **Idempotency** | Idempotency-Key header or unique reference mechanism documented |
| 8 | **Webhooks** | Event types, payload format, signature verification for async notifications |
| 9 | **Error Codes** | Enumerated error codes with meanings/descriptions |
| 10 | **curl Example** | Explicit curl command OR enough info to construct one (endpoint + auth + body all present) |

**Score each element**: YES (1 point), PARTIAL (0.5), NO (0)

**Aggregate score across ALL URLs found for this connector** (not per-URL):

- Score >= 7 → `"valid"` — Sufficient backend API documentation exists
- Score >= 4 and < 7 → `"problematic"` — Documentation has significant gaps
- Score < 4 → `"insufficient"` — Not enough documentation for backend integration

### 2C. Not Applicable Check

If the connector clearly does not support {{PAYMENT_METHOD}} (e.g., a crypto processor for bank debit, a voucher service for Apple Pay, a fraud tool like Signifyd), mark as `"not_applicable"`.

---

## PHASE 3: WRITE TO SHARED LINKS FILE

Read the current `data/integration-source-links.json` file, then add/update this connector's entry.

**The file structure is simple — just connector name → array of URLs:**

```json
{
  "{{CONNECTOR_NAME}}": [
    "https://docs.example.com/payments/apple-pay",
    "https://docs.example.com/api/reference",
    "https://docs.example.com/webhooks"
  ]
}
```

Only include URLs that were verified as accessible and contain actual backend API documentation. No metadata, no scores, no categories — just the raw verified links.

**CRITICAL RULES:**
- Read the existing file first — do NOT overwrite other connectors' entries
- If the connector already exists, replace its array with the new set of verified links

---

## PHASE 4: UPDATE connector-info.json

Update `data/connector-info.json` for this connector under the resolved `connector_info_key`:

```json
"<connector_info_key>": {
  "integration_doc_url": "<best_verified_url_or_null>",
  "notes": "<status>. Score: <X/10>. <what the doc covers>. Verified <YYYY-MM-DD>."
}
```

**Best URL selection:** Pick the single URL that has the highest individual coverage of the 10-point checklist. Prefer `payment_method_guide` category over generic `api_reference`.

Preserve ALL other existing fields in the connector's entry. Only update the specific feature key block.

---

## PHASE 5: SUMMARY OUTPUT

Return a structured summary to the orchestrator:

```
## Links Result: {{CONNECTOR_NAME}} / {{PAYMENT_METHOD}}

**Status:** valid | problematic | insufficient | not_found | not_applicable
**Score:** X/10
**Links Found:** N

### Verified Links
1. https://docs.example.com/payments/apple-pay
2. https://docs.example.com/api/reference
3. https://docs.example.com/webhooks

### Key Gaps
- No idempotency documentation found
- Webhook signature verification not documented

### Files Updated
- data/integration-source-links.json (UPDATED — added {{CONNECTOR_NAME}})
- data/connector-info.json (UPDATED — {{CONNECTOR_NAME}}.{{connector_info_key}})
```
````

---

## Usage

This prompt is invoked by the orchestrator (`docs/LINKS-ORCHESTRATOR-PROMPT.md`) once per connector. It is designed to run as an independent subagent with full context from repo files.

### Single Invocation Example

Replace the variables and invoke as a `general` subagent:

```
{{CONNECTOR_NAME}} = "Adyen"
{{PAYMENT_METHOD}} = "Apple Pay"
```

The subagent will:
1. Resolve file-write keys (feature ID, connector_info_key) from repo metadata
2. Discover documentation URLs from scratch (fresh search, no existing data used)
3. Verify each URL by fetching it and scoring against 10-point backend checklist
4. Write verified links to `data/integration-source-links.json`
5. Update `connector-info.json` with the best URL
6. Return a summary to the orchestrator
