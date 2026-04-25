# The off-by-100x bug, the floating-point bug, and why fintech can't afford either

> A small thing — how Prism represents money — that quietly prevents the kind of bug that ends careers.
> *Yashasvi · Hyperswitch Prism · Week 1 / Post 3*

---

## A confession

I will name no names, but: there is a real production system, at a real company, that for **eleven days** charged customers `$0.10` instead of `$10.00` because someone passed a `float` "10.0" where an integer "minor units" was expected, and somewhere downstream `int(amount)` truncated it to `10` cents.

The weekly board review caught it. Nothing else did. Not the unit tests. Not the staging environment. Not the merchant dashboard, because the merchant dashboard *also* read minor units — it just looked weirdly low and nobody noticed.

This is not a bug. This is a **category** of bug. And every payments engineer has either lived through one or watched a colleague do it. Today, I want to show you why the Prism SDK refuses to let you commit it — and why the way it represents money is one of the most underrated parts of the whole library.

---

## The three killers, by name

### 1. The off-by-100x bug

Stripe wants cents. Adyen wants minor units. PayPal wants whole units in a string. Some processors, regretfully, want floats.

```python
# Stripe
{ "amount": 1000, "currency": "usd" }              # $10.00
# Adyen
{ "amount": { "value": 1000, "currency": "USD" } } # $10.00
# Some others
{ "amount": "10.00", "currency": "USD" }           # $10.00
```

Now look at the developer who has to write this code at 4 PM on a Friday. They've been swapping between connector docs all week. They write `amount: 10` and ship. Your customers get charged 10 cents. Until someone notices.

This is not a "junior dev" problem. This is a problem caused by **the absence of a type that won't let you express the wrong thing**.

### 2. The floating-point arithmetic bug

```python
>>> 10.99 + 20.99
31.980000000000004
```

You laugh. Then you ship a refund engine that uses `float` and over six months your books are off by a few hundred dollars, and your finance team spends a week reconciling it. (I have, again, seen this happen.)

Currency is **not** a continuous quantity. It's discrete. The smallest unit of USD is one cent. The smallest unit of JPY is one yen. The smallest unit of BHD (Bahraini dinar) is *one-thousandth* of a dinar. Floating point cannot represent these without rounding error, and rounding errors in money compound the wrong way.

### 3. The currency confusion bug

```json
{ "amount": 1000 }
```

Is that $10.00 USD? ¥1,000 JPY? Or 1,000 BHD (which would be 1 actual dinar, because BHD has 3 decimal places)? You cannot tell. Neither can your code. Neither can the auditor in two years when you're under SOC review.

If you are passing `amount` without `currency` in the same struct, **you are one rename away from disaster**.

---

## How Prism represents money

```proto
message Money {
  int64 minor_amount = 1;  // smallest currency unit
  Currency currency  = 2;  // ISO 4217 enum
}
```

That's it. Two fields. Both required. Bound together at the type level. Used **everywhere** — `PaymentServiceAuthorizeRequest.amount`, `PaymentServiceCaptureRequest.amount_to_capture`, `RefundServiceRefundRequest.refund_amount`, dispute responses, payout requests.

```typescript
// JavaScript / TypeScript
const request: types.PaymentServiceAuthorizeRequest = {
    merchantTransactionId: "authorize_123",
    amount: {
        minorAmount: 1000,             // $10.00 (1000 cents)
        currency: types.Currency.USD,
    },
    // ...
};
```

```python
# Python
request = PaymentServiceAuthorizeRequest(
    merchant_transaction_id="authorize_123",
    amount=Money(minor_amount=1000, currency=Currency.USD),
    # ...
)
```

```kotlin
// Kotlin
val request = PaymentServiceAuthorizeRequest(
    merchantTransactionId = "authorize_123",
    amount = Money(minorAmount = 1000, currency = Currency.USD),
    // ...
)
```

This is the same shape across every SDK because (see [post 2](./02-proto-source-of-truth-ffi.md)) it's the same proto. There is no way to construct a `Money` without specifying both. There is no way to use a `float`. There is no way to ambiguate the currency. The compiler / the proto validator says no.

## The three killers, defused

### 1. Off-by-100x — gone

`minor_amount` is unambiguous. Cents in for USD, yen in for JPY, fils-thousandths in for BHD. The library knows what each connector expects and converts. From the docs:

> You send `{"minor_amount": 1000, "currency": "USD"}`. Prism handles the rest. No more wondering if Stripe wants cents or dollars. No more Adyen amount-in-minus conversion bugs. One format. Every processor.

Concretely:

```text
You send                         Prism sends to Stripe         Prism sends to Adyen
─────────────────                ─────────────────────         ────────────────────
minor_amount: 2500               "amount": 2500                "amount": {
currency: USD                    "currency": "usd"                "value": 2500,
                                                                  "currency": "USD"
                                                              }
```

You did not have to know which connector wants which shape. **And the conversion is tested against the merchant dashboard, not just the API response** — because as the Prism docs note in their amount framework:

> The amount reflecting on the payment processor dashboard should read exactly the same as the amount intended to be processed. Just verifying the API response from the processor may not be enough!!

That's the level of paranoia you want from a payments library.

### 2. Floating point — refused at the type system

`minor_amount` is `int64`. Not `double`. Not `decimal`. Not `string`. **Integer.**

You literally cannot pass a float without truncating it yourself, and even then your linter will yell at you. The same field is `int64` in Rust, in the proto, in the Python typed stubs, in TypeScript, and in Kotlin. The bug *can't be expressed*.

If you take **one** thing away from this library and use it in your own code: **store amounts as integers in minor units. Always. Forever. In your DB, in your cache, on the wire, in your logs.** Convert to a display format (`$59.99`) only at the UI boundary. Floats and money do not mix and never will.

### 3. Currency confusion — gone, even for the weird currencies

`Currency` is an ISO 4217 enum — `USD`, `EUR`, `GBP`, `JPY`, `INR`, `BHD`, all 160+ of them. You can't ship a `Money` without specifying it. And Prism handles the *real* trap:

**Zero-decimal currencies.** Some currencies do not have minor units. `JPY` has no "yen-cent." `KRW` has no sub-won. If you're charging 1,000 yen, "minor units" is just 1,000 yen.

```text
JPY → minor_amount: 1000  → ¥1,000
KRW → minor_amount: 10000 → ₩10,000
VND → minor_amount: 50000 → ₫50,000
```

And the inverse trap — **three-decimal currencies.** `BHD` has 1,000 fils to a dinar.

```text
BHD → minor_amount: 1500 → BD 1.500 (one and a half dinars)
```

You as a developer write `minor_amount: 1500, currency: BHD`. Prism knows BHD has three decimals, knows the connector wants whole-dinar floats / two-decimal strings / whatever, and **converts correctly**. You do not memorize this table. You do not maintain `if currency == "JPY"` branches in your code. You do not get a 4 AM page because somebody changed the connector and the conversion broke.

---

## Why this is "just a struct" and also the most important struct in the library

I want to be clear about what's special here. It's not the *idea* of representing money in minor units — that idea is decades old, every careful payments engineer knows it. It's that:

1. **The whole API surface uses it consistently.** Authorize amount, capture amount, refund amount, dispute amount, payout amount, multi-capture amounts — all `Money`. Not "amount is `int64` here, but `Decimal` there, but a `string` in this one webhook payload." Same type, every flow, every connector, every language.

2. **It survives the FFI boundary.** Because `Money` is a proto message and the FFI boundary is bytes-of-proto, every language sees the *same* `Money` shape. There is no JavaScript SDK that quietly accepts numbers and Python SDK that requires `Decimal` and they "mostly agree." They literally cannot disagree.

3. **The connector layer is the only thing that knows about per-connector quirks.** Floats, strings, divided-by-100, divided-by-1000 — those translations live in *one* place inside `connector-integration` and you the user never see them. If a new connector ships tomorrow with a unique amount serialization, exactly *one* file in the Rust core changes, and every SDK gets the fix on the next release.

4. **It refuses to do FX for you.** From the docs:

   > Prism does **not** convert between currencies. If you authorize in `USD`, you capture in `USD`. If you need forex, handle it before triggering Prism.

   This is the right call. Currency conversion is a *business* decision (which rate? when? whose rate? logged where for compliance?). A library that silently does FX is a library that silently introduces an audit nightmare.

---

## The bigger lesson, for any fintech library you build

The Money struct is a tiny thing. Two fields, three rules. But it's a perfect example of the principle that should run through any financial library:

> **Make wrong states unrepresentable.**

You can't represent USD-without-currency. You can't represent half-a-cent. You can't represent ten dollars as `10.0`. The compiler is on your side, in every language, because the proto is the source of truth and the proto says no.

If your current payments code lets you write `amount=10.99` somewhere, even by accident — do yourself a favor: go fix it tonight. Either adopt Prism, or steal the pattern. Your future self, your finance team, your auditors and your customers will all be quietly grateful.

---

## TL;DR

- `Money { minor_amount: int64, currency: Currency }` everywhere.
- **Integers only.** No floats. The off-by-100x and the rounding-error bugs are not possible to write.
- **Currency is required.** No more "is this dollars or yen?" guessing.
- **Zero-decimal and three-decimal currencies are handled.** JPY, KRW, BHD — Prism converts to whatever the connector expects.
- **Same shape in every SDK.** Because the proto is the contract.
- **No silent FX.** Authorize in USD → capture in USD. By design.

A library that costs you no money is one that doesn't let you accidentally charge the wrong amount.

Code: [github.com/juspay/hyperswitch-prism](https://github.com/juspay/hyperswitch-prism) · `proto/payment.proto` defines `Money` · `docs/architecture/frameworks/money-struct.md` is the longer write-up.
