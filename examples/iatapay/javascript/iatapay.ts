// iatapay SDK Examples

function _buildrefundRequest(arg) {
    const payload = {
    "merchant_refund_id": "probe_refund_001",
    "connector_transaction_id": "probe_connector_txn_001",
    "payment_amount": 1000,
    "refund_amount": {
        "minor_amount": 1000,
        "currency": "USD"
    },
    "reason": "customer_request",
    "webhook_url": "https://example.com/webhook",
    "state": {
        "access_token": {
            "token": "probe_access_token",
            "expires_in_seconds": 3600,
            "token_type": "Bearer"
        }
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

function _buildgetRequest(arg) {
    const payload = {
    "merchant_transaction_id": "probe_merchant_txn_001",
    "connector_transaction_id": "probe_connector_txn_001",
    "amount": {
        "minor_amount": 1000,
        "currency": "USD"
    },
    "state": {
        "access_token": {
            "token": "probe_access_token",
            "expires_in_seconds": 3600,
            "token_type": "Bearer"
        }
    },
    "connector_order_reference_id": "probe_order_ref_001"
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
