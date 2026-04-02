# This file is auto-generated. Do not edit manually.
# Regenerate: python3 scripts/generators/docs/generate.py stripe --probe-path data/field_probe
# Stripe — integration scenarios

from google.protobuf.json_format import ParseDict
from payments import (
    PaymentClient,
    RecurringPaymentClient,
    PaymentMethodClient,
    MerchantAuthenticationClient,
    CustomerClient,
    PaymentMethodAuthenticationClient,
)
from payments.generated import sdk_config_pb2, payment_pb2

_default_config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
)
ConnectorConfig = sdk_config_pb2.ConnectorConfig

async def process_checkout_card(merchant_transaction_id: str, config: "ConnectorConfig" = _default_config):
    """
    Standard card authorization and capture flow
    """
    payment_client = PaymentClient(config)
    request = ParseDict({"merchant_transaction_id": "probe_txn_001", "amount": {"minor_amount": 1000, "currency": "USD"}, "payment_method": {"card": {"card_number": {"value": "4111111111111111"}, "card_exp_month": {"value": "03"}, "card_exp_year": {"value": "2030"}, "card_cvc": {"value": "737"}, "card_holder_name": {"value": "John Doe"}}}, "capture_method": "AUTOMATIC", "address": {"billing_address": {}}, "auth_type": "NO_THREE_DS", "return_url": "https://example.com/return"}, payment_pb2.PaymentServiceAuthorizeRequest())
    authorize_response = await payment_client.authorize(request)
    if authorize_response.status in ["FAILED", "AUTHORIZATION_FAILED"]:
        raise RuntimeError("Payment authorization failed: " + str(authorize_response.error) + "")
    if authorize_response.status == "PENDING":
        return { "status": getattr(authorize_response, "status", None), "transaction_id": getattr(authorize_response, "connector_transaction_id", None) }

    request = ParseDict({"merchant_capture_id": "probe_capture_001", "connector_transaction_id": authorize_response.connector_transaction_id, "amount_to_capture": {"minor_amount": 1000, "currency": "USD"}}, payment_pb2.PaymentServiceCaptureRequest())
    capture_response = await payment_client.capture(request)
    if capture_response.status == "FAILED":
        raise RuntimeError("Capture failed: " + str(capture_response.error) + "")

    return {
        "status": getattr(capture_response, "status", None),
        "transaction_id": getattr(capture_response, "connector_transaction_id", None),
        "amount": getattr(capture_response, "amount", None),
    }

async def process_checkout_bank(merchant_transaction_id: str, config: "ConnectorConfig" = _default_config):
    """
    Bank transfer or debit payment flow
    """
    payment_client = PaymentClient(config)
    request = ParseDict({"merchant_transaction_id": "probe_txn_001", "amount": {"minor_amount": 1000, "currency": "USD"}, "payment_method": {"ach": {"account_number": {"value": "000123456789"}, "routing_number": {"value": "110000000"}, "bank_account_holder_name": {"value": "John Doe"}}}, "capture_method": "AUTOMATIC", "address": {"billing_address": {}}, "auth_type": "NO_THREE_DS", "return_url": "https://example.com/return"}, payment_pb2.PaymentServiceAuthorizeRequest())
    authorize_response = await payment_client.authorize(request)
    if authorize_response.status == "FAILED":
        raise RuntimeError("Bank transfer failed: " + str(authorize_response.error) + "")

    return {
        "status": getattr(authorize_response, "status", None),
        "transaction_id": getattr(authorize_response, "connector_transaction_id", None),
    }

async def process_checkout_wallet(merchant_transaction_id: str, config: "ConnectorConfig" = _default_config):
    """
    Apple Pay, Google Pay, or other wallet payment
    """
    payment_client = PaymentClient(config)
    request = ParseDict({"merchant_transaction_id": "probe_txn_001", "amount": {"minor_amount": 1000, "currency": "USD"}, "payment_method": {"apple_pay": {"payment_data": {"encrypted_data": "eyJ2ZXJzaW9uIjoiRUNfdjEiLCJkYXRhIjoicHJvYmUiLCJzaWduYXR1cmUiOiJwcm9iZSJ9"}, "payment_method": {"display_name": "Visa 1111", "network": "Visa", "type": "debit"}, "transaction_identifier": "probe_txn_id"}}, "capture_method": "AUTOMATIC", "address": {"billing_address": {}}, "auth_type": "NO_THREE_DS", "return_url": "https://example.com/return", "payment_method_token": {"value": "probe_pm_token"}}, payment_pb2.PaymentServiceAuthorizeRequest())
    authorize_response = await payment_client.authorize(request)
    if authorize_response.status in ["FAILED", "AUTHORIZATION_FAILED"]:
        raise RuntimeError("Wallet payment failed: " + str(authorize_response.error) + "")

    request = ParseDict({"merchant_capture_id": "probe_capture_001", "connector_transaction_id": authorize_response.connector_transaction_id, "amount_to_capture": {"minor_amount": 1000, "currency": "USD"}}, payment_pb2.PaymentServiceCaptureRequest())
    capture_response = await payment_client.capture(request)

    return {
        "status": getattr(capture_response, "status", None),
        "transaction_id": getattr(capture_response, "connector_transaction_id", None),
    }

async def process_refund_payment(merchant_transaction_id: str, config: "ConnectorConfig" = _default_config):
    """
    Refund a completed payment
    """
    payment_client = PaymentClient(config)
    request = ParseDict({"merchant_refund_id": "probe_refund_001", "connector_transaction_id": authorize_response.connector_transaction_id, "payment_amount": 1000, "refund_amount": {"minor_amount": 1000, "currency": "USD"}, "reason": "customer_request"}, payment_pb2.PaymentServiceRefundRequest())
    refund_response = await payment_client.refund(request)
    if refund_response.status == "FAILED":
        raise RuntimeError("Refund failed: " + str(refund_response.error) + "")

    return {
        "status": getattr(refund_response, "status", None),
        "refund_id": getattr(refund_response, "connector_refund_id", None),
    }

async def process_setup_recurring(merchant_transaction_id: str, config: "ConnectorConfig" = _default_config):
    """
    Create a mandate for recurring charges
    """
    payment_client = PaymentClient(config)
    request = ParseDict({"merchant_recurring_payment_id": "probe_mandate_001", "amount": {"minor_amount": 0, "currency": "USD"}, "payment_method": {"card": {"card_number": {"value": "4111111111111111"}, "card_exp_month": {"value": "03"}, "card_exp_year": {"value": "2030"}, "card_cvc": {"value": "737"}, "card_holder_name": {"value": "John Doe"}}}, "address": {"billing_address": {}}, "auth_type": "NO_THREE_DS", "enrolled_for_3ds": False, "return_url": "https://example.com/mandate-return", "setup_future_usage": "OFF_SESSION", "request_incremental_authorization": False, "customer_acceptance": {"acceptance_type": "OFFLINE", "accepted_at": 0}}, payment_pb2.PaymentServiceSetupRecurringRequest())
    setup_recurring_response = await payment_client.setup_recurring(request)
    if setup_recurring_response.status == "FAILED":
        raise RuntimeError("Failed to setup recurring payment: " + str(setup_recurring_response.error) + "")

    return {
        "status": getattr(setup_recurring_response, "status", None),
        "mandate_id": getattr(setup_recurring_response, "mandate_reference.connector_mandate_id", None),
    }

async def process_recurring_charge(merchant_transaction_id: str, config: "ConnectorConfig" = _default_config):
    """
    Charge against an existing mandate
    """
    customer_client = CustomerClient(config)
    recurring_client = RecurringPaymentClient(config)
    # Prerequisite: Create customer profile
    request = ParseDict({"merchant_customer_id": "cust_probe_123", "customer_name": "John Doe", "email": {"value": "test@example.com"}, "phone_number": "4155552671"}, payment_pb2.CustomerServiceCreateRequest())
    create_customer_response = await customer_client.create_customer(request)

    request = ParseDict({"connector_recurring_payment_id": {"mandate_id_type": {"connector_mandate_id": {"connector_mandate_id": "probe-mandate-123"}}, "connector_mandate_id": {"connector_mandate_id": setup_recurring_response.mandate_reference}}, "amount": {"minor_amount": 1000, "currency": "USD"}, "payment_method": {"token": {"token": {"value": "probe_pm_token"}}}, "return_url": "https://example.com/recurring-return", "connector_customer_id": create_customer_response.connector_customer_id, "payment_method_type": "PAY_PAL", "off_session": True}, payment_pb2.RecurringPaymentServiceChargeRequest())
    recurring_charge_response = await recurring_client.charge(request)
    if recurring_charge_response.status == "FAILED":
        raise RuntimeError("Recurring charge failed: " + str(recurring_charge_response.error) + "")

    return {
        "status": getattr(recurring_charge_response, "status", None),
        "transaction_id": getattr(recurring_charge_response, "connector_transaction_id", None),
    }

async def process_tokenize_payment_method(merchant_transaction_id: str, config: "ConnectorConfig" = _default_config):
    """
    Tokenize a card or bank account for later use
    """
    payment_method_client = PaymentMethodClient(config)
    request = ParseDict({"amount": {"minor_amount": 1000, "currency": "USD"}, "payment_method": {"card": {"card_number": {"value": "4111111111111111"}, "card_exp_month": {"value": "03"}, "card_exp_year": {"value": "2030"}, "card_cvc": {"value": "737"}, "card_holder_name": {"value": "John Doe"}}}, "address": {"billing_address": {}}}, payment_pb2.PaymentMethodServiceTokenizeRequest())
    tokenize_response = await payment_method_client.tokenize(request)
    if tokenize_response.status == "FAILED":
        raise RuntimeError("Tokenization failed: " + str(tokenize_response.error) + "")

    return {
        "status": getattr(tokenize_response, "status", None),
        "token": getattr(tokenize_response, "payment_method_token", None),
    }

async def process_void_authorization(merchant_transaction_id: str, config: "ConnectorConfig" = _default_config):
    """
    Cancel an uncaptured authorization
    """
    payment_client = PaymentClient(config)
    request = ParseDict({"merchant_void_id": "probe_void_001", "connector_transaction_id": authorize_response.connector_transaction_id}, payment_pb2.PaymentServiceVoidRequest())
    void_response = await payment_client.void(request)
    if void_response.status == "FAILED":
        raise RuntimeError("Void failed: " + str(void_response.error) + "")

    return {
        "status": getattr(void_response, "status", None),
    }

async def process_get_payment_status(merchant_transaction_id: str, config: "ConnectorConfig" = _default_config):
    """
    Retrieve current status of a payment
    """
    payment_client = PaymentClient(config)
    request = ParseDict({"merchant_transaction_id": "probe_merchant_txn_001", "connector_transaction_id": authorize_response.connector_transaction_id, "amount": {"minor_amount": 1000, "currency": "USD"}}, payment_pb2.PaymentServiceGetRequest())
    get_response = await payment_client.get(request)

    return {
        "status": getattr(get_response, "status", None),
        "amount": getattr(get_response, "amount", None),
    }

async def process_partial_refund(merchant_transaction_id: str, config: "ConnectorConfig" = _default_config):
    """
    Refund a portion of a captured payment
    """
    payment_client = PaymentClient(config)
    request = ParseDict({"merchant_transaction_id": "probe_txn_001", "amount": {"minor_amount": 1000, "currency": "USD"}, "payment_method": {"card": {"card_number": {"value": "4111111111111111"}, "card_exp_month": {"value": "03"}, "card_exp_year": {"value": "2030"}, "card_cvc": {"value": "737"}, "card_holder_name": {"value": "John Doe"}}}, "capture_method": "AUTOMATIC", "address": {"billing_address": {}}, "auth_type": "NO_THREE_DS", "return_url": "https://example.com/return"}, payment_pb2.PaymentServiceAuthorizeRequest())
    authorize_response = await payment_client.authorize(request)
    if authorize_response.status in ["FAILED", "AUTHORIZATION_FAILED"]:
        raise RuntimeError("Payment authorization failed: " + str(authorize_response.error) + "")

    request = ParseDict({"merchant_capture_id": "probe_capture_001", "connector_transaction_id": authorize_response.connector_transaction_id, "amount_to_capture": {"minor_amount": 1000, "currency": "USD"}}, payment_pb2.PaymentServiceCaptureRequest())
    capture_response = await payment_client.capture(request)
    if capture_response.status == "FAILED":
        raise RuntimeError("Capture failed: " + str(capture_response.error) + "")

    request = ParseDict({"merchant_refund_id": "probe_refund_001", "connector_transaction_id": authorize_response.connector_transaction_id, "payment_amount": 1000, "refund_amount": {"minor_amount": 1000, "currency": "USD"}, "reason": "customer_request"}, payment_pb2.PaymentServiceRefundRequest())
    refund_response = await payment_client.refund(request)
    if refund_response.status == "FAILED":
        raise RuntimeError("Refund failed: " + str(refund_response.error) + "")

    return {
        "status": getattr(refund_response, "status", None),
        "refund_id": getattr(refund_response, "connector_refund_id", None),
        "refunded_amount": getattr(refund_response, "amount", None),
    }

async def process_multi_capture(merchant_transaction_id: str, config: "ConnectorConfig" = _default_config):
    """
    Split a single authorization into multiple captures (e.g., for split shipments)
    """
    payment_client = PaymentClient(config)
    request = ParseDict({"merchant_transaction_id": "probe_txn_001", "amount": {"minor_amount": 1000, "currency": "USD"}, "payment_method": {"card": {"card_number": {"value": "4111111111111111"}, "card_exp_month": {"value": "03"}, "card_exp_year": {"value": "2030"}, "card_cvc": {"value": "737"}, "card_holder_name": {"value": "John Doe"}}}, "capture_method": "AUTOMATIC", "address": {"billing_address": {}}, "auth_type": "NO_THREE_DS", "return_url": "https://example.com/return"}, payment_pb2.PaymentServiceAuthorizeRequest())
    authorize_response = await payment_client.authorize(request)
    if authorize_response.status in ["FAILED", "AUTHORIZATION_FAILED"]:
        raise RuntimeError("Payment authorization failed: " + str(authorize_response.error) + "")

    request = ParseDict({"merchant_capture_id": "probe_capture_001", "connector_transaction_id": authorize_response.connector_transaction_id, "amount_to_capture": {"minor_amount": 1000, "currency": "USD"}}, payment_pb2.PaymentServiceCaptureRequest())
    capture_response = await payment_client.capture(request)
    if capture_response.status == "FAILED":
        raise RuntimeError("Capture failed: " + str(capture_response.error) + "")

    return {
        "status": getattr(capture_response, "status", None),
        "transaction_id": getattr(capture_response, "connector_transaction_id", None),
        "captured_amount": getattr(capture_response, "amount", None),
    }

async def process_incremental_authorization(merchant_transaction_id: str, config: "ConnectorConfig" = _default_config):
    """
    Increase the authorized amount after initial authorization
    """
    payment_client = PaymentClient(config)
    request = ParseDict({"merchant_transaction_id": "probe_txn_001", "amount": {"minor_amount": 1000, "currency": "USD"}, "payment_method": {"card": {"card_number": {"value": "4111111111111111"}, "card_exp_month": {"value": "03"}, "card_exp_year": {"value": "2030"}, "card_cvc": {"value": "737"}, "card_holder_name": {"value": "John Doe"}}}, "capture_method": "AUTOMATIC", "address": {"billing_address": {}}, "auth_type": "NO_THREE_DS", "return_url": "https://example.com/return"}, payment_pb2.PaymentServiceAuthorizeRequest())
    authorize_response = await payment_client.authorize(request)
    if authorize_response.status in ["FAILED", "AUTHORIZATION_FAILED"]:
        raise RuntimeError("Payment authorization failed: " + str(authorize_response.error) + "")

    request = ParseDict({"merchant_capture_id": "probe_capture_001", "connector_transaction_id": authorize_response.connector_transaction_id, "amount_to_capture": {"minor_amount": 1000, "currency": "USD"}}, payment_pb2.PaymentServiceCaptureRequest())
    capture_response = await payment_client.capture(request)

    return {
        "status": getattr(capture_response, "status", None),
        "transaction_id": getattr(capture_response, "connector_transaction_id", None),
        "authorized_amount": getattr(capture_response, "amount", None),
    }

async def process_checkout_3ds(merchant_transaction_id: str, config: "ConnectorConfig" = _default_config):
    """
    Card payment with 3D Secure authentication
    """
    payment_client = PaymentClient(config)
    request = ParseDict({"merchant_transaction_id": "probe_txn_001", "amount": {"minor_amount": 1000, "currency": "USD"}, "payment_method": {"card": {"card_number": {"value": "4111111111111111"}, "card_exp_month": {"value": "03"}, "card_exp_year": {"value": "2030"}, "card_cvc": {"value": "737"}, "card_holder_name": {"value": "John Doe"}}}, "capture_method": "AUTOMATIC", "address": {"billing_address": {}}, "auth_type": "NO_THREE_DS", "return_url": "https://example.com/return"}, payment_pb2.PaymentServiceAuthorizeRequest())
    authorize_response = await payment_client.authorize(request)
    if authorize_response.status in ["FAILED", "AUTHORIZATION_FAILED"]:
        raise RuntimeError("Payment authorization failed: " + str(authorize_response.error) + "")
    if authorize_response.status == "PENDING_AUTHENTICATION":
        return { "status": getattr(authorize_response, "status", None), "transaction_id": getattr(authorize_response, "connector_transaction_id", None), "redirect_url": getattr(authorize_response, "next_action.redirect_url", None) }

    request = ParseDict({"merchant_capture_id": "probe_capture_001", "connector_transaction_id": authorize_response.connector_transaction_id, "amount_to_capture": {"minor_amount": 1000, "currency": "USD"}}, payment_pb2.PaymentServiceCaptureRequest())
    capture_response = await payment_client.capture(request)

    return {
        "status": getattr(capture_response, "status", None),
        "transaction_id": getattr(capture_response, "connector_transaction_id", None),
    }

async def process_checkout_bnpl(merchant_transaction_id: str, config: "ConnectorConfig" = _default_config):
    """
    Buy Now Pay Later payment flow (Klarna, Afterpay, Affirm)
    """
    payment_client = PaymentClient(config)
    request = ParseDict({"merchant_transaction_id": "probe_txn_001", "amount": {"minor_amount": 1000, "currency": "USD"}, "payment_method": {"klarna": {}}, "capture_method": "AUTOMATIC", "address": {"billing_address": {}}, "auth_type": "NO_THREE_DS", "return_url": "https://example.com/return"}, payment_pb2.PaymentServiceAuthorizeRequest())
    authorize_response = await payment_client.authorize(request)
    if authorize_response.status in ["FAILED", "AUTHORIZATION_FAILED"]:
        raise RuntimeError("BNPL authorization failed: " + str(authorize_response.error) + "")
    if authorize_response.status == "PENDING_AUTHENTICATION":
        return { "status": getattr(authorize_response, "status", None), "transaction_id": getattr(authorize_response, "connector_transaction_id", None), "redirect_url": getattr(authorize_response, "next_action.redirect_url", None) }

    request = ParseDict({"merchant_capture_id": "probe_capture_001", "connector_transaction_id": authorize_response.connector_transaction_id, "amount_to_capture": {"minor_amount": 1000, "currency": "USD"}}, payment_pb2.PaymentServiceCaptureRequest())
    capture_response = await payment_client.capture(request)
    if capture_response.status == "FAILED":
        raise RuntimeError("BNPL capture failed: " + str(capture_response.error) + "")

    return {
        "status": getattr(capture_response, "status", None),
        "transaction_id": getattr(capture_response, "connector_transaction_id", None),
    }
