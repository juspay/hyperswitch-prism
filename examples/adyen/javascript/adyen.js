// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py adyen
//
// Adyen — all integration scenarios and flows in one file.
// Run a scenario:  node adyen.js checkout_card
'use strict';

const { PaymentClient, RecurringPaymentClient, DisputeClient } = require('hs-playlib');
const { ConnectorConfig, ConnectorSpecificConfig, SdkOptions, Environment } = require('hs-playlib').types;

const _defaultConfig = ConnectorConfig.create({
    options: SdkOptions.create({ environment: Environment.SANDBOX }),
});
// Standalone credentials (field names depend on connector auth type):
// _defaultConfig.connectorConfig = ConnectorSpecificConfig.create({
//     adyen: { apiKey: { value: 'YOUR_API_KEY' } }
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
                "cardNumber": "4111111111111111",  // Card Identification
                "cardExpMonth": "03",
                "cardExpYear": "2030",
                "cardCvc": "737",
                "cardHolderName": "John Doe"  // Cardholder Information
            }
        },
        "captureMethod": captureMethod,  // Method for capturing the payment
        "address": {  // Address Information
            "billingAddress": {
            }
        },
        "authType": "NO_THREE_DS",  // Authentication Details
        "returnUrl": "https://example.com/return",  // URLs for Redirection and Webhooks
        "browserInfo": {
            "colorDepth": 24,  // Display Information
            "screenHeight": 900,
            "screenWidth": 1440,
            "javaEnabled": false,  // Browser Settings
            "javaScriptEnabled": true,
            "language": "en-US",
            "timeZoneOffsetMinutes": -480,
            "acceptHeader": "application/json",  // Browser Headers
            "userAgent": "Mozilla/5.0 (probe-bot)",
            "acceptLanguage": "en-US,en;q=0.9",
            "ipAddress": "1.2.3.4"  // Device Information
        }
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

function _buildVoidRequest(connectorTransactionId) {
    return {
        "merchantVoidId": "probe_void_001",  // Identification
        "connectorTransactionId": connectorTransactionId
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

// Wallet Payment (Google Pay / Apple Pay)
// Wallet payments pass an encrypted token from the browser/device SDK. Pass the token blob directly — do not decrypt client-side.
async function processCheckoutWallet(merchantTransactionId, config = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    // Step 1: Authorize — reserve funds on the payment method
    const authorizeResponse = await paymentClient.authorize({
        "merchantTransactionId": "probe_txn_001",  // Identification
        "amount": {  // The amount for the payment
            "minorAmount": 1000,  // Amount in minor units (e.g., 1000 = $10.00)
            "currency": "USD"  // ISO 4217 currency code (e.g., "USD", "EUR")
        },
        "paymentMethod": {  // Payment method to be used
            "googlePay": {  // Google Pay
                "type": "CARD",  // Type of payment method
                "description": "Visa 1111",  // User-facing description of the payment method
                "info": {
                    "cardNetwork": "VISA",  // Card network name
                    "cardDetails": "1111"  // Card details (usually last 4 digits)
                },
                "tokenizationData": {
                    "encryptedData": {  // Encrypted Google Pay payment data
                        "tokenType": "PAYMENT_GATEWAY",  // The type of the token
                        "token": "{\"id\":\"tok_probe_gpay\",\"object\":\"token\",\"type\":\"card\"}"  // Token generated for the wallet
                    }
                }
            }
        },
        "captureMethod": "AUTOMATIC",  // Method for capturing the payment
        "address": {  // Address Information
            "billingAddress": {
            }
        },
        "authType": "NO_THREE_DS",  // Authentication Details
        "returnUrl": "https://example.com/return",  // URLs for Redirection and Webhooks
        "browserInfo": {
            "colorDepth": 24,  // Display Information
            "screenHeight": 900,
            "screenWidth": 1440,
            "javaEnabled": false,  // Browser Settings
            "javaScriptEnabled": true,
            "language": "en-US",
            "timeZoneOffsetMinutes": -480,
            "acceptHeader": "application/json",  // Browser Headers
            "userAgent": "Mozilla/5.0 (probe-bot)",
            "acceptLanguage": "en-US,en;q=0.9",
            "ipAddress": "1.2.3.4"  // Device Information
        }
    });

    if (authorizeResponse.status === 'FAILED') {
        throw new Error(`Payment failed: ${authorizeResponse.error?.message}`);
    }
    if (authorizeResponse.status === 'PENDING') {
        // Awaiting async confirmation — handle via webhook
        return { status: 'pending', transactionId: authorizeResponse.connectorTransactionId };
    }

    return { status: authorizeResponse.status, transactionId: authorizeResponse.connectorTransactionId, error: authorizeResponse.error };
}

// Bank Transfer (SEPA / ACH / BACS)
// Direct bank debit (Sepa). Bank transfers typically use `capture_method=AUTOMATIC`.
async function processCheckoutBank(merchantTransactionId, config = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    // Step 1: Authorize — reserve funds on the payment method
    const authorizeResponse = await paymentClient.authorize({
        "merchantTransactionId": "probe_txn_001",  // Identification
        "amount": {  // The amount for the payment
            "minorAmount": 1000,  // Amount in minor units (e.g., 1000 = $10.00)
            "currency": "EUR"  // ISO 4217 currency code (e.g., "USD", "EUR")
        },
        "paymentMethod": {  // Payment method to be used
            "sepa": {  // Sepa - Single Euro Payments Area direct debit
                "iban": "DE89370400440532013000",  // International bank account number (iban) for SEPA
                "bankAccountHolderName": "John Doe"  // Owner name for bank debit
            }
        },
        "captureMethod": "AUTOMATIC",  // Method for capturing the payment
        "address": {  // Address Information
            "billingAddress": {
                "firstName": "John"  // Personal Information
            }
        },
        "authType": "NO_THREE_DS",  // Authentication Details
        "returnUrl": "https://example.com/return"  // URLs for Redirection and Webhooks
    });

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
                "cardNumber": "4111111111111111",  // Card Identification
                "cardExpMonth": "03",
                "cardExpYear": "2030",
                "cardCvc": "737",
                "cardHolderName": "John Doe"  // Cardholder Information
            }
        },
        "customer": {
            "id": "cust_probe_123"  // Internal customer ID
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
        },
        "browserInfo": {  // Information about the customer's browser
            "colorDepth": 24,  // Display Information
            "screenHeight": 900,
            "screenWidth": 1440,
            "javaEnabled": false,  // Browser Settings
            "javaScriptEnabled": true,
            "language": "en-US",
            "timeZoneOffsetMinutes": -480,
            "acceptHeader": "application/json",  // Browser Headers
            "userAgent": "Mozilla/5.0 (probe-bot)",
            "acceptLanguage": "en-US,en;q=0.9",
            "ipAddress": "1.2.3.4"  // Device Information
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

// Void a Payment
// Authorize funds with a manual capture flag, then cancel the authorization with Void before any capture occurs. Releases the hold on the customer's funds.
async function processVoidPayment(merchantTransactionId, config = _defaultConfig) {
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

    // Step 2: Void — release reserved funds (cancel authorization)
    const voidResponse = await paymentClient.void(_buildVoidRequest(authorizeResponse.connectorTransactionId));

    return { status: voidResponse.status, transactionId: authorizeResponse.connectorTransactionId, error: voidResponse.error };
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

// Flow: DisputeService.Accept
async function disputeAccept(merchantTransactionId, config = _defaultConfig) {
    // Step 1: dispute_accept
    const disputeResponse = await disputeClient.accept({
        "merchantDisputeId": "probe_dispute_001",  // Identification
        "connectorTransactionId": "probe_txn_001",
        "disputeId": "probe_dispute_id_001"
    });

    return { status: disputeResponse.status };
}

// Flow: DisputeService.Defend
async function disputeDefend(merchantTransactionId, config = _defaultConfig) {
    // Step 1: dispute_defend
    const disputeResponse = await disputeClient.defend({
        "merchantDisputeId": "probe_dispute_001",  // Identification
        "connectorTransactionId": "probe_txn_001",
        "disputeId": "probe_dispute_id_001",
        "reasonCode": "probe_reason"  // Defend Details
    });

    return { status: disputeResponse.status };
}

// Flow: DisputeService.SubmitEvidence
async function disputeSubmitEvidence(merchantTransactionId, config = _defaultConfig) {
    // Step 1: dispute_submit_evidence
    const disputeResponse = await disputeClient.submitEvidence({
        "merchantDisputeId": "probe_dispute_001",  // Identification
        "connectorTransactionId": "probe_txn_001",
        "disputeId": "probe_dispute_id_001",
        "evidenceDocuments": [{"evidence_type": "SERVICE_DOCUMENTATION", "file_content": [112, 114, 111, 98, 101, 32, 101, 118, 105, 100, 101, 110, 99, 101, 32, 99, 111, 110, 116, 101, 110, 116], "file_mime_type": "application/pdf"}]  // Collection of evidence documents
    });

    return { status: disputeResponse.status };
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
            "token": "probe_pm_token"  // Payment tokens
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
                "cardNumber": "4111111111111111",  // Card Identification
                "cardExpMonth": "03",
                "cardExpYear": "2030",
                "cardCvc": "737",
                "cardHolderName": "John Doe"  // Cardholder Information
            }
        },
        "customer": {
            "id": "cust_probe_123"  // Internal customer ID
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
        },
        "browserInfo": {  // Information about the customer's browser
            "colorDepth": 24,  // Display Information
            "screenHeight": 900,
            "screenWidth": 1440,
            "javaEnabled": false,  // Browser Settings
            "javaScriptEnabled": true,
            "language": "en-US",
            "timeZoneOffsetMinutes": -480,
            "acceptHeader": "application/json",  // Browser Headers
            "userAgent": "Mozilla/5.0 (probe-bot)",
            "acceptLanguage": "en-US,en;q=0.9",
            "ipAddress": "1.2.3.4"  // Device Information
        }
    });

    if (setupResponse.status === 'FAILED') {
        throw new Error(`Recurring setup failed: ${setupResponse.error?.message}`);
    }

    return { status: setupResponse.status, mandateId: setupResponse.connectorTransactionId };
}

// Flow: PaymentService.Void
async function voidPayment(merchantTransactionId, config = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    const voidResponse = await paymentClient.void(_buildVoidRequest('probe_connector_txn_001'));

    return { status: voidResponse.status };
}


module.exports = { processCheckoutCard, processCheckoutAutocapture, processCheckoutWallet, processCheckoutBank, processRefund, processRecurring, processVoidPayment, authorize, capture, disputeAccept, disputeDefend, disputeSubmitEvidence, recurringCharge, setupRecurring, voidPayment };

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
