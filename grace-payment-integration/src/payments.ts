import { types } from 'hs-paylib';
import { routePayment, getPayPalAccessToken, parseCurrency, ConnectorName } from './router';

const BROWSER_INFO = {
  colorDepth: 24,
  screenHeight: 900,
  screenWidth: 1440,
  javaEnabled: false,
  javaScriptEnabled: true,
  language: 'en-US',
  timeZoneOffsetMinutes: 0,
  acceptHeader: 'text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8',
  userAgent: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)',
};

export interface AuthorizeRequest {
  currency: string;
  minorAmount: number;
  cardNumber: string;
  cardExpMonth: string;
  cardExpYear: string;
  cardCvc: string;
  cardHolderName?: string;
  captureMethod?: 'automatic' | 'manual';
}

export interface PaymentResponse {
  connector: ConnectorName;
  status: number;
  statusName: string;
  connectorTransactionId?: string;
  connectorFeatureData?: string;
  error?: { message?: string; code?: string; reason?: string };
}

export interface RefundRequest {
  connectorTransactionId: string;
  refundMinorAmount: number;
  originalMinorAmount: number;
  currency: string;
  reason?: string;
  connectorFeatureData?: string;
}

export interface RefundResponse {
  connector: ConnectorName;
  status: number;
  statusName: string;
  connectorRefundId?: string;
  error?: { message?: string; code?: string; reason?: string };
}

function getPaymentStatusName(status: number): string {
  const map: Record<number, string> = {
    0: 'UNSPECIFIED', 1: 'STARTED', 2: 'AUTHENTICATION_FAILED',
    3: 'ROUTER_DECLINED', 4: 'AUTHENTICATION_PENDING',
    5: 'AUTHENTICATION_SUCCESSFUL', 6: 'AUTHORIZED',
    7: 'AUTHORIZATION_FAILED', 8: 'CHARGED', 11: 'VOIDED',
    12: 'VOID_INITIATED', 13: 'CAPTURE_INITIATED', 14: 'CAPTURE_FAILED',
    15: 'VOID_FAILED', 17: 'PARTIAL_CHARGED', 19: 'UNRESOLVED',
    20: 'PENDING', 21: 'FAILURE', 25: 'PARTIALLY_AUTHORIZED', 26: 'EXPIRED',
  };
  return map[status] || `UNKNOWN(${status})`;
}

function getRefundStatusName(status: number): string {
  const map: Record<number, string> = {
    0: 'UNSPECIFIED', 1: 'REFUND_FAILURE', 2: 'REFUND_MANUAL_REVIEW',
    3: 'REFUND_PENDING', 4: 'REFUND_SUCCESS', 5: 'REFUND_TRANSACTION_FAILURE',
  };
  return map[status] || `UNKNOWN(${status})`;
}

export async function authorize(req: AuthorizeRequest): Promise<PaymentResponse> {
  const currency = parseCurrency(req.currency);
  const { connector, client } = routePayment(currency, req.minorAmount);

  const captureMethod = req.captureMethod === 'manual'
    ? types.CaptureMethod.MANUAL
    : types.CaptureMethod.AUTOMATIC;

  // Build state for PayPal (requires access token)
  let state: any;
  if (connector === 'paypal') {
    const accessToken = await getPayPalAccessToken();
    state = {
      accessToken: {
        token: { value: accessToken.token },
        tokenType: 'Bearer',
        expiresInSeconds: accessToken.expiresInSeconds,
      },
    };
  }

  const response = await client.authorize({
    merchantTransactionId: `txn_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`,
    amount: { minorAmount: req.minorAmount, currency },
    captureMethod,
    paymentMethod: {
      card: {
        cardNumber: { value: req.cardNumber },
        cardExpMonth: { value: req.cardExpMonth },
        cardExpYear: { value: req.cardExpYear },
        cardCvc: { value: req.cardCvc },
        cardHolderName: { value: req.cardHolderName || 'Test User' },
      },
    },
    customer: {
      email: { value: 'test@example.com' },
    },
    address: {
      billingAddress: {
        firstName: { value: 'Test' },
        lastName: { value: 'User' },
        line1: { value: '123 Main St' },
        city: { value: 'San Francisco' },
        state: { value: 'CA' },
        zipCode: { value: '94105' },
        countryAlpha2Code: types.CountryAlpha2.US,
      },
    },
    authType: types.AuthenticationType.NO_THREE_DS,
    returnUrl: 'https://example.com/return',
    orderDetails: [],
    browserInfo: (connector === 'adyen' || connector === 'cybersource') ? BROWSER_INFO : undefined,
    state,
    testMode: true,
  });

  return {
    connector,
    status: response.status,
    statusName: getPaymentStatusName(response.status),
    connectorTransactionId: response.connectorTransactionId || undefined,
    connectorFeatureData: response.connectorFeatureData?.value ?? undefined,
    error: response.error ? {
      message: (response.error.unifiedDetails?.message || response.error.connectorDetails?.message) ?? undefined,
      code: (response.error.unifiedDetails?.code || response.error.connectorDetails?.code) ?? undefined,
      reason: (response.error.connectorDetails?.reason || response.error.unifiedDetails?.description) ?? undefined,
    } : undefined,
  };
}

export async function refund(req: RefundRequest): Promise<RefundResponse> {
  const currency = parseCurrency(req.currency);
  const { connector, client } = routePayment(currency, req.originalMinorAmount);

  // PayPal refund requires access token in state
  let state: any;
  if (connector === 'paypal') {
    const accessToken = await getPayPalAccessToken();
    state = {
      accessToken: {
        token: { value: accessToken.token },
        tokenType: 'Bearer',
        expiresInSeconds: accessToken.expiresInSeconds,
      },
    };
  }

  const response = await client.refund({
    merchantRefundId: `ref_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`,
    connectorTransactionId: req.connectorTransactionId,
    refundAmount: { minorAmount: req.refundMinorAmount, currency },
    paymentAmount: req.originalMinorAmount,
    reason: req.reason || 'OTHER',
    connectorFeatureData: req.connectorFeatureData ? { value: req.connectorFeatureData } : undefined,
    state,
    testMode: true,
  });

  return {
    connector,
    status: response.status,
    statusName: getRefundStatusName(response.status),
    connectorRefundId: response.connectorRefundId || undefined,
    error: response.error ? {
      message: (response.error.unifiedDetails?.message || response.error.connectorDetails?.message) ?? undefined,
      code: (response.error.unifiedDetails?.code || response.error.connectorDetails?.code) ?? undefined,
      reason: (response.error.connectorDetails?.reason || response.error.unifiedDetails?.description) ?? undefined,
    } : undefined,
  };
}
