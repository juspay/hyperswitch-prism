// Documented sandbox test cards (see medusa-unified-payment/README.md).
export interface TestCard {
  number: string
  expMonth: string
  expYear: string
  cvc: string
}

export const TEST_CARDS: Record<string, TestCard> = {
  stripe: { number: "4242424242424242", expMonth: "03", expYear: "30", cvc: "737" },
  adyen: { number: "4111111145551142", expMonth: "03", expYear: "30", cvc: "737" },
  globalpay: { number: "4263970000005262", expMonth: "03", expYear: "30", cvc: "737" },
  // PayPal "Debit or Credit Card" guest path inside the approval popup.
  paypal: { number: "4032036691705063", expMonth: "10", expYear: "28", cvc: "901" },
}
