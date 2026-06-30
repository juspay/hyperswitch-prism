// Shared test data: sample order and test cards.
// These are public sandbox test cards — never real PANs.

import { types } from 'hyperswitch-prism';

export interface TestCard {
  cardNumber: string;
  cardExpMonth: string;
  cardExpYear: string;
  cardCvc: string;
  cardHolderName: string;
}

// A card most sandbox processors accept and approve.
export const APPROVED_CARD: TestCard = {
  cardNumber: '4111111111111111',
  cardExpMonth: '03',
  cardExpYear: '2030',
  cardCvc: '737',
  cardHolderName: 'Jane Workshop',
};

// A card most sandbox processors decline — handy for the retry demo.
export const DECLINED_CARD: TestCard = {
  cardNumber: '4000000000000002',
  cardExpMonth: '03',
  cardExpYear: '2030',
  cardCvc: '737',
  cardHolderName: 'Jane Workshop',
};

export interface Order {
  merchantTransactionId: string;
  minorAmount: number; // amount in minor units (cents). 1000 = $10.00
  currency: string; // ISO 4217, e.g. 'USD'
  card: TestCard;
}

// Build the SDK card object from a TestCard.
export function toSdkCard(card: TestCard) {
  return {
    cardNumber: { value: card.cardNumber },
    cardExpMonth: { value: card.cardExpMonth },
    cardExpYear: { value: card.cardExpYear },
    cardCvc: { value: card.cardCvc },
    cardHolderName: { value: card.cardHolderName },
  };
}

// Map an ISO currency string to the SDK Currency enum (defaults to USD).
export function toSdkCurrency(currency: string): types.Currency {
  const C = types.Currency as unknown as Record<string, types.Currency>;
  return C[currency.toUpperCase()] ?? types.Currency.USD;
}
