# Bringing unified payments to Medusa: why the second processor shouldn't cost a quarter

If you run a store on Medusa, the first payment integration feels easy. You drop in Stripe, wire up the checkout, test a card, and you are taking money. It is a good day.

The second one is where the mood changes. Maybe you are expanding into a market where shoppers reach for a local method that Stripe does not cover well. Maybe finance wants a backup processor so a single outage does not freeze every order. Maybe a partnership comes with PayPal attached. Whatever the reason, you go to add a second provider — and you discover that "add a payment method" is not one task you do twice. It is a fresh project each time.

We have watched this happen enough times that it stopped being surprising and started being the thing we wanted to fix. This post is about that: the quiet tax of every new processor, and how a single unified payment provider for Medusa v2 removes it.

---

## The second processor is never half the work

Here is the uncomfortable part. The first integration taught you almost nothing reusable. Each processor has its own idea of how credentials are passed, how a payment session is created, how amounts are represented, what a "successful" status even means. So the second integration starts close to zero again.

Then it keeps going. On the storefront you are pulling in a different client SDK for each one — Stripe Elements, Adyen's Drop-in, PayPal's Buttons, hosted card fields for someone else — each with its own mounting quirks, its own callbacks, its own failure modes. On the backend you are writing webhook handlers, and webhooks are the part nobody enjoys: signatures to verify, payloads that differ per provider, security-sensitive code that is easy to get subtly wrong and hard to notice when you do.

And none of it is "done" when it ships. Every processor is a relationship you now maintain — an API that changes, a dashboard to log into, a separate stream of transactions to reconcile. Three processors is not three times the integration. It is three times the integration *plus* three times the ongoing upkeep, forever.

> **Q: Is this really worth solving, or is it just annoying?**
>
> It is worth solving, because the cost is not only engineering time. A meaningful share of shoppers abandon checkout when they do not see a payment option they trust, and that share climbs in every region where your single processor feels foreign. So the calculus is bad in both directions: adding processors is expensive, and *not* adding them quietly loses sales. The whole point of a unified layer is to make "support the method this market expects" cheap enough that you stop treating it as a project.

---

## One provider, many processors

The shift is to stop integrating processors one by one and instead integrate one layer that already speaks to all of them. In the payments world this idea is called orchestration: a single interface in front of many connectors, so adding the next one is a configuration change rather than a rewrite.

For Medusa v2 stores, that layer is **Medusa Unified Payment**, powered by [Hyperswitch Prism](https://github.com/juspay/hyperswitch-prism) — the open connector library we unbundled out of Hyperswitch so anyone could use the integrations without adopting a whole platform. The Medusa plugin wraps it in two pieces: a backend payment provider and a set of React components for the storefront checkout.

What makes it click is that every connector shares one shape. You are not learning a new config object per processor — you register the same provider, change one `connector` string, and supply that processor's credentials:

```ts
providers: [
  { resolve: "@juspay-tech/medusa-unified-payment", id: "stripe",
    options: { connector: "stripe",
      connectorConfig: { apiKey: { value: process.env.STRIPE_API_KEY ?? "" } },
      environment: "SANDBOX" } },
  { resolve: "@juspay-tech/medusa-unified-payment", id: "adyen",
    options: { connector: "adyen",
      connectorConfig: { apiKey: { value: process.env.ADYEN_API_KEY ?? "" },
        merchantAccount: { value: process.env.ADYEN_MERCHANT_ACCOUNT ?? "" } },
      environment: "SANDBOX" } },
]
```

Adding PayPal next is another block that looks exactly like these. That is the whole point — the third processor really is half the work, and the fourth even less.

---

## What you actually get

It helps to map the benefits back to the pains, because that is where the value lives.

**Breadth without breadth of code.** The backend speaks to seven connectors today — Stripe, Adyen, PayPal, GlobalPay, Braintree, Cybersource, and Mollie — and the storefront ships ready-made UI for four of them (Stripe, Adyen, PayPal, GlobalPay). You are not hand-mounting each SDK; you render one component and it shows the right instrument for the selected method.

**Webhooks you don't hand-roll per processor.** Inbound events flow through one path, with verification handled centrally and required by default — an event that cannot be verified is rejected rather than quietly trusted. That is the security-sensitive code you no longer write four times.

**A consistent lifecycle.** Authorize, capture, void, refund behave through one model across connectors, instead of four mental models you have to keep straight.

**Per-region routing in the Admin.** You assign different providers to different regions from the Medusa Admin, so EU shoppers can hit one processor and another market hits a second — without touching code to make that decision.

**One switch to go live, and no lock-in.** A single `environment` toggle moves a provider from sandbox to production, and swapping a processor later is a config change, not a migration.

> **Q: Does this actually run, or is it a nice diagram?**
>
> It runs. We wired all four storefront connectors end to end on a live Medusa store and took sandbox payments through each of them — card entry, redirect, wallet, and hosted fields included. The flows in the support matrix are exercised against each connector's sandbox.

---

## Start with one config block

If any of this sounds like work you have done before — the second integration that felt like the first all over again — that is exactly the feeling this removes.

```bash
npm install @juspay-tech/medusa-unified-payment        # backend provider
npm install @juspay-tech/medusa-unified-payment-react   # storefront UI
```

From there, register a provider, assign it to a region, and take a sandbox payment. The [backend README](../../plugins/medusa/medusa-unified-payment/README.md) walks through provider setup and webhooks, and the [React README](../../plugins/medusa/medusa-unified-payment-react/README.md) covers the checkout components. The first processor stays easy. The difference is that, this time, so does the next one.
