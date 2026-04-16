// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py iatapay
//
// Iatapay — all integration scenarios and flows in one file.
// Run a scenario:  npx tsx iatapay.ts checkout_autocapture

import { PaymentClient, MerchantAuthenticationClient, RefundClient, types } from 'hyperswitch-prism';
const { ConnectorConfig, ConnectorSpecificConfig, SdkOptions, Environment, AuthenticationType, CaptureMethod, Currency } = types;

const _defaultConfig: ConnectorConfig = {
    options: {
        environment: Environment.SANDBOX,
    },
};
// Standalone credentials (field names depend on connector auth type):
// _defaultConfig.connectorConfig = {
//     iatapay: { apiKey: { value: 'YOUR_API_KEY' } }
// };


function _buildAuthorizeRequest(captureMethod: CaptureMethod): PaymentServiceAuthorizeRequest {
    return {
        "merchantTransactionId": "probe_txn_001",  // Identification.
        "amount": {  // The amount for the payment.
            "minorAmount": 1000,  // Amount in minor units (e.g., 1000 = $10.00).
            "currency": Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        },
        "paymentMethod": {  // Payment method to be used.
            "ideal": {
            }
        },
        "captureMethod": captureMethod,  // Method for capturing the payment.
        "address": {  // Address Information.
            "billingAddress": {
            }
        },
        "authType": AuthenticationType.NO_THREE_DS,  // Authentication Details.
        "returnUrl": "https://example.com/return",  // URLs for Redirection and Webhooks.
        "webhookUrl": "https://example.com/webhook",
        "state": {  // State Information.
            "accessToken": {  // Access token obtained from connector.
                "token": {"value": "probe_access_token"},  // The token string.
                "expiresInSeconds": 3600,  // Expiration timestamp (seconds since epoch).
                "tokenType": "Bearer"  // Token type (e.g., "Bearer", "Basic").
            }
        }
    };
}

function _buildCreateServerAuthenticationTokenRequest(): MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest {
    return {
    };
}

function _buildGetRequest(connectorTransactionId: string): PaymentServiceGetRequest {
    return {
        "merchantTransactionId": "probe_merchant_txn_001",  // Identification.
        "connectorTransactionId": connectorTransactionId,
        "amount": {  // Amount Information.
            "minorAmount": 1000,  // Amount in minor units (e.g., 1000 = $10.00).
            "currency": Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        },
        "state": {  // State Information.
            "accessToken": {  // Access token obtained from connector.
                "token": {"value": "probe_access_token"},  // The token string.
                "expiresInSeconds": 3600,  // Expiration timestamp (seconds since epoch).
                "tokenType": "Bearer"  // Token type (e.g., "Bearer", "Basic").
            }
        },
        "connectorOrderReferenceId": "probe_order_ref_001"  // Connector Reference Id.
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
        "reason": "customer_request",  // Reason for the refund.
        "webhookUrl": "https://example.com/webhook",  // URL for webhook notifications.
        "state": {  // State data for access token storage and.
            "accessToken": {  // Access token obtained from connector.
                "token": {"value": "probe_access_token"},  // The token string.
                "expiresInSeconds": 3600,  // Expiration timestamp (seconds since epoch).
                "tokenType": "Bearer"  // Token type (e.g., "Bearer", "Basic").
            }
        }
    };
}

function _buildRefundGetRequest(): RefundServiceGetRequest {
    return {
        "merchantRefundId": "probe_refund_001",  // Identification.
        "connectorTransactionId": "probe_connector_txn_001",
        "refundId": "probe_refund_id_001",
        "state": {  // State Information.
            "accessToken": {  // Access token obtained from connector.
                "token": {"value": "probe_access_token"},  // The token string.
                "expiresInSeconds": 3600,  // Expiration timestamp (seconds since epoch).
                "tokenType": "Bearer"  // Token type (e.g., "Bearer", "Basic").
            }
        }
    };
}


// ANCHOR: scenario_functions
// Flow: PaymentService.Authorize (Ideal)
async function authorize(merchantTransactionId: string, config: ConnectorConfig = _defaultConfig): Promise<PaymentServiceAuthorizeResponse> {
    const paymentClient = new PaymentClient(config);

    const authorizeResponse = await paymentClient.authorize(_buildAuthorizeRequest(CaptureMethod.AUTOMATIC));

    return { status: authorizeResponse.status, transactionId: authorizeResponse.connectorTransactionId };
}

// Flow: MerchantAuthenticationService.CreateServerAuthenticationToken
async function createServerAuthenticationToken(merchantTransactionId: string, config: ConnectorConfig = _defaultConfig): Promise<MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse> {
    const merchantAuthenticationClient = new MerchantAuthenticationClient(config);

    const createResponse = await merchantAuthenticationClient.createServerAuthenticationToken(_buildCreateServerAuthenticationTokenRequest());

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


// Export all process* functions for the smoke test
export {
    authorize, createServerAuthenticationToken, get, refund, refundGet, _buildAuthorizeRequest, _buildCreateServerAuthenticationTokenRequest, _buildGetRequest, _buildRefundRequest, _buildRefundGetRequest
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
