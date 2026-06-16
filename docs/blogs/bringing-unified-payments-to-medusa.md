# Add multiple payment providers to Medusa v2 with medusa-custom-payments plugin

*A stateless, unified, production-grade payment provider abstraction layer for Medusa V2, simplifying extensibility to local/regional payment providers and adding respective payment processor checkout UI*

**It starts with Stripe.** Medusa ships Stripe as its one first-party, documented provider, alongside a manual payment placeholder that doesn't actually move money. You drop Stripe in, wire up the checkout, test a card, and you're taking money.
 
**Then something drives a second provider.** Maybe you're expanding into Europe, where shoppers in the Netherlands expect iDEAL, Germans reach for Paypal or SEPA or Klarna. Or you're launching your store in Brazil, where payments are generally powered with local payment providers offering Pix and Boleto.

Or maybe finance wants a backup payment processor so a single outage doesn't freeze every order. Whatever the trigger, you now need a payment processor for redundancy or global expansion purposes.
 
**If you're lucky, a plugin already exists.** Medusa has a real community plugin ecosystem — Mollie, Adyen, PayPal, Paystack, Braintree. So you find one, adopt it, and wire its storefront SDK and webhooks into your checkout. It works — but it taught you nothing reusable, because it shares no interface with the Stripe integration you already have or the next plugin you'll add. Coverage could also be uneven because plenty of processors have no official Medusa plugin. 

**If you're not lucky, you build a custom payment provider.** No plugin, or none you trust, means extending the custom payment provider of Medusa to your preferred payment processor. 
 
That's the staircase: Stripe is free, more plugins create fragmentation, and a custom payment provider is a project — and every step is a fresh start because nothing in Medusa gives all payment processors a shared shape with the assurance of production quality.

This post is how **`medusa-custom-payments`** — one stateless library that runs inside your own Medusa process — solves for a shared and extensible payment processor interface. It leverages the production-grade payment processor integrations from [**`juspay/hyperswitch`**](https://github.com/juspay/hyperswitch) — an open-source, composable payment platform (42K+ GitHub stars), which orchestrates payments across 100+ payment processors.

## The second processor is never half the work

The uncomfortable part is that the first integration taught you almost nothing reusable. Each processor has its own idea of how credentials are passed, how a payment session is created, how amounts are represented, what "successful" even means. So the second integration starts close to zero again.

In Medusa specifically, you have three ways forward. Here's the comparison:

| | Community Plugins| Custom payment provider | medusa-custom-payments |
|---|---|---|---|
| **Integrations covered** | One per plugin | One per build | Multiple, with shared interface |
| **Adding/Switching payment processor** | New plugin | New build | Config change, no migration |
| **Storefront checkout UI** | Yours to assemble per plugin | Yours to build | Shared reusable React components across payment processors |
| **Production-readiness** | Varies; several warn against live-store testing | Only as much as you test | Part of a production-grade payment platform (juspay/hyperswitch) |

And then it keeps going. On the storefront you pull in a different client SDK per processor — Stripe Elements, Adyen's Drop-in, PayPal's Buttons, hosted card fields for someone else — each with its own mounting quirks, context, and callbacks, and your checkout fills with a connector-specific branch for every one. On the backend you're writing webhook handlers, and webhooks are the part nobody enjoys: signatures to verify, payloads that differ per provider, security-sensitive code that's easy to get subtly wrong and hard to notice when you do.

And none of it is "done" when it ships. Every processor is a relationship you now maintain — an API that changes, a dashboard to log into, a separate stream of transactions to reconcile. Three processors isn't three times the integration; it's that plus three times the upkeep, forever.

> **Does Medusa support payment orchestration?**
>
> Not out of the box — Medusa ships Stripe first-party and leaves every other processor to the ecosystem. That's a deliberate, composable design choice, but it means *orchestration* (one interface in front of many connectors) is something you will have to implement. And `medusa-custom-payments` serves as that layer: a single provider that speaks to many payment processors, so adding the next one is a config change rather than a rewrite. It doesn't replace Medusa's payment module — it fills the payment processor gap which is not fulfilled by the community plugins.

## One provider, many processors — running in your own process

The shift is to stop integrating processors one by one and integrate one layer that already speaks to all of them. In payments this is called orchestration: a single interface in front of many connectors.

For Medusa v2, that layer is **`medusa-custom-payments`**, powered by [Hyperswitch Prism](https://github.com/juspay/hyperswitch-prism) — the open connector library we unbundled out of [Hyperswitch](https://github.com/juspay/hyperswitch) (40k+ GitHub stars, production payment volume at Juspay) so anyone could use the integrations without adopting a whole platform.

The detail that matters most for anyone evaluating this seriously: **Prism is stateless, and the connector logic executes inside your Medusa process.** There is no Juspay server in your payment path. Card data and credentials flow from your app to the processor exactly as they would with a hand-rolled provider — we are never in the middle, you take on no additional PCI scope, and nothing about your money movement depends on our uptime. It's open-source connector code you compile into your own app, not a gateway you route through.

What makes it click day-to-day is that every connector shares one shape. You don't learn a new config object per processor — you register the same provider, change one `connector` string, and supply that processor's credentials:

```ts
providers: [
  { resolve: "medusa-custom-payments", id: "stripe",
    options: { connector: "stripe",
      connectorConfig: { apiKey: { value: process.env.STRIPE_API_KEY ?? "" } },
      environment: "SANDBOX" } },
  { resolve: "medusa-custom-payments", id: "adyen",
    options: { connector: "adyen",
      connectorConfig: { apiKey: { value: process.env.ADYEN_API_KEY ?? "" },
        merchantAccount: { value: process.env.ADYEN_MERCHANT_ACCOUNT ?? "" } },
      environment: "SANDBOX" } },
]
```

Adding PayPal next is another block that looks exactly like these. That's the whole point — the third processor really is half the work, and the fourth even less.

> **How do I add a second payment provider in Medusa?**
>
> Register `medusa-custom-payments` a second time with a different `id` and `connector` string, supply that processor's credentials, and assign it to a region in the Admin. That's it — no new provider class, no new webhook handler, no new checkout SDK to wire up. The block above is the entire diff.

## One package for the whole checkout

The backend is only half the story, and the storefront is the half that usually has no unified answer — Medusa has no shared package for checkout elements, so each SDK is yours to assemble and maintain.

The companion React package, **`medusa-custom-payments-react`**, closes that gap. It gives you two components that behave the same regardless of connector: `HyperswitchPrismConnectorPanel` renders the right payment instrument for the selected method, and `HyperswitchPrismPaymentButton` dispatches the correct place-order behavior behind it. Drop the panel into the payment step and the button into review, and the per-connector branches disappear from your own code — the components absorb the differences in mounting, callbacks, and result handling.

That's the piece most stacks miss: a backend abstraction is common, but a matching multi-processor checkout UI usually isn't. For a fully custom flow, the package also exposes the individual payment processor wrappers.

## What are the benefits?

**One provider class instead of one per processor.** The plugin implements `AbstractPaymentProvider` once and speaks to every connector behind it, so you never write `authorizePayment`, `capturePayment`, `refundPayment`, and the rest again.

**Breadth without breadth of code.** The backend speaks to seven connectors today, with ready-made storefront UI for four. The breadth is easily extensible to any of the 100+ connectors of [**`juspay/hyperswitch`**](https://github.com/juspay/hyperswitch).

| Connector | Backend | Storefront UI |
|---|---|---|
| Stripe | ✅ | ✅ |
| Adyen | ✅ | ✅ |
| PayPal | ✅ | ✅ |
| GlobalPay | ✅ | ✅ |
| Braintree | ✅ | — |
| Cybersource | ✅ | — |
| Mollie | ✅ | — |

**Webhooks you don't hand-roll per processor.** Inbound events flow through one path, with verification handled centrally and required by default — an event that can't be verified is rejected rather than quietly trusted. That's the security-sensitive code you no longer write four times.

**A consistent lifecycle.** Authorize, capture, void, and refund behave through one model across connectors, not four.

**Per-region routing in the Admin portal.** Assign different providers to different regions from the Medusa Admin, so EU shoppers hit one processor and another market hits a second — no code change to make that call.

**One switch to go live, and no lock-in.** A single `environment` toggle moves a provider from sandbox to production, and swapping a processor later is a config change, not a migration. The connectors are open source and the library is useful outside Medusa too — you're adopting code, not a dependency on us.

> **Does this actually run?**
>
> Yes. We wired all four storefront connectors end to end on a live Medusa store and took sandbox payments through each — card entry, redirect, and hosted fields alike. The flows in the support matrix are exercised against each connector's sandbox. These are the same payment processors that run in production inside juspay/hyperswitch; the path to production volume on Medusa is what we're hardening next, and progress lands in the open in the juspay/medusa-custom-payments repo.


## How to get started?

Get started with an npm install and a single config block for all the payment processors you wish to enable.

```bash
npm install medusa-custom-payments         # backend provider
npm install medusa-custom-payments-react    # storefront UI
```

```ts
providers: [
  { resolve: "medusa-custom-payments", id: "stripe",
    options: { connector: "stripe",
      connectorConfig: { apiKey: { value: process.env.STRIPE_API_KEY ?? "" } },
      environment: "SANDBOX" } },
  { resolve: "medusa-custom-payments", id: "adyen",
    options: { connector: "adyen",
      connectorConfig: { apiKey: { value: process.env.ADYEN_API_KEY ?? "" },
        merchantAccount: { value: process.env.ADYEN_MERCHANT_ACCOUNT ?? "" } },
      environment: "SANDBOX" } },
]
```

From there, register a provider, assign it to a region, and take a sandbox payment. 

The [backend README](https://github.com/juspay/hyperswitch-prism/tree/main/plugins/medusa/medusa-custom-payments) walks through provider setup and webhooks, and the [React README](https://github.com/juspay/hyperswitch-prism/tree/main/plugins/medusa/medusa-custom-payments-react) covers the checkout components.

---

## About the project 

`medusa-custom-payments` is Apache-2.0 licensed and built using the core payment integration library of [**`juspay/hyperswitch`**](https://github.com/juspay/hyperswitch) — Juspay's open-source enterprise payment platform (42K+ GitHub stars), which orchestrates payments across 100+ payment processors. It is actively maintained by the team at [Juspay](https://juspay.io/us).

And juspay/hyperswitch-prism is the integration layer, unbundled so developers can use the integrations without adopting the full platform. `medusa-custom-payments` brings that same library to Medusa. 

Requests, queries, bugs, and integrations asks can be logged at [juspay/hyperswitch-prism repo](https://github.com/juspay/hyperswitch-prism/issues).
