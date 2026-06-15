🚀 New Medusa v2 Plugin: Unified Payments (Stripe, Adyen, PayPal, GlobalPay & more)

Hey everyone! 👋

I just published **@juspay-tech/medusa-unified-payment** — a Medusa v2 payment provider that lets your store accept payments across **many processors through a single provider**, powered by [Hyperswitch Prism](https://github.com/juspay/hyperswitch-prism), the open unified connector service. It ships with a companion React package so the storefront checkout UI is handled for you too. One integration instead of one per processor 🎉

---

✨ What it does

- 💳 **One provider, many processors** — Stripe, Adyen, PayPal, GlobalPay, Braintree, Cybersource, and Mollie, all behind a single Medusa payment provider
- 🧩 **Built for Medusa v2** — registers under the Payment module (requires Medusa `>= 2.15`, Node `>= 20`)
- ⚙️ **One config shape** — add a connector by changing one `connector` string and its credentials; secret-manager-friendly `{ value }` credential format
- 🛒 **Ready-made checkout UI** — `@juspay-tech/medusa-unified-payment-react` gives you `HyperswitchPrismConnectorPanel` + `HyperswitchPrismPaymentButton` (storefront UI for Stripe, Adyen, PayPal, GlobalPay), plus low-level connector wrappers for fully custom checkouts
- 🔁 **Full payment lifecycle** — authorize, capture, void, and refund through one consistent model across connectors
- 🔐 **Centralized, verified webhooks** — inbound events flow through Prism with mandatory signature verification (events that can't be verified are rejected, never silently trusted)
- 🌍 **Per-region routing** — assign different providers to different regions right from the Medusa Admin
- 🧪 **Sandbox & production ready** — one `environment` toggle per provider (flows are verified in sandbox; validate production before go-live)

---

🔗 Links

- 📦 npm (backend) → https://www.npmjs.com/package/@juspay-tech/medusa-unified-payment
- 🛍️ npm (storefront) → https://www.npmjs.com/package/@juspay-tech/medusa-unified-payment-react
- ⭐ Hyperswitch Prism → https://github.com/juspay/hyperswitch-prism

Would love your feedback — happy to answer any questions! 🙌
