# Technical Specification: Multi-Language SDK Architecture

**Status:** Mandatory Standard for all Connector Service SDKs.
**Position:** Senior SDE Core Requirement.

---

## 1. Architectural Mandates

### Instance-Owned Resources
- **Prohibition:** No global connection pools, static caches, or top-level singletons for network clients.
- **Requirement:** Every `ConnectorClient` instance must own its lifecycle.
- **Implementation:** Initialize the platform-native pool (`Session` in Python, `Dispatcher` in Node.js, `OkHttpClient` in Kotlin) inside the class constructor.

### Binary-First FFI Boundary
- **Requirement:** All data exchange across the FFI boundary must use **Protobuf-over-Bytes**.
- **Transformation:** SDKs must decode the raw binary response from Rust directly into generated Protobuf objects. JSON is prohibited at the boundary.

---

## 2. Networking & Proxy Standards

### Hostname-Suffix Matching
- **Requirement:** Proxy bypass logic must use hostname matching, not full URL string matching.
- **Standard:** Logic must extract the `hostname` and check for suffix matches (e.g., `target.endsWith("stripe.com")`).

### Protocol Enforcement
- **Requirement:** Proxy mapping must be protocol-aware.
- **Standard:** Map `http_url` to the "http" scheme and `https_url` to the "https" scheme. Do not leak insecure traffic through secure proxy ports.

---

## 3. Implementation Checklist by Language

| Feature | Python | Node.js | Kotlin |
| :--- | :--- | :--- | :--- |
| **Naming** | `snake_case` | `camelCase` | `camelCase` |
| **Pool Class** | `requests.Session` | `undici.Dispatcher` | `okhttp3.OkHttpClient` |
| **Timeout Handling** | Manual "Hard Gate" post-call | `AbortController` | `callTimeout` |

---

## 4. Metadata Standard (FFI Routing)

Every FFI request (Authorize, Sync, Refund, etc.) must include these mandatory metadata keys for correct Rust routing:

1. `connector`: The target connector name (e.g., "Stripe").
2. `x-connector`: Routing hint for the backend.
3. `connector_auth_type`: Validated JSON string matching the Rust auth struct.
4. `x-api-key`: The raw credential for the backend.
5. `x-auth`: Method hint (e.g., `body-key` or `header-key`).

---

## 5. Binary Protocol (Manual FFI)

For environments without generated UniFFI wrappers (e.g., Node.js via `koffi`):
- **Arguments:** Prepend a 4-byte Big-Endian length integer to binary buffers.
- **Memory:** `RustBuffer.len` must include the 4 bytes of the prefix.
- **Returns:** Expect and strip the 4-byte length prefix from the returned `RustBuffer` data.
