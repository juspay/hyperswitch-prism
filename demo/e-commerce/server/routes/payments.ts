import { Router, Request, Response } from 'express';
import { PaymentClient, types, IntegrationError, ConnectorError } from 'hyperswitch-prism';
import { getConnectorConfig, getConnectorName, config } from '../config.js';
import { createClientAuthToken } from '../utils/auth.js';
import { getPaymentStatusText } from '../utils/payment-status.js';
const router = Router();
const { CaptureMethod, Currency } = types;


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

    // Get connector config based on currency and amount (amount > 50 uses Adyen)
    const connectorConfig = getConnectorConfig(currencyStr, amountNum);
    const connectorName = getConnectorName(currencyStr, amountNum);

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
      const { sessionResponse } = await createClientAuthToken(
        currencyStr,
        amountNum
      );
      const gpData = (sessionResponse as any).sessionData?.connectorSpecific?.globalpay;
      const serverToken = gpData?.accessToken?.value || '';
      // Add state with server token (as shown in transformer.js)
      authorizeRequest.state = {
        accessToken: {
          token: { value: serverToken },
          tokenType: 'Bearer'
        }
      };

      console.log('[Token Authorize] Added server token (via SDK) to GlobalPay request');
    }

    // Authorize payment using token
    const response = await paymentClient.tokenAuthorize(authorizeRequest);

    console.log(`[Token Authorize] Status: ${response.status}, Transaction ID: ${response.connectorTransactionId}`);

    // Get error message if present
    const errorMsg = response.error?.unifiedDetails?.message ||
      response.error?.issuerDetails?.message ||
      response.error?.connectorDetails?.message || null;

    // Return response
    res.json({
      status: response.status,
      statusText: getPaymentStatusText(response.status),
      connectorTransactionId: response.connectorTransactionId,
      error: errorMsg
    });

  } catch (error: any) {
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

export default router;
