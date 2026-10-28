# This file is auto-generated. Do not edit manually.
# Replace YOUR_API_KEY and placeholder values with real data.
# Regenerate: python3 scripts/generate-connector-docs.py ecommpay
#
# Ecommpay — all integration scenarios and flows in one file.
# Run a scenario:  python3 ecommpay.py checkout_card

import asyncio
import sys
from payments import PaymentClient
from payments import EventClient
from payments import RecurringPaymentClient
from payments import RefundClient
from payments.generated import common_types_pb2, events_pb2, payment_methods_pb2, payment_pb2, sdk_config_pb2

SUPPORTED_FLOWS = ["authorize", "capture", "get", "parse_event", "proxy_authorize", "proxy_setup_recurring", "recurring_charge", "refund", "refund_get", "setup_recurring", "void"]

_default_config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
    connector_config=payment_pb2.ConnectorSpecificConfig(
        ecommpay=payment_pb2.EcommpayConfig(
            project_id=payment_methods_pb2.SecretString(value="YOUR_PROJECT_ID"),
            secret_key=payment_methods_pb2.SecretString(value="YOUR_SECRET_KEY"),
            base_url="YOUR_BASE_URL",
        ),
    ),
)




def _build_authorize_request(capture_method: str):
    return payment_pb2.PaymentServiceAuthorizeRequest(
        merchant_transaction_id="probe_txn_001",  # Identification.
        amount=common_types_pb2.Money(  # The amount for the payment.
            minor_amount=1000,
            currency=common_types_pb2.Currency.Value("USD"),
        ),
        payment_method=payment_methods_pb2.PaymentMethod(  # Payment method to be used.
            card=payment_methods_pb2.CardDetails(
                card_number=payment_methods_pb2.CardNumberType(value="4111111111111111"),  # Card Identification.
                card_exp_month=payment_methods_pb2.SecretString(value="03"),
                card_exp_year=payment_methods_pb2.SecretString(value="2030"),
                card_cvc=payment_methods_pb2.SecretString(value="737"),
                card_holder_name=payment_methods_pb2.SecretString(value="John Doe"),  # Cardholder Information.
            ),
        ),
        capture_method=payment_pb2.CaptureMethod.Value(capture_method),  # Method for capturing the payment.
        customer=common_types_pb2.Customer(  # Customer Information.
            connector_customer_id="cust_probe_123",
        ),
        address=payment_pb2.PaymentAddress(  # Address Information.
            shipping_address=common_types_pb2.Address(
                first_name=payment_methods_pb2.SecretString(value="Jane"),
                line1=payment_methods_pb2.SecretString(value="12 Main Street"),
                city=payment_methods_pb2.SecretString(value="Seattle"),
                state=payment_methods_pb2.SecretString(value="WA"),
                zip_code=payment_methods_pb2.SecretString(value="98101"),
                country_alpha2_code=payment_methods_pb2.CountryAlpha2.Value("US"),
            ),
            billing_address=common_types_pb2.Address(),
        ),
        auth_type=payment_pb2.AuthenticationType.Value("NO_THREE_DS"),  # Authentication Details.
        authentication_data=payment_pb2.AuthenticationData(
            eci="05",  # Electronic Commerce Indicator (ECI) from 3DS.
            cavv="cHJvYmVfY2F2dl92YWx1ZQ==",  # Cardholder Authentication Verification Value (CAVV).
            threeds_server_transaction_id="probe_3ds_server_txn_001",  # 3DS Server Transaction ID.
            message_version="2.2.0",  # 3DS Message Version (e.g., "2.1.0", "2.2.0").
            ds_transaction_id="probe_ds_trans_001",  # Directory Server Transaction ID (DS Trans ID).
            trans_status=payment_pb2.TransactionStatus.Value("TRANSACTION_STATUS_SUCCESS"),  # Transaction status from authentication/verification process.
        ),
        return_url="https://example.com/payment/return",  # URLs for Redirection and Webhooks.
        complete_authorize_url="https://example.com/payment/complete",
        setup_future_usage=payment_pb2.FutureUsage.Value("OFF_SESSION"),  # Behavioral Flags and Preferences.
        browser_info=payment_pb2.BrowserInformation(
            screen_height=1080,
            screen_width=1920,
            ip_address="203.0.113.10",  # Device Information.
        ),
        billing_descriptor=payment_pb2.BillingDescriptor(  # Statement Descriptor.
            statement_descriptor="PROBE DESCRIPTOR",  # Complete description that appears on customers' statements.
        ),
        l2_l3_data=payment_pb2.L2L3Data(  # Level 2 / Level 3 data for enhanced payment processing.
            tax_info=payment_pb2.TaxInfo(  # Tax information.
                order_tax_amount=100,  # Total tax amount for the order (in minor currency units).
            ),
        ),
    )

def _build_capture_request(connector_transaction_id: str):
    return payment_pb2.PaymentServiceCaptureRequest(
        merchant_capture_id="probe_capture_001",  # Identification.
        connector_transaction_id=connector_transaction_id,
        amount_to_capture=common_types_pb2.Money(  # Capture Details.
            minor_amount=1000,
            currency=common_types_pb2.Currency.Value("USD"),
        ),
    )

def _build_get_request(connector_transaction_id: str):
    return payment_pb2.PaymentServiceGetRequest(
        merchant_transaction_id="probe_merchant_txn_001",  # Identification.
        connector_transaction_id=connector_transaction_id,
        amount=common_types_pb2.Money(  # Amount Information.
            minor_amount=1000,
            currency=common_types_pb2.Currency.Value("USD"),
        ),
    )

def _build_parse_event_request():
    return events_pb2.EventServiceParseRequest(
        request_details=payment_pb2.RequestDetails(
            method=payment_pb2.HttpMethod.Value("HTTP_METHOD_POST"),  # HTTP method of the request (e.g., GET, POST).
            uri="https://example.com/webhook",  # URI of the request.
            headers={},  # Headers of the HTTP request.
            body="{\"project_id\":1,\"payment\":{\"id\":\"probe_payment\",\"status\":\"success\"},\"operation\":{\"id\":1,\"type\":\"sale\",\"status\":\"success\",\"request_id\":\"probe_request\"},\"signature\":\"\"}".encode(),  # Body of the HTTP request.
        ),
    )

def _build_proxy_authorize_request():
    return payment_pb2.PaymentServiceProxyAuthorizeRequest(
        merchant_transaction_id="probe_proxy_txn_001",
        amount=common_types_pb2.Money(
            minor_amount=1000,
            currency=common_types_pb2.Currency.Value("USD"),
        ),
        card_proxy=payment_methods_pb2.ProxyCardDetails(  # Card proxy for vault-aliased payments (VGS, Basis Theory, Spreedly). Real card values are substituted by the proxy before reaching the connector.
            card_number=payment_methods_pb2.SecretString(value="4111111111111111"),  # Card Identification.
            card_exp_month=payment_methods_pb2.SecretString(value="03"),
            card_exp_year=payment_methods_pb2.SecretString(value="2030"),
            card_cvc=payment_methods_pb2.SecretString(value="123"),
            card_holder_name=payment_methods_pb2.SecretString(value="John Doe"),  # Cardholder Information.
            card_network=payment_methods_pb2.CardNetwork.Value("VISA"),
        ),
        customer=common_types_pb2.Customer(
            connector_customer_id="cust_probe_123",
        ),
        address=payment_pb2.PaymentAddress(
            billing_address=common_types_pb2.Address(),
        ),
        capture_method=payment_pb2.CaptureMethod.Value("AUTOMATIC"),
        auth_type=payment_pb2.AuthenticationType.Value("NO_THREE_DS"),
        return_url="https://example.com/return",
        browser_info=payment_pb2.BrowserInformation(
            ip_address="1.2.3.4",  # Device Information.
        ),
    )

def _build_proxy_setup_recurring_request():
    return payment_pb2.PaymentServiceProxySetupRecurringRequest(
        merchant_recurring_payment_id="probe_proxy_mandate_001",
        amount=common_types_pb2.Money(
            minor_amount=0,
            currency=common_types_pb2.Currency.Value("USD"),
        ),
        card_proxy=payment_methods_pb2.ProxyCardDetails(  # Card proxy for vault-aliased payments.
            card_number=payment_methods_pb2.SecretString(value="4111111111111111"),  # Card Identification.
            card_exp_month=payment_methods_pb2.SecretString(value="03"),
            card_exp_year=payment_methods_pb2.SecretString(value="2030"),
            card_cvc=payment_methods_pb2.SecretString(value="123"),
            card_holder_name=payment_methods_pb2.SecretString(value="John Doe"),  # Cardholder Information.
            card_network=payment_methods_pb2.CardNetwork.Value("VISA"),
        ),
        customer=common_types_pb2.Customer(
            connector_customer_id="probe_customer_001",
        ),
        address=payment_pb2.PaymentAddress(
            billing_address=common_types_pb2.Address(),
        ),
        return_url="https://example.com/return",
        customer_acceptance=payment_pb2.CustomerAcceptance(
            acceptance_type=payment_pb2.AcceptanceType.Value("OFFLINE"),  # Type of acceptance (e.g., online, offline).
            accepted_at=0,  # Timestamp when the acceptance was made (Unix timestamp, seconds since epoch).
        ),
        auth_type=payment_pb2.AuthenticationType.Value("NO_THREE_DS"),
        setup_future_usage=payment_pb2.FutureUsage.Value("OFF_SESSION"),
        browser_info=payment_pb2.BrowserInformation(
            color_depth=24,  # Display Information.
            screen_height=900,
            screen_width=1440,
            java_enabled=False,  # Browser Settings.
            java_script_enabled=True,
            language="en-US",
            time_zone_offset_minutes=-480,
            accept_header="application/json",  # Browser Headers.
            user_agent="Mozilla/5.0 (probe-bot)",
            accept_language="en-US,en;q=0.9",
            ip_address="1.2.3.4",  # Device Information.
        ),
    )

def _build_recurring_charge_request():
    return payment_pb2.RecurringPaymentServiceChargeRequest(
        connector_recurring_payment_id=payment_pb2.MandateReference(  # Reference to existing mandate.
            connector_mandate_id=payment_pb2.ConnectorMandateReferenceId(  # mandate_id sent by the connector.
                connector_mandate_id="1821234041",
            ),
        ),
        amount=common_types_pb2.Money(  # Amount Information.
            minor_amount=1000,
            currency=common_types_pb2.Currency.Value("USD"),
        ),
        payment_method=payment_methods_pb2.PaymentMethod(  # Optional payment Method Information (for network transaction flows).
            token=payment_methods_pb2.TokenPaymentMethodType(
                token=payment_methods_pb2.SecretString(value="probe_pm_token"),  # The token string representing a payment method.
            ),
        ),
        return_url="https://example.com/recurring-return",
        connector_customer_id="cust_probe_123",
        browser_info=payment_pb2.BrowserInformation(  # Browser Information.
            ip_address="203.0.113.10",  # Device Information.
        ),
        payment_method_type=payment_pb2.PaymentMethodType.Value("PAY_PAL"),
        off_session=True,  # Behavioral Flags and Preferences.
        customer=common_types_pb2.Customer(  # Customer Information.
            id="cust_probe_123",
        ),
    )

def _build_refund_request(connector_transaction_id: str):
    return payment_pb2.PaymentServiceRefundRequest(
        merchant_refund_id="probe_refund_001",  # Identification.
        connector_transaction_id=connector_transaction_id,
        payment_amount=1000,  # Amount Information.
        refund_amount=common_types_pb2.Money(
            minor_amount=1000,
            currency=common_types_pb2.Currency.Value("USD"),
        ),
        reason="customer_request",  # Reason for the refund.
    )

def _build_refund_get_request():
    return payment_pb2.RefundServiceGetRequest(
        merchant_refund_id="probe_refund_001",  # Identification.
        connector_transaction_id="probe_connector_txn_001",
        refund_id="probe_refund_id_001",  # Deprecated.
    )

def _build_setup_recurring_request():
    return payment_pb2.PaymentServiceSetupRecurringRequest(
        merchant_recurring_payment_id="probe_mandate_001",  # Identification.
        amount=common_types_pb2.Money(  # Mandate Details.
            minor_amount=0,
            currency=common_types_pb2.Currency.Value("USD"),
        ),
        payment_method=payment_methods_pb2.PaymentMethod(
            card=payment_methods_pb2.CardDetails(
                card_number=payment_methods_pb2.CardNumberType(value="4111111111111111"),  # Card Identification.
                card_exp_month=payment_methods_pb2.SecretString(value="03"),
                card_exp_year=payment_methods_pb2.SecretString(value="2030"),
                card_cvc=payment_methods_pb2.SecretString(value="737"),
                card_holder_name=payment_methods_pb2.SecretString(value="John Doe"),  # Cardholder Information.
            ),
        ),
        customer=common_types_pb2.Customer(
            id="cust_probe_123",
        ),
        address=payment_pb2.PaymentAddress(  # Address Information.
            billing_address=common_types_pb2.Address(),
        ),
        auth_type=payment_pb2.AuthenticationType.Value("NO_THREE_DS"),  # Type of authentication to be used.
        enrolled_for_3ds=False,  # Indicates if the customer is enrolled for 3D Secure.
        return_url="https://example.com/payment/return",  # URL to redirect after setup.
        complete_authorize_url="https://example.com/payment/complete",  # URL to complete authorization.
        setup_future_usage=payment_pb2.FutureUsage.Value("OFF_SESSION"),  # Indicates future usage intention.
        request_incremental_authorization=False,  # Indicates if incremental authorization is requested.
        customer_acceptance=payment_pb2.CustomerAcceptance(  # Details of customer acceptance.
            acceptance_type=payment_pb2.AcceptanceType.Value("OFFLINE"),  # Type of acceptance (e.g., online, offline).
            accepted_at=0,  # Timestamp when the acceptance was made (Unix timestamp, seconds since epoch).
        ),
        browser_info=payment_pb2.BrowserInformation(  # Information about the customer's browser.
            ip_address="203.0.113.10",  # Device Information.
        ),
    )

def _build_void_request(connector_transaction_id: str):
    return payment_pb2.PaymentServiceVoidRequest(
        merchant_void_id="probe_void_001",  # Identification.
        connector_transaction_id=connector_transaction_id,
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


async def process_authorize(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.Authorize (Card)"""
    payment_client = PaymentClient(config)

    authorize_response = await payment_client.authorize(_build_authorize_request("AUTOMATIC"))

    return {"status": authorize_response.status, "transaction_id": authorize_response.connector_transaction_id}


async def process_capture(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.Capture"""
    payment_client = PaymentClient(config)

    capture_response = await payment_client.capture(_build_capture_request("probe_connector_txn_001"))

    return {"status": capture_response.status}


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


async def process_proxy_authorize(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.ProxyAuthorize"""
    payment_client = PaymentClient(config)

    proxy_response = await payment_client.proxy_authorize(_build_proxy_authorize_request())

    return {"status": proxy_response.status}


async def process_proxy_setup_recurring(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.ProxySetupRecurring"""
    payment_client = PaymentClient(config)

    proxy_response = await payment_client.proxy_setup_recurring(_build_proxy_setup_recurring_request())

    return {"status": proxy_response.status}


async def process_recurring_charge(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: RecurringPaymentService.Charge"""
    recurringpayment_client = RecurringPaymentClient(config)

    recurring_response = await recurringpayment_client.charge(_build_recurring_charge_request())

    return {"status": recurring_response.status}


async def process_refund_get(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: RefundService.Get"""
    refund_client = RefundClient(config)

    refund_response = await refund_client.refund_get(_build_refund_get_request())

    return {"status": refund_response.status}


async def process_setup_recurring(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.SetupRecurring"""
    payment_client = PaymentClient(config)

    setup_response = await payment_client.setup_recurring(_build_setup_recurring_request())

    return {"status": setup_response.status, "mandate_id": setup_response.connector_recurring_payment_id}


async def process_void(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
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
