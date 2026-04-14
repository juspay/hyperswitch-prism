import { Router, Request, Response } from 'express';
import { PaymentClient, types, IntegrationError, ConnectorError } from 'hyperswitch-prism';
import { getConnectorConfig, getConnectorName, config } from '../config.js';
import { createClientAuthToken } from '../utils/auth.js';
const router = Router();
const { PaymentStatus, CaptureMethod, Currency } = types;


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

export default router;
