// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py bamboraapac
//
// Bamboraapac — all integration scenarios and flows in one file.
// Run a scenario:  node bamboraapac.js checkout_card
'use strict';

const { PaymentClient, RecurringPaymentClient } = require('hyperswitch-prism');
const { ConnectorConfig, ConnectorSpecificConfig, SdkOptions, Environment } = require('hyperswitch-prism').types;

const _defaultConfig = ConnectorConfig.create({
    options: SdkOptions.create({ environment: Environment.SANDBOX }),
});
// Standalone credentials (field names depend on connector auth type):
// _defaultConfig.connectorConfig = ConnectorSpecificConfig.create({
//     bamboraapac: { apiKey: { value: 'YOUR_API_KEY' } }
// });


function _buildAuthorizeRequest(captureMethod) {
    return {
        "merchantTransactionId": "probe_txn_001",  // Identification
        "amount": {  // The amount for the payment
            "minorAmount": 1000,  // Amount in minor units (e.g., 1000 = $10.00)
            "currency": "USD"  // ISO 4217 currency code (e.g., "USD", "EUR")
        },
        "paymentMethod": {  // Payment method to be used
            "card": {  // Generic card payment
                "cardNumber": {"value": "4111111111111111"},  // Card Identification
                "cardExpMonth": {"value": "03"},
                "cardExpYear": {"value": "2030"},
                "cardCvc": {"value": "737"},
                "cardHolderName": {"value": "John Doe"}  // Cardholder Information
            }
        },
        "captureMethod": captureMethod,  // Method for capturing the payment
        "address": {  // Address Information
            "billingAddress": {
            }
        },
        "authType": "NO_THREE_DS",  // Authentication Details
        "returnUrl": "https://example.com/return"  // URLs for Redirection and Webhooks
    };
}

function _buildCaptureRequest(connectorTransactionId) {
    return {
        "merchantCaptureId": "probe_capture_001",  // Identification
        "connectorTransactionId": connectorTransactionId,
        "amountToCapture": {  // Capture Details
            "minorAmount": 1000,  // Amount in minor units (e.g., 1000 = $10.00)
            "currency": "USD"  // ISO 4217 currency code (e.g., "USD", "EUR")
        }
    };
}

function _buildGetRequest(connectorTransactionId) {
    return {
        "merchantTransactionId": "probe_merchant_txn_001",  // Identification
        "connectorTransactionId": connectorTransactionId,
        "amount": {  // Amount Information
            "minorAmount": 1000,  // Amount in minor units (e.g., 1000 = $10.00)
            "currency": "USD"  // ISO 4217 currency code (e.g., "USD", "EUR")
        }
    };
}

// Card Payment (Authorize + Capture)
// Reserve funds with Authorize, then settle with a separate Capture call. Use for physical goods or delayed fulfillment where capture happens later.
async function processCheckoutCard(merchantTransactionId, config = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    // Step 1: Authorize — reserve funds on the payment method
    const authorizeResponse = await paymentClient.authorize(_buildAuthorizeRequest('MANUAL'));

    if (authorizeResponse.status === 'FAILED') {
        throw new Error(`Payment failed: ${authorizeResponse.error?.message}`);
    }
    if (authorizeResponse.status === 'PENDING') {
        // Awaiting async confirmation — handle via webhook
        return { status: 'pending', transactionId: authorizeResponse.connectorTransactionId };
    }

    // Step 2: Capture — settle the reserved funds
    const captureResponse = await paymentClient.capture(_buildCaptureRequest(authorizeResponse.connectorTransactionId));

    if (captureResponse.status === 'FAILED') {
        throw new Error(`Capture failed: ${captureResponse.error?.message}`);
    }

    return { status: captureResponse.status, transactionId: authorizeResponse.connectorTransactionId, error: authorizeResponse.error };
}

// Card Payment (Automatic Capture)
// Authorize and capture in one call using `capture_method=AUTOMATIC`. Use for digital goods or immediate fulfillment.
async function processCheckoutAutocapture(merchantTransactionId, config = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    // Step 1: Authorize — reserve funds on the payment method
    const authorizeResponse = await paymentClient.authorize(_buildAuthorizeRequest('AUTOMATIC'));

    if (authorizeResponse.status === 'FAILED') {
        throw new Error(`Payment failed: ${authorizeResponse.error?.message}`);
    }
    if (authorizeResponse.status === 'PENDING') {
        // Awaiting async confirmation — handle via webhook
        return { status: 'pending', transactionId: authorizeResponse.connectorTransactionId };
    }

    return { status: authorizeResponse.status, transactionId: authorizeResponse.connectorTransactionId, error: authorizeResponse.error };
}

// Refund a Payment
// Authorize with automatic capture, then refund the captured amount. `connector_transaction_id` from the Authorize response is reused for the Refund call.
async function processRefund(merchantTransactionId, config = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    // Step 1: Authorize — reserve funds on the payment method
    const authorizeResponse = await paymentClient.authorize(_buildAuthorizeRequest('AUTOMATIC'));

    if (authorizeResponse.status === 'FAILED') {
        throw new Error(`Payment failed: ${authorizeResponse.error?.message}`);
    }
    if (authorizeResponse.status === 'PENDING') {
        // Awaiting async confirmation — handle via webhook
        return { status: 'pending', transactionId: authorizeResponse.connectorTransactionId };
    }

    // Step 2: Refund — return funds to the customer
    const refundResponse = await paymentClient.refund({
        "merchantRefundId": "probe_refund_001",  // Identification
        "connectorTransactionId": authorizeResponse.connectorTransactionId,  // from authorize response
        "paymentAmount": 1000,  // Amount Information
        "refundAmount": {
            "minorAmount": 1000,  // Amount in minor units (e.g., 1000 = $10.00)
            "currency": "USD"  // ISO 4217 currency code (e.g., "USD", "EUR")
        },
        "reason": "customer_request"  // Reason for the refund
    });

    if (refundResponse.status === 'FAILED') {
        throw new Error(`Refund failed: ${refundResponse.error?.message}`);
    }

    return { status: refundResponse.status, error: refundResponse.error };
}

// Recurring / Mandate Payments
// Store a payment mandate with SetupRecurring, then charge it repeatedly with RecurringPaymentService.Charge without requiring customer action.
async function processRecurring(merchantTransactionId, config = _defaultConfig) {
    const paymentClient = new PaymentClient(config);
    const recurringPaymentClient = new RecurringPaymentClient(config);

    // Step 1: Setup Recurring — store the payment mandate
    const setupResponse = await paymentClient.setupRecurring({
        "merchantRecurringPaymentId": "probe_mandate_001",  // Identification
        "amount": {  // Mandate Details
            "minorAmount": 0,  // Amount in minor units (e.g., 1000 = $10.00)
            "currency": "USD"  // ISO 4217 currency code (e.g., "USD", "EUR")
        },
        "paymentMethod": {
            "card": {  // Generic card payment
                "cardNumber": {"value": "4111111111111111"},  // Card Identification
                "cardExpMonth": {"value": "03"},
                "cardExpYear": {"value": "2030"},
                "cardCvc": {"value": "737"},
                "cardHolderName": {"value": "John Doe"}  // Cardholder Information
            }
        },
        "address": {  // Address Information
            "billingAddress": {
            }
        },
        "authType": "NO_THREE_DS",  // Type of authentication to be used
        "enrolledFor3Ds": false,  // Indicates if the customer is enrolled for 3D Secure
        "returnUrl": "https://example.com/mandate-return",  // URL to redirect after setup
        "setupFutureUsage": "OFF_SESSION",  // Indicates future usage intention
        "requestIncrementalAuthorization": false,  // Indicates if incremental authorization is requested
        "customerAcceptance": {  // Details of customer acceptance
            "acceptanceType": "OFFLINE",  // Type of acceptance (e.g., online, offline).
            "acceptedAt": 0  // Timestamp when the acceptance was made (Unix timestamp, seconds since epoch).
        }
    });

    if (setupResponse.status === 'FAILED') {
        throw new Error(`Recurring setup failed: ${setupResponse.error?.message}`);
    }

    // Step 2: Recurring Charge — charge against the stored mandate
    const recurringResponse = await recurringPaymentClient.charge({
        "connectorRecurringPaymentId": { connectorMandateId: { connectorMandateId: setupResponse.mandateReference?.connectorMandateId?.connectorMandateId } },  // from setup response
        "amount": {  // Amount Information
            "minorAmount": 1000,  // Amount in minor units (e.g., 1000 = $10.00)
            "currency": "USD"  // ISO 4217 currency code (e.g., "USD", "EUR")
        },
        "returnUrl": "https://example.com/recurring-return",
        "connectorCustomerId": "cust_probe_123",
        "offSession": true  // Behavioral Flags and Preferences
    });

    if (recurringResponse.status === 'FAILED') {
        throw new Error(`Recurring_Charge failed: ${recurringResponse.error?.message}`);
    }

    return { status: recurringResponse.status, transactionId: recurringResponse.connectorTransactionId ?? '', error: recurringResponse.error };
}

// Get Payment Status
// Authorize a payment, then poll the connector for its current status using Get. Use this to sync payment state when webhooks are unavailable or delayed.
async function processGetPayment(merchantTransactionId, config = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    // Step 1: Authorize — reserve funds on the payment method
    const authorizeResponse = await paymentClient.authorize(_buildAuthorizeRequest('MANUAL'));

    if (authorizeResponse.status === 'FAILED') {
        throw new Error(`Payment failed: ${authorizeResponse.error?.message}`);
    }
    if (authorizeResponse.status === 'PENDING') {
        // Awaiting async confirmation — handle via webhook
        return { status: 'pending', transactionId: authorizeResponse.connectorTransactionId };
    }

    // Step 2: Get — retrieve current payment status from the connector
    const getResponse = await paymentClient.get(_buildGetRequest(authorizeResponse.connectorTransactionId));

    return { status: getResponse.status, transactionId: getResponse.connectorTransactionId, error: getResponse.error };
}

// Flow: PaymentService.Authorize (Card)
async function authorize(merchantTransactionId, config = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    const authorizeResponse = await paymentClient.authorize(_buildAuthorizeRequest('AUTOMATIC'));

    return { status: authorizeResponse.status, transactionId: authorizeResponse.connectorTransactionId };
}

// Flow: PaymentService.Capture
async function capture(merchantTransactionId, config = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    const captureResponse = await paymentClient.capture(_buildCaptureRequest('probe_connector_txn_001'));

    return { status: captureResponse.status };
}

// Flow: PaymentService.Get
async function get(merchantTransactionId, config = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    const getResponse = await paymentClient.get(_buildGetRequest('probe_connector_txn_001'));

    return { status: getResponse.status };
}

// Flow: RecurringPaymentService.Charge
async function recurringCharge(merchantTransactionId, config = _defaultConfig) {
    // Step 1: Recurring Charge — charge against the stored mandate
    const recurringResponse = await recurringPaymentClient.charge({
        "connectorRecurringPaymentId": {  // Reference to existing mandate
            "mandateIdType": {
                "connectorMandateId": "probe-mandate-123"
            }
        },
        "amount": {  // Amount Information
            "minorAmount": 1000,  // Amount in minor units (e.g., 1000 = $10.00)
            "currency": "USD"  // ISO 4217 currency code (e.g., "USD", "EUR")
        },
        "paymentMethod": {  // Optional payment Method Information (for network transaction flows)
            "token": {"token": {"value": "probe_pm_token"}}  // Payment tokens
        },
        "returnUrl": "https://example.com/recurring-return",
        "connectorCustomerId": "cust_probe_123",
        "paymentMethodType": "PAY_PAL",
        "offSession": true  // Behavioral Flags and Preferences
    });

    if (recurringResponse.status === 'FAILED') {
        throw new Error(`Recurring_Charge failed: ${recurringResponse.error?.message}`);
    }

    return { status: recurringResponse.status };
}

// Flow: PaymentService.SetupRecurring
async function setupRecurring(merchantTransactionId, config = _defaultConfig) {
    // Step 1: Setup Recurring — store the payment mandate
    const setupResponse = await paymentClient.setupRecurring({
        "merchantRecurringPaymentId": "probe_mandate_001",  // Identification
        "amount": {  // Mandate Details
            "minorAmount": 0,  // Amount in minor units (e.g., 1000 = $10.00)
            "currency": "USD"  // ISO 4217 currency code (e.g., "USD", "EUR")
        },
        "paymentMethod": {
            "card": {  // Generic card payment
                "cardNumber": {"value": "4111111111111111"},  // Card Identification
                "cardExpMonth": {"value": "03"},
                "cardExpYear": {"value": "2030"},
                "cardCvc": {"value": "737"},
                "cardHolderName": {"value": "John Doe"}  // Cardholder Information
            }
        },
        "address": {  // Address Information
            "billingAddress": {
            }
        },
        "authType": "NO_THREE_DS",  // Type of authentication to be used
        "enrolledFor3Ds": false,  // Indicates if the customer is enrolled for 3D Secure
        "returnUrl": "https://example.com/mandate-return",  // URL to redirect after setup
        "setupFutureUsage": "OFF_SESSION",  // Indicates future usage intention
        "requestIncrementalAuthorization": false,  // Indicates if incremental authorization is requested
        "customerAcceptance": {  // Details of customer acceptance
            "acceptanceType": "OFFLINE",  // Type of acceptance (e.g., online, offline).
            "acceptedAt": 0  // Timestamp when the acceptance was made (Unix timestamp, seconds since epoch).
        }
    });

    if (setupResponse.status === 'FAILED') {
        throw new Error(`Recurring setup failed: ${setupResponse.error?.message}`);
    }

    return { status: setupResponse.status, mandateId: setupResponse.connectorTransactionId };
}


module.exports = { processCheckoutCard, processCheckoutAutocapture, processRefund, processRecurring, processGetPayment, authorize, capture, get, recurringCharge, setupRecurring };

if (require.main === module) {
    const scenario = process.argv[2] || 'checkout_card';
    const key = 'process' + scenario.replace(/_([a-z])/g, (_, l) => l.toUpperCase()).replace(/^(.)/, c => c.toUpperCase());
    const fn = module.exports[key];
    if (!fn) {
        const available = Object.keys(module.exports).map(k =>
            k.replace(/^process/, '').replace(/([A-Z])/g, '_$1').toLowerCase().replace(/^_/, '')
        );
        console.error(`Unknown scenario: ${scenario}. Available: ${available.join(', ')}`);
        process.exit(1);
    }
    fn('order_001').catch(console.error);
}
