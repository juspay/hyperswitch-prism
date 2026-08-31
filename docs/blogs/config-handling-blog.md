# Every Integration Wants Something Different

*How to keep your core clean when the outside world refuses to be consistent.*

---

## A Problem Every Backend Engineer Hits

At some point, you build a system that needs to talk to multiple external services. Maybe it's payment providers, cloud storage buckets, notification services, or data warehouses. The business logic is the same across all of them. But every single one has a different idea of what "connecting" looks like.

One wants an API key. Another wants a key and a secret. A third wants a username and password. A fourth wants a signed certificate.

None of them are wrong. They just aren't consistent with each other.

So the question becomes: where does all that difference live in your system?

---

## Where It Usually Goes Wrong

The path of least resistance is to pass the raw config straight through and let the core figure it out. It works fine for the first integration. By the third it starts getting messy. By the tenth, your core is full of conditionals like this:

```javascript
function processRequest(config, payload) {
  if (config.provider === "stripe") {
    headers["Authorization"] = `Bearer ${config.apiKey}`;

  } else if (config.provider === "adyen") {
    headers["X-API-Key"] = config.apiKey;
    body["merchantAccount"] = config.merchantAccount;

  } else if (config.provider === "braintree") {
    const token = btoa(`${config.publicKey}:${config.privateKey}`);
    headers["Authorization"] = `Basic ${token}`;
  }

  // ... keeps growing with every new integration
}
```

Now the core knows the internal shape of every integration's credentials. Adding a new integration means touching the core. Rotating a credential means hunting through the core to find every reference. Testing the core means setting up mocks for every integration's config shape.

> The core didn't get complex because the business logic got complex. It got complex because integration details had no other place to go.

---

## The Pattern: Absorb at the Boundary

The fix is simple in concept. Integration-specific config belongs at the boundary, not in the core.

The boundary is the place where your system first receives external input. It is the only place that should know about integration-specific shapes. Its job is to take all that variation, absorb it, and hand the core one consistent type regardless of which integration is being used.

```
┌─────────────────────────────────────────────────────────────┐
│                                                             │
│   Integration A     Integration B     Integration C        │
│   { api_key }       { key + secret }  { user + pass }      │
│                                                             │
│        (each caller speaks a completely different language) │
└──────────────────────────┬──────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────┐
│                      The Boundary                           │
│                                                             │
│         absorbs all variation, translates everything        │
│               into one single canonical form                │
└──────────────────────────┬──────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────┐
│                   Canonical Config                          │
│                                                             │
│       one consistent shape, the only thing                  │
│               the core ever sees                            │
└─────────────────────────────────────────────────────────────┘
```

Here is what that changes in practice:

| | Without a boundary | With a boundary |
|---|---|---|
| Core receives | Raw integration config | One canonical type |
| Adding an integration | Touches core logic | Adds a boundary variant only |
| Credential rotation | Trace through the entire core | Update the boundary |
| Core tests | Need every integration's config | Independent of integrations |

---

## The Hard Part: When Config Changes Per Request

Static config is the easy case. The harder case is when a caller wants to override something at request time. Maybe they want to point an integration at a staging environment, or use a different endpoint for one specific request.

The tempting fix is to pass the override straight into the core and handle it there. But the moment you do that, the boundary breaks. The core starts needing to know that integration A has a `disputeUrl` field and integration B has a `secondaryEndpoint` field. You are back to the same problem.

The right approach is to keep overrides behind the boundary too. The boundary accepts the override in the integration's own language, extracts what is relevant, merges it into the canonical form, and only then passes it to the core.

```
┌──────────────────────────────────────────────────────────┐
│  Request arrives with an integration-specific override   │
│  e.g. "use this staging URL for this request"            │
└─────────────────────────┬────────────────────────────────┘
                          │
                          ▼
┌──────────────────────────────────────────────────────────┐
│                    The Boundary                          │
│                                                          │
│  1. Accept override in the integration's own language    │
│  2. Extract only what is relevant                        │
│  3. Merge into the canonical form                        │
└─────────────────────────┬────────────────────────────────┘
                          │
                          ▼
┌──────────────────────────────────────────────────────────┐
│         Core receives the merged canonical config        │
│                                                          │
│    No knowledge of the override.                         │
│    No knowledge of which integration was called.         │
└──────────────────────────────────────────────────────────┘
```

The core gets a clean, merged config. It never knows an override happened.

---

## How Prism Does This Across 60+ Payment Connectors

[Prism](https://hyperswitch.io) is a payment infrastructure layer that routes transactions across 60+ payment connectors including Stripe, Adyen, Braintree, Razorpay, and many more. Every connector has a different credential shape. This is exactly the problem from above, running in production at scale.

### The vocabulary problem

Here is a sample of what a few real connectors expect:

| Connector | Credential fields |
|---|---|
| Stripe | `api_key` |
| Adyen | `api_key` + `merchant_account` + optional `review_key` |
| Authorize.net | `name` + `transaction_key` |
| Bluesnap | `username` + `password` |
| Braintree | `public_key` + `private_key` + optional `merchant_account_id` |
| Revolut | `secret_api_key` + optional `signing_secret` |

Same concept across all of them: authenticate with this connector. Six completely different shapes.

### The boundary in Prism

Prism defines a typed enum with one variant per connector. The caller sends credentials in their connector's own language. Prism parses it into the enum at the boundary. From that point on, the rest of the system never sees connector-specific field names:

```rust
// One variant per connector, each capturing exactly what that connector needs
enum ConnectorConfig {
    Stripe          { api_key: Secret },
    Adyen           { api_key: Secret, merchant_account: Secret, review_key: Option<Secret> },
    Authorizedotnet { name: Secret, transaction_key: Secret },
    Bluesnap        { username: Secret, password: Secret },
    Braintree       { public_key: Secret, private_key: Secret, merchant_account_id: Option<Secret> },
    Revolut         { secret_api_key: Secret, signing_secret: Option<Secret> },
    // ... 60+ more
}
```

### What the core sees

The core works with a completely different type. A flat struct with just URLs. No credentials, no connector-specific field names:

```rust
// What the core always receives, regardless of which connector is being used
struct ConnectorEndpoints {
    base_url:      String,
    dispute_url:   Option<String>,
    secondary_url: Option<String>,
}
```

Every connector collapses to this same shape before the core touches it. The core reads `endpoints.base_url`. It has no idea which connector is on the other end or what credentials were used to get there.

### Overrides in Prism

When a caller wants to override a URL at request time, they include it in their connector config. Prism pulls it out at the boundary, translates it to the canonical field name, and merges it into `ConnectorEndpoints` before anything reaches the core.

The comment in the source code says it plainly:

> "This is the only path by which URL overrides in ConnectorConfig should influence request execution."

There is no side door. Every connector-specific value, whether it is a credential or a URL override, goes through the boundary and gets translated before the core sees it.

---

## What You Get From This

The payoff shows up in everyday engineering work.

Adding a new connector means adding a variant to the boundary enum and writing a translator. The core stays untouched.

Rotating a credential means updating the boundary. You do not need to trace where it flows through the rest of the system.

Testing the core means testing against the canonical type. No integration-specific mock setup needed.

Debugging is cleaner too. Integration-specific problem? Look at the boundary. Flow problem? Look at the core. The separation tells you exactly where to look.

In Prism's case, 60+ connectors and the core has never needed a conditional for any of them. The boundary does the work so the core does not have to.

---

## Final Thought

The outside world will never be consistent. Every integration you connect will have a different shape, a different auth scheme, a different set of fields it cares about.

That is fine, as long as your system has one place where all that inconsistency gets absorbed and translated. The core should never know what is on the other side of that boundary.

The pattern itself is not complicated. The discipline to maintain it, to not let one quick fix bypass the boundary, is the hard part. But once it holds, adding integration number 61 costs exactly as much as adding integration number 1.

That is the foundation [Prism](https://hyperswitch.io) is built on. And it applies to any system that talks to more than one external service.

---
