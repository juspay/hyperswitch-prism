# The Money Struct

Payment integrations are often confusing due to different formats in which payment amount is accepted and processed. A small error can cause large ramifications in terms of business impact.
The money framework eliminates the confusion and streamlines the acceptance format for the most important parameter in the payment workflow.

For example:
- Stripe wants cents: `amount: 1000` means $10.00.
- Adyen wants minor units but uses different field names.
- Some payment processors use floats (to be honest, it is quite dangerous!!).
- And a lot of them simply use strings.

You send `{"minor_amount": 1000, "currency": "USD"}`. Prism handles the rest. No more wondering if Stripe wants cents or dollars. No more Adyen amount-in-minus conversion bugs. One format. Every processor.

## What Are the Typical Problems While Handling Amount?

Diversity in the payment processor spec causes three types of bugs:

1. **The Off-by-100x Bug**: Sending `10` instead of `1000` charges $0.10 for a $10 product. You lose money on every transaction. Sending `100000` instead of `1000` attempts to charge $1,000 — there are high chances that the customer abandons the checkout or you may have to refund angry customers.

2. **Floating-Point Arithmetic Bug**: When two floats are added it is bound to cause tiny errors which magnify at scale. For example, 10.99 + 20.99 = 31.980000000000004 (not 31.98) 

3. **Currency Confusion**: Passing `"amount": 1000` without specifying currency. Is this $10.00 USD or ¥1,000 JPY?

## How Money Struct Solves for It?

The Prism standardizes on one representation for amount — in minor units, with ISO standard currency, and very importantly — in integer.

```json
{
  "minor_amount": 1000,
  "currency": "USD"
}
```

| Field | Type | Description |
|-------|------|-------------|
| `minor_amount` | `int64` | Amount in smallest currency unit. USD: cents. JPY: yen (no decimals). BHD: 1/1000th dinar. |
| `currency` | `Currency` | ISO 4217 three-letter code: `USD`, `EUR`, `GBP`, `JPY`, `INR`, etc. |

The payment request is then transformed into each processor's native format, and tested for final verification. The amount reflecting on the payment processor dashboard should read exactly the same as the amount intended to be processed. Just verifying the API response from the processor may not be enough!!

```javascript
// You send this to Prism
const payment = {
  amount: {
    minor_amount: 2500,
    currency: "USD"
  }
};

// Prism sends to Stripe
{
  "amount": 2500,  // Stripe expects cents
  "currency": "usd"
}

// Prism sends to Adyen
{
  "amount": {
    "value": 2500,
    "currency": "USD"
  }
}
```

## More Information

### Zero decimal currencies

Some currencies have no decimal places. The Prism handles this automatically.

| Currency | Unit | Example |
|----------|------|---------|
| JPY | 1 yen | `{"minor_amount": 1000, "currency": "JPY"}` = ¥1,000 |
| KRW | 1 won | `{"minor_amount": 10000, "currency": "KRW"}` = ₩10,000 |
| VND | 1 đồng | `{"minor_amount": 50000, "currency": "VND"}` = ₫50,000 |

However, you still send amounts in minor units. Prism adjusts for processors that expect whole units for these currencies.

### Supported Currencies

The Prism supports 160+ currencies via the ISO 4217 standard:

**Major Currencies:**
`USD`, `EUR`, `GBP`, `JPY`, `AUD`, `CAD`, `CHF`, `CNY`, `INR` and more

**Regional:**
`AED`, `BRL`, `DKK`, `HKD`, `MXN`, `NOK`, `NZD`, `PLN`, `SEK`, `SGD`, `ZAR` and more

### Recommended Best Practices

1. **Always specify currency explicitly** — Never rely on defaults.
2. **Store amounts in minor units** — In your database, store `5999` not `59.99`. Floats cause rounding errors.
3. **Validate before sending** — Check `minor_amount > 0` and currency code length is 3.
4. **Display formatting is a UX concern** — Convert `5999` to `$59.99` in your frontend to improve readability for the user. Do not do the conversion in your API calls.
5. **Handling currency conversions** — Prism does **not** convert between currencies. If you authorize in `USD`, you capture in `USD`. If you need forex, handle it before triggering the Prism. For multi-currency applications, track the original currency and amount. Do not convert for storage — you'll lose precision.
