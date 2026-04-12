# This file is auto-generated. Do not edit manually.
# Replace YOUR_API_KEY and placeholder values with real data.
# Regenerate: python3 scripts/generate-connector-docs.py cybersource
#
# Cybersource — all integration scenarios and flows in one file.
# Run a scenario:  python3 cybersource.py checkout_card

import asyncio
import sys
from google.protobuf.json_format import ParseDict
from payments import PaymentClient
from payments import PaymentMethodAuthenticationClient
from payments import RecurringPaymentClient
from payments import RefundClient
from payments.generated import sdk_config_pb2, payment_pb2, payment_methods_pb2

_default_config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
)
# Standalone credentials (field names depend on connector auth type):
# _default_config.connector_config.CopyFrom(payment_pb2.ConnectorSpecificConfig(
#     cybersource=payment_pb2.CybersourceConfig(api_key=...),
# ))




def _build_authenticate_request():
    return ParseDict(
        {
            "amount": {  # Amount Information.
                "minor_amount": 1000,  # Amount in minor units (e.g., 1000 = $10.00).
                "currency": "USD"  # ISO 4217 currency code (e.g., "USD", "EUR").
            },
            "payment_method": {  # Payment Method.
                "card": {  # Generic card payment.
                    "card_number": {"value": "4111111111111111"},  # Card Identification.
                    "card_exp_month": {"value": "03"},
                    "card_exp_year": {"value": "2030"},
                    "card_cvc": {"value": "737"},
                    "card_holder_name": {"value": "John Doe"}  # Cardholder Information.
                }
            },
            "customer": {  # Customer Information.
                "email": {"value": "test@example.com"}  # Customer's email address.
            },
            "address": {  # Address Information.
                "billing_address": {
                }
            },
            "return_url": "https://example.com/3ds-return",  # URLs for Redirection.
            "continue_redirection_url": "https://example.com/3ds-continue",
            "redirection_response": {  # Redirection Information after DDC step.
                "params": "probe_redirect_params",
                "payload": {
                    "transaction_id": "probe_txn_123"
                }
            }
        },
        payment_pb2.PaymentMethodAuthenticationServiceAuthenticateRequest(),
    )

def _build_authorize_request(capture_method: str):
    return ParseDict(
        {
            "merchant_transaction_id": "probe_txn_001",  # Identification.
            "amount": {  # The amount for the payment.
                "minor_amount": 1000,  # Amount in minor units (e.g., 1000 = $10.00).
                "currency": "USD"  # ISO 4217 currency code (e.g., "USD", "EUR").
            },
            "payment_method": {  # Payment method to be used.
                "card": {  # Generic card payment.
                    "card_number": {"value": "4111111111111111"},  # Card Identification.
                    "card_exp_month": {"value": "03"},
                    "card_exp_year": {"value": "2030"},
                    "card_cvc": {"value": "737"},
                    "card_holder_name": {"value": "John Doe"}  # Cardholder Information.
                }
            },
            "capture_method": capture_method,  # Method for capturing the payment.
            "customer": {  # Customer Information.
                "email": {"value": "test@example.com"}  # Customer's email address.
            },
            "address": {  # Address Information.
                "billing_address": {
                }
            },
            "auth_type": "NO_THREE_DS",  # Authentication Details.
            "return_url": "https://example.com/return"  # URLs for Redirection and Webhooks.
        },
        payment_pb2.PaymentServiceAuthorizeRequest(),
    )

def _build_capture_request(connector_transaction_id: str):
    return ParseDict(
        {
            "merchant_capture_id": "probe_capture_001",  # Identification.
            "connector_transaction_id": connector_transaction_id,
            "amount_to_capture": {  # Capture Details.
                "minor_amount": 1000,  # Amount in minor units (e.g., 1000 = $10.00).
                "currency": "USD"  # ISO 4217 currency code (e.g., "USD", "EUR").
            }
        },
        payment_pb2.PaymentServiceCaptureRequest(),
    )

def _build_get_request(connector_transaction_id: str):
    return ParseDict(
        {
            "merchant_transaction_id": "probe_merchant_txn_001",  # Identification.
            "connector_transaction_id": connector_transaction_id,
            "amount": {  # Amount Information.
                "minor_amount": 1000,  # Amount in minor units (e.g., 1000 = $10.00).
                "currency": "USD"  # ISO 4217 currency code (e.g., "USD", "EUR").
            }
        },
        payment_pb2.PaymentServiceGetRequest(),
    )

def _build_post_authenticate_request():
    return ParseDict(
        {
            "amount": {  # Amount Information.
                "minor_amount": 1000,  # Amount in minor units (e.g., 1000 = $10.00).
                "currency": "USD"  # ISO 4217 currency code (e.g., "USD", "EUR").
            },
            "payment_method": {  # Payment Method.
                "card": {  # Generic card payment.
                    "card_number": {"value": "4111111111111111"},  # Card Identification.
                    "card_exp_month": {"value": "03"},
                    "card_exp_year": {"value": "2030"},
                    "card_cvc": {"value": "737"},
                    "card_holder_name": {"value": "John Doe"}  # Cardholder Information.
                }
            },
            "address": {  # Address Information.
                "billing_address": {
                }
            },
            "redirection_response": {  # Redirection Information after DDC step.
                "params": "probe_redirect_params",
                "payload": {
                    "transaction_id": "probe_txn_123"
                }
            }
        },
        payment_pb2.PaymentMethodAuthenticationServicePostAuthenticateRequest(),
    )

def _build_pre_authenticate_request():
    return ParseDict(
        {
            "amount": {  # Amount Information.
                "minor_amount": 1000,  # Amount in minor units (e.g., 1000 = $10.00).
                "currency": "USD"  # ISO 4217 currency code (e.g., "USD", "EUR").
            },
            "payment_method": {  # Payment Method.
                "card": {  # Generic card payment.
                    "card_number": {"value": "4111111111111111"},  # Card Identification.
                    "card_exp_month": {"value": "03"},
                    "card_exp_year": {"value": "2030"},
                    "card_cvc": {"value": "737"},
                    "card_holder_name": {"value": "John Doe"}  # Cardholder Information.
                }
            },
            "address": {  # Address Information.
                "billing_address": {
                }
            },
            "enrolled_for_3ds": False,  # Authentication Details.
            "return_url": "https://example.com/3ds-return"  # URLs for Redirection.
        },
        payment_pb2.PaymentMethodAuthenticationServicePreAuthenticateRequest(),
    )

def _build_proxy_authorize_request():
    return ParseDict(
        {
            "merchant_transaction_id": "probe_proxy_txn_001",
            "amount": {
                "minor_amount": 1000,  # Amount in minor units (e.g., 1000 = $10.00).
                "currency": "USD"  # ISO 4217 currency code (e.g., "USD", "EUR").
            },
            "card_proxy": {  # Card proxy for vault-aliased payments (VGS, Basis Theory, Spreedly). Real card values are substituted by the proxy before reaching the connector.
                "card_number": {"value": "4111111111111111"},  # Card Identification.
                "card_exp_month": {"value": "03"},
                "card_exp_year": {"value": "2030"},
                "card_cvc": {"value": "123"},
                "card_holder_name": {"value": "John Doe"}  # Cardholder Information.
            },
            "customer": {
                "email": {"value": "test@example.com"}  # Customer's email address.
            },
            "address": {
                "billing_address": {
                }
            },
            "capture_method": "AUTOMATIC",
            "auth_type": "NO_THREE_DS",
            "return_url": "https://example.com/return"
        },
        payment_pb2.PaymentServiceProxyAuthorizeRequest(),
    )

def _build_recurring_charge_request():
    return ParseDict(
        {
            "connector_recurring_payment_id": {  # Reference to existing mandate.
                "connector_mandate_id": {  # mandate_id sent by the connector.
                    "connector_mandate_id": "probe-mandate-123"
                }
            },
            "amount": {  # Amount Information.
                "minor_amount": 1000,  # Amount in minor units (e.g., 1000 = $10.00).
                "currency": "USD"  # ISO 4217 currency code (e.g., "USD", "EUR").
            },
            "payment_method": {  # Optional payment Method Information (for network transaction flows).
                "token": {  # Payment tokens.
                    "token": {"value": "probe_pm_token"}  # The token string representing a payment method.
                }
            },
            "return_url": "https://example.com/recurring-return",
            "connector_customer_id": "cust_probe_123",
            "payment_method_type": "PAY_PAL",
            "off_session": True  # Behavioral Flags and Preferences.
        },
        payment_pb2.RecurringPaymentServiceChargeRequest(),
    )

def _build_recurring_revoke_request():
    return ParseDict(
        {
            "merchant_revoke_id": "probe_revoke_001",  # Identification.
            "merchant_mandate_id": "probe_mandate_001",  # Mandate Details Merchant-side identifier for the mandate being revoked.
            "mandate_reference_id": {  # Typed mandate reference supporting connector mandate ids, network transaction ids, and network-token-with-NTI references. Preferred over the legacy `connector_mandate_id` field above.
                "connector_mandate_id": {  # mandate_id sent by the connector.
                    "connector_mandate_id": "probe_connector_mandate_001"
                }
            }
        },
        payment_pb2.RecurringPaymentServiceRevokeRequest(),
    )

def _build_refund_request(connector_transaction_id: str):
    return ParseDict(
        {
            "merchant_refund_id": "probe_refund_001",  # Identification.
            "connector_transaction_id": connector_transaction_id,
            "payment_amount": 1000,  # Amount Information.
            "refund_amount": {
                "minor_amount": 1000,  # Amount in minor units (e.g., 1000 = $10.00).
                "currency": "USD"  # ISO 4217 currency code (e.g., "USD", "EUR").
            },
            "reason": "customer_request"  # Reason for the refund.
        },
        payment_pb2.PaymentServiceRefundRequest(),
    )

def _build_refund_get_request():
    return ParseDict(
        {
            "merchant_refund_id": "probe_refund_001",  # Identification.
            "connector_transaction_id": "probe_connector_txn_001",
            "refund_id": "probe_refund_id_001"
        },
        payment_pb2.RefundServiceGetRequest(),
    )

def _build_token_authorize_request():
    return ParseDict(
        {
            "merchant_transaction_id": "probe_tokenized_txn_001",
            "amount": {
                "minor_amount": 1000,  # Amount in minor units (e.g., 1000 = $10.00).
                "currency": "USD"  # ISO 4217 currency code (e.g., "USD", "EUR").
            },
            "connector_token": {"value": "pm_1AbcXyzStripeTestToken"},  # Connector-issued token. Replaces PaymentMethod entirely. Examples: Stripe pm_xxx, Adyen recurringDetailReference, Braintree nonce.
            "customer": {
                "email": {"value": "test@example.com"}  # Customer's email address.
            },
            "address": {
                "billing_address": {
                }
            },
            "capture_method": "AUTOMATIC",
            "return_url": "https://example.com/return"
        },
        payment_pb2.PaymentServiceTokenAuthorizeRequest(),
    )

def _build_void_request(connector_transaction_id: str):
    return ParseDict(
        {
            "merchant_void_id": "probe_void_001",  # Identification.
            "connector_transaction_id": connector_transaction_id,
            "cancellation_reason": "requested_by_customer",  # Void Details.
            "amount": {  # Amount Information.
                "minor_amount": 1000,  # Amount in minor units (e.g., 1000 = $10.00).
                "currency": "USD"  # ISO 4217 currency code (e.g., "USD", "EUR").
            }
        },
        payment_pb2.PaymentServiceVoidRequest(),
    )
async def process_checkout_autocapture(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """One-step Payment (Authorize + Capture)

    Simple payment that authorizes and captures in one call. Use for immediate charges.
    """
    payment_client = PaymentClient(config)

    # Step 1: Authorize — reserve funds on the payment method
    authorize_response = await payment_client.authorize(_build_authorize_request("AUTOMATIC"))

    if authorize_response.status == "FAILED":
        raise RuntimeError(f"Payment failed: {authorize_response.error}")
    if authorize_response.status == "PENDING":
        # Awaiting async confirmation — handle via webhook
        return {"status": "pending", "transaction_id": authorize_response.connector_transaction_id}

    return {"status": getattr(authorize_response, "status", ""), "transaction_id": getattr(authorize_response, "connector_transaction_id", ""), "error": getattr(authorize_response, "error", None)}


async def process_checkout_card(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Card Payment (Authorize + Capture)

    Two-step card payment. First authorize, then capture. Use when you need to verify funds before finalizing.
    """
    payment_client = PaymentClient(config)

    # Step 1: Authorize — reserve funds on the payment method
    authorize_response = await payment_client.authorize(_build_authorize_request("MANUAL"))

    if authorize_response.status == "FAILED":
        raise RuntimeError(f"Payment failed: {authorize_response.error}")
    if authorize_response.status == "PENDING":
        # Awaiting async confirmation — handle via webhook
        return {"status": "pending", "transaction_id": authorize_response.connector_transaction_id}

    # Step 2: Capture — settle the reserved funds
    capture_response = await payment_client.capture(_build_capture_request(authorize_response.connector_transaction_id))

    if capture_response.status == "FAILED":
        raise RuntimeError(f"Capture failed: {capture_response.error}")

    return {"status": getattr(capture_response, "status", ""), "transaction_id": getattr(authorize_response, "connector_transaction_id", ""), "error": getattr(capture_response, "error", None)}


async def process_refund(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Refund

    Return funds to the customer for a completed payment.
    """
    payment_client = PaymentClient(config)

    # Step 1: Authorize — reserve funds on the payment method
    authorize_response = await payment_client.authorize(_build_authorize_request("AUTOMATIC"))

    if authorize_response.status == "FAILED":
        raise RuntimeError(f"Payment failed: {authorize_response.error}")
    if authorize_response.status == "PENDING":
        # Awaiting async confirmation — handle via webhook
        return {"status": "pending", "transaction_id": authorize_response.connector_transaction_id}

    # Step 2: Refund — return funds to the customer
    refund_response = await payment_client.refund(_build_refund_request(authorize_response.connector_transaction_id))

    if refund_response.status == "FAILED":
        raise RuntimeError(f"Refund failed: {refund_response.error}")

    return {"status": getattr(refund_response, "status", ""), "error": getattr(refund_response, "error", None)}


async def process_void_payment(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Void Payment

    Cancel an authorized but not-yet-captured payment.
    """
    payment_client = PaymentClient(config)

    # Step 1: Authorize — reserve funds on the payment method
    authorize_response = await payment_client.authorize(_build_authorize_request("MANUAL"))

    if authorize_response.status == "FAILED":
        raise RuntimeError(f"Payment failed: {authorize_response.error}")
    if authorize_response.status == "PENDING":
        # Awaiting async confirmation — handle via webhook
        return {"status": "pending", "transaction_id": authorize_response.connector_transaction_id}

    # Step 2: Void — release reserved funds (cancel authorization)
    void_response = await payment_client.void(_build_void_request(authorize_response.connector_transaction_id))

    return {"status": getattr(void_response, "status", ""), "transaction_id": getattr(authorize_response, "connector_transaction_id", ""), "error": getattr(void_response, "error", None)}


async def process_get_payment(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Get Payment Status

    Retrieve current payment status from the connector.
    """
    payment_client = PaymentClient(config)

    # Step 1: Authorize — reserve funds on the payment method
    authorize_response = await payment_client.authorize(_build_authorize_request("MANUAL"))

    if authorize_response.status == "FAILED":
        raise RuntimeError(f"Payment failed: {authorize_response.error}")
    if authorize_response.status == "PENDING":
        # Awaiting async confirmation — handle via webhook
        return {"status": "pending", "transaction_id": authorize_response.connector_transaction_id}

    # Step 2: Get — retrieve current payment status from the connector
    get_response = await payment_client.get(_build_get_request(authorize_response.connector_transaction_id))

    return {"status": getattr(get_response, "status", ""), "transaction_id": getattr(get_response, "connector_transaction_id", ""), "error": getattr(get_response, "error", None)}


async def authenticate(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentMethodAuthenticationService.Authenticate"""
    paymentmethodauthentication_client = PaymentMethodAuthenticationClient(config)

    authenticate_response = await paymentmethodauthentication_client.authenticate(_build_authenticate_request())

    return {"status": authenticate_response.status}


async def authorize(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.Authorize (Card)"""
    payment_client = PaymentClient(config)

    authorize_response = await payment_client.authorize(_build_authorize_request("AUTOMATIC"))

    return {"status": authorize_response.status, "transaction_id": authorize_response.connector_transaction_id}


async def capture(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.Capture"""
    payment_client = PaymentClient(config)

    capture_response = await payment_client.capture(_build_capture_request("probe_connector_txn_001"))

    return {"status": capture_response.status}


async def get(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.Get"""
    payment_client = PaymentClient(config)

    get_response = await payment_client.get(_build_get_request("probe_connector_txn_001"))

    return {"status": get_response.status}


async def post_authenticate(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentMethodAuthenticationService.PostAuthenticate"""
    paymentmethodauthentication_client = PaymentMethodAuthenticationClient(config)

    post_response = await paymentmethodauthentication_client.post_authenticate(_build_post_authenticate_request())

    return {"status": post_response.status}


async def pre_authenticate(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentMethodAuthenticationService.PreAuthenticate"""
    paymentmethodauthentication_client = PaymentMethodAuthenticationClient(config)

    pre_response = await paymentmethodauthentication_client.pre_authenticate(_build_pre_authenticate_request())

    return {"status": pre_response.status}


async def proxy_authorize(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.ProxyAuthorize"""
    payment_client = PaymentClient(config)

    proxy_response = await payment_client.proxy_authorize(_build_proxy_authorize_request())

    return {"status": proxy_response.status}


async def recurring_charge(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: RecurringPaymentService.Charge"""
    recurringpayment_client = RecurringPaymentClient(config)

    recurring_response = await recurringpayment_client.charge(_build_recurring_charge_request())

    return {"status": recurring_response.status}


async def recurring_revoke(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: RecurringPaymentService.Revoke"""
    recurringpayment_client = RecurringPaymentClient(config)

    recurring_response = await recurringpayment_client.recurring_revoke(_build_recurring_revoke_request())

    return {"status": recurring_response.status}


async def refund(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.Refund"""
    payment_client = PaymentClient(config)

    refund_response = await payment_client.refund(_build_refund_request("probe_connector_txn_001"))

    return {"status": refund_response.status}


async def refund_get(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: RefundService.Get"""
    refund_client = RefundClient(config)

    refund_response = await refund_client.refund_get(_build_refund_get_request())

    return {"status": refund_response.status}


async def token_authorize(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.TokenAuthorize"""
    payment_client = PaymentClient(config)

    token_response = await payment_client.token_authorize(_build_token_authorize_request())

    return {"status": token_response.status}


async def void(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.Void"""
    payment_client = PaymentClient(config)

    void_response = await payment_client.void(_build_void_request("probe_connector_txn_001"))

    return {"status": void_response.status}

if __name__ == "__main__":
    scenario = sys.argv[1] if len(sys.argv) > 1 else "checkout_autocapture"
    fn = globals().get(f"process_{scenario}")
    if not fn:
        available = [k[8:] for k in globals() if k.startswith("process_")]
        print(f"Unknown scenario: {scenario}. Available: {available}", file=sys.stderr)
        sys.exit(1)
    asyncio.run(fn("order_001"))
