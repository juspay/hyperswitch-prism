// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py cybersource
//
// Cybersource — all integration scenarios and flows in one file.
// Run a scenario:  npx tsx cybersource.ts checkout_autocapture

import { PaymentClient, types } from 'hyperswitch-prism';
const { Environment } = types;
export const SUPPORTED_FLOWS = ["authenticate", "authorize", "capture", "get", "post_authenticate", "pre_authenticate", "proxy_authorize", "recurring_charge", "recurring_revoke", "refund", "refund_get", "token_authorize", "void"];

const _defaultConfig: types.IConnectorConfig = {
    options: {
        environment: Environment.SANDBOX,
    },
    // connectorConfig: { cybersource: { apiKey: { value: 'YOUR_API_KEY' } } },
};


// ANCHOR: scenario_functions
// One-step Payment (Authorize + Capture)
// Simple payment that authorizes and captures in one call. Use for immediate charges.
async function processCheckoutAutocapture(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    // Step 1: Authorize — reserve funds on the payment method
    const authorizeResponse = await paymentClient.authorize({
        "merchantTransactionId": "probe_txn_001",
        "amount": {
        },
        "paymentMethod": {
        },
        "captureMethod": "AUTOMATIC",
        "customer": {
        },
        "address": {
        },
        "authType": "NO_THREE_DS",
        "returnUrl": "https://example.com/return"
    });

    if (authorizeResponse.status === types.PaymentStatus.FAILURE) {
        throw new Error(`Payment failed: ${JSON.stringify(authorizeResponse.error)}`);
    }
    if (authorizeResponse.status === types.PaymentStatus.PENDING) {
        // Awaiting async confirmation — handle via webhook
        return { status: 'pending', transactionId: authorizeResponse.connectorTransactionId } as any;
    }

    return { status: authorizeResponse.status, transactionId: authorizeResponse.connectorTransactionId!, error: authorizeResponse.error } as any;
}

// Card Payment (Authorize + Capture)
// Two-step card payment. First authorize, then capture. Use when you need to verify funds before finalizing.
async function processCheckoutCard(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    // Step 1: Authorize — reserve funds on the payment method
    const authorizeResponse = await paymentClient.authorize({
        "merchantTransactionId": "probe_txn_001",
        "amount": {
        },
        "paymentMethod": {
        },
        "captureMethod": "MANUAL",
        "customer": {
        },
        "address": {
        },
        "authType": "NO_THREE_DS",
        "returnUrl": "https://example.com/return"
    });

    if (authorizeResponse.status === types.PaymentStatus.FAILURE) {
        throw new Error(`Payment failed: ${JSON.stringify(authorizeResponse.error)}`);
    }
    if (authorizeResponse.status === types.PaymentStatus.PENDING) {
        // Awaiting async confirmation — handle via webhook
        return { status: 'pending', transactionId: authorizeResponse.connectorTransactionId } as any;
    }

    // Step 2: Capture — settle the reserved funds
    const captureResponse = await paymentClient.capture({
        "merchantCaptureId": "probe_capture_001",
        "connectorTransactionId": authorizeResponse.connectorTransactionId,  // from authorize response
        "amountToCapture": {
        }
    });

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
    const authorizeResponse = await paymentClient.authorize({
        "merchantTransactionId": "probe_txn_001",
        "amount": {
        },
        "paymentMethod": {
        },
        "captureMethod": "AUTOMATIC",
        "customer": {
        },
        "address": {
        },
        "authType": "NO_THREE_DS",
        "returnUrl": "https://example.com/return"
    });

    if (authorizeResponse.status === types.PaymentStatus.FAILURE) {
        throw new Error(`Payment failed: ${JSON.stringify(authorizeResponse.error)}`);
    }
    if (authorizeResponse.status === types.PaymentStatus.PENDING) {
        // Awaiting async confirmation — handle via webhook
        return { status: 'pending', transactionId: authorizeResponse.connectorTransactionId } as any;
    }

    // Step 2: Refund — return funds to the customer
    const refundResponse = await paymentClient.refund({
        "merchantRefundId": "probe_refund_001",
        "connectorTransactionId": authorizeResponse.connectorTransactionId,  // from authorize response
        "paymentAmount": 1000,
        "refundAmount": {
        },
        "reason": "customer_request"
    });

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
    const authorizeResponse = await paymentClient.authorize({
        "merchantTransactionId": "probe_txn_001",
        "amount": {
        },
        "paymentMethod": {
        },
        "captureMethod": "MANUAL",
        "customer": {
        },
        "address": {
        },
        "authType": "NO_THREE_DS",
        "returnUrl": "https://example.com/return"
    });

    if (authorizeResponse.status === types.PaymentStatus.FAILURE) {
        throw new Error(`Payment failed: ${JSON.stringify(authorizeResponse.error)}`);
    }
    if (authorizeResponse.status === types.PaymentStatus.PENDING) {
        // Awaiting async confirmation — handle via webhook
        return { status: 'pending', transactionId: authorizeResponse.connectorTransactionId } as any;
    }

    // Step 2: Void — release reserved funds (cancel authorization)
    const voidResponse = await paymentClient.void({
        "merchantVoidId": "probe_void_001",
        "connectorTransactionId": authorizeResponse.connectorTransactionId,  // from authorize response
        "cancellationReason": "requested_by_customer",
        "amount": {
        }
    });

    return { status: voidResponse.status, transactionId: authorizeResponse.connectorTransactionId!, error: voidResponse.error } as any;
}

// Get Payment Status
// Retrieve current payment status from the connector.
async function processGetPayment(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    // Step 1: Authorize — reserve funds on the payment method
    const authorizeResponse = await paymentClient.authorize({
        "merchantTransactionId": "probe_txn_001",
        "amount": {
        },
        "paymentMethod": {
        },
        "captureMethod": "MANUAL",
        "customer": {
        },
        "address": {
        },
        "authType": "NO_THREE_DS",
        "returnUrl": "https://example.com/return"
    });

    if (authorizeResponse.status === types.PaymentStatus.FAILURE) {
        throw new Error(`Payment failed: ${JSON.stringify(authorizeResponse.error)}`);
    }
    if (authorizeResponse.status === types.PaymentStatus.PENDING) {
        // Awaiting async confirmation — handle via webhook
        return { status: 'pending', transactionId: authorizeResponse.connectorTransactionId } as any;
    }

    // Step 2: Get — retrieve current payment status from the connector
    const getResponse = await paymentClient.get({
        "merchantTransactionId": "probe_merchant_txn_001",
        "connectorTransactionId": authorizeResponse.connectorTransactionId,  // from authorize response
        "amount": {
        }
    });

    return { status: getResponse.status, transactionId: getResponse.connectorTransactionId!, error: getResponse.error } as any;
}

// Flow: PaymentService.authenticate
async function authenticate(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    // Step 1: Authenticate — execute 3DS challenge or frictionless verification
    const authenticateResponse = await paymentClient.authenticate({
        "amount": {
        },
        "paymentMethod": {
        },
        "customer": {
        },
        "address": {
        },
        "returnUrl": "https://example.com/3ds-return",
        "continueRedirectionUrl": "https://example.com/3ds-continue",
        "redirectionResponse": {
        }
    });

    return authenticateResponse;
}

// Flow: PaymentService.authorize (Card)
async function authorize(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    // Step 1: Authorize — reserve funds on the payment method
    const authorizeResponse = await paymentClient.authorize({
        "merchantTransactionId": "probe_txn_001",
        "amount": {
        },
        "paymentMethod": {
        },
        "captureMethod": "AUTOMATIC",
        "customer": {
        },
        "address": {
        },
        "authType": "NO_THREE_DS",
        "returnUrl": "https://example.com/return"
    });

    if (authorizeResponse.status === types.PaymentStatus.FAILURE) {
        throw new Error(`Payment failed: ${JSON.stringify(authorizeResponse.error)}`);
    }
    if (authorizeResponse.status === types.PaymentStatus.PENDING) {
        // Awaiting async confirmation — handle via webhook
        return { status: 'pending', transactionId: authorizeResponse.connectorTransactionId } as any;
    }

    return authorizeResponse;
}

// Flow: PaymentService.capture
async function capture(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    // Step 1: Capture — settle the reserved funds
    const captureResponse = await paymentClient.capture({
        "merchantCaptureId": "probe_capture_001",
        "connectorTransactionId": "probe_connector_txn_001",
        "amountToCapture": {
        }
    });

    if (captureResponse.status === types.PaymentStatus.FAILURE) {
        throw new Error(`Capture failed: ${JSON.stringify(captureResponse.error)}`);
    }

    return captureResponse;
}

// Flow: PaymentService.get
async function get(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    // Step 1: Get — retrieve current payment status from the connector
    const getResponse = await paymentClient.get({
        "merchantTransactionId": "probe_merchant_txn_001",
        "connectorTransactionId": "probe_connector_txn_001",
        "amount": {
        }
    });

    return getResponse;
}

// Flow: PaymentService.post_authenticate
async function postAuthenticate(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    // Step 1: Post-Authenticate — validate authentication result with the issuing bank
    const postAuthenticateresponse = await paymentClient.postAuthenticate({
        "amount": {
        },
        "paymentMethod": {
        },
        "address": {
        },
        "redirectionResponse": {
        }
    });

    return postResponse;
}

// Flow: PaymentService.pre_authenticate
async function preAuthenticate(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    // Step 1: Pre-Authenticate — initiate 3DS flow (collect device/browser data)
    const preAuthenticateresponse = await paymentClient.preAuthenticate({
        "amount": {
        },
        "paymentMethod": {
        },
        "address": {
        },
        "enrolledFor_3ds": false,
        "returnUrl": "https://example.com/3ds-return"
    });

    return preResponse;
}

// Flow: PaymentService.proxy_authorize
async function proxyAuthorize(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    // Step 1: proxy_authorize
    const proxyResponse = await paymentClient.proxyAuthorize({
        "merchantTransactionId": "probe_proxy_txn_001",
        "amount": {
        },
        "cardProxy": {
        },
        "customer": {
        },
        "address": {
        },
        "captureMethod": "AUTOMATIC",
        "authType": "NO_THREE_DS",
        "returnUrl": "https://example.com/return"
    });

    return proxyResponse;
}

// Flow: PaymentService.recurring_charge
async function recurringCharge(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    // Step 1: Recurring Charge — charge against the stored mandate
    const recurringResponse = await paymentClient.charge({
        "connectorRecurringPaymentId": {
        },
        "amount": {
        },
        "paymentMethod": {
        },
        "returnUrl": "https://example.com/recurring-return",
        "connectorCustomerId": "cust_probe_123",
        "paymentMethodType": "PAY_PAL",
        "offSession": true
    });

    if (recurringResponse.status === types.PaymentStatus.FAILURE) {
        throw new Error(`Recurring Charge failed: ${JSON.stringify(recurringResponse.error)}`);
    }

    return recurringResponse;
}

// Flow: PaymentService.recurring_revoke
async function recurringRevoke(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    // Step 1: recurring_revoke
    const recurringResponse = await paymentClient.recurringRevoke({
        "merchantRevokeId": "probe_revoke_001",
        "mandateId": "probe_mandate_001",
        "connectorMandateId": "probe_connector_mandate_001"
    });

    return recurringResponse;
}

// Flow: PaymentService.refund
async function refund(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    // Step 1: Refund — return funds to the customer
    const refundResponse = await paymentClient.refund({
        "merchantRefundId": "probe_refund_001",
        "connectorTransactionId": "probe_connector_txn_001",
        "paymentAmount": 1000,
        "refundAmount": {
        },
        "reason": "customer_request"
    });

    if (refundResponse.status === types.RefundStatus.REFUND_FAILURE) {
        throw new Error(`Refund failed: ${JSON.stringify(refundResponse.error)}`);
    }

    return refundResponse;
}

// Flow: PaymentService.refund_get
async function refundGet(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    // Step 1: refund_get
    const refundResponse = await paymentClient.refundGet({
        "merchantRefundId": "probe_refund_001",
        "connectorTransactionId": "probe_connector_txn_001",
        "refundId": "probe_refund_id_001"
    });

    return refundResponse;
}

// Flow: PaymentService.token_authorize
async function tokenAuthorize(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    // Step 1: token_authorize
    const tokenResponse = await paymentClient.tokenAuthorize({
        "merchantTransactionId": "probe_tokenized_txn_001",
        "amount": {
        },
        "connectorToken": "pm_1AbcXyzStripeTestToken",
        "customer": {
        },
        "address": {
        },
        "captureMethod": "AUTOMATIC",
        "returnUrl": "https://example.com/return"
    });

    return tokenResponse;
}

// Flow: PaymentService.void
async function voidPayment(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    // Step 1: Void — release reserved funds (cancel authorization)
    const voidResponse = await paymentClient.void({
        "merchantVoidId": "probe_void_001",
        "connectorTransactionId": "probe_connector_txn_001",
        "cancellationReason": "requested_by_customer",
        "amount": {
        }
    });

    return voidResponse;
}


// Export all process* functions for the smoke test
export {
    processCheckoutAutocapture, processCheckoutCard, processRefund, processVoidPayment, processGetPayment, authenticate, authorize, capture, get, postAuthenticate, preAuthenticate, proxyAuthorize, recurringCharge, recurringRevoke, refund, refundGet, tokenAuthorize, voidPayment
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
