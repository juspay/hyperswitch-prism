# From Authorization to Settlement: A Payment Is a Lifecycle

A payment isn't a thing you fetch. It's a thing that happens.

A card gets authorized in one request. The money gets captured in another, maybe
the next morning when the warehouse confirms stock. A refund shows up a week
later. A 3DS challenge parks the whole thing in a "waiting on the customer" limbo
that resolves in a browser tab your server never sees. Confirmations trickle in
by webhook, out of order, sometimes twice.

Most SDKs flatten all of that into a `Payment` object with a single `status`
field. That shape is convenient, and it's exactly where things start to go
wrong:

```ts
const payment = await sdk.payments.get(id)

if (payment.status === "succeeded") {
  await fulfillOrder(payment.order_id)   // looks fine, ships a bug
}
```

Here's the catch: `"succeeded"` isn't one thing. A succeeded authorization and a
succeeded capture are different events, and a success you polled for is different
again from one that turned up in a settlement webhook. Authorizing reserves the
money. Capturing actually moves it. So if you fulfill on the authorization,
you've shipped goods against a hold that can still be declined later, or just
quietly expire.

And the code passes review. The test card authorizes and captures in one go, so
the demo is green and everyone moves on. Then a real customer pays with a 3DS
card, the `else` branch fires on a payment that actually went through, and you're
debugging it in production.

Good SDKs hand you the whole lifecycle. The ones that don't give you a noun with
a status field, and let you write code that reads correctly and behaves wrong.

## How Hyperswitch Prism models it

[Prism](https://github.com/juspay/hyperswitch-prism) doesn't hand you one mutable `Payment` to poke at. A payment has a
position in a state machine, with named flows instead of a single status. Its
`PaymentStatus` enum carries thirty-odd states, and they're grouped on purpose:

```
STARTED → AUTHENTICATION_PENDING → AUTHORIZED → CHARGED → (refund sub-process)
                  │                     │           │
            (3DS / SCA)          (money reserved)  (money moved)
```

The API mirrors that machine. There's no status setter sitting on an object.
The lifecycle shows up as verbs spread across a few clients, and every call tells
you where the payment ended up:

```ts
import {
  PaymentClient,                 // get / capture / refund / void
  MerchantAuthenticationClient,  // open the session, mint client token
  EventClient,                   // parse + verify asynchronous webhooks
  types,
} from "hyperswitch-prism"

await paymentClient.capture({ connectorTransactionId, /* ... */ })
```

Notice `capture` is its own method, not `update({ status: "captured" })`. You
can't fat-finger a capture onto a payment that was never authorized and have it
pass as a harmless-looking status string. The methods you can call are the
transitions you're actually allowed to make.

## One word, three enums

A flat `status` field is a trap because "succeeded" turns up in three different
places that mean three different things. [Prism](https://github.com/juspay/hyperswitch-prism) gives each one its own type:

- `PaymentStatus.AUTHORIZED` vs `PaymentStatus.CHARGED` — reserved vs moved.
- Webhook events are a separate enum: `PAYMENT_INTENT_AUTHORIZATION_SUCCESS` vs
  `PAYMENT_INTENT_CAPTURE_SUCCESS`. Both say "success." Only one means you got
  paid.
- Refunds get their own enum again: `RefundStatus.REFUND_SUCCESS`, with its own
  pending and failure states, because a refund is its own sub-process.

Cram all three into one `status` field and you're overloading it, and the
overloading is where the bug lives. Keep the flows apart and you read the one you
actually meant.

## Two non-PCI flows, two shapes

"Non-PCI" means the sensitive part (the card number, the PayPal login) gets
entered in the connector's own client-side SDK and never reaches your server.
Your backend only ever holds tokens and ids. [Prism](https://github.com/juspay/hyperswitch-prism) supports this, but the
lifecycle looks different from one connector to the next, which is another reason
a flat `status` falls short. Take Adyen and PayPal:

```
NON-PCI: the card / login is entered in the connector's client SDK.
Your server only holds tokens and ids — never raw card data.

ADYEN  ·  authorize now, capture later, outcome via webhook
  Browser Drop-in        Your backend           Prism / UCS        Adyen
       │                      │                      │               │
       │  open session ──────►│ createClientAuth ───►│ ── /sessions ►│
       │ ◄─ clientToken ──────│ ◄────────────────────│ ◄─ session ───│
       │                      │                      │               │
       │  card entered in Drop-in, authorized client-side ──────────►│
       │                      │ ◄═══ AUTHORISATION webhook ══════════│
       │                      │ EventClient.handleEvent (verify HMAC)│
       │                      │ record pspReference                  │
       │  complete checkout ─►│ authorizePayment                     │
       │                      │   polls verified outcome (~12s)      │
       │ ◄── AUTHORIZED ──────│   capture is a SEPARATE call later   │
       ▼                      ▼                      ▼               ▼

PAYPAL  ·  approve in popup, then auto-capture server-side
  Browser Buttons        Your backend           Prism / UCS        PayPal
       │                      │                      │               │
       │  open session ──────►│ create order ───────►│ ── /orders ──►│
       │ ◄─ orderId ──────────│ ◄────────────────────│ ◄─ orderId ───│
       │                      │                      │               │
       │  buyer approves in popup (no card on server) ─────────────►│
       │  complete checkout ─►│ authorizePayment                     │
       │                      │   PaymentClient.authorize            │
       │                      │   paypalSdk token + AUTOMATIC ──────►│ capture
       │ ◄── CAPTURED ────────│ ◄────────────────────│ ◄─ CHARGED ───│
       ▼                      ▼                      ▼               ▼
```

With **Adyen**, the card is authorized inside the Drop-in, and the result only
reaches your server by webhook. So `authorizePayment` can't just read a status.
It waits for the verified event, then picks up the `pspReference` from that
webhook as the transaction id, which it needs for the capture you run separately:

```ts
// initiate: hand the browser a client token; the card stays client-side
const session = await authClient.createClientAuthenticationToken({ /* amount */ })
// → { clientToken, publishableKey }  — server never sees the card

// authorize: the outcome only exists once the verified webhook lands
const outcome = getWebhookOutcome(sessionId)   // recorded by EventClient
// → AUTHORIZED + pspReference   (capture is a later, separate call)
```

**PayPal** works the other way around. The order is created up front, the buyer
approves it in the popup, and your server captures the approved order in one shot
with `AUTOMATIC` capture:

```ts
// initiate: create the order server-side, hand the browser its id
const orderId = await createPaypalOrder({ /* amount, currency */ })
// → buyer approves orderId in the PayPal popup; no card on your server

// authorize: capture the approved order using the JS-SDK token
const res = await paymentClient.authorize({
  paymentMethod: { paypalSdk: { token: { value: orderId } } },
  captureMethod: types.CaptureMethod.AUTOMATIC,
})
// → CHARGED   (already captured; only refunds remain)
```

Same vocabulary, two real paths. Adyen ends on `AUTHORIZED` with a capture still
owed; PayPal ends on `CHARGED` right away. A plain `status` field would call both
of those "succeeded" and hide the fact that one of them still needs a capture.
The named states won't let that slip.

## Truth that isn't synchronous

Sometimes the answer isn't there yet when you ask for it. In client-side flows
the authorization decision lands later, over a webhook, so a synchronous read is
really just a guess. [Prism](https://github.com/juspay/hyperswitch-prism) treats it as a guess rather than pretending
otherwise:

- A status read with nothing behind it yet comes back `PENDING`, not `ERROR`.
  Unknown means "check again in a bit," not "decline someone who already paid."
- An async event has to prove itself before it can move state. `EventClient`
  parses the webhook, checks the source, and only then maps it. A webhook it
  can't verify is just a forged transition, so it gets rejected, and there's no
  dev-mode shortcut around that.
- Even the amount isn't a plain number. It's a currency-scaled minor unit, so
  `1000` is ¥1000 in one currency and $10.00 in another.

## The takeaway

Treat a payment as a lifecycle and a few habits follow:

- **Don't branch on one `status`.** Ask the specific question: authorized,
  captured, or refunded? They're different flows.
- **Treat a sync read as provisional.** Default unknowns to pending and let the
  verified event settle it.
- **Never fulfill on authorization.** Only a capture means you've been paid.
- **Money is a currency-scaled integer**, not a number.

A payment runs from authorization to settlement, and treating it as a lifecycle
rather than a single status is the gap between code that works and code that only
looks like it does. [Prism](https://github.com/juspay/hyperswitch-prism) puts the
lifecycle in front of you, with separate verbs, separate flows, and events it
actually verifies, so the obvious thing to write tends to be the right thing too.

---

**Reference:** [Hyperswitch Prism on GitHub](https://github.com/juspay/hyperswitch-prism)
