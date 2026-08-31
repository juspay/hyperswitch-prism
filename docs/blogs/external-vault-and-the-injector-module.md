# External Vault Support in Prism — Keeping Card Data Out of Your Stack

If you have ever built a payment integration, you know the uncomfortable reality: somewhere in your
pipeline, raw card numbers touch your servers. That means PCI DSS scope, compliance audits, SAQ
forms, and the constant anxiety of being a breach vector.

The industry's answer is **vaulting** — store the card once with a specialist provider and
replace it everywhere else with a harmless token. But vaulting introduces its own problem: every
time you want to charge a card, you need the real number back. The naive approach is to
de-tokenize first, then call the processor. That puts raw card data right back on your server —
exactly what vaulting was supposed to prevent.

Prism takes a different approach — and the key insight is that modern vault providers already
solve half the problem. Vaults like VGS and HyperswitchVault expose their own proxy or
detokenization endpoints. Rather than adding vault-specific logic into every connector, Prism
reuses its existing connector layer to route outbound payment requests *through* those vault
endpoints, configured entirely at runtime via the input the merchant provides.

The result: any merchant can plug in any supported vault — no connector code changes, no
re-deployment, just configuration. This is what makes vault support a first-class capability of
the Unified Connector Service (UCS) rather than a bespoke integration bolted onto each processor.

This post explains how that works.

---

## The challenge in plain terms

A card processor like Stripe or Adyen expects a real card number in the payment request. If you
hold only a vault token (something like `tok_abc123`), you cannot send that to the processor —
the processor has never heard of your vault.

The two obvious options both have problems:

| Approach | The catch |
|---|---|
| De-tokenize → call processor | Raw card data touches your server. PCI scope stays large. |
| Send token directly to processor | The processor rejects it. It does not know your vault. |

Prism introduces a third option: **the connector builds the request with vault tokens in place,
and a separate module substitutes the real values at the network boundary — before the bytes leave
your infrastructure.**

Your application never holds the resolved card number. The connector code never knows the
difference.

---

## How the same connector code handles both cases

The most interesting design decision in Prism is that connector integrations are completely
unaware of whether they are dealing with a real card or a vault token. The same code path handles
both.

This is made possible by a small but powerful abstraction: **the card structure is generic over
what kind of "card number" it holds.**

### Two kinds of card data, one structure

Think of it this way. A `Card` in Prism is not tied to a specific type of card number. It is
parameterised:

- **`Card<DefaultPCIHolder>`** — holds a real, validated card number (PAN). Used for direct
  payments where the merchant is PCI-compliant and holds card data themselves.
- **`Card<VaultTokenHolder>`** — holds a vault token string. Used when the card is stored in an
  external vault like VGS or HyperswitchVault.

The difference is in what the card number field actually holds under the hood. When the holder is
`DefaultPCIHolder`, the card number is a strongly-typed `CardNumber` — a validated type that runs
a Luhn check on construction and refuses to accept a value that is not a structurally valid PAN.
When the holder is `VaultTokenHolder`, the card number is simply a `Secret<String>` — an opaque
token accepted as-is, with no format validation applied, because the vault provider owns the
meaning of that string.

From the outside, both look identical. They have the same fields — card number, expiry month,
expiry year, CVV. They serialize to JSON the same way. A connector transformer that reads
`card.card_number.peek()` gets back a string in both cases — it just happens to be a validated
PAN in one case and a vault token in the other.

When the vault-token variant is serialized into the outgoing request body, it produces something
like:

```json
{
  "number": "tok_sandbox_vgs_abc123",
  "expiry_month": "12",
  "expiry_year": "2027",
  "cvv": "tok_sandbox_vgs_cvv456"
}
```

That request body, with tokens still embedded, is what gets handed to the Injector.

### One connector implementation, two runtime paths

Every payment flow — authorize, capture, setup recurring — is written once. At runtime, the
system instantiates it with either `DefaultPCIHolder` or `VaultTokenHolder` depending on what the
caller sent. There is no `if vault_mode` logic anywhere in the connector code.

The decision happens at the gRPC server entry point, when the incoming payment method is
inspected:

- Raw card details → `Card<DefaultPCIHolder>`, no token data, direct HTTP call
- Proxy card details (vault token) → `Card<VaultTokenHolder>`, token data attached, Injector
  handles the outbound call

---

## The two paths, visualised

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        Incoming payment request                             │
└───────────────────────────────┬─────────────────────────────────────────── ┘
                                │
              Is the payment method a raw card or a vault token?
                                │
               ┌────────────────┴─────────────────┐
               │                                   │
          Raw card                            Vault token
          (DefaultPCIHolder)                  (VaultTokenHolder)
          token_data = None                   token_data = Some(...)
               │                                   │
               └─────────────────┬─────────────────┘
                                 │
               Connector transformer runs — same code for both
               Builds HTTP request body (with real PAN or vault token)
                                 │
               ┌─────────────────┴─────────────────┐
               │                                   │
          Direct HTTP call               Injector handles call
          to processor                   (resolves tokens first)
               │                                   │
               └─────────────────┬─────────────────┘
                                 │
               Unified response handling — same code for both
```

---

## Sequence diagrams

### Path 1 — Raw card, no vault

```
Your App       gRPC Server       Connector Code        Processor
    │               │                  │                   │
    │── Authorize ──▶                  │                   │
    │   (real PAN)  │                  │                   │
    │               │── build req ─────▶                   │
    │               │                  │── {"number":      │
    │               │                  │   "4242..."}      │
    │               │◀── request ──────│                   │
    │               │── HTTP POST ──────────────────────────▶
    │               │◀── 200 OK ────────────────────────────│
    │◀── response ──│                  │                   │
```

### Path 2 — Vault token (VGS or HyperswitchVault)

```
Your App       gRPC Server       Connector Code     Injector         Vault / Processor
    │               │                  │               │                    │
    │── Authorize ──▶                  │               │                    │
    │   (vault tok) │                  │               │                    │
    │   + vault      │                  │               │                    │
    │     metadata  │                  │               │                    │
    │               │── build req ─────▶               │                    │
    │               │   (token string) │               │                    │
    │               │                  │── {"number":  │                    │
    │               │                  │   "tok_abc"}} │                    │
    │               │◀── request ──────│               │                    │
    │               │── hand to injector ──────────────▶                    │
    │               │   (template + token_data          │                    │
    │               │    + vault config)                │                    │
    │               │                                   │ VGS: route through │
    │               │                                   │── forward proxy ───▶
    │               │                                   │ HyperswitchVault:  │
    │               │                                   │── detokenize ──────▶
    │               │                                   │── call processor ──▶
    │               │◀── response ──────────────────────│                    │
    │◀── response ──│                  │               │                    │
```

In both cases, **your application code sees only a unified response**. The difference in how the
outbound call was made is invisible above the Injector layer.

---

## The Injector — what it actually does

The Injector is the component that bridges "connector request with vault tokens" and "actual HTTP
call with resolved card data."

When a vault-token payment is made, the Injector receives:

- The connector's request URL and HTTP method
- The request body — a template with token strings in the card fields
- The token reference — metadata identifying which vault entry to resolve
- Vault configuration — which vault to talk to, how to authenticate, proxy details if needed

It then does one of two things, depending on the vault type:

### VGS (proxy mode)

VGS acts as a TLS-intercepting forward proxy. The Injector routes the outbound HTTP request
*through* the VGS proxy. VGS recognises its own token aliases in the request body and replaces
them with the real card values before forwarding to the processor. The substitution happens
entirely inside the VGS network — your server only ever sends tokens.

```
Your Server ──(token in body)──▶ VGS ──(PAN in body)──▶ Processor
                                  ↑
                        Token → PAN substitution
                        happens here, not in your code
```

### HyperswitchVault (transformation mode)

HyperswitchVault works differently — it is a detokenization service rather than a proxy. The
Injector first calls the vault to exchange the token for the real card data, then constructs a
fresh request with the resolved values and sends it directly to the processor.

```
Your Server ──(token)──▶ HyperswitchVault
                                │
                           (returns PAN)
                                │
                         Injector ──(PAN in body)──▶ Processor
```

The choice between the two modes is determined by the `vault_connector_type` field in the vault
metadata header — `proxy` for VGS, `transformation` for HyperswitchVault. Everything else is
handled automatically.

---

## Telling the Injector which vault to use

When a client sends a vault-token payment, it includes a metadata header
(`x-external-vault-metadata`) alongside the request. This header carries a small JSON payload
(base64-encoded) that describes the vault:

**For VGS:**
```json
{
  "vault_connector_type": "proxy",
  "vault_connector_id": "vgs",
  "metadata": {
    "proxy_url": "https://tn123.sandbox.verygoodproxy.com",
    "certificate": "<CA cert for TLS verification>"
  }
}
```

**For HyperswitchVault:**
```json
{
  "vault_connector_type": "transformation",
  "vault_connector_id": "hyperswitch_vault",
  "metadata": {
    "vault_endpoint": "https://vault.hyperswitch.io",
    "vault_auth_data": {
      "api_key": "...",
      "profile_id": "..."
    }
  }
}
```

Prism decodes this header, parses the configuration, and passes it into the Injector before the
outbound call is made. The connector code never sees it — it is threaded through a separate
field (`vault_headers`) in the payment flow context.

---

## What this means if you are adding a connector

You do not need to write any vault-specific logic. At all.

Implement your connector transformer once, using the generic card structure. When the card fields
are serialised into the request body, vault tokens and raw PANs produce the same JSON shape — they
are both strings in the right fields. The Injector handles token resolution at runtime based on
context that is entirely outside the connector's responsibility.

The vault support is a concern of the execution layer, not the integration layer. Connector
authors get it for free.

---

## The full picture

```
┌──────────────────────────────────────────────────────────────────────────┐
│  What Prism gives you                                                    │
│                                                                          │
│  ┌─────────────────────────────────────────────────────────────────┐    │
│  │  Connector transformer                                          │    │
│  │  Generic over card type — same code for raw PAN and vault token │    │
│  └─────────────────────────────────────────────────────────────────┘    │
│                           │                                              │
│          ┌────────────────┴────────────────┐                            │
│          │                                 │                            │
│  ┌───────▼──────┐                 ┌────────▼───────────────────────┐   │
│  │ Direct HTTP  │                 │ Injector                       │   │
│  │ (raw PAN)    │                 │                                │   │
│  │              │                 │  Resolves tokens before        │   │
│  │              │                 │  the HTTP call leaves          │   │
│  │              │                 │                                │   │
│  │              │                 │  Supports VGS (proxy) and      │   │
│  │              │                 │  HyperswitchVault (transform)  │   │
│  └──────────────┘                 └────────────────────────────────┘   │
│          │                                         │                    │
│          └─────────────────────┬───────────────────┘                    │
│                                │                                        │
│                  Unified response handling                               │
│                  Your application code sees no difference                │
└──────────────────────────────────────────────────────────────────────────┘
```

Card data stays out of your stack. PCI scope shrinks to the vault boundary. Connector integrations
stay clean and focused on the one thing they should do — translating payment intent into a
connector-specific HTTP call.

---

## Links

- [Source: `service.rs` — Injector request assembly and vault config parsing](../../crates/common/external-services/src/service.rs)
- [Source: `payment_method_data.rs` — card data abstraction](../../crates/types-traits/domain_types/src/payment_method_data.rs)
- [Source: `payments.rs` — payment method dispatch at the gRPC boundary](../../crates/grpc-server/grpc-server/src/server/payments.rs)
