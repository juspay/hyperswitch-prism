// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py adyen
//
// Adyen — all integration scenarios and flows in one file.
// Run a scenario:  npx tsx adyen.ts checkout_autocapture

import { PaymentClient, MerchantAuthenticationClient, DisputeClient, EventClient, RecurringPaymentClient, types } from 'hyperswitch-prism';
const { ConnectorConfig, ConnectorSpecificConfig, SdkOptions, Environment, AcceptanceType, AuthenticationType, CaptureMethod, Currency, FutureUsage, PaymentMethodType } = types;

const _defaultConfig: ConnectorConfig = {
    options: {
        environment: Environment.SANDBOX,
    },
};
// Standalone credentials (field names depend on connector auth type):
// _defaultConfig.connectorConfig = {
//     adyen: { apiKey: { value: 'YOUR_API_KEY' } }
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
        "address": {  // Address Information.
            "billingAddress": {
            }
        },
        "authType": AuthenticationType.NO_THREE_DS,  // Authentication Details.
        "returnUrl": "https://example.com/return",  // URLs for Redirection and Webhooks.
        "browserInfo": {
            "colorDepth": 24,  // Display Information.
            "screenHeight": 900,
            "screenWidth": 1440,
            "javaEnabled": false,  // Browser Settings.
            "javaScriptEnabled": true,
            "language": "en-US",
            "timeZoneOffsetMinutes": -480,
            "acceptHeader": "application/json",  // Browser Headers.
            "userAgent": "Mozilla/5.0 (probe-bot)",
            "acceptLanguage": "en-US,en;q=0.9",
            "ipAddress": "1.2.3.4"  // Device Information.
        }
    };
}

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

function _buildCreateOrderRequest(): PaymentServiceCreateOrderRequest {
    return {
        "merchantOrderId": "probe_order_001",  // Identification.
        "amount": {  // Amount Information.
            "minorAmount": 1000,  // Amount in minor units (e.g., 1000 = $10.00).
            "currency": Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        }
    };
}

function _buildDisputeAcceptRequest(): DisputeServiceAcceptRequest {
    return {
        "merchantDisputeId": "probe_dispute_001",  // Identification.
        "connectorTransactionId": "probe_txn_001",
        "disputeId": "probe_dispute_id_001"
    };
}

function _buildDisputeDefendRequest(): DisputeServiceDefendRequest {
    return {
        "merchantDisputeId": "probe_dispute_001",  // Identification.
        "connectorTransactionId": "probe_txn_001",
        "disputeId": "probe_dispute_id_001",
        "reasonCode": "probe_reason"  // Defend Details.
    };
}

function _buildDisputeSubmitEvidenceRequest(): DisputeServiceSubmitEvidenceRequest {
    return {
        "merchantDisputeId": "probe_dispute_001",  // Identification.
        "connectorTransactionId": "probe_txn_001",
        "disputeId": "probe_dispute_id_001",
        "evidenceDocuments": [{"evidence_type": "SERVICE_DOCUMENTATION", "file_content": [112, 114, 111, 98, 101, 32, 101, 118, 105, 100, 101, 110, 99, 101, 32, 99, 111, 110, 116, 101, 110, 116], "file_mime_type": "application/pdf"}]  // Collection of evidence documents.
    };
}

function _buildHandleEventRequest(): EventServiceHandleRequest {
    return {
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
        "address": {
            "billingAddress": {
            }
        },
        "captureMethod": CaptureMethod.AUTOMATIC,
        "authType": AuthenticationType.NO_THREE_DS,
        "returnUrl": "https://example.com/return",
        "browserInfo": {
            "colorDepth": 24,  // Display Information.
            "screenHeight": 900,
            "screenWidth": 1440,
            "javaEnabled": false,  // Browser Settings.
            "javaScriptEnabled": true,
            "language": "en-US",
            "timeZoneOffsetMinutes": -480,
            "acceptHeader": "application/json",  // Browser Headers.
            "userAgent": "Mozilla/5.0 (probe-bot)",
            "acceptLanguage": "en-US,en;q=0.9",
            "ipAddress": "1.2.3.4"  // Device Information.
        }
    };
}

function _buildProxySetupRecurringRequest(): PaymentServiceProxySetupRecurringRequest {
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
            "cardHolderName": {"value": "John Doe"}  // Cardholder Information.
        },
        "customer": {
            "connectorCustomerId": "probe_customer_001"  // Customer ID in the connector system.
        },
        "address": {
            "billingAddress": {
            }
        },
        "returnUrl": "https://example.com/return",
        "customerAcceptance": {
            "acceptanceType": AcceptanceType.OFFLINE,  // Type of acceptance (e.g., online, offline).
            "acceptedAt": 0  // Timestamp when the acceptance was made (Unix timestamp, seconds since epoch).
        },
        "authType": AuthenticationType.NO_THREE_DS,
        "setupFutureUsage": FutureUsage.OFF_SESSION,
        "browserInfo": {
            "colorDepth": 24,  // Display Information.
            "screenHeight": 900,
            "screenWidth": 1440,
            "javaEnabled": false,  // Browser Settings.
            "javaScriptEnabled": true,
            "language": "en-US",
            "timeZoneOffsetMinutes": -480,
            "acceptHeader": "application/json",  // Browser Headers.
            "userAgent": "Mozilla/5.0 (probe-bot)",
            "acceptLanguage": "en-US,en;q=0.9",
            "ipAddress": "1.2.3.4"  // Device Information.
        }
    };
}

function _buildRecurringChargeRequest(): RecurringPaymentServiceChargeRequest {
    return {
        "connectorRecurringPaymentId": {  // Reference to existing mandate.
            "connectorMandateId": {  // mandate_id sent by the connector.
                "connectorMandateId": "probe-mandate-123"
            }
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

function _buildSetupRecurringRequest(): PaymentServiceSetupRecurringRequest {
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
        "customer": {
            "id": "cust_probe_123"  // Internal customer ID.
        },
        "address": {  // Address Information.
            "billingAddress": {
            }
        },
        "authType": AuthenticationType.NO_THREE_DS,  // Type of authentication to be used.
        "enrolledFor3Ds": false,  // Indicates if the customer is enrolled for 3D Secure.
        "returnUrl": "https://example.com/mandate-return",  // URL to redirect after setup.
        "setupFutureUsage": FutureUsage.OFF_SESSION,  // Indicates future usage intention.
        "requestIncrementalAuthorization": false,  // Indicates if incremental authorization is requested.
        "customerAcceptance": {  // Details of customer acceptance.
            "acceptanceType": AcceptanceType.OFFLINE,  // Type of acceptance (e.g., online, offline).
            "acceptedAt": 0  // Timestamp when the acceptance was made (Unix timestamp, seconds since epoch).
        },
        "browserInfo": {  // Information about the customer's browser.
            "colorDepth": 24,  // Display Information.
            "screenHeight": 900,
            "screenWidth": 1440,
            "javaEnabled": false,  // Browser Settings.
            "javaScriptEnabled": true,
            "language": "en-US",
            "timeZoneOffsetMinutes": -480,
            "acceptHeader": "application/json",  // Browser Headers.
            "userAgent": "Mozilla/5.0 (probe-bot)",
            "acceptLanguage": "en-US,en;q=0.9",
            "ipAddress": "1.2.3.4"  // Device Information.
        }
    };
}

function _buildTokenAuthorizeRequest(): PaymentServiceTokenAuthorizeRequest {
    return {
        "merchantTransactionId": "probe_tokenized_txn_001",
        "amount": {
            "minorAmount": 1000,  // Amount in minor units (e.g., 1000 = $10.00).
            "currency": Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        },
        "connectorToken": {"value": "pm_1AbcXyzStripeTestToken"},  // Connector-issued token. Replaces PaymentMethod entirely. Examples: Stripe pm_xxx, Adyen recurringDetailReference, Braintree nonce.
        "address": {
            "billingAddress": {
            }
        },
        "captureMethod": CaptureMethod.AUTOMATIC,
        "returnUrl": "https://example.com/return"
    };
}

function _buildVoidRequest(connectorTransactionId: string): PaymentServiceVoidRequest {
    return {
        "merchantVoidId": "probe_void_001",  // Identification.
        "connectorTransactionId": connectorTransactionId
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

// Card Payment (Authorize + Capture)
// Two-step card payment. First authorize, then capture. Use when you need to verify funds before finalizing.
async function processCheckoutCard(merchantTransactionId: string, config: ConnectorConfig = _defaultConfig): Promise<PaymentServiceCaptureResponse> {
    const paymentClient = new PaymentClient(config);

    // Step 1: Authorize — reserve funds on the payment method
    const authorizeResponse = await paymentClient.authorize(_buildAuthorizeRequest(CaptureMethod.MANUAL));

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

// Void Payment
// Cancel an authorized but not-yet-captured payment.
async function processVoidPayment(merchantTransactionId: string, config: ConnectorConfig = _defaultConfig): Promise<PaymentServiceVoidResponse> {
    const paymentClient = new PaymentClient(config);

    // Step 1: Authorize — reserve funds on the payment method
    const authorizeResponse = await paymentClient.authorize(_buildAuthorizeRequest(CaptureMethod.MANUAL));

    if (authorizeResponse.status === 'FAILED') {
        throw new Error(`Payment failed: ${authorizeResponse.error?.message}`);
    }
    if (authorizeResponse.status === 'PENDING') {
        // Awaiting async confirmation — handle via webhook
        return { status: 'pending', transactionId: authorizeResponse.connectorTransactionId };
    }

    // Step 2: Void — release reserved funds (cancel authorization)
    const voidResponse = await paymentClient.void(_buildVoidRequest(authorizeResponse.connectorTransactionId));

    return { status: voidResponse.status, transactionId: authorizeResponse.connectorTransactionId, error: voidResponse.error };
}

// Flow: PaymentService.Authorize (Card)
async function authorize(merchantTransactionId: string, config: ConnectorConfig = _defaultConfig): Promise<PaymentServiceAuthorizeResponse> {
    const paymentClient = new PaymentClient(config);

    const authorizeResponse = await paymentClient.authorize(_buildAuthorizeRequest(CaptureMethod.AUTOMATIC));

    return { status: authorizeResponse.status, transactionId: authorizeResponse.connectorTransactionId };
}

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

// Flow: PaymentService.CreateOrder
async function createOrder(merchantTransactionId: string, config: ConnectorConfig = _defaultConfig): Promise<PaymentServiceCreateOrderResponse> {
    const paymentClient = new PaymentClient(config);

    const createResponse = await paymentClient.createOrder(_buildCreateOrderRequest());

    return { status: createResponse.status };
}

// Flow: DisputeService.Accept
async function disputeAccept(merchantTransactionId: string, config: ConnectorConfig = _defaultConfig): Promise<DisputeServiceAcceptResponse> {
    const disputeClient = new DisputeClient(config);

    const disputeResponse = await disputeClient.accept(_buildDisputeAcceptRequest());

    return { status: disputeResponse.status };
}

// Flow: DisputeService.Defend
async function disputeDefend(merchantTransactionId: string, config: ConnectorConfig = _defaultConfig): Promise<DisputeServiceDefendResponse> {
    const disputeClient = new DisputeClient(config);

    const disputeResponse = await disputeClient.defend(_buildDisputeDefendRequest());

    return { status: disputeResponse.status };
}

// Flow: DisputeService.SubmitEvidence
async function disputeSubmitEvidence(merchantTransactionId: string, config: ConnectorConfig = _defaultConfig): Promise<DisputeServiceSubmitEvidenceResponse> {
    const disputeClient = new DisputeClient(config);

    const disputeResponse = await disputeClient.submitEvidence(_buildDisputeSubmitEvidenceRequest());

    return { status: disputeResponse.status };
}

// Flow: EventService.HandleEvent
async function handleEvent(merchantTransactionId: string, config: ConnectorConfig = _defaultConfig): Promise<EventServiceHandleResponse> {
    const eventClient = new EventClient(config);

    const handleResponse = await eventClient.handleEvent(_buildHandleEventRequest());

    return { status: handleResponse.status };
}

// Flow: PaymentService.ProxyAuthorize
async function proxyAuthorize(merchantTransactionId: string, config: ConnectorConfig = _defaultConfig): Promise<PaymentServiceAuthorizeResponse> {
    const paymentClient = new PaymentClient(config);

    const proxyResponse = await paymentClient.proxyAuthorize(_buildProxyAuthorizeRequest());

    return { status: proxyResponse.status };
}

// Flow: PaymentService.ProxySetupRecurring
async function proxySetupRecurring(merchantTransactionId: string, config: ConnectorConfig = _defaultConfig): Promise<PaymentServiceSetupRecurringResponse> {
    const paymentClient = new PaymentClient(config);

    const proxyResponse = await paymentClient.proxySetupRecurring(_buildProxySetupRecurringRequest());

    return { status: proxyResponse.status };
}

// Flow: RecurringPaymentService.Charge
async function recurringCharge(merchantTransactionId: string, config: ConnectorConfig = _defaultConfig): Promise<RecurringPaymentServiceChargeResponse> {
    const recurringPaymentClient = new RecurringPaymentClient(config);

    const recurringResponse = await recurringPaymentClient.charge(_buildRecurringChargeRequest());

    return { status: recurringResponse.status };
}

// Flow: PaymentService.Refund
async function refund(merchantTransactionId: string, config: ConnectorConfig = _defaultConfig): Promise<RefundResponse> {
    const paymentClient = new PaymentClient(config);

    const refundResponse = await paymentClient.refund(_buildRefundRequest('probe_connector_txn_001'));

    return { status: refundResponse.status };
}

// Flow: PaymentService.SetupRecurring
async function setupRecurring(merchantTransactionId: string, config: ConnectorConfig = _defaultConfig): Promise<PaymentServiceSetupRecurringResponse> {
    const paymentClient = new PaymentClient(config);

    const setupResponse = await paymentClient.setupRecurring(_buildSetupRecurringRequest());

    return { status: setupResponse.status, mandateId: setupResponse.connectorTransactionId };
}

// Flow: PaymentService.TokenAuthorize
async function tokenAuthorize(merchantTransactionId: string, config: ConnectorConfig = _defaultConfig): Promise<PaymentServiceAuthorizeResponse> {
    const paymentClient = new PaymentClient(config);

    const tokenResponse = await paymentClient.tokenAuthorize(_buildTokenAuthorizeRequest());

    return { status: tokenResponse.status };
}

// Flow: PaymentService.Void
async function voidPayment(merchantTransactionId: string, config: ConnectorConfig = _defaultConfig): Promise<PaymentServiceVoidResponse> {
    const paymentClient = new PaymentClient(config);

    const voidResponse = await paymentClient.void(_buildVoidRequest('probe_connector_txn_001'));

    return { status: voidResponse.status };
}


// Export all process* functions for the smoke test
export {
    processCheckoutAutocapture, processCheckoutCard, processRefund, processVoidPayment, authorize, capture, createClientAuthenticationToken, createOrder, disputeAccept, disputeDefend, disputeSubmitEvidence, handleEvent, proxyAuthorize, proxySetupRecurring, recurringCharge, refund, setupRecurring, tokenAuthorize, voidPayment, _buildAuthorizeRequest, _buildCaptureRequest, _buildCreateClientAuthenticationTokenRequest, _buildCreateOrderRequest, _buildDisputeAcceptRequest, _buildDisputeDefendRequest, _buildDisputeSubmitEvidenceRequest, _buildHandleEventRequest, _buildProxyAuthorizeRequest, _buildProxySetupRecurringRequest, _buildRecurringChargeRequest, _buildRefundRequest, _buildSetupRecurringRequest, _buildTokenAuthorizeRequest, _buildVoidRequest
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
