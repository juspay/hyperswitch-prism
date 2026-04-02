// helcim SDK Examples

function _buildauthorizeRequest(arg) {
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
        "billing_address": {
            "first_name": "John",
            "line1": "123 Main St",
            "zip_code": "98101"
        }
    },
    "auth_type": "NO_THREE_DS",
    "return_url": "https://example.com/return",
    "browser_info": {
        "ip_address": "1.2.3.4"
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

function _buildcaptureRequest(arg) {
    const payload = {
    "merchant_capture_id": "probe_capture_001",
    "connector_transaction_id": "12345",
    "amount_to_capture": {
        "minor_amount": 1000,
        "currency": "USD"
    },
    "browser_info": {
        "color_depth": 24,
        "screen_height": 900,
        "screen_width": 1440,
        "java_enabled": false,
        "java_script_enabled": true,
        "language": "en-US",
        "time_zone_offset_minutes": -480,
        "accept_header": "application/json",
        "user_agent": "Mozilla/5.0 (probe-bot)",
        "accept_language": "en-US,en;q=0.9",
        "ip_address": "1.2.3.4"
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

function _buildrefundRequest(arg) {
    const payload = {
    "merchant_refund_id": "probe_refund_001",
    "connector_transaction_id": "12345",
    "payment_amount": 1000,
    "refund_amount": {
        "minor_amount": 1000,
        "currency": "USD"
    },
    "reason": "customer_request",
    "browser_info": {
        "ip_address": "1.2.3.4"
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

function _buildvoidRequest(arg) {
    const payload = {
    "merchant_void_id": "probe_void_001",
    "connector_transaction_id": "12345",
    "browser_info": {
        "color_depth": 24,
        "screen_height": 900,
        "screen_width": 1440,
        "java_enabled": false,
        "java_script_enabled": true,
        "language": "en-US",
        "time_zone_offset_minutes": -480,
        "accept_header": "application/json",
        "user_agent": "Mozilla/5.0 (probe-bot)",
        "accept_language": "en-US,en;q=0.9",
        "ip_address": "1.2.3.4"
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
    "connector_transaction_id": "12345",
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
