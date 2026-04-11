import { PaymentClient, MerchantAuthenticationClient, types } from 'hs-paylib';
import { CYBERSOURCE_CONFIG, PAYPAL_CONFIG, ADYEN_CONFIG } from './config';

// Pre-initialized clients for connection pool reuse
const cybersourceClient = new PaymentClient(CYBERSOURCE_CONFIG);
const paypalClient = new PaymentClient(PAYPAL_CONFIG);
const paypalAuthClient = new MerchantAuthenticationClient(PAYPAL_CONFIG);
const adyenClient = new PaymentClient(ADYEN_CONFIG);

export type ConnectorName = 'cybersource' | 'paypal' | 'adyen';

export interface RouteResult {
  connector: ConnectorName;
  client: PaymentClient;
}

/**
 * Routes payments based on currency and amount:
 * - EUR -> PayPal
 * - USD amount > 100 (minor units > 10000) -> Cybersource
 * - USD amount <= 100 (minor units <= 10000) -> Adyen
 */
export function routePayment(currency: types.Currency, minorAmount: number): RouteResult {
  if (currency === types.Currency.EUR) {
    return { connector: 'paypal', client: paypalClient };
  }

  // USD routing by amount threshold (100 USD = 10000 minor units)
  if (minorAmount > 10000) {
    return { connector: 'cybersource', client: cybersourceClient };
  }

  return { connector: 'adyen', client: adyenClient };
}

/**
 * Obtain a PayPal access token for card payments.
 */
export async function getPayPalAccessToken(): Promise<{
  token: string;
  expiresInSeconds: number;
}> {
  const tokenResponse = await paypalAuthClient.createServerAuthenticationToken({
    merchantAccessTokenId: `token_${Date.now()}`,
    connector: types.Connector.PAYPAL,
    testMode: true,
  });

  if (!tokenResponse.accessToken?.value) {
    throw new Error('Failed to obtain PayPal access token');
  }

  return {
    token: tokenResponse.accessToken.value,
    expiresInSeconds: Number(tokenResponse.expiresInSeconds || 0),
  };
}

export function parseCurrency(currency: string): types.Currency {
  const value = (types.Currency as unknown as Record<string, number>)[currency];
  if (value === undefined) {
    throw new Error(`Unsupported currency: ${currency}`);
  }
  return value;
}
