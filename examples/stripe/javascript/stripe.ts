// stripe SDK Examples

function _buildAuthorizeRequest(arg) {
    const payload = {
    "merchant_transaction_id": "probe_txn_001",
    "amount": {
        "minor_amount": 1000,
        "currency": "USD"
    },
    "payment_method": {
        "card": {
            "card_number": "4111111111111111",
            "card_exp_month": "03",
            "card_exp_year": "2030",
            "card_cvc": "737",
            "card_holder_name": "John Doe"
        }
    },
    "capture_method": "AUTOMATIC",
    "address": {
        "billing_address": {}
    },
    "auth_type": "NO_THREE_DS",
    "return_url": "https://example.com/return"
};
    if (arg) {
        if (arg === 'AUTOMATIC' || arg === 'MANUAL') {
            payload.capture_method = arg;
        } else if (typeof arg === 'string') {
            payload.connector_transaction_id = arg;
        }
    }
    return payload;
}

function _buildCaptureRequest(arg) {
    const payload = {
    "merchant_capture_id": "probe_capture_001",
    "connector_transaction_id": "probe_connector_txn_001",
    "amount_to_capture": {
        "minor_amount": 1000,
        "currency": "USD"
    }
};
    if (arg) {
        if (arg === 'AUTOMATIC' || arg === 'MANUAL') {
            payload.capture_method = arg;
        } else if (typeof arg === 'string') {
            payload.connector_transaction_id = arg;
        }
    }
    return payload;
}

function _buildRefundRequest(arg) {
    const payload = {
    "merchant_refund_id": "probe_refund_001",
    "connector_transaction_id": "probe_connector_txn_001",
    "payment_amount": 1000,
    "refund_amount": {
        "minor_amount": 1000,
        "currency": "USD"
    },
    "reason": "customer_request"
};
    if (arg) {
        if (arg === 'AUTOMATIC' || arg === 'MANUAL') {
            payload.capture_method = arg;
        } else if (typeof arg === 'string') {
            payload.connector_transaction_id = arg;
        }
    }
    return payload;
}

function _buildSetupRecurringRequest(arg) {
    const payload = {
    "merchant_recurring_payment_id": "probe_mandate_001",
    "amount": {
        "minor_amount": 0,
        "currency": "USD"
    },
    "payment_method": {
        "card": {
            "card_number": "4111111111111111",
            "card_exp_month": "03",
            "card_exp_year": "2030",
            "card_cvc": "737",
            "card_holder_name": "John Doe"
        }
    },
    "address": {
        "billing_address": {}
    },
    "auth_type": "NO_THREE_DS",
    "enrolled_for_3ds": false,
    "return_url": "https://example.com/mandate-return",
    "setup_future_usage": "OFF_SESSION",
    "request_incremental_authorization": false,
    "customer_acceptance": {
        "acceptance_type": "OFFLINE",
        "accepted_at": 0
    }
};
    if (arg) {
        if (arg === 'AUTOMATIC' || arg === 'MANUAL') {
            payload.capture_method = arg;
        } else if (typeof arg === 'string') {
            payload.connector_transaction_id = arg;
        }
    }
    return payload;
}

function _buildRecurringChargeRequest(arg) {
    const payload = {
    "connector_recurring_payment_id": {
        "mandate_id_type": {
            "connector_mandate_id": {
                "connector_mandate_id": "probe-mandate-123"
            }
        }
    },
    "amount": {
        "minor_amount": 1000,
        "currency": "USD"
    },
    "payment_method": {
        "token": {
            "token": "probe_pm_token"
        }
    },
    "return_url": "https://example.com/recurring-return",
    "connector_customer_id": "cust_probe_123",
    "payment_method_type": "PAY_PAL",
    "off_session": true
};
    if (arg) {
        if (arg === 'AUTOMATIC' || arg === 'MANUAL') {
            payload.capture_method = arg;
        } else if (typeof arg === 'string') {
            payload.connector_transaction_id = arg;
        }
    }
    return payload;
}

function _buildTokenizeRequest(arg) {
    const payload = {
    "amount": {
        "minor_amount": 1000,
        "currency": "USD"
    },
    "payment_method": {
        "card": {
            "card_number": "4111111111111111",
            "card_exp_month": "03",
            "card_exp_year": "2030",
            "card_cvc": "737",
            "card_holder_name": "John Doe"
        }
    },
    "address": {
        "billing_address": {}
    }
};
    if (arg) {
        if (arg === 'AUTOMATIC' || arg === 'MANUAL') {
            payload.capture_method = arg;
        } else if (typeof arg === 'string') {
            payload.connector_transaction_id = arg;
        }
    }
    return payload;
}

function _buildVoidRequest(arg) {
    const payload = {
    "merchant_void_id": "probe_void_001",
    "connector_transaction_id": "probe_connector_txn_001"
};
    if (arg) {
        if (arg === 'AUTOMATIC' || arg === 'MANUAL') {
            payload.capture_method = arg;
        } else if (typeof arg === 'string') {
            payload.connector_transaction_id = arg;
        }
    }
    return payload;
}

function _buildGetRequest(arg) {
    const payload = {
    "merchant_transaction_id": "probe_merchant_txn_001",
    "connector_transaction_id": "probe_connector_txn_001",
    "amount": {
        "minor_amount": 1000,
        "currency": "USD"
    }
};
    if (arg) {
        if (arg === 'AUTOMATIC' || arg === 'MANUAL') {
            payload.capture_method = arg;
        } else if (typeof arg === 'string') {
            payload.connector_transaction_id = arg;
        }
    }
    return payload;
}

async function processCheckoutCard() {
    // Standard card authorization and capture flow
    const authorizeRequest = {merchant_transaction_id: "probe_txn_001", amount: {minor_amount: 1000, currency: "USD"}, payment_method: {card: {card_number: "4111111111111111", card_exp_month: "03", card_exp_year: "2030", card_cvc: "737", card_holder_name: "John Doe"}}, capture_method: "AUTOMATIC", address: {billing_address: {}}, auth_type: "NO_THREE_DS", return_url: "https://example.com/return"};
    const authorizeResponse = await client.authorize(authorizeRequest);
    if (["FAILED", "AUTHORIZATION_FAILED"].includes(authorizeResponse.status)) {
        throw new Error("Payment authorization failed: " + authorizeResponse.error?.message + "");
    }
    if (authorizeResponse.status === "PENDING") {
        return { status: authorizeResponse.status, transaction_id: authorizeResponse.connectorTransactionId };
    }

    const captureRequest = {merchant_capture_id: "probe_capture_001", connector_transaction_id: authorizeResponse.connectorTransactionId, amount_to_capture: {minor_amount: 1000, currency: "USD"}};
    const captureResponse = await client.capture(captureRequest);
    if (captureResponse.status === "FAILED") {
        throw new Error("Capture failed: " + captureResponse.error?.message + "");
    }

    return {
        status: captureResponse.status,
        transaction_id: captureResponse.connectorTransactionId,
        amount: captureResponse.amount,
    };
}

async function processCheckoutBank() {
    // Bank transfer or debit payment flow
    const authorizeRequest = {merchant_transaction_id: "probe_txn_001", amount: {minor_amount: 1000, currency: "USD"}, payment_method: {ach: {account_number: "000123456789", routing_number: "110000000", bank_account_holder_name: "John Doe"}}, capture_method: "AUTOMATIC", address: {billing_address: {}}, auth_type: "NO_THREE_DS", return_url: "https://example.com/return"};
    const authorizeResponse = await client.authorize(authorizeRequest);
    if (authorizeResponse.status === "FAILED") {
        throw new Error("Bank transfer failed: " + authorizeResponse.error?.message + "");
    }

    return {
        status: authorizeResponse.status,
        transaction_id: authorizeResponse.connectorTransactionId,
    };
}

async function processCheckoutWallet() {
    // Apple Pay, Google Pay, or other wallet payment
    const authorizeRequest = {merchant_transaction_id: "probe_txn_001", amount: {minor_amount: 1000, currency: "USD"}, payment_method: {apple_pay: {payment_data: {encrypted_data: "eyJ2ZXJzaW9uIjoiRUNfdjEiLCJkYXRhIjoicHJvYmUiLCJzaWduYXR1cmUiOiJwcm9iZSJ9"}, payment_method: {display_name: "Visa 1111", network: "Visa", type: "debit"}, transaction_identifier: "probe_txn_id"}}, capture_method: "AUTOMATIC", address: {billing_address: {}}, auth_type: "NO_THREE_DS", return_url: "https://example.com/return", payment_method_token: "probe_pm_token"};
    const authorizeResponse = await client.authorize(authorizeRequest);
    if (["FAILED", "AUTHORIZATION_FAILED"].includes(authorizeResponse.status)) {
        throw new Error("Wallet payment failed: " + authorizeResponse.error?.message + "");
    }

    const captureRequest = {merchant_capture_id: "probe_capture_001", connector_transaction_id: authorizeResponse.connectorTransactionId, amount_to_capture: {minor_amount: 1000, currency: "USD"}};
    const captureResponse = await client.capture(captureRequest);

    return {
        status: captureResponse.status,
        transaction_id: captureResponse.connectorTransactionId,
    };
}

async function processRefundPayment() {
    // Refund a completed payment
    const refundRequest = {merchant_refund_id: "probe_refund_001", connector_transaction_id: authorize_response.connectorTransactionId, payment_amount: 1000, refund_amount: {minor_amount: 1000, currency: "USD"}, reason: "customer_request"};
    const refundResponse = await client.refund(refundRequest);
    if (refundResponse.status === "FAILED") {
        throw new Error("Refund failed: " + refundResponse.error?.message + "");
    }

    return {
        status: refundResponse.status,
        refund_id: refundResponse.connectorRefundId,
    };
}

async function processSetupRecurring() {
    // Create a mandate for recurring charges
    const setupRecurringRequest = {merchant_recurring_payment_id: "probe_mandate_001", amount: {minor_amount: 0, currency: "USD"}, payment_method: {card: {card_number: "4111111111111111", card_exp_month: "03", card_exp_year: "2030", card_cvc: "737", card_holder_name: "John Doe"}}, address: {billing_address: {}}, auth_type: "NO_THREE_DS", enrolled_for_3ds: false, return_url: "https://example.com/mandate-return", setup_future_usage: "OFF_SESSION", request_incremental_authorization: false, customer_acceptance: {acceptance_type: "OFFLINE", accepted_at: 0}};
    const setupRecurringResponse = await client.setup_recurring(setupRecurringRequest);
    if (setupRecurringResponse.status === "FAILED") {
        throw new Error("Failed to setup recurring payment: " + setupRecurringResponse.error?.message + "");
    }

    return {
        status: setupRecurringResponse.status,
        mandate_id: setupRecurringResponse.mandateReference.connectorMandateId,
    };
}

async function processRecurringCharge() {
    // Charge against an existing mandate
    // Prerequisite: Create customer profile
    const request = {merchant_customer_id: "cust_probe_123", customer_name: "John Doe", email: "test@example.com", phone_number: "4155552671"};
    const createCustomerResponse = await client.create_customer(request);

    const recurringChargeRequest = {connector_recurring_payment_id: {mandate_id_type: {connector_mandate_id: {connector_mandate_id: "probe-mandate-123"}}, connector_mandate_id: {connector_mandate_id: setup_recurring_response.mandateReference}}, amount: {minor_amount: 1000, currency: "USD"}, payment_method: {token: {token: "probe_pm_token"}}, return_url: "https://example.com/recurring-return", connector_customer_id: create_customer_response.connectorCustomerId, payment_method_type: "PAY_PAL", off_session: true};
    const recurringChargeResponse = await client.recurring_charge(recurringChargeRequest);
    if (recurringChargeResponse.status === "FAILED") {
        throw new Error("Recurring charge failed: " + recurringChargeResponse.error?.message + "");
    }

    return {
        status: recurringChargeResponse.status,
        transaction_id: recurringChargeResponse.connectorTransactionId,
    };
}

async function processTokenizePaymentMethod() {
    // Tokenize a card or bank account for later use
    const tokenizeRequest = {amount: {minor_amount: 1000, currency: "USD"}, payment_method: {card: {card_number: "4111111111111111", card_exp_month: "03", card_exp_year: "2030", card_cvc: "737", card_holder_name: "John Doe"}}, address: {billing_address: {}}};
    const tokenizeResponse = await client.tokenize(tokenizeRequest);
    if (tokenizeResponse.status === "FAILED") {
        throw new Error("Tokenization failed: " + tokenizeResponse.error?.message + "");
    }

    return {
        status: tokenizeResponse.status,
        token: tokenizeResponse.paymentMethodToken,
    };
}

async function processVoidAuthorization() {
    // Cancel an uncaptured authorization
    const voidRequest = {merchant_void_id: "probe_void_001", connector_transaction_id: authorize_response.connectorTransactionId};
    const voidResponse = await client.void(voidRequest);
    if (voidResponse.status === "FAILED") {
        throw new Error("Void failed: " + voidResponse.error?.message + "");
    }

    return {
        status: voidResponse.status,
    };
}

async function processGetPaymentStatus() {
    // Retrieve current status of a payment
    const getRequest = {merchant_transaction_id: "probe_merchant_txn_001", connector_transaction_id: authorize_response.connectorTransactionId, amount: {minor_amount: 1000, currency: "USD"}};
    const getResponse = await client.get(getRequest);

    return {
        status: getResponse.status,
        amount: getResponse.amount,
    };
}

async function processPartialRefund() {
    // Refund a portion of a captured payment
    const authorizeRequest = {merchant_transaction_id: "probe_txn_001", amount: {minor_amount: 1000, currency: "USD"}, payment_method: {card: {card_number: "4111111111111111", card_exp_month: "03", card_exp_year: "2030", card_cvc: "737", card_holder_name: "John Doe"}}, capture_method: "AUTOMATIC", address: {billing_address: {}}, auth_type: "NO_THREE_DS", return_url: "https://example.com/return"};
    const authorizeResponse = await client.authorize(authorizeRequest);
    if (["FAILED", "AUTHORIZATION_FAILED"].includes(authorizeResponse.status)) {
        throw new Error("Payment authorization failed: " + authorizeResponse.error?.message + "");
    }

    const captureRequest = {merchant_capture_id: "probe_capture_001", connector_transaction_id: authorizeResponse.connectorTransactionId, amount_to_capture: {minor_amount: 1000, currency: "USD"}};
    const captureResponse = await client.capture(captureRequest);
    if (captureResponse.status === "FAILED") {
        throw new Error("Capture failed: " + captureResponse.error?.message + "");
    }

    const refundRequest = {merchant_refund_id: "probe_refund_001", connector_transaction_id: authorizeResponse.connectorTransactionId, payment_amount: 1000, refund_amount: {minor_amount: 1000, currency: "USD"}, reason: "customer_request"};
    const refundResponse = await client.refund(refundRequest);
    if (refundResponse.status === "FAILED") {
        throw new Error("Refund failed: " + refundResponse.error?.message + "");
    }

    return {
        status: refundResponse.status,
        refund_id: refundResponse.connectorRefundId,
        refunded_amount: refundResponse.amount,
    };
}

async function processMultiCapture() {
    // Split a single authorization into multiple captures (e.g., for split shipments)
    const authorizeRequest = {merchant_transaction_id: "probe_txn_001", amount: {minor_amount: 1000, currency: "USD"}, payment_method: {card: {card_number: "4111111111111111", card_exp_month: "03", card_exp_year: "2030", card_cvc: "737", card_holder_name: "John Doe"}}, capture_method: "AUTOMATIC", address: {billing_address: {}}, auth_type: "NO_THREE_DS", return_url: "https://example.com/return"};
    const authorizeResponse = await client.authorize(authorizeRequest);
    if (["FAILED", "AUTHORIZATION_FAILED"].includes(authorizeResponse.status)) {
        throw new Error("Payment authorization failed: " + authorizeResponse.error?.message + "");
    }

    const captureRequest = {merchant_capture_id: "probe_capture_001", connector_transaction_id: authorizeResponse.connectorTransactionId, amount_to_capture: {minor_amount: 1000, currency: "USD"}};
    const captureResponse = await client.capture(captureRequest);
    if (captureResponse.status === "FAILED") {
        throw new Error("Capture failed: " + captureResponse.error?.message + "");
    }

    return {
        status: captureResponse.status,
        transaction_id: captureResponse.connectorTransactionId,
        captured_amount: captureResponse.amount,
    };
}

async function processIncrementalAuthorization() {
    // Increase the authorized amount after initial authorization
    const authorizeRequest = {merchant_transaction_id: "probe_txn_001", amount: {minor_amount: 1000, currency: "USD"}, payment_method: {card: {card_number: "4111111111111111", card_exp_month: "03", card_exp_year: "2030", card_cvc: "737", card_holder_name: "John Doe"}}, capture_method: "AUTOMATIC", address: {billing_address: {}}, auth_type: "NO_THREE_DS", return_url: "https://example.com/return"};
    const authorizeResponse = await client.authorize(authorizeRequest);
    if (["FAILED", "AUTHORIZATION_FAILED"].includes(authorizeResponse.status)) {
        throw new Error("Payment authorization failed: " + authorizeResponse.error?.message + "");
    }

    const captureRequest = {merchant_capture_id: "probe_capture_001", connector_transaction_id: authorizeResponse.connectorTransactionId, amount_to_capture: {minor_amount: 1000, currency: "USD"}};
    const captureResponse = await client.capture(captureRequest);

    return {
        status: captureResponse.status,
        transaction_id: captureResponse.connectorTransactionId,
        authorized_amount: captureResponse.amount,
    };
}

async function processCheckout3ds() {
    // Card payment with 3D Secure authentication
    const authorizeRequest = {merchant_transaction_id: "probe_txn_001", amount: {minor_amount: 1000, currency: "USD"}, payment_method: {card: {card_number: "4111111111111111", card_exp_month: "03", card_exp_year: "2030", card_cvc: "737", card_holder_name: "John Doe"}}, capture_method: "AUTOMATIC", address: {billing_address: {}}, auth_type: "NO_THREE_DS", return_url: "https://example.com/return"};
    const authorizeResponse = await client.authorize(authorizeRequest);
    if (["FAILED", "AUTHORIZATION_FAILED"].includes(authorizeResponse.status)) {
        throw new Error("Payment authorization failed: " + authorizeResponse.error?.message + "");
    }
    if (authorizeResponse.status === "PENDING_AUTHENTICATION") {
        return { status: authorizeResponse.status, transaction_id: authorizeResponse.connectorTransactionId, redirect_url: authorizeResponse.nextAction.redirectUrl };
    }

    const captureRequest = {merchant_capture_id: "probe_capture_001", connector_transaction_id: authorizeResponse.connectorTransactionId, amount_to_capture: {minor_amount: 1000, currency: "USD"}};
    const captureResponse = await client.capture(captureRequest);

    return {
        status: captureResponse.status,
        transaction_id: captureResponse.connectorTransactionId,
    };
}

async function processCheckoutBnpl() {
    // Buy Now Pay Later payment flow (Klarna, Afterpay, Affirm)
    const authorizeRequest = {merchant_transaction_id: "probe_txn_001", amount: {minor_amount: 1000, currency: "USD"}, payment_method: {klarna: {}}, capture_method: "AUTOMATIC", address: {billing_address: {}}, auth_type: "NO_THREE_DS", return_url: "https://example.com/return"};
    const authorizeResponse = await client.authorize(authorizeRequest);
    if (["FAILED", "AUTHORIZATION_FAILED"].includes(authorizeResponse.status)) {
        throw new Error("BNPL authorization failed: " + authorizeResponse.error?.message + "");
    }
    if (authorizeResponse.status === "PENDING_AUTHENTICATION") {
        return { status: authorizeResponse.status, transaction_id: authorizeResponse.connectorTransactionId, redirect_url: authorizeResponse.nextAction.redirectUrl };
    }

    const captureRequest = {merchant_capture_id: "probe_capture_001", connector_transaction_id: authorizeResponse.connectorTransactionId, amount_to_capture: {minor_amount: 1000, currency: "USD"}};
    const captureResponse = await client.capture(captureRequest);
    if (captureResponse.status === "FAILED") {
        throw new Error("BNPL capture failed: " + captureResponse.error?.message + "");
    }

    return {
        status: captureResponse.status,
        transaction_id: captureResponse.connectorTransactionId,
    };
}
