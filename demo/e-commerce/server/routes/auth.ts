import { Router, Request, Response } from 'express';
import { types, IntegrationError, ConnectorError } from 'hyperswitch-prism';
import { getConnectorName } from '../config.js';
import { createClientAuthToken, fetchGlobalPayAccessToken } from '../utils/auth.js';
import { v4 as uuidv4 } from 'uuid';

const router = Router();

interface SessionResponse {
  connector: string;
  clientToken: string;
  publishableKey: string;
  sessionData: Record<string, unknown>;
  merchantTransactionId: string;
  amount: number;
  currency: string;
}

/**
 * Validates query parameters for SDK session request
 */
function validateSessionParams(req: Request): { currency: string; amount: number } | null {
  const { currency, amount } = req.query;

  if (!currency || !amount) {
    return null;
  }

  const currencyStr = String(currency).toUpperCase();
  const amountNum = parseInt(String(amount), 10);

  if (isNaN(amountNum) || amountNum <= 0) {
    return null;
  }

  return { currency: currencyStr, amount: amountNum };
}

/**
 * Creates Stripe SDK session
 */
async function createStripeSession(
  currency: string,
  amount: number
): Promise<{ clientToken: string; publishableKey: string; sessionData: Record<string, unknown> }> {
  const { sessionResponse } = await createClientAuthToken(currency, amount);

  const stripeData = (sessionResponse as types.MerchantAuthenticationServiceCreateClientAuthenticationTokenResponse).sessionData?.connectorSpecific?.stripe;
  const clientToken = stripeData?.clientSecret?.value || '';
  const publishableKey = process.env.STRIPE_PUBLISHABLE_KEY || '';

  return {
    clientToken,
    publishableKey,
    sessionData: sessionResponse as unknown as Record<string, unknown>
  };
}

/**
 * Creates GlobalPay SDK session
 */
async function createGlobalPaySession(): Promise<{ clientToken: string; publishableKey: string; sessionData: Record<string, unknown> }> {
  const appId = process.env.GLOBALPAY_APP_ID!;
  const appKey = process.env.GLOBALPAY_APP_KEY!;

  const clientToken = await fetchGlobalPayAccessToken(
    appId,
    appKey,
    ['PMT_POST_Create_Single']
  );

  return {
    clientToken,
    publishableKey: '',
    sessionData: {}
  };
}

/**
 * Creates Adyen SDK session
 */
async function createAdyenSession(
  currency: string,
  amount: number
): Promise<{ clientToken: string; publishableKey: string; sessionData: Record<string, unknown> }> {
  const { sessionResponse } = await createClientAuthToken(currency, amount);

  const adyenData = (sessionResponse as any).sessionData?.connectorSpecific?.adyen;
  const sessionId = adyenData?.sessionId || '';
  const sessionDataValue = adyenData?.sessionData?.value || '';

  return {
    clientToken: sessionId,
    publishableKey: process.env.ADYEN_CLIENT_KEY || '',
    sessionData: {
      sessionData: sessionDataValue,
      connectorSpecific: { adyen: adyenData }
    }
  };
}

/**
 * Handles errors and sends appropriate response
 */
function handleError(res: Response, error: unknown): void {
  console.error('[SDK Session Error]', error);

  if (error instanceof IntegrationError) {
    res.status(400).json({
      error: 'Integration error',
      code: error.errorCode,
      message: error.message
    });
    return;
  }

  if (error instanceof ConnectorError) {
    res.status(502).json({
      error: 'Connector error',
      code: error.errorCode,
      message: error.message
    });
    return;
  }

  const errorMessage = error instanceof Error ? error.message : 'Unknown error';
  res.status(500).json({
    error: 'Failed to create SDK session',
    details: errorMessage
  });
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
    const params = validateSessionParams(req);

    if (!params) {
      return res.status(400).json({
        error: 'Missing or invalid query parameters: currency and amount (positive number)'
      });
    }

    const { currency, amount } = params;
    const connectorName = getConnectorName(currency, amount);

    let sessionResult: { clientToken: string; publishableKey: string; sessionData: Record<string, unknown> };

    switch (connectorName) {
      case 'stripe':
        sessionResult = await createStripeSession(currency, amount);
        break;
      case 'globalpay':
        sessionResult = await createGlobalPaySession();
        break;
      case 'adyen':
        sessionResult = await createAdyenSession(currency, amount);
        break;
      default:
        return res.status(400).json({
          error: `Unsupported connector: ${connectorName}`
        });
    }

    const response: SessionResponse = {
      connector: connectorName,
      ...sessionResult,
      merchantTransactionId: `txn_${uuidv4().replace(/-/g, '').substring(0, 16)}`,
      amount,
      currency
    };

    res.json(response);
  } catch (error) {
    handleError(res, error);
  }
});

export default router;
