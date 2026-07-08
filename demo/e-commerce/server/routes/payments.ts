import { Router, Request, Response } from 'express';
import { PaymentClient, types, IntegrationError, ConnectorError } from 'hyperswitch-prism';
import { getConnectorConfig, getConnectorName, config } from '../config.js';
import { createClientAuthToken } from '../utils/auth.js';
import { getPaymentStatusText } from '../utils/payment-status.js';

const router = Router();
const { Currency, CaptureMethod } = types;

interface AuthorizeRequestBody {
  token: string;
  merchantTransactionId: string;
  amount: number;
  currency: string;
}

interface AuthorizeResponse {
  status: number;
  statusText: string;
  connectorTransactionId?: string | null;
  error: string | null;
}

/**
 * Validates the token authorize request body
 */
function validateAuthorizeRequest(body: any): AuthorizeRequestBody | null {
  const { token, merchantTransactionId, amount, currency } = body;

  if (!token || !merchantTransactionId || !amount || !currency) {
    return null;
  }

  const amountNum = parseInt(String(amount), 10);
  if (isNaN(amountNum) || amountNum <= 0) {
    return null;
  }

  return {
    token,
    merchantTransactionId,
    amount: amountNum,
    currency: String(currency).toUpperCase()
  };
}

/**
 * Builds the authorize request object for the payment client
 */
function buildAuthorizeRequest(
  params: AuthorizeRequestBody,
  serverToken?: string
): types.PaymentServiceTokenAuthorizeRequest {
  const currencyEnum = params.currency === 'EUR' ? Currency.EUR : Currency.USD;

  const request: types.PaymentServiceTokenAuthorizeRequest = {
    merchantTransactionId: params.merchantTransactionId,
    amount: {
      minorAmount: params.amount,
      currency: currencyEnum
    },
    connectorToken: { value: params.token },
    captureMethod: CaptureMethod.AUTOMATIC,
    returnUrl: `${config.baseUrl}/checkout/return`,
    address: {}
  };

  // For GlobalPay, add server access token in state
  if (serverToken) {
    request.state = {
      accessToken: {
        token: { value: serverToken },
        tokenType: 'Bearer'
      }
    };
  }

  return request;
}

/**
 * Fetches GlobalPay server access token for authorization
 */
async function fetchGlobalPayServerToken(
  currency: string,
  amount: number
): Promise<string> {
  const { sessionResponse } = await createClientAuthToken(currency, amount);

  const gpData = (sessionResponse as types.MerchantAuthenticationServiceCreateClientAuthenticationTokenResponse)
    .sessionData?.connectorSpecific?.globalpay;

  return gpData?.accessToken?.value || '';
}

/**
 * Processes payment authorization
 */
async function processAuthorization(
  params: AuthorizeRequestBody
): Promise<AuthorizeResponse> {
  const connectorConfig = getConnectorConfig(params.currency, params.amount);
  const connectorName = getConnectorName(params.currency, params.amount);

  const paymentClient = new PaymentClient(connectorConfig);

  // Fetch server token for GlobalPay if needed
  let serverToken: string | undefined;
  if (connectorName === 'globalpay') {
    serverToken = await fetchGlobalPayServerToken(params.currency, params.amount);
  }

  const authorizeRequest = buildAuthorizeRequest(params, serverToken);
  const response: types.PaymentServiceAuthorizeResponse = await paymentClient.tokenAuthorize(authorizeRequest);

  // Extract error message if present
  const errorMsg = response.error?.unifiedDetails?.message ||
    response.error?.issuerDetails?.message ||
    response.error?.connectorDetails?.message || null;

  return {
    status: response.status,
    statusText: getPaymentStatusText(response.status),
    connectorTransactionId: response.connectorTransactionId,
    error: errorMsg
  };
}

/**
 * Handles errors and sends appropriate response
 */
function handleAuthorizeError(res: Response, error: unknown): void {
  console.error('[Token Authorize Error]', error);

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
    error: 'Failed to authorize payment',
    details: errorMessage
  });
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
    const params = validateAuthorizeRequest(req.body);

    if (!params) {
      return res.status(400).json({
        error: 'Missing or invalid required fields: token, merchantTransactionId, amount (positive number), currency'
      });
    }

    const result = await processAuthorization(params);
    res.json(result);
  } catch (error) {
    handleAuthorizeError(res, error);
  }
});

export default router;
