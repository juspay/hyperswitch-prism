import { Router, Request, Response } from 'express';
import { MerchantAuthenticationClient, types } from 'hyperswitch-prism';
import { getConnectorConfig, getConnectorName, getPublishableKey, config } from '../config.js';
import { v4 as uuidv4 } from 'uuid';
import crypto from 'crypto';

const router = Router();
const { Environment, Currency } = types;

/**
 * Fetch GlobalPay access token with specific permissions
 * Based on: /Users/jeeva.ramachandran/Downloads/archive/connector/globalpay/transformer.js
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

  const data = await resp.json() as { token?: string; [key: string]: unknown };

  if (!data.token) {
    console.error('[GlobalPay Token] Error:', data);
    throw new Error(`GlobalPay access token request failed: ${JSON.stringify(data)}`);
  }

  console.log('[GlobalPay Token] Received token:', data.token.substring(0, 15) + '...');
  return data.token;
}

/**
 * GET /api/auth/sdk-session
 * Creates an SDK session for client-side payment tokenization
 *
 * Query params:
 * - currency: USD | EUR
 * - amount: Payment amount in minor units
 */
router.get('/sdk-session', async (req: Request, res: Response) => {
  try {
    const { currency, amount } = req.query;

    // Validate inputs
    if (!currency || !amount) {
      return res.status(400).json({
        error: 'Missing required query parameters: currency and amount'
      });
    }

    const currencyStr = String(currency).toUpperCase();
    const amountNum = parseInt(String(amount), 10);

    if (isNaN(amountNum) || amountNum <= 0) {
      return res.status(400).json({
        error: 'Invalid amount. Must be a positive number.'
      });
    }

    // Determine connector based on currency
    const connectorName = getConnectorName(currencyStr);

    console.log(`[SDK Session] Currency: ${currencyStr}, Connector: ${connectorName}`);

    let clientToken: string;
    let publishableKey: string;
    let sessionData: Record<string, unknown>;

    if (connectorName === 'stripe') {
      // Stripe flow - use SDK
      const connectorConfig = getConnectorConfig(currencyStr);
      publishableKey = getPublishableKey(currencyStr);

      const authClient = new MerchantAuthenticationClient(connectorConfig);
      const sessionResponse = await authClient.createClientAuthenticationToken({
        merchantClientSessionId: `session_${Date.now()}`,
        payment: {
          amount: {
            minorAmount: amountNum,
            currency: Currency.USD
          }
        }
      });

      // Extract Stripe client secret
      const stripeData = (sessionResponse as any).sessionData?.connectorSpecific?.stripe;
      clientToken = stripeData?.clientSecret?.value || '';
      sessionData = sessionResponse as unknown as Record<string, unknown>;

    } else {
      // GlobalPay flow - fetch token directly with tokenization permissions
      const appId = process.env.GLOBALPAY_APP_ID;
      const appKey = process.env.GLOBALPAY_APP_KEY;

      if (!appId || !appKey) {
        return res.status(500).json({
          error: 'GlobalPay credentials not configured'
        });
      }

      // Fetch token with PMT_POST_Create_Single permission for client-side tokenization
      const accessToken = await fetchGlobalPayAccessToken(
        appId,
        appKey,
        ['PMT_POST_Create_Single']  // Permission needed for Hosted Fields tokenization
      );

      clientToken = accessToken;
      publishableKey = ''; // Not used for GlobalPay
      sessionData = {
        accessToken: accessToken,
        currency: currencyStr,
        amount: amountNum
      };
    }

    const merchantTransactionId = `txn_${uuidv4().replace(/-/g, '').substring(0, 16)}`;

    console.log(`[SDK Session] Session created successfully for ${connectorName}`);

    // Return unified response
    res.json({
      connector: connectorName,
      clientToken,
      publishableKey,
      sessionData,
      merchantTransactionId,
      amount: amountNum,
      currency: currencyStr
    });

  } catch (error: unknown) {
    console.error('[SDK Session Error]', error);

    const errorMessage = error instanceof Error ? error.message : 'Unknown error';
    res.status(500).json({
      error: 'Failed to create SDK session',
      details: errorMessage
    });
  }
});

export default router;
