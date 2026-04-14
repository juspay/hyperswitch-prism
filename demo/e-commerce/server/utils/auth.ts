import { MerchantAuthenticationClient, types } from 'hyperswitch-prism';
import crypto from 'crypto';
import { getConnectorConfig } from '../config.js';
const { Currency } = types;
/**
 * Creates a client authentication token using the SDK (wrapper that adds publishableKey)
 */
async function createClientAuthToken(
  currencyStr: string,
  amountNum: number
): Promise<{ sessionResponse: any }> {
  try {
    const connectorConfig = getConnectorConfig(currencyStr, amountNum);
    const authClient = new MerchantAuthenticationClient(connectorConfig);
    const currencyEnum = currencyStr === 'EUR' ? Currency.EUR : Currency.USD;

    const sessionResponse = await authClient.createClientAuthenticationToken({
      merchantClientSessionId: `server_session_${Date.now()}`,
      payment: {
        amount: {
          minorAmount: amountNum,
          currency: currencyEnum
        }
      }
    });
    return { sessionResponse };
  } catch (error) {
    console.error('[createClientAuthToken] Error creating client auth token:', error);
    throw error;
  }
}


/**
 * Fetch GlobalPay access token with specific permissions
 */
async function fetchGlobalPayAccessToken(appId: string, appKey: string, permissions?: string[]) {
  const nonce = new Date().toISOString();
  const secret = crypto.createHash('sha512').update(nonce + appKey).digest('hex');

  const body: any = {
    app_id: appId,
    secret,
    grant_type: 'client_credentials',
    nonce,
    interval_to_expire: '1_HOUR'
  };

  if (permissions && permissions.length > 0) {
    body.permissions = permissions;
  }

  console.log('[GlobalPay Token] Requesting token with permissions:', permissions || 'default');

  const resp = await fetch('https://apis.sandbox.globalpay.com/ucp/accesstoken', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      'X-GP-Version': '2021-03-22'
    },
    body: JSON.stringify(body)
  });

  const data = await resp.json() as { token?: string;[key: string]: unknown };
  console.log(JSON.stringify(data))
  if (!data.token) {
    console.error('[GlobalPay Token] Error:', data);
    throw new Error(`GlobalPay access token request failed: ${JSON.stringify(data)}`);
  }

  console.log('[GlobalPay Token] Received token:', data.token.substring(0, 15) + '...');
  return data.token;
}

export { createClientAuthToken, fetchGlobalPayAccessToken }