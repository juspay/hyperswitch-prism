// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py imerchantsolutions
//
// Imerchantsolutions — all integration scenarios and flows in one file.
// Run a scenario:  npx tsx imerchantsolutions.ts checkout_autocapture

import { PaymentClient, EventClient, RefundClient, types } from 'hyperswitch-prism';
const { Environment, AuthenticationType, CaptureMethod, CardNetwork, Currency, HttpMethod } = types;
export const SUPPORTED_FLOWS = ["authorize", "capture", "get", "parse_event", "proxy_authorize", "refund", "refund_get", "void"];

const _defaultConfig: types.IConnectorConfig = {
    options: {
        environment: Environment.SANDBOX,
    },
    connectorConfig: {
        imerchantsolutions: {
            apiKey: { value: 'YOUR_API_KEY' },
            merchantId: { value: 'YOUR_MERCHANT_ID' },
            baseUrl: 'YOUR_BASE_URL',
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
            "card": {  // Generic card payment.
                "cardNumber": {"value": "4111111111111111"},  // Card Identification.
                "cardExpMonth": {"value": "03"},
                "cardExpYear": {"value": "2030"},
                "cardCvc": {"value": "737"},
                "cardHolderName": {"value": "John Doe"}  // Cardholder Information.
            }
        },
        "captureMethod": captureMethod,  // Method for capturing the payment.
        "address": {  // Address Information.
            "billingAddress": {
            }
        },
        "authType": AuthenticationType.NO_THREE_DS,  // Authentication Details.
        "returnUrl": "https://example.com/return"  // URLs for Redirection and Webhooks.
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
            "body": {"value": new Uint8Array([123, 34, 116, 121, 112, 101, 34, 58, 32, 34, 112, 97, 121, 109, 101, 110, 116, 46, 99, 111, 109, 112, 108, 101, 116, 101, 100, 34, 44, 34, 112, 97, 121, 109, 101, 110, 116, 73, 100, 34, 58, 32, 34, 99, 109, 109, 108, 49, 50, 51, 52, 97, 98, 99, 100, 34, 44, 34, 112, 115, 112, 82, 101, 102, 101, 114, 101, 110, 99, 101, 34, 58, 32, 34, 65, 66, 67, 49, 50, 51, 68, 69, 70, 52, 53, 54, 34, 44, 34, 114, 101, 102, 101, 114, 101, 110, 99, 101, 34, 58, 32, 34, 111, 114, 100, 101, 114, 45, 49, 50, 51, 52, 53, 34, 44, 34, 97, 109, 111, 117, 110, 116, 34, 58, 32, 53, 48, 48, 48, 44, 34, 99, 117, 114, 114, 101, 110, 99, 121, 34, 58, 32, 34, 85, 83, 68, 34, 44, 34, 115, 116, 97, 116, 117, 115, 34, 58, 32, 34, 99, 97, 112, 116, 117, 114, 101, 100, 34, 44, 34, 112, 114, 111, 99, 101, 115, 115, 111, 114, 34, 58, 32, 34, 65, 100, 121, 101, 110, 34, 44, 34, 99, 97, 114, 100, 76, 97, 115, 116, 52, 34, 58, 32, 34, 49, 49, 49, 49, 34, 44, 34, 99, 97, 114, 100, 66, 114, 97, 110, 100, 34, 58, 32, 34, 118, 105, 115, 97, 34, 44, 34, 99, 117, 115, 116, 111, 109, 101, 114, 69, 109, 97, 105, 108, 34, 58, 32, 34, 99, 117, 115, 116, 111, 109, 101, 114, 64, 101, 120, 97, 109, 112, 108, 101, 46, 99, 111, 109, 34, 44, 34, 112, 97, 114, 116, 110, 101, 114, 73, 100, 34, 58, 32, 34, 121, 111, 117, 114, 95, 112, 97, 114, 116, 110, 101, 114, 95, 105, 100, 34, 44, 34, 109, 101, 114, 99, 104, 97, 110, 116, 73, 100, 34, 58, 32, 34, 109, 101, 114, 99, 104, 97, 110, 116, 95, 105, 100, 34, 44, 34, 116, 105, 109, 101, 115, 116, 97, 109, 112, 34, 58, 32, 34, 50, 48, 50, 54, 45, 48, 51, 45, 51, 48, 84, 49, 53, 58, 52, 53, 58, 48, 48, 46, 48, 48, 48, 90, 34, 125, 125, 125])},  // Secret-typed replacements. Presence takes precedence over the legacy field, including an explicitly empty body, map, URI or query string.
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
            "body": {"value": new Uint8Array([123, 34, 116, 121, 112, 101, 34, 58, 32, 34, 112, 97, 121, 109, 101, 110, 116, 46, 99, 111, 109, 112, 108, 101, 116, 101, 100, 34, 44, 34, 112, 97, 121, 109, 101, 110, 116, 73, 100, 34, 58, 32, 34, 99, 109, 109, 108, 49, 50, 51, 52, 97, 98, 99, 100, 34, 44, 34, 112, 115, 112, 82, 101, 102, 101, 114, 101, 110, 99, 101, 34, 58, 32, 34, 65, 66, 67, 49, 50, 51, 68, 69, 70, 52, 53, 54, 34, 44, 34, 114, 101, 102, 101, 114, 101, 110, 99, 101, 34, 58, 32, 34, 111, 114, 100, 101, 114, 45, 49, 50, 51, 52, 53, 34, 44, 34, 97, 109, 111, 117, 110, 116, 34, 58, 32, 53, 48, 48, 48, 44, 34, 99, 117, 114, 114, 101, 110, 99, 121, 34, 58, 32, 34, 85, 83, 68, 34, 44, 34, 115, 116, 97, 116, 117, 115, 34, 58, 32, 34, 99, 97, 112, 116, 117, 114, 101, 100, 34, 44, 34, 112, 114, 111, 99, 101, 115, 115, 111, 114, 34, 58, 32, 34, 65, 100, 121, 101, 110, 34, 44, 34, 99, 97, 114, 100, 76, 97, 115, 116, 52, 34, 58, 32, 34, 49, 49, 49, 49, 34, 44, 34, 99, 97, 114, 100, 66, 114, 97, 110, 100, 34, 58, 32, 34, 118, 105, 115, 97, 34, 44, 34, 99, 117, 115, 116, 111, 109, 101, 114, 69, 109, 97, 105, 108, 34, 58, 32, 34, 99, 117, 115, 116, 111, 109, 101, 114, 64, 101, 120, 97, 109, 112, 108, 101, 46, 99, 111, 109, 34, 44, 34, 112, 97, 114, 116, 110, 101, 114, 73, 100, 34, 58, 32, 34, 121, 111, 117, 114, 95, 112, 97, 114, 116, 110, 101, 114, 95, 105, 100, 34, 44, 34, 109, 101, 114, 99, 104, 97, 110, 116, 73, 100, 34, 58, 32, 34, 109, 101, 114, 99, 104, 97, 110, 116, 95, 105, 100, 34, 44, 34, 116, 105, 109, 101, 115, 116, 97, 109, 112, 34, 58, 32, 34, 50, 48, 50, 54, 45, 48, 51, 45, 51, 48, 84, 49, 53, 58, 52, 53, 58, 48, 48, 46, 48, 48, 48, 90, 34, 125, 125, 125])},  // Secret-typed replacements. Presence takes precedence over the legacy field, including an explicitly empty body, map, URI or query string.
            "uri": {"value": "https://example.com/webhook"}
        }
    };
}

function _buildProxyAuthorizeRequest(): types.IPaymentServiceProxyAuthorizeRequest {
    return {
        "merchantTransactionId": "probe_proxy_txn_001",
        "amount": {
            "minorAmount": 1000,
            "currency": Currency.USD
        },
        "cardProxy": {  // Card proxy for vault-aliased payments (VGS, Basis Theory, Spreedly). Real card values are substituted by the proxy before reaching the connector.
            "cardNumber": {"value": "4111111111111111"},  // Card Identification.
            "cardExpMonth": {"value": "03"},
            "cardExpYear": {"value": "2030"},
            "cardCvc": {"value": "123"},
            "cardHolderName": {"value": "John Doe"},  // Cardholder Information.
            "cardNetwork": CardNetwork.VISA
        },
        "address": {
            "billingAddress": {
            }
        },
        "captureMethod": CaptureMethod.AUTOMATIC,
        "authType": AuthenticationType.NO_THREE_DS,
        "returnUrl": "https://example.com/return"
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

function _buildVoidRequest(connectorTransactionId: string): types.IPaymentServiceVoidRequest {
    return {
        "merchantVoidId": "probe_void_001",  // Identification.
        "connectorTransactionId": connectorTransactionId
    };
}


// ANCHOR: scenario_functions
// One-step Payment (Authorize + Capture)
// Simple payment that authorizes and captures in one call. Use for immediate charges.
async function processCheckoutAutocapture(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    // Step 1: Authorize — reserve funds on the payment method
    const authorizeResponse = await paymentClient.authorize(_buildAuthorizeRequest(CaptureMethod.AUTOMATIC));

    if (authorizeResponse.status === types.PaymentStatus.FAILURE) {
        throw new Error(`Payment failed: ${JSON.stringify(authorizeResponse.error)}`);
    }
    if (authorizeResponse.status === types.PaymentStatus.PENDING) {
        // Awaiting async confirmation — handle via webhook
        return { status: 'pending', connectorTransactionId: authorizeResponse.connectorTransactionId };
    }

    return { status: authorizeResponse.status, transactionId: authorizeResponse.connectorTransactionId!, error: authorizeResponse.error } as any;
}

// Card Payment (Authorize + Capture)
// Two-step card payment. First authorize, then capture. Use when you need to verify funds before finalizing.
async function processCheckoutCard(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    // Step 1: Authorize — reserve funds on the payment method
    const authorizeResponse = await paymentClient.authorize(_buildAuthorizeRequest(CaptureMethod.MANUAL));

    if (authorizeResponse.status === types.PaymentStatus.FAILURE) {
        throw new Error(`Payment failed: ${JSON.stringify(authorizeResponse.error)}`);
    }
    if (authorizeResponse.status === types.PaymentStatus.PENDING) {
        // Awaiting async confirmation — handle via webhook
        return { status: 'pending', connectorTransactionId: authorizeResponse.connectorTransactionId };
    }

    // Step 2: Capture — settle the reserved funds
    const captureResponse = await paymentClient.capture(_buildCaptureRequest(authorizeResponse.connectorTransactionId!));

    if (captureResponse.status === types.PaymentStatus.FAILURE) {
        throw new Error(`Capture failed: ${JSON.stringify(captureResponse.error)}`);
    }

    return { status: captureResponse.status, transactionId: authorizeResponse.connectorTransactionId!, error: authorizeResponse.error } as any;
}

// Refund
// Return funds to the customer for a completed payment.
async function processRefund(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    // Step 1: Authorize — reserve funds on the payment method
    const authorizeResponse = await paymentClient.authorize(_buildAuthorizeRequest(CaptureMethod.AUTOMATIC));

    if (authorizeResponse.status === types.PaymentStatus.FAILURE) {
        throw new Error(`Payment failed: ${JSON.stringify(authorizeResponse.error)}`);
    }
    if (authorizeResponse.status === types.PaymentStatus.PENDING) {
        // Awaiting async confirmation — handle via webhook
        return { status: 'pending', connectorTransactionId: authorizeResponse.connectorTransactionId };
    }

    // Step 2: Refund — return funds to the customer
    const refundResponse = await paymentClient.refund(_buildRefundRequest(authorizeResponse.connectorTransactionId!));

    if (refundResponse.status === types.RefundStatus.REFUND_FAILURE) {
        throw new Error(`Refund failed: ${JSON.stringify(refundResponse.error)}`);
    }

    return { status: refundResponse.status, error: refundResponse.error } as any;
}

// Void Payment
// Cancel an authorized but not-yet-captured payment.
async function processVoidPayment(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    // Step 1: Authorize — reserve funds on the payment method
    const authorizeResponse = await paymentClient.authorize(_buildAuthorizeRequest(CaptureMethod.MANUAL));

    if (authorizeResponse.status === types.PaymentStatus.FAILURE) {
        throw new Error(`Payment failed: ${JSON.stringify(authorizeResponse.error)}`);
    }
    if (authorizeResponse.status === types.PaymentStatus.PENDING) {
        // Awaiting async confirmation — handle via webhook
        return { status: 'pending', connectorTransactionId: authorizeResponse.connectorTransactionId };
    }

    // Step 2: Void — release reserved funds (cancel authorization)
    const voidResponse = await paymentClient.void(_buildVoidRequest(authorizeResponse.connectorTransactionId!));

    return { status: voidResponse.status, transactionId: authorizeResponse.connectorTransactionId!, error: voidResponse.error } as any;
}

// Get Payment Status
// Retrieve current payment status from the connector.
async function processGetPayment(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    // Step 1: Authorize — reserve funds on the payment method
    const authorizeResponse = await paymentClient.authorize(_buildAuthorizeRequest(CaptureMethod.MANUAL));

    if (authorizeResponse.status === types.PaymentStatus.FAILURE) {
        throw new Error(`Payment failed: ${JSON.stringify(authorizeResponse.error)}`);
    }
    if (authorizeResponse.status === types.PaymentStatus.PENDING) {
        // Awaiting async confirmation — handle via webhook
        return { status: 'pending', connectorTransactionId: authorizeResponse.connectorTransactionId };
    }

    // Step 2: Get — retrieve current payment status from the connector
    const getResponse = await paymentClient.get(_buildGetRequest(authorizeResponse.connectorTransactionId!));

    return { status: getResponse.status, transactionId: getResponse.connectorTransactionId!, error: getResponse.error } as any;
}

// Flow: PaymentService.Authorize (Card)
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

// Flow: PaymentService.ProxyAuthorize
async function proxyAuthorize(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    const proxyResponse = await paymentClient.proxyAuthorize(_buildProxyAuthorizeRequest());

    return proxyResponse;
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

// Flow: PaymentService.Void
async function voidPayment(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    const voidResponse = await paymentClient.void(_buildVoidRequest('probe_connector_txn_001'));

    return voidResponse;
}


// Export all process* functions for the smoke test
export {
    processCheckoutAutocapture, processCheckoutCard, processRefund, processVoidPayment, processGetPayment, authorize, capture, get, handleEvent, parseEvent, proxyAuthorize, refund, refundGet, voidPayment, _buildAuthorizeRequest, _buildCaptureRequest, _buildGetRequest, _buildHandleEventRequest, _buildParseEventRequest, _buildProxyAuthorizeRequest, _buildRefundRequest, _buildRefundGetRequest, _buildVoidRequest
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
