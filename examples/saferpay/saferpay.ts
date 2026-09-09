// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py saferpay
//
// Saferpay — all integration scenarios and flows in one file.
// Run a scenario:  npx tsx saferpay.ts checkout_autocapture

import { PaymentClient, PaymentMethodAuthenticationClient, RecurringPaymentClient, RefundClient, types } from 'hyperswitch-prism';
const { Environment, AcceptanceType, AuthenticationType, CaptureMethod, CardNetwork, Currency, FutureUsage, PaymentMethodType } = types;
export const SUPPORTED_FLOWS = ["capture", "get", "pre_authenticate", "proxy_setup_recurring", "recurring_charge", "refund_get", "setup_recurring", "void"];

const _defaultConfig: types.IConnectorConfig = {
    options: {
        environment: Environment.SANDBOX,
    },
    connectorConfig: {
        saferpay: {
            apiKey: { value: 'YOUR_API_KEY' },
            key1: { value: 'YOUR_KEY1' },
            apiSecret: { value: 'YOUR_API_SECRET' },
            key2: { value: 'YOUR_KEY2' },
            baseUrl: 'YOUR_BASE_URL',
        }
    },
};


function _buildCaptureRequest(connectorTransactionId: string): types.IPaymentServiceCaptureRequest {
    return {
        "merchantCaptureId": "probe_capture_001",  // Identification.
        "connectorTransactionId": connectorTransactionId,
        "amountToCapture": {  // Capture Details.
            "minorAmount": 1000,  // Amount in minor units (e.g., 1000 = $10.00).
            "currency": Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        }
    };
}

function _buildGetRequest(connectorTransactionId: string): types.IPaymentServiceGetRequest {
    return {
        "merchantTransactionId": "probe_merchant_txn_001",  // Identification.
        "connectorTransactionId": connectorTransactionId,
        "amount": {  // Amount Information.
            "minorAmount": 1000,  // Amount in minor units (e.g., 1000 = $10.00).
            "currency": Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        }
    };
}

function _buildPreAuthenticateRequest(): types.IPaymentMethodAuthenticationServicePreAuthenticateRequest {
    return {
        "amount": {  // Amount Information.
            "minorAmount": 1000,  // Amount in minor units (e.g., 1000 = $10.00).
            "currency": Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        },
        "paymentMethod": {  // Payment Method.
            "card": {  // Generic card payment.
                "cardNumber": {"value": "4111111111111111"},  // Card Identification.
                "cardExpMonth": {"value": "03"},
                "cardExpYear": {"value": "2030"},
                "cardCvc": {"value": "737"},
                "cardHolderName": {"value": "John Doe"}  // Cardholder Information.
            }
        },
        "address": {  // Address Information.
            "billingAddress": {
            }
        },
        "enrolledFor_3ds": false,  // Authentication Details.
        "returnUrl": "https://example.com/3ds-return"  // URLs for Redirection.
    };
}

function _buildProxySetupRecurringRequest(): types.IPaymentServiceProxySetupRecurringRequest {
    return {
        "merchantRecurringPaymentId": "probe_proxy_mandate_001",
        "amount": {
            "minorAmount": 0,  // Amount in minor units (e.g., 1000 = $10.00).
            "currency": Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        },
        "cardProxy": {  // Card proxy for vault-aliased payments.
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
        "customerAcceptance": {
            "acceptanceType": AcceptanceType.OFFLINE,  // Type of acceptance (e.g., online, offline).
            "acceptedAt": 0  // Timestamp when the acceptance was made (Unix timestamp, seconds since epoch).
        },
        "authType": AuthenticationType.NO_THREE_DS,
        "setupFutureUsage": FutureUsage.OFF_SESSION
    };
}

function _buildRecurringChargeRequest(): types.IRecurringPaymentServiceChargeRequest {
    return {
        "connectorRecurringPaymentId": {  // Reference to existing mandate.
        },
        "amount": {  // Amount Information.
            "minorAmount": 1000,  // Amount in minor units (e.g., 1000 = $10.00).
            "currency": Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        },
        "paymentMethod": {  // Optional payment Method Information (for network transaction flows).
            "token": {  // Payment tokens.
                "token": {"value": "probe_pm_token"}  // The token string representing a payment method.
            }
        },
        "returnUrl": "https://example.com/recurring-return",
        "captureMethod": CaptureMethod.MANUAL,  // Capture Settings.
        "connectorCustomerId": "cust_probe_123",
        "paymentMethodType": PaymentMethodType.PAY_PAL,
        "offSession": true  // Behavioral Flags and Preferences.
    };
}

function _buildRefundGetRequest(): types.IRefundServiceGetRequest {
    return {
        "merchantRefundId": "probe_refund_001",  // Identification.
        "connectorTransactionId": "probe_connector_txn_001",
        "refundId": "probe_refund_id_001"  // Deprecated.
    };
}

function _buildSetupRecurringRequest(): types.IPaymentServiceSetupRecurringRequest {
    return {
        "merchantRecurringPaymentId": "probe_mandate_001",  // Identification.
        "amount": {  // Mandate Details.
            "minorAmount": 0,  // Amount in minor units (e.g., 1000 = $10.00).
            "currency": Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        },
        "paymentMethod": {
            "card": {  // Generic card payment.
                "cardNumber": {"value": "4111111111111111"},  // Card Identification.
                "cardExpMonth": {"value": "03"},
                "cardExpYear": {"value": "2030"},
                "cardCvc": {"value": "737"},
                "cardHolderName": {"value": "John Doe"}  // Cardholder Information.
            }
        },
        "address": {  // Address Information.
            "billingAddress": {
            }
        },
        "authType": AuthenticationType.NO_THREE_DS,  // Type of authentication to be used.
        "enrolledFor_3ds": false,  // Indicates if the customer is enrolled for 3D Secure.
        "returnUrl": "https://example.com/mandate-return",  // URL to redirect after setup.
        "setupFutureUsage": FutureUsage.OFF_SESSION,  // Indicates future usage intention.
        "requestIncrementalAuthorization": false,  // Indicates if incremental authorization is requested.
        "customerAcceptance": {  // Details of customer acceptance.
            "acceptanceType": AcceptanceType.OFFLINE,  // Type of acceptance (e.g., online, offline).
            "acceptedAt": 0  // Timestamp when the acceptance was made (Unix timestamp, seconds since epoch).
        }
    };
}

function _buildVoidRequest(connectorTransactionId: string): types.IPaymentServiceVoidRequest {
    return {
        "merchantVoidId": "probe_void_001",  // Identification.
        "connectorTransactionId": connectorTransactionId
    };
}


// ANCHOR: scenario_functions
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

// Flow: PaymentMethodAuthenticationService.PreAuthenticate
async function preAuthenticate(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentMethodAuthenticationClient = new PaymentMethodAuthenticationClient(config);

    const preResponse = await paymentMethodAuthenticationClient.preAuthenticate(_buildPreAuthenticateRequest());

    return preResponse;
}

// Flow: PaymentService.ProxySetupRecurring
async function proxySetupRecurring(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    const proxyResponse = await paymentClient.proxySetupRecurring(_buildProxySetupRecurringRequest());

    return proxyResponse;
}

// Flow: RecurringPaymentService.Charge
async function recurringCharge(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const recurringPaymentClient = new RecurringPaymentClient(config);

    const recurringResponse = await recurringPaymentClient.charge(_buildRecurringChargeRequest());

    return recurringResponse;
}

// Flow: RefundService.Get
async function refundGet(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const refundClient = new RefundClient(config);

    const refundResponse = await refundClient.refundGet(_buildRefundGetRequest());

    return refundResponse;
}

// Flow: PaymentService.SetupRecurring
async function setupRecurring(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    const setupResponse = await paymentClient.setupRecurring(_buildSetupRecurringRequest());

    return setupResponse;
}

// Flow: PaymentService.Void
async function voidPayment(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    const voidResponse = await paymentClient.void(_buildVoidRequest('probe_connector_txn_001'));

    return voidResponse;
}


// Export all process* functions for the smoke test
export {
    capture, get, preAuthenticate, proxySetupRecurring, recurringCharge, refundGet, setupRecurring, voidPayment, _buildCaptureRequest, _buildGetRequest, _buildPreAuthenticateRequest, _buildProxySetupRecurringRequest, _buildRecurringChargeRequest, _buildRefundGetRequest, _buildSetupRecurringRequest, _buildVoidRequest
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
