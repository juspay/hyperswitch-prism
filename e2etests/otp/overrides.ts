import type { Overrides } from "../support/config.js";

// US2 connector_events, 2026-10-16, bounded one-hour sample. Field names and currencies
// only; every metadata value below is synthetic. This sample is not exhaustive.
export const builtinOverrides: Record<string, Overrides> = {
  "narvar/gooutdoors/adyen": {
    payment: { currency: "GBP", metadata: { order_id: "e2e-order" } },
  },
  "narvar/blacks-jd/adyen": {
    payment: { currency: "GBP", metadata: { order_id: "e2e-order" } },
  },
  "narvar/millets/adyen": {
    payment: { currency: "GBP", metadata: { order_id: "e2e-order" } },
  },
  "zurich_insurance/bu_hk_gi/adyen": {
    payment: {
      currency: "HKD",
      metadata: {
        businessUnit: "e2e",
        clientId: "e2e-client",
        country: "HK",
        eventType: "e2e",
        productName: "e2e",
      },
    },
  },
  "zurich_insurance/tg_qantas/adyen": {
    payment: {
      currency: "AUD",
      metadata: {
        clientId: "e2e-client",
        eventType: "e2e",
        merchant_invoice_id: "e2e-invoice",
      },
    },
  },
  "zurich_insurance/tg_lufthansa/adyen": {
    payment: {
      metadata: {
        clientId: "e2e-client",
        eventType: "e2e",
        merchant_invoice_id: "e2e-invoice",
      },
    },
    paymentVariants: { eur: { currency: "EUR" }, chf: { currency: "CHF" } },
  },
  "ConvesioPay Platform/merchant_1782222808739/stripe": {
    payment: { currency: "USD", statement_descriptor_suffix: "E2E" },
  },
};
