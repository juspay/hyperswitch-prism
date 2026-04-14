import { Router, Request, Response } from 'express';
import { getConnectorName, getPublishableKey, config } from '../config.js';
import { createClientAuthToken, fetchGlobalPayAccessToken } from '../utils/auth.js';
import { v4 as uuidv4 } from 'uuid';


const router = Router();
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
      const { sessionResponse } = await createClientAuthToken(
        currencyStr,
        amountNum
      );

      const gpData = (sessionResponse as any).sessionData?.connectorSpecific?.stripe;
      const serverToken = gpData?.clientSecret?.value || '';
      publishableKey = getPublishableKey(currencyStr);

      console.log('[Stripe Raw Request]', sessionResponse.rawConnectorRequest?.value);

      // Extract Stripe client secret
      clientToken = serverToken;
      sessionData = sessionResponse as unknown as Record<string, unknown>;

    } else if (connectorName === 'globalpay') {

      const appId = process.env.GLOBALPAY_APP_ID!;
      const appKey = process.env.GLOBALPAY_APP_KEY!;
      const fetchToken = await fetchGlobalPayAccessToken(
        appId,
        appKey,
        ["PMT_POST_Create_Single"],
      );
      clientToken = fetchToken;
      sessionData = {};
      publishableKey = getPublishableKey(currencyStr);
    } else {
      return res.status(400).json({
        error: `Unsupported connector: ${connectorName}`
      });
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
