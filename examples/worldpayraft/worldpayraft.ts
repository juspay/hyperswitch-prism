// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py worldpayraft
//
// Worldpayraft — all integration scenarios and flows in one file.
// Run a scenario:  npx tsx worldpayraft.ts checkout_autocapture

import { PaymentClient, RecurringPaymentClient, types } from 'hyperswitch-prism';
const { Environment, AcceptanceType, AuthenticationType, CaptureMethod, CardNetwork, Currency, FutureUsage, PaymentMethodType } = types;
export const SUPPORTED_FLOWS = ["authorize", "proxy_authorize", "proxy_setup_recurring", "recurring_charge", "setup_recurring"];

const _defaultConfig: types.IConnectorConfig = {
    options: {
        environment: Environment.SANDBOX,
    },
    connectorConfig: {
        worldpayraft: {
            license: { value: 'YOUR_LICENSE' },
            merchantId: { value: 'YOUR_MERCHANT_ID' },
            baseUrl: 'YOUR_BASE_URL',
        }
    },
};


function _buildAuthorizeRequest(captureMethod: types.CaptureMethod): types.IPaymentServiceAuthorizeRequest {
    return {
        "merchantTransactionId": "probe_txn_001",  // Identification.
        "amount": {  // The amount for the payment.
            "minorAmount": 1000,  // Amount in minor units (e.g., 1000 = $10.00).
            "currency": Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
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

function _buildProxyAuthorizeRequest(): types.IPaymentServiceProxyAuthorizeRequest {
    return {
        "merchantTransactionId": "probe_proxy_txn_001",
        "amount": {
            "minorAmount": 1000,  // Amount in minor units (e.g., 1000 = $10.00).
            "currency": Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
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
        "connectorCustomerId": "cust_probe_123",
        "paymentMethodType": PaymentMethodType.PAY_PAL,
        "offSession": true  // Behavioral Flags and Preferences.
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

// Flow: PaymentService.Authorize (Card)
async function authorize(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    const authorizeResponse = await paymentClient.authorize(_buildAuthorizeRequest(CaptureMethod.AUTOMATIC));

    return authorizeResponse;
}

// Flow: PaymentService.ProxyAuthorize
async function proxyAuthorize(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    const proxyResponse = await paymentClient.proxyAuthorize(_buildProxyAuthorizeRequest());

    return proxyResponse;
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

// Flow: PaymentService.SetupRecurring
async function setupRecurring(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    const setupResponse = await paymentClient.setupRecurring(_buildSetupRecurringRequest());

    return setupResponse;
}


// Export all process* functions for the smoke test
export {
    processCheckoutAutocapture, authorize, proxyAuthorize, proxySetupRecurring, recurringCharge, setupRecurring, _buildAuthorizeRequest, _buildProxyAuthorizeRequest, _buildProxySetupRecurringRequest, _buildRecurringChargeRequest, _buildSetupRecurringRequest
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
