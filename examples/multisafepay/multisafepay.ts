// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py multisafepay
//
// Multisafepay — all integration scenarios and flows in one file.
// Run a scenario:  npx tsx multisafepay.ts checkout_autocapture

import { PaymentClient, FraudClient, MerchantAuthenticationClient, RefundClient, types } from 'hyperswitch-prism';
const { ConnectorConfig, ConnectorSpecificConfig, SdkOptions, Environment, AuthenticationType, CaptureMethod, Currency } = types;

const _defaultConfig: ConnectorConfig = {
    options: {
        environment: Environment.SANDBOX,
    },
};
// Standalone credentials (field names depend on connector auth type):
// _defaultConfig.connectorConfig = {
//     multisafepay: { apiKey: { value: 'YOUR_API_KEY' } }
// };


function _buildAuthorizeRequest(captureMethod: CaptureMethod): PaymentServiceAuthorizeRequest {
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
        "customer": {  // Customer Information.
            "email": {"value": "test@example.com"}  // Customer's email address.
        },
        "address": {  // Address Information.
            "billingAddress": {
            }
        },
        "authType": AuthenticationType.NO_THREE_DS,  // Authentication Details.
        "returnUrl": "https://example.com/return",  // URLs for Redirection and Webhooks.
        "description": "Probe payment"
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

function _buildGetRequest(connectorTransactionId): FraudServiceGetRequest {
    return {
        "merchantTransactionId": "probe_merchant_txn_001",
        "connectorTransactionId": connectorTransactionId,
        "amount": {
            "minorAmount": 1000,
            "currency": "USD"
        }
    };
}

function _buildProxyAuthorizeRequest(): PaymentServiceProxyAuthorizeRequest {
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
            "cardHolderName": {"value": "John Doe"}  // Cardholder Information.
        },
        "customer": {
            "email": {"value": "test@example.com"}  // Customer's email address.
        },
        "address": {
            "billingAddress": {
            }
        },
        "captureMethod": CaptureMethod.AUTOMATIC,
        "authType": AuthenticationType.NO_THREE_DS,
        "returnUrl": "https://example.com/return",
        "description": "Probe payment"  // Description of the transaction.
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
        "refundId": "probe_refund_id_001"
    };
}


// ANCHOR: scenario_functions
// One-step Payment (Authorize + Capture)
// Simple payment that authorizes and captures in one call. Use for immediate charges.
async function processCheckoutAutocapture(merchantTransactionId: string, config: ConnectorConfig = _defaultConfig): Promise<PaymentServiceAuthorizeResponse> {
    const paymentClient = new PaymentClient(config);

    // Step 1: Authorize — reserve funds on the payment method
    const authorizeResponse = await paymentClient.authorize(_buildAuthorizeRequest(CaptureMethod.AUTOMATIC));

    if (authorizeResponse.status === 'FAILED') {
        throw new Error(`Payment failed: ${authorizeResponse.error?.message}`);
    }
    if (authorizeResponse.status === 'PENDING') {
        // Awaiting async confirmation — handle via webhook
        return { status: 'pending', transactionId: authorizeResponse.connectorTransactionId };
    }

    return { status: authorizeResponse.status, transactionId: authorizeResponse.connectorTransactionId, error: authorizeResponse.error };
}

// Refund
// Return funds to the customer for a completed payment.
async function processRefund(merchantTransactionId: string, config: ConnectorConfig = _defaultConfig): Promise<RefundResponse> {
    const paymentClient = new PaymentClient(config);

    // Step 1: Authorize — reserve funds on the payment method
    const authorizeResponse = await paymentClient.authorize(_buildAuthorizeRequest(CaptureMethod.AUTOMATIC));

    if (authorizeResponse.status === 'FAILED') {
        throw new Error(`Payment failed: ${authorizeResponse.error?.message}`);
    }
    if (authorizeResponse.status === 'PENDING') {
        // Awaiting async confirmation — handle via webhook
        return { status: 'pending', transactionId: authorizeResponse.connectorTransactionId };
    }

    // Step 2: Refund — return funds to the customer
    const refundResponse = await paymentClient.refund(_buildRefundRequest(authorizeResponse.connectorTransactionId));

    if (refundResponse.status === 'FAILED') {
        throw new Error(`Refund failed: ${refundResponse.error?.message}`);
    }

    return { status: refundResponse.status, error: refundResponse.error };
}

// Get Payment Status
// Retrieve current payment status from the connector.
async function processGetPayment(merchantTransactionId: string, config: ConnectorConfig = _defaultConfig): Promise<FraudServiceGetResponse> {
    const paymentClient = new PaymentClient(config);
    const fraudClient = new FraudClient(config);

    // Step 1: Authorize — reserve funds on the payment method
    const authorizeResponse = await paymentClient.authorize(_buildAuthorizeRequest(CaptureMethod.MANUAL));

    if (authorizeResponse.status === 'FAILED') {
        throw new Error(`Payment failed: ${authorizeResponse.error?.message}`);
    }
    if (authorizeResponse.status === 'PENDING') {
        // Awaiting async confirmation — handle via webhook
        return { status: 'pending', transactionId: authorizeResponse.connectorTransactionId };
    }

    // Step 2: Get — retrieve current payment status from the connector
    const getResponse = await fraudClient.get(_buildGetRequest(authorizeResponse.connectorTransactionId));

    return { status: getResponse.status, transactionId: getResponse.connectorTransactionId, error: getResponse.error };
}

// Flow: PaymentService.Authorize (Card)
async function authorize(merchantTransactionId: string, config: ConnectorConfig = _defaultConfig): Promise<PaymentServiceAuthorizeResponse> {
    const paymentClient = new PaymentClient(config);

    const authorizeResponse = await paymentClient.authorize(_buildAuthorizeRequest(CaptureMethod.AUTOMATIC));

    return { status: authorizeResponse.status, transactionId: authorizeResponse.connectorTransactionId };
}

// Flow: MerchantAuthenticationService.CreateClientAuthenticationToken
async function createClientAuthenticationToken(merchantTransactionId: string, config: ConnectorConfig = _defaultConfig): Promise<MerchantAuthenticationServiceCreateClientAuthenticationTokenResponse> {
    const merchantAuthenticationClient = new MerchantAuthenticationClient(config);

    const createResponse = await merchantAuthenticationClient.createClientAuthenticationToken(_buildCreateClientAuthenticationTokenRequest());

    return { status: createResponse.status };
}

// Flow: FraudService.Get
async function get(merchantTransactionId: string, config: ConnectorConfig = _defaultConfig): Promise<FraudServiceGetResponse> {
    const fraudClient = new FraudClient(config);

    const getResponse = await fraudClient.get(_buildGetRequest('probe_connector_txn_001'));

    return { status: getResponse.status };
}

// Flow: PaymentService.ProxyAuthorize
async function proxyAuthorize(merchantTransactionId: string, config: ConnectorConfig = _defaultConfig): Promise<PaymentServiceAuthorizeResponse> {
    const paymentClient = new PaymentClient(config);

    const proxyResponse = await paymentClient.proxyAuthorize(_buildProxyAuthorizeRequest());

    return { status: proxyResponse.status };
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


// Export all process* functions for the smoke test
export {
    processCheckoutAutocapture, processRefund, processGetPayment, authorize, createClientAuthenticationToken, get, proxyAuthorize, refund, refundGet, _buildAuthorizeRequest, _buildCreateClientAuthenticationTokenRequest, _buildGetRequest, _buildProxyAuthorizeRequest, _buildRefundRequest, _buildRefundGetRequest
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
