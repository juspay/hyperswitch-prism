import { Router, Request, Response } from 'express';
import { PaymentClient, types, IntegrationError, ConnectorError } from 'hyperswitch-prism';
import { getConnectorConfig, getConnectorName, config } from '../config.js';
import { v4 as uuidv4 } from 'uuid';

const router = Router();
const { PaymentStatus, RefundStatus, CaptureMethod, Currency } = types;

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

    // Authorize payment using token
    const response = await paymentClient.tokenAuthorize({
      merchantTransactionId,
      amount: {
        minorAmount: amountNum,
        currency: currencyEnum
      },
      connectorToken: { value: token },
      captureMethod: CaptureMethod.AUTOMATIC,
      returnUrl: `${config.baseUrl}/checkout/return`
    });

    console.log(`[Token Authorize] Status: ${response.status}, Transaction ID: ${response.connectorTransactionId}`);

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