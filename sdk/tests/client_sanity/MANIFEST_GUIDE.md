# HTTP Client Sanity — Manifest guide for test writers

Each scenario in `manifest.json` is the **contract**: request (what the client sends) and **expected_response** (what the echo server returns).

- **Golden captures** are generated from the manifest: `node sdk/tests/client_sanity/generate_golden.js`. Golden files have the same shape as echo server captures (method, url, headers, body, response).
- **Certification runner** (`run_client_certification.js`) orchestrates test execution across all SDK languages.
- **Judge** compares **golden_&lt;id&gt;.json** (from manifest) vs **actual_&lt;lang&gt;_&lt;id&gt;.json**. **Request** in actual is what the echo server received (what the SDK sent). **Response** in actual is what the SDK's `execute()` returned (status, headers, body)—so we certify the SDK's response handling, not the echo server's self-reported response.
- **Capture URL:** Echo server stores the **full URL** (scheme from connection, host from `Host` header, path+query from request).
- **Proxy:** Makefile starts `sdk/tests/client_sanity/simple_proxy.js` (port 9082) alongside the echo server. Scenario `CASE_PROXY_FORWARD` sends the request via the proxy. Port 9082 is used to avoid conflict with other services on 8082.

## How to give `expected_response`

One shape; the server infers behavior from types.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `status_code` | number | **Yes** | HTTP status (e.g. `200`, `204`, `302`, `500`). |
| `headers` | object | No | Response headers. Keys lowercase. Value: string or array of strings (e.g. for `Set-Cookie`). **Set `content-type` here for non-JSON bodies.** |
| `body` | object **or** string | No | **Object** → server sends `JSON.stringify(body)` and sets `Content-Type: application/json` if not in `headers`. **String** → sent as-is (set `content-type` in `headers` for XML, HTML, etc.). Omit for no body. |

**Rules:**

- **JSON response:** use `"body": { "status": "ok" }`. No need to set `content-type`; the server sets it.
- **Other content types (XML, HTML, plain text):** use `"body": "<xml>...</xml>"` (string) and set `"headers": { "content-type": "application/xml" }` (or the right type).
- **204 / 302:** set `status_code`, set `headers` if needed (e.g. `location`), omit `body`.

### Multi-value headers

Use an **array** of strings for any header that can appear multiple times (e.g. `Set-Cookie`, `Link`, `Vary`). The echo server sends each element as a separate header line.

```json
"headers": { "set-cookie": ["session=abc", "theme=dark"] }
```

Single-value headers stay as a string: `"content-type": "application/json"`.

### Response body types (full coverage)

| Type | How to specify | Example scenario |
|------|-----------------|-------------------|
| **JSON** | `body` = object | `"body": { "status": "ok" }` |
| **Form-url-encoded** | `body` = string, `content-type` in headers | `"headers": { "content-type": "application/x-www-form-urlencoded" }, "body": "key=value&foo=bar"` |
| **Multipart** | `body` = string (with boundary), `content-type` with boundary | `"headers": { "content-type": "multipart/form-data; boundary=Boundary" }, "body": "--Boundary\\r\\n..."` |
| **Raw bytes** | `body` = string with prefix `base64:` | `"body": "base64:AAECAwQFBgcICQ=="` → server decodes and sends binary; capture stores base64. |
| **Text / URL-encoded-like** | `body` = string, `content-type: text/plain` (or other) | `"headers": { "content-type": "text/plain" }, "body": "id=123&name=ok"` |

### Optional: server behavior

| Field | Type | Description |
|-------|------|-------------|
| `server_delay_ms` | number | Delay in ms before the server sends the response (e.g. for timeout tests). |
| `expected_error` | string | For the judge only: scenario is "expected to error" (e.g. client timeout); no change to what the server sends. |
| `client_timeout_ms` | number | Optional per-scenario client timeout override (runner passes this to the SDK). Useful for timeout tests. |

### Optional: proxy (per scenario)

If present, runners configure the SDK to use the proxy **only for that scenario**.

```json
"proxy": { "http_url": "http://localhost:9082" }
```

For invalid-proxy tests:

```json
"proxy": { "http_url": "invalid://bad" }
```

> Note: the Makefile / CI starts `sdk/tests/client_sanity/simple_proxy.js` on port 9082 for proxy scenarios.

### Runner wait (derived; not a manifest field)

Test writers do **not** need to specify a "wait after" value. Runners derive an appropriate wait only for timeout/error scenarios:

- If `expected_error` is set and both `server_delay_ms` and `client_timeout_ms` are present, runners wait approximately:
  - \( \max(\text{default}, (\text{server_delay_ms} - \text{client_timeout_ms}) + \text{buffer}) \)

This ensures the echo server has time to finish its delay and write the capture file even though the SDK timed out earlier.

## Examples (copy-paste friendly)

**JSON (object body; Content-Type set automatically):**
```json
"expected_response": { "status_code": 200, "body": { "status": "ok" } }
```

**No body (204, redirect):**
```json
"expected_response": { "status_code": 204 }
```
```json
"expected_response": { "status_code": 302, "headers": { "location": "http://localhost:8081/sanity/v1/target" } }
```

**JSON error:**
```json
"expected_response": { "status_code": 500, "body": { "error": "internal_server_error", "code": 500 } }
```

**Different content type (XML): set body as string + content-type in headers:**
```json
"expected_response": {
  "status_code": 200,
  "headers": { "content-type": "application/xml" },
  "body": "<result><status>ok</status></result>"
}
```

**Multi-value headers (array = multiple header lines):**
```json
"expected_response": {
  "status_code": 200,
  "headers": { "set-cookie": ["session=abc", "theme=dark"] },
  "body": { "status": "ok" }
}
```

**Form-url-encoded response:**
```json
"expected_response": {
  "status_code": 200,
  "headers": { "content-type": "application/x-www-form-urlencoded" },
  "body": "result=success&code=0"
}
```

**Multipart response:**
```json
"expected_response": {
  "status_code": 200,
  "headers": { "content-type": "multipart/form-data; boundary=ResponseBoundary" },
  "body": "--ResponseBoundary\r\nContent-Disposition: form-data; name=\"field\"\r\n\r\nvalue\r\n--ResponseBoundary--\r\n"
}
```

**Raw bytes response (base64 in manifest):**
```json
"expected_response": {
  "status_code": 200,
  "headers": { "content-type": "application/octet-stream" },
  "body": "base64:AAECAwQFBgcICQoLDA0ODxAREhM="
}
```

**Text/plain response (e.g. URL-encoded-like):**
```json
"expected_response": {
  "status_code": 200,
  "headers": { "content-type": "text/plain" },
  "body": "id=123&name=caf%C3%A9&status=ok"
}
```

New test case = add one scenario with `id`, `description`, `request`, and `expected_response`. No echo server code changes.
