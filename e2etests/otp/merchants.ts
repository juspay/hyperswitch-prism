import type { Merchant } from "../support/config.js";

// Supplied payment-flow sheet: card + ONETIME_PAYMENT + No3DS only.
// Merchant inventory is evidence of scope, not a claim of captured production payloads.
export const merchants: Merchant[] = [
  {
    org: "narvar",
    merchant: "gooutdoors",
    connector: "adyen",
  },
  {
    org: "narvar",
    merchant: "blacks-jd",
    connector: "adyen",
  },
  {
    org: "narvar",
    merchant: "footpatrol-jdsports-emea",
    connector: "adyen",
  },
  {
    org: "narvar",
    merchant: "millets",
    connector: "adyen",
  },
  {
    org: "narvar",
    merchant: "naylors",
    connector: "adyen",
  },
  {
    org: "zurich_insurance",
    merchant: "bu_au_life",
    connector: "adyen",
  },
  {
    org: "zurich_insurance",
    merchant: "au_edge",
    connector: "adyen",
  },
  {
    org: "zurich_insurance",
    merchant: "bu_hk_gi",
    connector: "adyen",
  },
  {
    org: "zurich_insurance",
    merchant: "tg_lufthansa",
    connector: "adyen",
  },
  {
    org: "zurich_insurance",
    merchant: "tg_qantas",
    connector: "adyen",
  },
  {
    org: "zurich_insurance",
    merchant: "tg_sg_dtc",
    connector: "adyen",
  },
  {
    org: "zurich_insurance",
    merchant: "tg_united",
    connector: "adyen",
  },
  {
    org: "zurich_insurance",
    merchant: "tg_virgin_money",
    connector: "adyen",
  },
  {
    org: "ConvesioPay Platform",
    merchant: "merchant_1772579803929",
    connector: "adyen",
  },
  {
    org: "ConvesioPay Platform",
    merchant: "merchant_1774556192507",
    connector: "adyen",
  },
  {
    org: "ConvesioPay Platform",
    merchant: "merchant_1774648848727",
    connector: "adyen",
  },
  {
    org: "ConvesioPay Platform",
    merchant: "merchant_1777060545486",
    connector: "adyen",
  },
  {
    org: "ConvesioPay Platform",
    merchant: "merchant_1779128597717",
    connector: "adyen",
  },
  {
    org: "ConvesioPay Platform",
    merchant: "merchant_1780011007165",
    connector: "adyen",
  },
  {
    org: "ConvesioPay Platform",
    merchant: "merchant_1780943489661",
    connector: "adyen",
  },
  {
    org: "ConvesioPay Platform",
    merchant: "merchant_1781539137449",
    connector: "adyen",
  },
  {
    org: "ConvesioPay Platform",
    merchant: "merchant_1781727428757",
    connector: "adyen",
  },
  {
    org: "ConvesioPay Platform",
    merchant: "merchant_1781877215259",
    connector: "adyen",
  },
  {
    org: "ConvesioPay Platform",
    merchant: "merchant_1781877241974",
    connector: "adyen",
  },
  {
    org: "ConvesioPay Platform",
    merchant: "merchant_1781877375334",
    connector: "adyen",
  },
  {
    org: "ConvesioPay Platform",
    merchant: "merchant_1781877395994",
    connector: "adyen",
  },
  {
    org: "ConvesioPay Platform",
    merchant: "merchant_1781897973601",
    connector: "adyen",
  },
  {
    org: "ConvesioPay Platform",
    merchant: "merchant_1782220301176",
    connector: "adyen",
  },
  {
    org: "ConvesioPay Platform",
    merchant: "merchant_1782222808739",
    connector: "adyen",
  },
  {
    org: "ConvesioPay Platform",
    merchant: "merchant_1782240439410",
    connector: "adyen",
  },
  {
    org: "Baskhealth",
    merchant: "gimme",
    connector: "stripe",
  },
  {
    org: "Baskhealth",
    merchant: "amble",
    connector: "stripe",
  },
  {
    org: "ABO FACTORY",
    merchant: "abo_factory",
    connector: "stripe",
  },
  {
    org: "Starlings",
    merchant: "consolidcorpo",
    connector: "stripe",
  },
  {
    org: "ConvesioPay Platform",
    merchant: "merchant_1774556192507",
    connector: "stripe",
  },
  {
    org: "ConvesioPay Platform",
    merchant: "merchant_1782111849079",
    connector: "stripe",
  },
  {
    org: "ConvesioPay Platform",
    merchant: "merchant_1782222808739",
    connector: "stripe",
  },
  {
    org: "ConvesioPay Platform",
    merchant: "merchant_1787949012943",
    connector: "stripe",
  },
  {
    org: "ConvesioPay Platform",
    merchant: "merchant_1771273863496",
    connector: "stripe",
  },
  {
    org: "narvar",
    merchant: "gregnormanshopify",
    connector: "paypal",
  },
  {
    org: "narvar",
    merchant: "oodie",
    connector: "paypal",
  },
  {
    org: "opencommerce",
    merchant: "opencommerce",
    connector: "paypal",
  },
  {
    org: "1st_phorm_international",
    merchant: "1st_phorm_international",
    connector: "authorizedotnet",
  },
  {
    org: "5 Axis Health",
    merchant: "vigoremd",
    connector: "authorizedotnet",
  },
  {
    org: "suiteop",
    merchant: "merchant_17696492920",
    connector: "authorizedotnet",
  },
  {
    org: "Flowbird",
    merchant: "flowbird",
    connector: "cybersource",
  },
  {
    org: "5 Axis Health",
    merchant: "md_exam",
    connector: "cybersource",
  },
  {
    org: "Recurly",
    merchant: "4478264500912691130",
    connector: "checkout",
  },
];
