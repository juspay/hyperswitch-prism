// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py braintree
//
// Braintree — all integration scenarios and flows in one file.
// Run a scenario:  npx tsx braintree.ts checkout_autocapture

import { PaymentClient, MerchantAuthenticationClient, RefundClient, PaymentMethodClient, types } from 'hyperswitch-prism';
const { ConnectorConfig, ConnectorSpecificConfig, SdkOptions, Environment, AcceptanceType, Currency, FutureUsage } = types;

const _defaultConfig: ConnectorConfig = {
    options: {
        environment: Environment.SANDBOX,
    },
};
// Standalone credentials (field names depend on connector auth type):
// _defaultConfig.connectorConfig = {
//     braintree: { apiKey: { value: 'YOUR_API_KEY' } }
// };


function _buildCaptureRequest(connectorTransactionId: string): PaymentServiceCaptureRequest {
    return {
        "merchantCaptureId": "probe_capture_001",  // Identification.
        "connectorTransactionId": connectorTransactionId,
        "amountToCapture": {  // Capture Details.
            "minorAmount": 1000,  // Amount in minor units (e.g., 1000 = $10.00).
            "currency": Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        }
    };
}

function _buildCreateClientAuthenticationTokenRequest(): MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest {
    return {
        "merchantClientSessionId": "probe_sdk_session_001",  // Infrastructure.
        "domainContext": {
            "minorAmount": 1000,
            "currency": "USD"
        }
    };
}

function _buildGetRequest(connectorTransactionId: string): PaymentServiceGetRequest {
    return {
        "merchantTransactionId": "probe_merchant_txn_001",  // Identification.
        "connectorTransactionId": connectorTransactionId,
        "amount": {  // Amount Information.
            "minorAmount": 1000,  // Amount in minor units (e.g., 1000 = $10.00).
            "currency": Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        }
    };
}

function _buildRefundRequest(connectorTransactionId: string): PaymentServiceRefundRequest {
    return {
        "merchantRefundId": "probe_refund_001",  // Identification.
        "connectorTransactionId": connectorTransactionId,
        "paymentAmount": 1000,  // Amount Information.
        "refundAmount": {
            "minorAmount": 1000,  // Amount in minor units (e.g., 1000 = $10.00).
            "currency": Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        },
        "reason": "customer_request"  // Reason for the refund.
    };
}

function _buildRefundGetRequest(): RefundServiceGetRequest {
    return {
        "merchantRefundId": "probe_refund_001",  // Identification.
        "connectorTransactionId": "probe_connector_txn_001",
        "refundId": "probe_refund_id_001",
        "refundMetadata": {"value": "{\"currency\":\"USD\"}"}  // Metadata specific to the refund sync.
    };
}

function _buildTokenSetupRecurringRequest(): PaymentServiceTokenSetupRecurringRequest {
    return {
        "merchantRecurringPaymentId": "probe_tokenized_mandate_001",
        "amount": {
            "minorAmount": 0,  // Amount in minor units (e.g., 1000 = $10.00).
            "currency": Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        },
        "connectorToken": {"value": "pm_1AbcXyzStripeTestToken"},
        "address": {
            "billingAddress": {
            }
        },
        "customerAcceptance": {
            "acceptanceType": AcceptanceType.ONLINE,  // Type of acceptance (e.g., online, offline).
            "acceptedAt": 0,  // Timestamp when the acceptance was made (Unix timestamp, seconds since epoch).
            "onlineMandateDetails": {  // Details if the acceptance was an online mandate.
                "ipAddress": "127.0.0.1",  // IP address from which the mandate was accepted.
                "userAgent": "Mozilla/5.0"  // User agent string of the browser used for mandate acceptance.
            }
        },
        "setupMandateDetails": {
            "mandateType": {  // Type of mandate (single_use or multi_use) with amount details.
                "multiUse": {  // Multi use mandate with amount details (for recurring payments).
                    "amount": 0,  // Amount.
                    "currency": Currency.USD  // Currency code (ISO 4217).
                }
            }
        },
        "setupFutureUsage": FutureUsage.OFF_SESSION
    };
}

function _buildTokenizeRequest(): PaymentMethodServiceTokenizeRequest {
    return {
        "amount": {  // Payment Information.
            "minorAmount": 1000,  // Amount in minor units (e.g., 1000 = $10.00).
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
        }
    };
}

function _buildVoidRequest(connectorTransactionId: string): PaymentServiceVoidRequest {
    return {
        "merchantVoidId": "probe_void_001",  // Identification.
        "connectorTransactionId": connectorTransactionId
    };
}


// ANCHOR: scenario_functions
// Flow: PaymentService.Capture
async function capture(merchantTransactionId: string, config: ConnectorConfig = _defaultConfig): Promise<PaymentServiceCaptureResponse> {
    const paymentClient = new PaymentClient(config);

    const captureResponse = await paymentClient.capture(_buildCaptureRequest('probe_connector_txn_001'));

    return { status: captureResponse.status };
}

// Flow: MerchantAuthenticationService.CreateClientAuthenticationToken
async function createClientAuthenticationToken(merchantTransactionId: string, config: ConnectorConfig = _defaultConfig): Promise<MerchantAuthenticationServiceCreateClientAuthenticationTokenResponse> {
    const merchantAuthenticationClient = new MerchantAuthenticationClient(config);

    const createResponse = await merchantAuthenticationClient.createClientAuthenticationToken(_buildCreateClientAuthenticationTokenRequest());

    return { status: createResponse.status };
}

// Flow: PaymentService.Get
async function get(merchantTransactionId: string, config: ConnectorConfig = _defaultConfig): Promise<PaymentServiceGetResponse> {
    const paymentClient = new PaymentClient(config);

    const getResponse = await paymentClient.get(_buildGetRequest('probe_connector_txn_001'));

    return { status: getResponse.status };
}

// Flow: PaymentService.Refund
async function refund(merchantTransactionId: string, config: ConnectorConfig = _defaultConfig): Promise<RefundResponse> {
    const paymentClient = new PaymentClient(config);

    const refundResponse = await paymentClient.refund(_buildRefundRequest('probe_connector_txn_001'));

    return { status: refundResponse.status };
}

// Flow: RefundService.Get
async function refundGet(merchantTransactionId: string, config: ConnectorConfig = _defaultConfig): Promise<RefundResponse> {
    const refundClient = new RefundClient(config);

    const refundResponse = await refundClient.refundGet(_buildRefundGetRequest());

    return { status: refundResponse.status };
}

// Flow: PaymentService.TokenSetupRecurring
async function tokenSetupRecurring(merchantTransactionId: string, config: ConnectorConfig = _defaultConfig): Promise<PaymentServiceSetupRecurringResponse> {
    const paymentClient = new PaymentClient(config);

    const tokenResponse = await paymentClient.tokenSetupRecurring(_buildTokenSetupRecurringRequest());

    return { status: tokenResponse.status };
}

// Flow: PaymentMethodService.Tokenize
async function tokenize(merchantTransactionId: string, config: ConnectorConfig = _defaultConfig): Promise<PaymentMethodServiceTokenizeResponse> {
    const paymentMethodClient = new PaymentMethodClient(config);

    const tokenizeResponse = await paymentMethodClient.tokenize(_buildTokenizeRequest());

    return { status: tokenizeResponse.status };
}

// Flow: PaymentService.Void
async function voidPayment(merchantTransactionId: string, config: ConnectorConfig = _defaultConfig): Promise<PaymentServiceVoidResponse> {
    const paymentClient = new PaymentClient(config);

    const voidResponse = await paymentClient.void(_buildVoidRequest('probe_connector_txn_001'));

    return { status: voidResponse.status };
}


// Export all process* functions for the smoke test
export {
    capture, createClientAuthenticationToken, get, refund, refundGet, tokenSetupRecurring, tokenize, voidPayment, _buildCaptureRequest, _buildCreateClientAuthenticationTokenRequest, _buildGetRequest, _buildRefundRequest, _buildRefundGetRequest, _buildTokenSetupRecurringRequest, _buildTokenizeRequest, _buildVoidRequest
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
