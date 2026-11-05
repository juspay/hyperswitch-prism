// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py nexixpay
//
// Nexixpay — all integration scenarios and flows in one file.
// Run a scenario:  npx tsx nexixpay.ts checkout_autocapture

import { PaymentClient, EventClient, RefundClient, types } from 'hyperswitch-prism';
const { Environment, AuthenticationType, CaptureMethod, Currency, FutureUsage, HttpMethod } = types;
export const SUPPORTED_FLOWS = ["authorize", "capture", "get", "parse_event", "refund", "refund_get", "void"];

const _defaultConfig: types.IConnectorConfig = {
    options: {
        environment: Environment.SANDBOX,
    },
    connectorConfig: {
        nexixpay: {
            apiKey: { value: 'YOUR_API_KEY' },
            baseUrl: 'YOUR_BASE_URL',
        }
    },
};


function _buildAuthorizeRequest(captureMethod: types.CaptureMethod): types.IPaymentServiceAuthorizeRequest {
    return {
        "merchantTransactionId": "probe_txn_001",  // Identification.
        "amount": {  // The amount for the payment.
            "minorAmount": 0,
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
                "firstName": {"value": "John"},
                "lastName": {"value": "Doe"}
            }
        },
        "authType": AuthenticationType.THREE_DS,  // Authentication Details.
        "returnUrl": "https://example.com/return",  // URLs for Redirection and Webhooks.
        "setupFutureUsage": FutureUsage.OFF_SESSION  // Behavioral Flags and Preferences.
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
            "body": {"value": new Uint8Array([123, 34, 101, 118, 101, 110, 116, 73, 100, 34, 58, 34, 53, 53, 52, 99, 99, 99, 48, 48, 45, 50, 56, 102, 98, 45, 52, 51, 52, 52, 45, 97, 51, 102, 97, 45, 52, 98, 98, 56, 100, 49, 57, 57, 57, 98, 100, 53, 34, 44, 34, 101, 118, 101, 110, 116, 84, 105, 109, 101, 34, 58, 34, 50, 48, 50, 50, 45, 48, 57, 45, 48, 49, 84, 48, 49, 58, 50, 48, 58, 48, 48, 46, 48, 48, 49, 90, 34, 44, 34, 115, 101, 99, 117, 114, 105, 116, 121, 84, 111, 107, 101, 110, 34, 58, 34, 50, 102, 48, 101, 97, 53, 48, 53, 57, 98, 52, 49, 52, 49, 52, 99, 97, 51, 55, 52, 52, 102, 101, 54, 55, 50, 51, 50, 55, 100, 56, 53, 34, 44, 34, 111, 112, 101, 114, 97, 116, 105, 111, 110, 34, 58, 123, 34, 111, 114, 100, 101, 114, 73, 100, 34, 58, 34, 98, 116, 105, 100, 50, 51, 56, 52, 57, 56, 51, 34, 44, 34, 111, 112, 101, 114, 97, 116, 105, 111, 110, 73, 100, 34, 58, 34, 51, 52, 55, 48, 55, 52, 52, 34, 44, 34, 99, 104, 97, 110, 110, 101, 108, 34, 58, 34, 69, 67, 79, 77, 77, 69, 82, 67, 69, 34, 44, 34, 111, 112, 101, 114, 97, 116, 105, 111, 110, 84, 121, 112, 101, 34, 58, 34, 67, 65, 80, 84, 85, 82, 69, 34, 44, 34, 111, 112, 101, 114, 97, 116, 105, 111, 110, 82, 101, 115, 117, 108, 116, 34, 58, 34, 69, 88, 69, 67, 85, 84, 69, 68, 34, 44, 34, 111, 112, 101, 114, 97, 116, 105, 111, 110, 84, 105, 109, 101, 34, 58, 34, 50, 48, 50, 50, 45, 48, 57, 45, 48, 49, 84, 48, 49, 58, 50, 48, 58, 48, 48, 46, 48, 48, 49, 90, 34, 44, 34, 112, 97, 121, 109, 101, 110, 116, 77, 101, 116, 104, 111, 100, 34, 58, 34, 67, 65, 82, 68, 34, 44, 34, 112, 97, 121, 109, 101, 110, 116, 67, 105, 114, 99, 117, 105, 116, 34, 58, 34, 86, 73, 83, 65, 34, 44, 34, 112, 97, 121, 109, 101, 110, 116, 73, 110, 115, 116, 114, 117, 109, 101, 110, 116, 73, 110, 102, 111, 34, 58, 34, 42, 42, 42, 54, 49, 53, 50, 34, 44, 34, 112, 97, 121, 109, 101, 110, 116, 69, 110, 100, 84, 111, 69, 110, 100, 73, 100, 34, 58, 34, 101, 55, 50, 51, 104, 101, 100, 115, 100, 101, 119, 34, 44, 34, 99, 97, 110, 99, 101, 108, 108, 101, 100, 79, 112, 101, 114, 97, 116, 105, 111, 110, 73, 100, 34, 58, 34, 34, 44, 34, 111, 112, 101, 114, 97, 116, 105, 111, 110, 65, 109, 111, 117, 110, 116, 34, 58, 34, 51, 53, 52, 53, 34, 44, 34, 111, 112, 101, 114, 97, 116, 105, 111, 110, 67, 117, 114, 114, 101, 110, 99, 121, 34, 58, 34, 69, 85, 82, 34, 44, 34, 97, 100, 100, 105, 116, 105, 111, 110, 97, 108, 68, 97, 116, 97, 34, 58, 123, 34, 97, 117, 116, 104, 111, 114, 105, 122, 97, 116, 105, 111, 110, 67, 111, 100, 101, 34, 58, 34, 54, 52, 55, 49, 56, 57, 34, 44, 34, 99, 97, 114, 100, 67, 111, 117, 110, 116, 114, 121, 34, 58, 34, 73, 84, 65, 34, 44, 34, 116, 104, 114, 101, 101, 68, 83, 34, 58, 34, 70, 85, 76, 76, 95, 83, 69, 67, 85, 82, 69, 34, 44, 34, 115, 99, 104, 101, 109, 97, 84, 73, 68, 34, 58, 34, 77, 67, 83, 48, 49, 49, 57, 56, 85, 34, 125, 125, 125])},  // Secret-typed replacements. Presence takes precedence over the legacy field, including an explicitly empty body, map, URI or query string.
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
            "body": {"value": new Uint8Array([123, 34, 101, 118, 101, 110, 116, 73, 100, 34, 58, 34, 53, 53, 52, 99, 99, 99, 48, 48, 45, 50, 56, 102, 98, 45, 52, 51, 52, 52, 45, 97, 51, 102, 97, 45, 52, 98, 98, 56, 100, 49, 57, 57, 57, 98, 100, 53, 34, 44, 34, 101, 118, 101, 110, 116, 84, 105, 109, 101, 34, 58, 34, 50, 48, 50, 50, 45, 48, 57, 45, 48, 49, 84, 48, 49, 58, 50, 48, 58, 48, 48, 46, 48, 48, 49, 90, 34, 44, 34, 115, 101, 99, 117, 114, 105, 116, 121, 84, 111, 107, 101, 110, 34, 58, 34, 50, 102, 48, 101, 97, 53, 48, 53, 57, 98, 52, 49, 52, 49, 52, 99, 97, 51, 55, 52, 52, 102, 101, 54, 55, 50, 51, 50, 55, 100, 56, 53, 34, 44, 34, 111, 112, 101, 114, 97, 116, 105, 111, 110, 34, 58, 123, 34, 111, 114, 100, 101, 114, 73, 100, 34, 58, 34, 98, 116, 105, 100, 50, 51, 56, 52, 57, 56, 51, 34, 44, 34, 111, 112, 101, 114, 97, 116, 105, 111, 110, 73, 100, 34, 58, 34, 51, 52, 55, 48, 55, 52, 52, 34, 44, 34, 99, 104, 97, 110, 110, 101, 108, 34, 58, 34, 69, 67, 79, 77, 77, 69, 82, 67, 69, 34, 44, 34, 111, 112, 101, 114, 97, 116, 105, 111, 110, 84, 121, 112, 101, 34, 58, 34, 67, 65, 80, 84, 85, 82, 69, 34, 44, 34, 111, 112, 101, 114, 97, 116, 105, 111, 110, 82, 101, 115, 117, 108, 116, 34, 58, 34, 69, 88, 69, 67, 85, 84, 69, 68, 34, 44, 34, 111, 112, 101, 114, 97, 116, 105, 111, 110, 84, 105, 109, 101, 34, 58, 34, 50, 48, 50, 50, 45, 48, 57, 45, 48, 49, 84, 48, 49, 58, 50, 48, 58, 48, 48, 46, 48, 48, 49, 90, 34, 44, 34, 112, 97, 121, 109, 101, 110, 116, 77, 101, 116, 104, 111, 100, 34, 58, 34, 67, 65, 82, 68, 34, 44, 34, 112, 97, 121, 109, 101, 110, 116, 67, 105, 114, 99, 117, 105, 116, 34, 58, 34, 86, 73, 83, 65, 34, 44, 34, 112, 97, 121, 109, 101, 110, 116, 73, 110, 115, 116, 114, 117, 109, 101, 110, 116, 73, 110, 102, 111, 34, 58, 34, 42, 42, 42, 54, 49, 53, 50, 34, 44, 34, 112, 97, 121, 109, 101, 110, 116, 69, 110, 100, 84, 111, 69, 110, 100, 73, 100, 34, 58, 34, 101, 55, 50, 51, 104, 101, 100, 115, 100, 101, 119, 34, 44, 34, 99, 97, 110, 99, 101, 108, 108, 101, 100, 79, 112, 101, 114, 97, 116, 105, 111, 110, 73, 100, 34, 58, 34, 34, 44, 34, 111, 112, 101, 114, 97, 116, 105, 111, 110, 65, 109, 111, 117, 110, 116, 34, 58, 34, 51, 53, 52, 53, 34, 44, 34, 111, 112, 101, 114, 97, 116, 105, 111, 110, 67, 117, 114, 114, 101, 110, 99, 121, 34, 58, 34, 69, 85, 82, 34, 44, 34, 97, 100, 100, 105, 116, 105, 111, 110, 97, 108, 68, 97, 116, 97, 34, 58, 123, 34, 97, 117, 116, 104, 111, 114, 105, 122, 97, 116, 105, 111, 110, 67, 111, 100, 101, 34, 58, 34, 54, 52, 55, 49, 56, 57, 34, 44, 34, 99, 97, 114, 100, 67, 111, 117, 110, 116, 114, 121, 34, 58, 34, 73, 84, 65, 34, 44, 34, 116, 104, 114, 101, 101, 68, 83, 34, 58, 34, 70, 85, 76, 76, 95, 83, 69, 67, 85, 82, 69, 34, 44, 34, 115, 99, 104, 101, 109, 97, 84, 73, 68, 34, 58, 34, 77, 67, 83, 48, 49, 49, 57, 56, 85, 34, 125, 125, 125])},  // Secret-typed replacements. Presence takes precedence over the legacy field, including an explicitly empty body, map, URI or query string.
            "uri": {"value": "https://example.com/webhook"}
        }
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
        "connectorTransactionId": connectorTransactionId,
        "amount": {  // Amount Information.
            "minorAmount": 1000,
            "currency": Currency.USD
        }
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
    processCheckoutAutocapture, processCheckoutCard, processRefund, processVoidPayment, processGetPayment, authorize, capture, get, handleEvent, parseEvent, refund, refundGet, voidPayment, _buildAuthorizeRequest, _buildCaptureRequest, _buildGetRequest, _buildHandleEventRequest, _buildParseEventRequest, _buildRefundRequest, _buildRefundGetRequest, _buildVoidRequest
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
