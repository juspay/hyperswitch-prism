import { Router, Request, Response } from 'express';
import { MerchantAuthenticationClient, types } from 'hyperswitch-prism';
import { getConnectorConfig, getConnectorName, getPublishableKey, config } from '../config.js';
import { v4 as uuidv4 } from 'uuid';

const router = Router();
const { Environment, Currency } = types;

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
    const connectorConfig = getConnectorConfig(currencyStr);
    const publishableKey = getPublishableKey(currencyStr);

    console.log(`[SDK Session] Currency: ${currencyStr}, Connector: ${connectorName}`);

    // Create Merchant Authentication Client
    const authClient = new MerchantAuthenticationClient(connectorConfig);

    // Create client authentication token
    const merchantTransactionId = `txn_${uuidv4().replace(/-/g, '').substring(0, 16)}`;
    
    const sessionResponse = await authClient.createClientAuthenticationToken({
      merchantClientSessionId: `session_${Date.now()}`,
      payment: {
        amount: {
          minorAmount: amountNum,
          currency: currencyStr === 'EUR' ? Currency.EUR : Currency.USD
        }
      }
    });

    console.log(`[SDK Session] Session created successfully for ${connectorName}`);

    // Return unified response
    res.json({
      connector: connectorName,
      clientToken: JSON.stringify(sessionResponse),
      publishableKey,
      sessionData: sessionResponse as unknown as Record<string, unknown>,
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