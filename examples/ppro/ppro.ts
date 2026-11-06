// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py ppro
//
// Ppro — all integration scenarios and flows in one file.
// Run a scenario:  npx tsx ppro.ts checkout_autocapture

import { PaymentClient, EventClient, RecurringPaymentClient, RefundClient, types } from 'hyperswitch-prism';
const { Environment, AuthenticationType, CaptureMethod, CountryAlpha2, Currency, HttpMethod, PaymentMethodType } = types;
export const SUPPORTED_FLOWS = ["authorize", "capture", "get", "parse_event", "recurring_charge", "refund", "refund_get"];

const _defaultConfig: types.IConnectorConfig = {
    options: {
        environment: Environment.SANDBOX,
    },
    connectorConfig: {
        ppro: {
            apiKey: { value: 'YOUR_API_KEY' },
            merchantId: { value: 'YOUR_MERCHANT_ID' },
            baseUrl: 'YOUR_BASE_URL',
            webhookSecret: { value: 'YOUR_WEBHOOK_SECRET' },
        }
    },
};


function _buildAuthorizeRequest(captureMethod: types.CaptureMethod): types.IPaymentServiceAuthorizeRequest {
    return {
        "merchantTransactionId": "probe_txn_001",  // Identification.
        "amount": {  // The amount for the payment.
            "minorAmount": 1000,
            "currency": Currency.USD
        },
        "paymentMethod": {  // Payment method to be used.
            "ideal": {
            }
        },
        "captureMethod": captureMethod,  // Method for capturing the payment.
        "address": {  // Address Information.
            "billingAddress": {
                "countryAlpha2Code": CountryAlpha2.US
            }
        },
        "authType": AuthenticationType.NO_THREE_DS,  // Authentication Details.
        "returnUrl": "https://example.com/return",  // URLs for Redirection and Webhooks.
        "webhookUrl": "https://example.com/webhook"
    };
}

function _buildCaptureRequest(connectorTransactionId: string): types.IPaymentServiceCaptureRequest {
    return {
        "merchantCaptureId": "probe_capture_001",  // Identification.
        "connectorTransactionId": connectorTransactionId,
        "amountToCapture": {  // Capture Details.
            "minorAmount": 1000,
            "currency": Currency.USD
        }
    };
}

function _buildGetRequest(connectorTransactionId: string): types.IPaymentServiceGetRequest {
    return {
        "merchantTransactionId": "probe_merchant_txn_001",  // Identification.
        "connectorTransactionId": connectorTransactionId,
        "amount": {  // Amount Information.
            "minorAmount": 1000,
            "currency": Currency.USD
        }
    };
}

function _buildHandleEventRequest(): types.IEventServiceHandleRequest {
    return {
        "merchantEventId": "probe_event_001",
        "requestDetails": {
            "method": HttpMethod.HTTP_METHOD_POST,  // HTTP method of the request (e.g., GET, POST).
            "legacyHeaders": {  // Headers of the HTTP request.
            },
            "body": {"value": new Uint8Array([123, 34, 115, 112, 101, 99, 118, 101, 114, 115, 105, 111, 110, 34, 58, 34, 49, 46, 48, 34, 44, 34, 116, 121, 112, 101, 34, 58, 34, 80, 65, 89, 77, 69, 78, 84, 95, 67, 72, 65, 82, 71, 69, 95, 83, 85, 67, 67, 69, 83, 83, 34, 44, 34, 115, 111, 117, 114, 99, 101, 34, 58, 34, 112, 114, 111, 98, 101, 95, 115, 111, 117, 114, 99, 101, 34, 44, 34, 105, 100, 34, 58, 34, 112, 114, 111, 98, 101, 95, 101, 118, 101, 110, 116, 95, 48, 48, 49, 34, 44, 34, 116, 105, 109, 101, 34, 58, 34, 50, 48, 50, 52, 45, 48, 49, 45, 48, 49, 84, 48, 48, 58, 48, 48, 58, 48, 48, 90, 34, 44, 34, 100, 97, 116, 97, 34, 58, 123, 34, 99, 104, 97, 114, 103, 101, 34, 58, 123, 34, 105, 100, 34, 58, 34, 112, 114, 111, 98, 101, 95, 116, 120, 110, 95, 48, 48, 49, 34, 44, 34, 115, 116, 97, 116, 117, 115, 34, 58, 34, 83, 85, 67, 67, 69, 69, 68, 69, 68, 34, 44, 34, 97, 109, 111, 117, 110, 116, 34, 58, 49, 48, 48, 48, 44, 34, 99, 117, 114, 114, 101, 110, 99, 121, 34, 58, 34, 69, 85, 82, 34, 125, 125, 125])},  // Secret-typed replacements. Presence takes precedence over the legacy field, including an explicitly empty body, map, URI or query string.
            "uri": {"value": "https://example.com/webhook"}
        }
    };
}

function _buildParseEventRequest(): types.IEventServiceParseRequest {
    return {
        "requestDetails": {
            "method": HttpMethod.HTTP_METHOD_POST,  // HTTP method of the request (e.g., GET, POST).
            "legacyHeaders": {  // Headers of the HTTP request.
            },
            "body": {"value": new Uint8Array([123, 34, 115, 112, 101, 99, 118, 101, 114, 115, 105, 111, 110, 34, 58, 34, 49, 46, 48, 34, 44, 34, 116, 121, 112, 101, 34, 58, 34, 80, 65, 89, 77, 69, 78, 84, 95, 67, 72, 65, 82, 71, 69, 95, 83, 85, 67, 67, 69, 83, 83, 34, 44, 34, 115, 111, 117, 114, 99, 101, 34, 58, 34, 112, 114, 111, 98, 101, 95, 115, 111, 117, 114, 99, 101, 34, 44, 34, 105, 100, 34, 58, 34, 112, 114, 111, 98, 101, 95, 101, 118, 101, 110, 116, 95, 48, 48, 49, 34, 44, 34, 116, 105, 109, 101, 34, 58, 34, 50, 48, 50, 52, 45, 48, 49, 45, 48, 49, 84, 48, 48, 58, 48, 48, 58, 48, 48, 90, 34, 44, 34, 100, 97, 116, 97, 34, 58, 123, 34, 99, 104, 97, 114, 103, 101, 34, 58, 123, 34, 105, 100, 34, 58, 34, 112, 114, 111, 98, 101, 95, 116, 120, 110, 95, 48, 48, 49, 34, 44, 34, 115, 116, 97, 116, 117, 115, 34, 58, 34, 83, 85, 67, 67, 69, 69, 68, 69, 68, 34, 44, 34, 97, 109, 111, 117, 110, 116, 34, 58, 49, 48, 48, 48, 44, 34, 99, 117, 114, 114, 101, 110, 99, 121, 34, 58, 34, 69, 85, 82, 34, 125, 125, 125])},  // Secret-typed replacements. Presence takes precedence over the legacy field, including an explicitly empty body, map, URI or query string.
            "uri": {"value": "https://example.com/webhook"}
        }
    };
}

function _buildRecurringChargeRequest(): types.IRecurringPaymentServiceChargeRequest {
    return {
        "connectorRecurringPaymentId": {  // Reference to existing mandate.
        },
        "amount": {  // Amount Information.
            "minorAmount": 1000,
            "currency": Currency.USD
        },
        "paymentMethod": {  // Optional payment Method Information (for network transaction flows).
            "token": {  // Payment tokens.
                "token": {"value": "probe_pm_token"}  // The token string representing a payment method.
            }
        },
        "returnUrl": "https://example.com/recurring-return",
        "connectorCustomerId": "cust_probe_123",
        "paymentMethodType": PaymentMethodType.PAY_PAL,
        "offSession": true  // Behavioral Flags and Preferences.
    };
}

function _buildRefundRequest(connectorTransactionId: string): types.IPaymentServiceRefundRequest {
    return {
        "merchantRefundId": "probe_refund_001",  // Identification.
        "connectorTransactionId": connectorTransactionId,
        "paymentAmount": 1000,  // Amount Information.
        "refundAmount": {
            "minorAmount": 1000,
            "currency": Currency.USD
        },
        "reason": "customer_request"  // Reason for the refund.
    };
}

function _buildRefundGetRequest(): types.IRefundServiceGetRequest {
    return {
        "merchantRefundId": "probe_refund_001",  // Identification.
        "connectorTransactionId": "probe_connector_txn_001",
        "refundId": "probe_refund_id_001"  // Deprecated.
    };
}

function _buildVerifyRedirectRequest(): types.IPaymentServiceVerifyRedirectResponseRequest {
    return {
    };
}


// ANCHOR: scenario_functions
// Flow: PaymentService.Authorize (Ideal)
async function authorize(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    const authorizeResponse = await paymentClient.authorize(_buildAuthorizeRequest(CaptureMethod.AUTOMATIC));

    return authorizeResponse;
}

// Flow: PaymentService.Capture
async function capture(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    const captureResponse = await paymentClient.capture(_buildCaptureRequest('probe_connector_txn_001'));

    return captureResponse;
}

// Flow: PaymentService.Get
async function get(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    const getResponse = await paymentClient.get(_buildGetRequest('probe_connector_txn_001'));

    return getResponse;
}

// Flow: EventService.HandleEvent
async function handleEvent(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const eventClient = new EventClient(config);

    const handleResponse = await eventClient.handleEvent(_buildHandleEventRequest());

    return handleResponse;
}

// Flow: EventService.ParseEvent
async function parseEvent(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const eventClient = new EventClient(config);

    const parseResponse = await eventClient.parseEvent(_buildParseEventRequest());

    return parseResponse;
}

// Flow: RecurringPaymentService.Charge
async function recurringCharge(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const recurringPaymentClient = new RecurringPaymentClient(config);

    const recurringResponse = await recurringPaymentClient.charge(_buildRecurringChargeRequest());

    return recurringResponse;
}

// Flow: PaymentService.Refund
async function refund(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    const refundResponse = await paymentClient.refund(_buildRefundRequest('probe_connector_txn_001'));

    return refundResponse;
}

// Flow: RefundService.Get
async function refundGet(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const refundClient = new RefundClient(config);

    const refundResponse = await refundClient.refundGet(_buildRefundGetRequest());

    return refundResponse;
}

// Flow: PaymentService.VerifyRedirectResponse
async function verifyRedirect(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    const verifyResponse = await paymentClient.verifyRedirectResponse(_buildVerifyRedirectRequest());

    return verifyResponse;
}


// Export all process* functions for the smoke test
export {
    authorize, capture, get, handleEvent, parseEvent, recurringCharge, refund, refundGet, verifyRedirect, _buildAuthorizeRequest, _buildCaptureRequest, _buildGetRequest, _buildHandleEventRequest, _buildParseEventRequest, _buildRecurringChargeRequest, _buildRefundRequest, _buildRefundGetRequest, _buildVerifyRedirectRequest
};

// CLI runner
if (require.main === module) {
    const scenario = process.argv[2] || 'checkout_autocapture';
    const key = 'process' + scenario.replace(/_([a-z])/g, (_, l) => l.toUpperCase()).replace(/^(.)/, c => c.toUpperCase());
    const fn = (globalThis as any)[key] || (exports as any)[key];
    if (!fn) {
        const available = Object.keys(exports).map(k =>
            k.replace(/^process/, '').replace(/([A-Z])/g, '_$1').toLowerCase().replace(/^_/, '')
        );
        console.error(`Unknown scenario: ${scenario}. Available: ${available.join(', ')}`);
        process.exit(1);
    }
    fn('order_001').catch(console.error);
}
