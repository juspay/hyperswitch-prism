# Why 1000 != 1000 in Payments

Money is deceptively simple. You have an amount, you have a currency. How hard can it be?

Turns out — very. A tiny mistake in how you represent a number can silently drain a business, trigger fraud alerts, or accidentally charge customers ten times what they owe. This post is about the money framework built into Prism — a payment library — and how it gets this right.

---

## Why Amount Is Harder Than It Looks

When you're building a payment system that talks to dozens of different payment processors, you quickly realize that every processor has its own opinion about what an "amount" looks like.

- **[Stripe](https://docs.stripe.com/api/payment_intents/create)** wants cents as an integer. `1000` means ten dollars. Their docs say: *"A positive integer representing how much to charge in the smallest currency unit."*
- **[Adyen](https://docs.adyen.com/api-explorer/Checkout/71/post/payments)** also wants minor units as an integer. Their docs say: *"The amount information for the transaction (in minor units)."* So `1000` for EUR means €10.00.
- **[PayPal](https://developer.paypal.com/docs/api/orders/v2/#orders_create)** wants a string with decimals — `"10.00"`. The Orders v2 API uses a JSON string, not a number.
- **Wells Fargo** also wants a string with decimals — `"10.00"` — but with completely different field names and request structure.
- **[Stax](https://docs.staxpayments.com/reference/charge)** wants a float — `10.0`. Yes, an actual floating-point number. Their API field `total` is a JSON numeric type in major units.

These aren't guesses — each format is documented in the processor's official API reference. The diversity is real, and it's the whole problem.

If you're writing integrations by hand, you're going to get this wrong at some point. Maybe not today. Maybe not in testing. But in production, at 2 AM, for a real customer.

The bugs this creates fall into three categories.

### The Off-by-100x Bug

This is the classic. You send `10` when the processor expects `1000`. Your customer gets charged $0.10 instead of $10.00. You process a thousand transactions and lose $9,990. Or worse — you send `100000` when you meant `1000`, and you're attempting to charge someone $1,000 for a $10 product. They abandon the checkout. Your conversion rate craters. Your support queue fills up.

### The Floating-Point Bug

This one is subtle. Floating-point arithmetic isn't exact. Try this in any language:

```text
10.99 + 20.99 = 31.980000000000004
```

Not `31.98`. That extra `0.000000000000004` doesn't matter for a single transaction. But when you're running thousands of transactions and reconciling at the end of the day, these errors accumulate. Regulatory reporting becomes a headache. Settlements don't match.

### The Currency Confusion Bug

`"amount": 1000` — is that $10.00 USD, ¥1,000 JPY, or 1.000 KWD (which is about $3,300)?

If currency isn't explicitly attached to every amount, you're one API call away from a serious mismatch.

---

## The Solution: Money as a First-Class Type

Prism solves this by establishing a single internal representation for all monetary values: the `Money` struct.

```rust
pub struct Money {
    pub amount: MinorUnit,
    pub currency: Currency,
}
```

Amount and currency are never separate. They travel together, always. You cannot have a `MinorUnit` floating around in your code without the currency that gives it meaning — by convention, every operation that touches an amount requires both.

`MinorUnit` itself is simple:

```rust
pub struct MinorUnit(pub i64);
```

A 64-bit signed integer, wrapped in a named type so the compiler won't let you confuse it with a raw number, a float, or a string. The "minor unit" is the smallest subdivision of the currency — cents for USD, euro-cents for EUR, yen for JPY (which has no subdivision).

**What "minor units" means across currencies:**

| Currency | Minor Unit | Example |
|----------|-----------|---------|
| USD | 1 cent | `Money { amount: 1000, currency: USD }` = $10.00 |
| INR | 1 paisa | `Money { amount: 1000, currency: INR }` = ₹10.00 |
| JPY | 1 yen | `Money { amount: 1000, currency: JPY }` = ¥1,000 |
| KWD | 1 fils | `Money { amount: 1000, currency: KWD }` = 1.000 dinars (~$3.30) |

Same struct. Radically different real-world values — because currency is always part of the picture.

---

## Currency Isn't Uniform

One thing that surprises people: different currencies have different numbers of decimal places, governed by [ISO 4217](https://en.wikipedia.org/wiki/ISO_4217) (the international standard for currency codes).

ISO 4217 defines the number of decimal places for every currency. Prism implements this classification into four groups:

**Zero-decimal currencies** — no subdivision. The amount you send is the final amount:
JPY, KRW, VND, and about a dozen others. `Money { amount: 1000, currency: JPY }` is ¥1,000 — not ¥10.00, there's no such thing.

**Two-decimal currencies** — the most common case:
USD, EUR, GBP, INR, and 150+ others. `Money { amount: 1000, currency: USD }` is $10.00.

**Three-decimal currencies** — a handful of currencies, mostly in the Arab world:
BHD, KWD, JOD, OMR, TND. `Money { amount: 1234, currency: KWD }` is 1.234 dinars.

**Four-decimal currencies** — just one: CLF (Chilean Unit of Account). Rare, but it exists.

Why does this matter? Because when converting from `MinorUnit` to whatever a payment processor expects, you divide by the right number for the currency. USD divides by 100. JPY doesn't divide at all. KWD divides by 1000. Get this wrong and you're sending the wrong amount — and the processor won't necessarily tell you.

---

## The Adapter Layer

Prism isn't a one-to-one integration — it's a library that speaks to many payment processors. Your application decides which connector to use; Prism handles the actual communication. That means the same `Money` value needs to become a Stripe request today, an Adyen request tomorrow, or a PayPal request next week — each expecting a completely different amount format. That's the problem this layer solves.

Here's where the design gets elegant. Prism doesn't pick one format and force every connector to deal with it. Instead, it uses a converter pattern — each connector declares exactly what format it needs, and the framework handles the translation automatically. One unified input; the right format per connector.

Here's how Prism implements it. There are four output types:

```text
MinorUnit        → raw integer          (Stripe, Adyen)
StringMinorUnit  → integer as a string  ("1000")
StringMajorUnit  → decimal string       ("10.00") (PayPal, Wells Fargo)
FloatMajorUnit   → floating-point       (10.0) (Stax)
```

Each converter implements a single trait:

```rust
pub trait AmountConvertor: Send {
    type Output;
    fn convert(&self, amount: MinorUnit, currency: Currency) -> Result<Self::Output, ...>;
    fn convert_back(&self, amount: Self::Output, currency: Currency) -> Result<MinorUnit, ...>;
}
```

Two directions: outbound (internal → connector format for the request) and inbound (connector format → internal, for parsing responses and webhooks). Each connector picks the converter it needs once, at initialization. Here's what that looks like in practice:

```rust
// Stripe, Adyen: integer minor units
amount_converter: &MinorUnitForConnector

// PayPal, Wells Fargo: string like "12.34"
amount_converter: &StringMajorUnitForConnector

// Stax: float like 12.34
amount_converter: &FloatMajorUnitForConnector
```

When a payment is processed, the connector calls `convert(amount, currency)` and gets back exactly the format it needs. When a response comes back, `convert_back(amount, currency)` normalizes it into `Money` before it ever touches shared code.

---

## Precision Without Floating-Point

`FloatMajorUnit` exists as an output type — so aren't we back to floating-point problems?

The key is where the float lives. Inside Prism, everything is `MinorUnit` (an integer). The float only exists at the boundary — in the JSON body going to a connector that requires it. You convert from integer to float at the last possible moment, and convert back immediately when parsing the response.

The conversion itself uses [`rust_decimal::Decimal`](https://docs.rs/rust_decimal/latest/rust_decimal/) as an intermediate — a fixed-precision decimal library, not native floating-point — so the arithmetic stays clean:

```text
// Converting Money { amount: MinorUnit(1234), currency: USD }
// to StringMajorUnit for a connector that wants "12.34":

1. Start with i64: 1234
2. Convert to Decimal (type change only — no value change, no precision loss)
3. Divide by 100 and format: Decimal(12.34) → "12.34"
```

Prism never does arithmetic on raw floats. The float only exists in the serialized request body — because some connectors require it, not because it's a good idea.

---

## What It Looks Like From the Outside

As a user of the Prism library, you construct one value:

```rust
let payment = Money {
    amount: MinorUnit::new(2500),
    currency: Currency::USD,
};
```

That's $25.00. You pass it to Prism along with the connector your application has chosen, and the rest is handled for you. No format decisions. No division by 100. No wondering what the processor expects.

What actually goes over the wire depends on the connector — but that's Prism's problem, not yours:

If the connector is Stripe:
```json
{ "amount": 2500, "currency": "usd" }
```

If the connector is PayPal:
```json
{ "value": "25.00", "currency_code": "USD" }
```

If the connector requires floats:
```json
{ "amount": 25.0 }
```

Same `Money` value in. The right format out. The caller never touches the conversion.

---

## This Problem Isn't Unique to Money

The three principles at work here aren't payment-specific:

1. **Pick one internal canonical unit and stick to it** — don't let different parts of your system disagree on what "1000" means
2. **Carry the unit alongside the value** — a number without its unit is ambiguous; treat them as one thing
3. **Convert at the boundary, not in the core** — the mess of external formats shouldn't leak into your internal logic

You see the same problem anywhere unit-bearing quantities cross system boundaries:

- **Time**: nanoseconds internally, milliseconds or seconds at API boundaries. The off-by-1000 bug is just as real here — observability systems get this wrong all the time.
- **Physical quantities**: distance in meters internally, miles or kilometers at the display layer. Temperature in Kelvin internally, Celsius or Fahrenheit for output.
- **Infrastructure**: Kubernetes already does this — CPU in millicores (`1000m` = 1 core), memory in bytes (`1Gi` = 1,073,741,824 bytes). One canonical unit, multiple display formats.

The payment world learned these lessons through production incidents. But the principle is general: pick a canonical unit, bundle it with its classification, and adapt at the edges.

---


## Closing

Money is where bugs have real consequences. Not "the test failed" consequences — "we owe customers a refund" or "we undercharged for six months" consequences.

The Prism money framework is small. A few hundred lines of Rust. But it encodes hard-won knowledge: amount and currency are one thing, not two; integers are safer than floats; the mess of the outside world should be isolated at the connector boundary, not scattered through your core logic.

One format in. Right format out. Every time.

And once you see the pattern, you start seeing where else it applies.

---

Prism is open source — built in Rust, with SDKs across languages.

{% github juspay/hyperswitch-prism %}

- Node.js: `npm install hyperswitch-prism`
- Python: `pip install hyperswitch-prism`
- Java: `io.hyperswitch:prism` on Maven Central
- Docs: [docs.hyperswitch.io/integrations/prism/prism/installation](https://docs.hyperswitch.io/integrations/prism/prism/installation)

---

*If this was useful, a ⭐ on GitHub goes a long way. And if your stack handles currency conversion differently — we'd love to hear how in the comments.*
