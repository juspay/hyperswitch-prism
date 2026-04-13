import { Router, Request, Response } from 'express';
import { PaymentClient, types, IntegrationError, ConnectorError } from 'hyperswitch-prism';
import { getConnectorConfig, getConnectorName, config } from '../config.js';
import { v4 as uuidv4 } from 'uuid';
import crypto from 'crypto';

const router = Router();
const { PaymentStatus, RefundStatus, CaptureMethod, Currency } = types;

/**
 * Fetch GlobalPay server access token for authorization
 * Based on: /Users/jeeva.ramachandran/Downloads/archive/connector/globalpay/transformer.js
 */
async function fetchGlobalPayServerToken(appId: string, appKey: string): Promise<string> {
  const nonce = new Date().toISOString();
  const secret = crypto.createHash('sha512').update(nonce + appKey).digest('hex');

  const body = {
    app_id: appId,
    secret,
    grant_type: 'client_credentials',
    nonce,
    interval_to_expire: '1_HOUR'
  };

  console.log('[GlobalPay Server Token] Requesting token...');

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
    console.error('[GlobalPay Server Token] Error:', data);
    throw new Error(`GlobalPay server token request failed: ${JSON.stringify(data)}`);
  }

  console.log('[GlobalPay Server Token] Received:', data.token.substring(0, 15) + '...');
  return data.token;
}

/**
 * POST /api/payments/token-authorize
 * Authorizes a payment using a token from the client SDK
 * 
 * Request body:
 * - token: Payment method token from client SDK (pm_xxx for Stripe, etc.)
 * - merchantTransactionId: Unique transaction ID
 * - amount: Payment amount in minor units
 * - currency: USD | EUR
 */
router.post('/token-authorize', async (req: Request, res: Response) => {
  try {
    const { token, merchantTransactionId, amount, currency } = req.body;

    // Validate inputs
    if (!token || !merchantTransactionId || !amount || !currency) {
      return res.status(400).json({ 
        error: 'Missing required fields: token, merchantTransactionId, amount, currency' 
      });
    }

    const currencyStr = String(currency).toUpperCase();
    const amountNum = parseInt(String(amount), 10);

    console.log(`[Token Authorize] Transaction: ${merchantTransactionId}, Amount: ${amountNum} ${currencyStr}`);

    // Get connector config based on currency
    const connectorConfig = getConnectorConfig(currencyStr);
    const connectorName = getConnectorName(currencyStr);

    // Create Payment Client
    const paymentClient = new PaymentClient(connectorConfig);

    // Map currency string to Currency enum
    const currencyEnum = currencyStr === 'EUR' ? Currency.EUR : Currency.USD;

    // Prepare authorize request
    const authorizeRequest: any = {
      merchantTransactionId,
      amount: {
        minorAmount: amountNum,
        currency: currencyEnum
      },
      connectorToken: { value: token },
      captureMethod: CaptureMethod.AUTOMATIC,
      returnUrl: `${config.baseUrl}/checkout/return`,
      address: {}
    };

    // For GlobalPay, we need to pass a server access token in state
    if (connectorName === 'globalpay') {
      const appId = process.env.GLOBALPAY_APP_ID;
      const appKey = process.env.GLOBALPAY_APP_KEY;

      if (!appId || !appKey) {
        return res.status(500).json({
          error: 'GlobalPay credentials not configured'
        });
      }

      // Fetch server access token (without tokenization permissions - just for auth)
      const serverToken = await fetchGlobalPayServerToken(appId, appKey);

      // Add state with server token (as shown in transformer.js)
      authorizeRequest.state = {
        accessToken: {
          token: { value: serverToken },
          tokenType: 'Bearer'
        }
      };

      console.log('[Token Authorize] Added server token to GlobalPay request');
    }

    // Authorize payment using token
    const response = await paymentClient.tokenAuthorize(authorizeRequest);

    console.log(`[Token Authorize] Status: ${response.status}, Transaction ID: ${response.connectorTransactionId}`);

    // Get error message if present
    const errorMsg = response.error?.unifiedDetails?.message || 
                     response.error?.issuerDetails?.message || 
                     response.error?.connectorDetails?.message || null;

    // Map PaymentStatus enum to readable string
    const statusMap: Record<number, string> = {
      [PaymentStatus.PAYMENT_STATUS_UNSPECIFIED]: 'UNSPECIFIED',
      [PaymentStatus.STARTED]: 'STARTED',
      [PaymentStatus.PAYMENT_METHOD_AWAITED]: 'PAYMENT_METHOD_AWAITED',
      [PaymentStatus.DEVICE_DATA_COLLECTION_PENDING]: 'DEVICE_DATA_COLLECTION_PENDING',
      [PaymentStatus.CONFIRMATION_AWAITED]: 'CONFIRMATION_AWAITED',
      [PaymentStatus.AUTHENTICATION_PENDING]: 'AUTHENTICATION_PENDING',
      [PaymentStatus.AUTHENTICATION_SUCCESSFUL]: 'AUTHENTICATION_SUCCESSFUL',
      [PaymentStatus.AUTHENTICATION_FAILED]: 'AUTHENTICATION_FAILED',
      [PaymentStatus.AUTHORIZING]: 'AUTHORIZING',
      [PaymentStatus.AUTHORIZED]: 'AUTHORIZED',
      [PaymentStatus.AUTHORIZATION_FAILED]: 'AUTHORIZATION_FAILED',
      [PaymentStatus.PARTIALLY_AUTHORIZED]: 'PARTIALLY_AUTHORIZED',
      [PaymentStatus.CHARGED]: 'CHARGED',
      [PaymentStatus.PARTIAL_CHARGED]: 'PARTIAL_CHARGED',
      [PaymentStatus.PARTIAL_CHARGED_AND_CHARGEABLE]: 'PARTIAL_CHARGED_AND_CHARGEABLE',
      [PaymentStatus.AUTO_REFUNDED]: 'AUTO_REFUNDED',
      [PaymentStatus.CAPTURE_INITIATED]: 'CAPTURE_INITIATED',
      [PaymentStatus.CAPTURE_FAILED]: 'CAPTURE_FAILED',
      [PaymentStatus.VOID_INITIATED]: 'VOID_INITIATED',
      [PaymentStatus.VOIDED]: 'VOIDED',
      [PaymentStatus.VOID_FAILED]: 'VOID_FAILED',
      [PaymentStatus.VOIDED_POST_CAPTURE]: 'VOIDED_POST_CAPTURE',
      [PaymentStatus.COD_INITIATED]: 'COD_INITIATED',
      [PaymentStatus.EXPIRED]: 'EXPIRED',
      [PaymentStatus.ROUTER_DECLINED]: 'ROUTER_DECLINED',
      [PaymentStatus.PENDING]: 'PENDING',
      [PaymentStatus.FAILURE]: 'FAILURE',
      [PaymentStatus.UNRESOLVED]: 'UNRESOLVED'
    };

    // Return response
    res.json({
      status: response.status,
      statusText: statusMap[response.status] || 'UNKNOWN',
      connectorTransactionId: response.connectorTransactionId,
      error: errorMsg
    });

  } catch (error: unknown) {
    console.error('[Token Authorize Error]', error);
    
    if (error instanceof IntegrationError) {
      return res.status(400).json({ 
        error: 'Integration error',
        code: error.errorCode,
        message: error.message 
      });
    }
    
    if (error instanceof ConnectorError) {
      return res.status(502).json({ 
        error: 'Connector error',
        code: error.errorCode,
        message: error.message 
      });
    }
    
    const errorMessage = error instanceof Error ? error.message : 'Unknown error';
    res.status(500).json({ 
      error: 'Failed to authorize payment',
      details: errorMessage 
    });
  }
});

/**
 * POST /api/payments/refund
 * Refunds a captured payment
 * 
 * Request body:
 * - connectorTransactionId: Transaction ID from the original payment
 * - refundAmount: Amount to refund in minor units
 * - currency: USD | EUR
 * - merchantRefundId: Unique refund ID
 */
router.post('/refund', async (req: Request, res: Response) => {
  try {
    const { connectorTransactionId, refundAmount, currency, merchantRefundId } = req.body;

    // Validate inputs
    if (!connectorTransactionId || !refundAmount || !currency) {
      return res.status(400).json({ 
        error: 'Missing required fields: connectorTransactionId, refundAmount, currency' 
      });
    }

    const currencyStr = String(currency).toUpperCase();
    const refundAmountNum = parseInt(String(refundAmount), 10);
    const refundId = merchantRefundId || `ref_${uuidv4().replace(/-/g, '').substring(0, 16)}`;

    console.log(`[Refund] Transaction: ${connectorTransactionId}, Refund Amount: ${refundAmountNum} ${currencyStr}`);

    // Get connector config based on currency
    const connectorConfig = getConnectorConfig(currencyStr);
    const currencyEnum = currencyStr === 'EUR' ? Currency.EUR : Currency.USD;

    // Create Payment Client
    const paymentClient = new PaymentClient(connectorConfig);

    // Process refund
    const response = await paymentClient.refund({
      merchantRefundId: refundId,
      connectorTransactionId,
      refundAmount: {
        minorAmount: refundAmountNum,
        currency: currencyEnum
      },
      paymentAmount: refundAmountNum,
      reason: 'CUSTOMER_REQUEST'
    });

    console.log(`[Refund] Status: ${response.status}`);

    // Get error message if present
    const errorMsg = response.error?.unifiedDetails?.message || 
                     response.error?.issuerDetails?.message || 
                     response.error?.connectorDetails?.message || null;

    // Return response
    res.json({
      status: response.status,
      refundId: response.connectorRefundId,
      error: errorMsg
    });

  } catch (error: unknown) {
    console.error('[Refund Error]', error);
    
    if (error instanceof IntegrationError) {
      return res.status(400).json({ 
        error: 'Integration error',
        code: error.errorCode,
        message: error.message 
      });
    }
    
    if (error instanceof ConnectorError) {
      return res.status(502).json({ 
        error: 'Connector error',
        code: error.errorCode,
        message: error.message 
      });
    }
    
    const errorMessage = error instanceof Error ? error.message : 'Unknown error';
    res.status(500).json({ 
      error: 'Failed to process refund',
      details: errorMessage 
    });
  }
});

/**
 * GET /api/payments/:id
 * Gets the status of a payment
 * 
 * Path params:
 * - id: Transaction ID
 * 
 * Query params:
 * - currency: USD | EUR
 * - amount: Original payment amount
 */
router.get('/:id', async (req: Request, res: Response) => {
  try {
    const { id } = req.params;
    const { currency, amount } = req.query;

    if (!currency || !amount) {
      return res.status(400).json({ 
        error: 'Missing required query parameters: currency and amount' 
      });
    }

    const currencyStr = String(currency).toUpperCase();
    const amountNum = parseInt(String(amount), 10);

    console.log(`[Get Payment] Transaction ID: ${id}`);

    // Get connector config based on currency
    const connectorConfig = getConnectorConfig(currencyStr);
    const currencyEnum = currencyStr === 'EUR' ? Currency.EUR : Currency.USD;

    // Create Payment Client
    const paymentClient = new PaymentClient(connectorConfig);

    // Get payment status
    const response = await paymentClient.get({
      merchantTransactionId: id,
      connectorTransactionId: id,
      amount: {
        minorAmount: amountNum,
        currency: currencyEnum
      }
    });

    console.log(`[Get Payment] Status: ${response.status}`);

    // Get error message if present
    const errorMsg = response.error?.unifiedDetails?.message || 
                     response.error?.issuerDetails?.message || 
                     response.error?.connectorDetails?.message || null;

    // Return response
    res.json({
      status: response.status,
      connectorTransactionId: response.connectorTransactionId,
      error: errorMsg
    });

  } catch (error: unknown) {
    console.error('[Get Payment Error]', error);
    
    if (error instanceof IntegrationError) {
      return res.status(400).json({ 
        error: 'Integration error',
        code: error.errorCode,
        message: error.message 
      });
    }
    
    if (error instanceof ConnectorError) {
      return res.status(502).json({ 
        error: 'Connector error',
        code: error.errorCode,
        message: error.message 
      });
    }
    
    const errorMessage = error instanceof Error ? error.message : 'Unknown error';
    res.status(500).json({ 
      error: 'Failed to get payment status',
      details: errorMessage 
    });
  }
});

export default router;
