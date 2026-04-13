import { MerchantAuthenticationClient, types } from 'hyperswitch-prism';

const { Currency } = types;

/**
 * Creates a client authentication token using the SDK
 * Can be used for both client-side and server-side authentication
 */
export async function createClientAuthToken(
  currencyStr: string,
  amountNum: number
): Promise<{ sessionResponse: any; serverToken: string; currencyEnum: types.Currency }> {
  const currencyEnum = currencyStr === 'EUR' ? Currency.EUR : Currency.USD;

  // Import dynamically to avoid circular dependency
  const { getConnectorConfig } = await import('../config.js');
  const connectorConfig = getConnectorConfig(currencyStr);

  const authClient = new MerchantAuthenticationClient(connectorConfig);
  const sessionResponse = await authClient.createClientAuthenticationToken({
    merchantClientSessionId: `session_${Date.now()}`,
    payment: {
      amount: {
        minorAmount: amountNum,
        currency: currencyEnum
      }
    }
  });

  // Extract GlobalPay server token from SDK response
  const gpData = (sessionResponse as any).sessionData?.connectorSpecific?.globalpay;
  const serverToken = gpData?.accessToken?.value || '';

  return { sessionResponse, serverToken, currencyEnum };
}
