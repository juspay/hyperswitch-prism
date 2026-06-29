# How We Built Multi-Language SDKs for Prism Using FFI

Prism's connector logic lives in Rust. The SDKs need to work in Node, Python, Java. The question was how to share that logic without rewriting it many times.

Two options were on the table — [[RPC vs gRPC|HTTP/gRPC]] or FFI. gRPC would've worked, but it means running a sidecar, adding network hops, all that overhead for something sitting on the same machine. FFI felt right: in-process calls, no network, no extra runtime to manage.

## The pattern every handler follows

Before getting into how we exposed the FFI, it's worth understanding what every handler in Prism actually does. Whether it's `authorize`, `capture`, `refund`, or `void` — they all follow the same three steps:

```rust
fun authorise(req) {
    // 1. build_request            → transform input into connector format
    // 2. HTTP call                → send to the connector
    // 3. handle_connector_response → transform the response back
}
```

That's it. Every handler is just transform → call → transform. Once we saw that, it became obvious what we actually needed to expose over FFI — not the full handler, just the two transformation functions. The HTTP call in the middle stays in the SDK, in whatever language it's written in. The Rust part is only the business logic of building and parsing connector-specific payloads.

This made the FFI surface tiny and kept things clean. Adding a new handler later means the same pattern — no surprises.

## UniFFI

Rust has a crate for this: `uniffi`. Annotate your functions, run the build, get a native compiled binary — `.so`, `.dylib`, or `.dll` depending on the platform. Any language with FFI support can load it and call those functions as if they were local.

The two transformer functions — `authorize_req_transformer` and `authorise_res_transformer` — got pulled into a dedicated FFI crate:

```rust
#[derive(uniffi)]
fn authorize_req_transformer { ... }

#[derive(uniffi)]
fn authorise_res_transformer { ... }
```

Same functions used by the HTTP handler, the gRPC handler, and now the FFI path. One place to change, three paths that benefit.

## How the SDK uses it

Each client maps to a proto service — `PaymentClient` for `PaymentService`, `RefundClient` for `RefundService`, etc. Inside the client, the binary loads once and the FFI functions get called directly as part of each operation:

```typescript
export class PaymentClient {
    async authorize(msg) {
        const req = ffi.authorize_req_transformer(msg)
        const result = await fetch(req);
        const res = ffi.authorize_res_transformer(result)
        return res
    }
}
```

Loading the binary is just one line:

```typescript
const ffi = require('./libprism_ffi.so'); // .dylib on macOS, .dll on Windows
```

## Getting data across the boundary

JSON doesn't go through FFI directly. The SDK converts the user's JSON to Protobuf bytes first, hands those bytes to the Rust function, Rust hands bytes back, SDK turns them into objects on the way out. Protobuf does double duty here — it's both the wire format and where the SDK's type definitions come from. Type safety in Node, Python, and Java all flows from the same `.proto` file.

```
User          SDK              FFI            Rust
 │             │                │              │
 │  JSON       │                │              │
 │────────────▶│                │              │
 │             │  Protobuf      │              │
 │             │  Bytes         │              │
 │             │───────────────▶│              │
 │             │                │    Bytes     │
 │             │                │─────────────▶│
 │             │                │    Process   │
 │             │                │◀─────────────│
 │             │◀───────────────│              │
 │◀────────────│                │              │
```

From the outside, none of this is visible. After `npm i hyperswitch-prism`:

```typescript
import { PaymentClient, types } from 'hyperswitch-prism';

const paymentClient = new PaymentClient(connectorConfig);
const response = await paymentClient.authorize(authorizeRequest);

if (response.status === types.PaymentStatus.CHARGED) {
    console.log("Payment successful");
}
```

That's it. The binary, the encoding, the Rust calls — all hidden behind `authorize()`.

The full implementation lives at [juspay/hyperswitch-prism](https://github.com/juspay/hyperswitch-prism) — the FFI crate, the SDK clients, and the proto definitions are all in there.
