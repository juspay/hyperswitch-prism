# This file is auto-generated. Do not edit manually.
# Replace YOUR_API_KEY and placeholder values with real data.
# Regenerate: python3 scripts/generate-connector-docs.py braintree
#
# Braintree — all integration scenarios and flows in one file.
# Run a scenario:  python3 braintree.py checkout_card

import asyncio
import sys
from payments import PaymentClient
from payments import MerchantAuthenticationClient
from payments import EventClient
from payments import PaymentMethodAuthenticationClient
from payments import RecurringPaymentClient
from payments import RefundClient
from payments.generated import sdk_config_pb2, payment_pb2, events_pb2, payment_methods_pb2

SUPPORTED_FLOWS = ["capture", "create_client_authentication_token", "get", "parse_event", "post_authenticate", "pre_authenticate", "recurring_revoke", "refund", "refund_get", "reverse", "token_authorize", "token_setup_recurring", "void"]

_default_config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
    connector_config=payment_pb2.ConnectorSpecificConfig(
        braintree=payment_pb2.BraintreeConfig(
            public_key=payment_methods_pb2.SecretString(value="YOUR_PUBLIC_KEY"),
            private_key=payment_methods_pb2.SecretString(value="YOUR_PRIVATE_KEY"),
            base_url="YOUR_BASE_URL",
            merchant_account_id=payment_methods_pb2.SecretString(value="YOUR_MERCHANT_ACCOUNT_ID"),
            merchant_config_currency="YOUR_MERCHANT_CONFIG_CURRENCY",
            apple_pay_supported_networks=["YOUR_APPLE_PAY_SUPPORTED_NETWORKS"],
            apple_pay_merchant_capabilities=["YOUR_APPLE_PAY_MERCHANT_CAPABILITIES"],
            apple_pay_label="YOUR_APPLE_PAY_LABEL",
            gpay_merchant_name="YOUR_GPAY_MERCHANT_NAME",
            gpay_merchant_id="YOUR_GPAY_MERCHANT_ID",
            gpay_allowed_auth_methods=["YOUR_GPAY_ALLOWED_AUTH_METHODS"],
            gpay_allowed_card_networks=["YOUR_GPAY_ALLOWED_CARD_NETWORKS"],
            paypal_client_id="YOUR_PAYPAL_CLIENT_ID",
            gpay_gateway_merchant_id="YOUR_GPAY_GATEWAY_MERCHANT_ID",
        ),
    ),
)




def _build_capture_request(connector_transaction_id: str):
    return payment_pb2.PaymentServiceCaptureRequest(
        merchant_capture_id="probe_capture_001",  # Identification.
        connector_transaction_id=connector_transaction_id,
        amount_to_capture=payment_pb2.Money(  # Capture Details.
            minor_amount=1000,  # Amount in minor units (e.g., 1000 = $10.00).
            currency=payment_pb2.Currency.Value("USD"),  # ISO 4217 currency code (e.g., "USD", "EUR").
        ),
    )

def _build_create_client_authentication_token_request():
    return payment_pb2.MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest(
        merchant_client_session_id="probe_sdk_session_001",  # Infrastructure.
        payment=payment_pb2.PaymentClientAuthenticationContext(
            amount=payment_pb2.Money(
                minor_amount=1000,  # Amount in minor units (e.g., 1000 = $10.00).
                currency=payment_pb2.Currency.Value("USD"),  # ISO 4217 currency code (e.g., "USD", "EUR").
            ),
        ),
    )

def _build_get_request(connector_transaction_id: str):
    return payment_pb2.PaymentServiceGetRequest(
        merchant_transaction_id="probe_merchant_txn_001",  # Identification.
        connector_transaction_id=connector_transaction_id,
        amount=payment_pb2.Money(  # Amount Information.
            minor_amount=1000,  # Amount in minor units (e.g., 1000 = $10.00).
            currency=payment_pb2.Currency.Value("USD"),  # ISO 4217 currency code (e.g., "USD", "EUR").
        ),
    )

def _build_parse_event_request():
    return events_pb2.EventServiceParseRequest(
        request_details=payment_pb2.RequestDetails(
            method=payment_pb2.HttpMethod.Value("HTTP_METHOD_POST"),  # HTTP method of the request (e.g., GET, POST).
            uri="https://example.com/webhook",  # URI of the request.
            headers={},  # Headers of the HTTP request.
            body="bt_signature=dummy_public_key%7Cdummy_signature&bt_payload=PG5vdGlmaWNhdGlvbj48dGltZXN0YW1wIHR5cGU9ImRhdGV0aW1lIj4yMDI2LTA5LTE2VDAwOjAwOjAwWjwvdGltZXN0YW1wPjxraW5kPmRpc3B1dGVfb3BlbmVkPC9raW5kPjxzdWJqZWN0PjxkaXNwdXRlPjxpZD5kdW1teV9kaXNwdXRlX2lkXzAwMTwvaWQ%2BPGFtb3VudD4xMC4wMDwvYW1vdW50PjxhbW91bnQtZGlzcHV0ZWQ%2BMTAuMDA8L2Ftb3VudC1kaXNwdXRlZD48YW1vdW50LXdvbiBuaWw9InRydWUiLz48Y2FzZS1udW1iZXI%2BQ0FTRS0wMDE8L2Nhc2UtbnVtYmVyPjxjcmVhdGVkLWF0IHR5cGU9ImRhdGV0aW1lIj4yMDI2LTA5LTE2VDAwOjAwOjAwWjwvY3JlYXRlZC1hdD48Y3VycmVuY3ktaXNvLWNvZGU%2BVVNEPC9jdXJyZW5jeS1pc28tY29kZT48Zm9yd2FyZGVkLWNvbW1lbnRzIG5pbD0idHJ1ZSIvPjxraW5kPmNoYXJnZWJhY2s8L2tpbmQ%2BPG1lcmNoYW50LWFjY291bnQtaWQ%2BZHVtbXlfbWVyY2hhbnRfYWNjb3VudDwvbWVyY2hhbnQtYWNjb3VudC1pZD48cmVhc29uPmZyYXVkPC9yZWFzb24%2BPHJlYXNvbi1jb2RlIG5pbD0idHJ1ZSIvPjxyZWNlaXZlZC1kYXRlIHR5cGU9ImRhdGUiPjIwMjYtMDktMTY8L3JlY2VpdmVkLWRhdGU%2BPHJlZmVyZW5jZS1udW1iZXI%2BUkVGLTAwMTwvcmVmZXJlbmNlLW51bWJlcj48cmVwbHktYnktZGF0ZSB0eXBlPSJkYXRlIj4yMDI2LTA5LTMwPC9yZXBseS1ieS1kYXRlPjxzdGF0dXM%2Bb3Blbjwvc3RhdHVzPjx1cGRhdGVkLWF0IHR5cGU9ImRhdGV0aW1lIj4yMDI2LTA5LTE2VDAwOjAwOjAwWjwvdXBkYXRlZC1hdD48c3RhdHVzLWhpc3RvcnkgdHlwZT0iYXJyYXkiPjxzdGF0dXMtaGlzdG9yeT48c3RhdHVzPm9wZW48L3N0YXR1cz48dGltZXN0YW1wIHR5cGU9ImRhdGV0aW1lIj4yMDI2LTA5LTE2VDAwOjAwOjAwWjwvdGltZXN0YW1wPjwvc3RhdHVzLWhpc3Rvcnk%2BPC9zdGF0dXMtaGlzdG9yeT48ZXZpZGVuY2UgdHlwZT0iYXJyYXkiLz48dHJhbnNhY3Rpb24%2BPGlkPmR1bW15X3R4bl9pZF8wMDE8L2lkPjxhbW91bnQ%2BMTAuMDA8L2Ftb3VudD48b3JkZXItaWQ%2BZHVtbXlfb3JkZXJfMDAxPC9vcmRlci1pZD48cGF5bWVudC1pbnN0cnVtZW50LXR5cGU%2BY3JlZGl0X2NhcmQ8L3BheW1lbnQtaW5zdHJ1bWVudC10eXBlPjwvdHJhbnNhY3Rpb24%2BPGRhdGUtb3BlbmVkIHR5cGU9ImRhdGUiPjIwMjYtMDktMTY8L2RhdGUtb3BlbmVkPjwvZGlzcHV0ZT48L3N1YmplY3Q%2BPC9ub3RpZmljYXRpb24%2B".encode(),  # Body of the HTTP request.
        ),
    )

def _build_post_authenticate_request():
    return payment_pb2.PaymentMethodAuthenticationServicePostAuthenticateRequest(
        amount=payment_pb2.Money(  # Amount Information.
            minor_amount=1000,  # Amount in minor units (e.g., 1000 = $10.00).
            currency=payment_pb2.Currency.Value("USD"),  # ISO 4217 currency code (e.g., "USD", "EUR").
        ),
        payment_method=payment_methods_pb2.PaymentMethod(  # Payment Method.
            card=payment_methods_pb2.CardDetails(
                card_number=payment_methods_pb2.CardNumberType(value="4111111111111111"),  # Card Identification.
                card_exp_month=payment_methods_pb2.SecretString(value="03"),
                card_exp_year=payment_methods_pb2.SecretString(value="2030"),
                card_cvc=payment_methods_pb2.SecretString(value="737"),
                card_holder_name=payment_methods_pb2.SecretString(value="John Doe"),  # Cardholder Information.
            ),
        ),
        address=payment_pb2.PaymentAddress(  # Address Information.
            billing_address=payment_pb2.Address(),
        ),
        connector_order_reference_id="probe_order_ref_001",
    )

def _build_pre_authenticate_request():
    return payment_pb2.PaymentMethodAuthenticationServicePreAuthenticateRequest(
        amount=payment_pb2.Money(  # Amount Information.
            minor_amount=1000,  # Amount in minor units (e.g., 1000 = $10.00).
            currency=payment_pb2.Currency.Value("USD"),  # ISO 4217 currency code (e.g., "USD", "EUR").
        ),
        payment_method=payment_methods_pb2.PaymentMethod(  # Payment Method.
            card=payment_methods_pb2.CardDetails(
                card_number=payment_methods_pb2.CardNumberType(value="4111111111111111"),  # Card Identification.
                card_exp_month=payment_methods_pb2.SecretString(value="03"),
                card_exp_year=payment_methods_pb2.SecretString(value="2030"),
                card_cvc=payment_methods_pb2.SecretString(value="737"),
                card_holder_name=payment_methods_pb2.SecretString(value="John Doe"),  # Cardholder Information.
            ),
        ),
        address=payment_pb2.PaymentAddress(  # Address Information.
            billing_address=payment_pb2.Address(),
        ),
        enrolled_for_3ds=False,  # Authentication Details.
        return_url="https://example.com/3ds-return",  # URLs for Redirection.
    )

def _build_recurring_revoke_request():
    return payment_pb2.RecurringPaymentServiceRevokeRequest(
        merchant_revoke_id="probe_revoke_001",  # Identification.
        mandate_id="probe_mandate_001",  # Mandate Details.
        connector_mandate_id="probe_connector_mandate_001",
    )

def _build_refund_request(connector_transaction_id: str):
    return payment_pb2.PaymentServiceRefundRequest(
        merchant_refund_id="probe_refund_001",  # Identification.
        connector_transaction_id=connector_transaction_id,
        payment_amount=1000,  # Amount Information.
        refund_amount=payment_pb2.Money(
            minor_amount=1000,  # Amount in minor units (e.g., 1000 = $10.00).
            currency=payment_pb2.Currency.Value("USD"),  # ISO 4217 currency code (e.g., "USD", "EUR").
        ),
        reason="customer_request",  # Reason for the refund.
    )

def _build_refund_get_request():
    return payment_pb2.RefundServiceGetRequest(
        merchant_refund_id="probe_refund_001",  # Identification.
        connector_transaction_id="probe_connector_txn_001",
        refund_id="probe_refund_id_001",  # Deprecated.
    )

def _build_reverse_request(connector_transaction_id: str):
    return payment_pb2.PaymentServiceReverseRequest(
        merchant_reverse_id="probe_reverse_001",  # Identification.
        connector_transaction_id=connector_transaction_id,
    )

def _build_token_authorize_request():
    return payment_pb2.PaymentServiceTokenAuthorizeRequest(
        merchant_transaction_id="probe_tokenized_txn_001",
        amount=payment_pb2.Money(
            minor_amount=1000,  # Amount in minor units (e.g., 1000 = $10.00).
            currency=payment_pb2.Currency.Value("USD"),  # ISO 4217 currency code (e.g., "USD", "EUR").
        ),
        connector_token=payment_methods_pb2.SecretString(value="pm_1AbcXyzStripeTestToken"),  # Connector-issued token. Replaces PaymentMethod entirely. Examples: Stripe pm_xxx, Adyen recurringDetailReference, Braintree nonce.
        address=payment_pb2.PaymentAddress(
            billing_address=payment_pb2.Address(),
        ),
        capture_method=payment_pb2.CaptureMethod.Value("AUTOMATIC"),
        return_url="https://example.com/return",
    )

def _build_token_setup_recurring_request():
    return payment_pb2.PaymentServiceTokenSetupRecurringRequest(
        merchant_recurring_payment_id="probe_tokenized_mandate_001",
        amount=payment_pb2.Money(
            minor_amount=0,  # Amount in minor units (e.g., 1000 = $10.00).
            currency=payment_pb2.Currency.Value("USD"),  # ISO 4217 currency code (e.g., "USD", "EUR").
        ),
        connector_token=payment_methods_pb2.SecretString(value="pm_1AbcXyzStripeTestToken"),
        address=payment_pb2.PaymentAddress(
            billing_address=payment_pb2.Address(),
        ),
        customer_acceptance=payment_pb2.CustomerAcceptance(
            acceptance_type=payment_pb2.AcceptanceType.Value("ONLINE"),  # Type of acceptance (e.g., online, offline).
            accepted_at=0,  # Timestamp when the acceptance was made (Unix timestamp, seconds since epoch).
            online_mandate_details=payment_pb2.OnlineMandate(  # Details if the acceptance was an online mandate.
                ip_address="127.0.0.1",  # IP address from which the mandate was accepted.
                user_agent="Mozilla/5.0",  # User agent string of the browser used for mandate acceptance.
            ),
        ),
        setup_mandate_details=payment_pb2.SetupMandateDetails(
            mandate_type=payment_pb2.MandateType(  # Type of mandate (single_use or multi_use) with amount details.
                multi_use=payment_pb2.MandateAmountData(
                    amount=0,  # Use amount_money instead (will be removed in a future release).
                    currency=payment_pb2.Currency.Value("USD"),  # Use amount_money.currency instead (will be removed in a future release).
                    amount_money=payment_pb2.Money(  # Amount in Money type.
                        minor_amount=0,  # Amount in minor units (e.g., 1000 = $10.00).
                        currency=payment_pb2.Currency.Value("USD"),  # ISO 4217 currency code (e.g., "USD", "EUR").
                    ),
                ),
            ),
        ),
        setup_future_usage=payment_pb2.FutureUsage.Value("OFF_SESSION"),
    )

def _build_void_request(connector_transaction_id: str):
    return payment_pb2.PaymentServiceVoidRequest(
        merchant_void_id="probe_void_001",  # Identification.
        connector_transaction_id=connector_transaction_id,
    )
async def process_capture(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.Capture"""
    payment_client = PaymentClient(config)

    capture_response = await payment_client.capture(_build_capture_request("probe_connector_txn_001"))

    return {"status": capture_response.status}


async def process_create_client_authentication_token(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: MerchantAuthenticationService.CreateClientAuthenticationToken"""
    merchantauthentication_client = MerchantAuthenticationClient(config)

    create_response = await merchantauthentication_client.create_client_authentication_token(_build_create_client_authentication_token_request())

    return {"session_data": create_response.session_data}


async def process_get(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.Get"""
    payment_client = PaymentClient(config)

    get_response = await payment_client.get(_build_get_request("probe_connector_txn_001"))

    return {"status": get_response.status}


async def process_parse_event(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: EventService.ParseEvent"""
    event_client = EventClient(config)

    parse_response = event_client.parse_event(_build_parse_event_request())

    return {"event_type": parse_response.event_type}


async def process_post_authenticate(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentMethodAuthenticationService.PostAuthenticate"""
    paymentmethodauthentication_client = PaymentMethodAuthenticationClient(config)

    post_response = await paymentmethodauthentication_client.post_authenticate(_build_post_authenticate_request())

    return {"status": post_response.status}


async def process_pre_authenticate(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentMethodAuthenticationService.PreAuthenticate"""
    paymentmethodauthentication_client = PaymentMethodAuthenticationClient(config)

    pre_response = await paymentmethodauthentication_client.pre_authenticate(_build_pre_authenticate_request())

    return {"status": pre_response.status}


async def process_recurring_revoke(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: RecurringPaymentService.Revoke"""
    recurringpayment_client = RecurringPaymentClient(config)

    recurring_response = await recurringpayment_client.recurring_revoke(_build_recurring_revoke_request())

    return {"status": recurring_response.status}


async def process_refund(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.Refund"""
    payment_client = PaymentClient(config)

    refund_response = await payment_client.refund(_build_refund_request("probe_connector_txn_001"))

    return {"status": refund_response.status}


async def process_refund_get(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: RefundService.Get"""
    refund_client = RefundClient(config)

    refund_response = await refund_client.refund_get(_build_refund_get_request())

    return {"status": refund_response.status}


async def process_reverse(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.Reverse"""
    payment_client = PaymentClient(config)

    reverse_response = await payment_client.reverse(_build_reverse_request("probe_connector_txn_001"))

    return {"status": reverse_response.status}


async def process_token_authorize(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.TokenAuthorize"""
    payment_client = PaymentClient(config)

    token_response = await payment_client.token_authorize(_build_token_authorize_request())

    return {"status": token_response.status}


async def process_token_setup_recurring(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.TokenSetupRecurring"""
    payment_client = PaymentClient(config)

    token_response = await payment_client.token_setup_recurring(_build_token_setup_recurring_request())

    return {"status": token_response.status}


async def process_void(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.Void"""
    payment_client = PaymentClient(config)

    void_response = await payment_client.void(_build_void_request("probe_connector_txn_001"))

    return {"status": void_response.status}

if __name__ == "__main__":
    scenario = sys.argv[1] if len(sys.argv) > 1 else "capture"
    fn = globals().get(f"process_{scenario}")
    if not fn:
        available = [k[8:] for k in globals() if k.startswith("process_")]
        print(f"Unknown scenario: {scenario}. Available: {available}", file=sys.stderr)
        sys.exit(1)
    asyncio.run(fn("order_001"))
