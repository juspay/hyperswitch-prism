# This file is auto-generated. Do not edit manually.
# Replace YOUR_API_KEY and placeholder values with real data.
# Regenerate: python3 scripts/generate-connector-docs.py cybersource
#
# Cybersource — all integration scenarios and flows in one file.
# Run a scenario:  python3 cybersource.py checkout_card

import asyncio
import sys
from payments import PaymentClient
from payments import PaymentMethodAuthenticationClient
from payments import EventClient
from payments import RecurringPaymentClient
from payments import RefundClient
from payments.generated import common_types_pb2, events_pb2, payment_methods_pb2, payment_pb2, sdk_config_pb2

SUPPORTED_FLOWS = ["authenticate", "authorize", "capture", "get", "incremental_authorization", "parse_event", "post_authenticate", "pre_authenticate", "proxy_authorize", "proxy_setup_recurring", "recurring_charge", "recurring_revoke", "refund", "refund_get", "reverse", "setup_recurring", "token_authorize", "void"]

_default_config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
    connector_config=payment_pb2.ConnectorSpecificConfig(
        cybersource=payment_pb2.CybersourceConfig(
            api_key=payment_methods_pb2.SecretString(value="YOUR_API_KEY"),
            merchant_account=payment_methods_pb2.SecretString(value="YOUR_MERCHANT_ACCOUNT"),
            api_secret=payment_methods_pb2.SecretString(value="YOUR_API_SECRET"),
            base_url="YOUR_BASE_URL",
            disable_avs=False,
            disable_cvn=False,
        ),
    ),
)




def _build_authenticate_request():
    return payment_pb2.PaymentMethodAuthenticationServiceAuthenticateRequest(
        amount=common_types_pb2.Money(  # Amount Information.
            minor_amount=1000,
            currency=common_types_pb2.Currency.Value("USD"),
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
        customer=common_types_pb2.Customer(  # Customer Information.
            email=payment_methods_pb2.SecretString(value="test@example.com"),
        ),
        address=payment_pb2.PaymentAddress(  # Address Information.
            billing_address=common_types_pb2.Address(),
        ),
        metadata=payment_methods_pb2.SecretString(value="{\"device_channel\":\"BRW\"}"),  # Additional metadata for the connector.
        return_url="https://example.com/3ds-return",  # URLs for Redirection.
        continue_redirection_url="https://example.com/3ds-continue",
        browser_info=payment_pb2.BrowserInformation(  # Contextual Information.
            color_depth=24,  # Display Information.
            screen_height=900,
            screen_width=1440,
            java_enabled=False,  # Browser Settings.
            java_script_enabled=True,
            language="en-US",
            time_zone_offset_minutes=-480,
            accept_header="application/json",  # Browser Headers.
            user_agent="Mozilla/5.0 (probe-bot)",
            ip_address="1.2.3.4",  # Device Information.
        ),
        redirection_response=payment_pb2.RedirectionResponse(  # Redirection Information after DDC step.
            params="probe_redirect_params",
            payload={"transaction_id": "probe_txn_123"},
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
            email=payment_methods_pb2.SecretString(value="test@example.com"),
        ),
        address=payment_pb2.PaymentAddress(  # Address Information.
            shipping_address=common_types_pb2.Address(
                first_name=payment_methods_pb2.SecretString(value="Jane"),
                last_name=payment_methods_pb2.SecretString(value="Doe"),
                line1=payment_methods_pb2.SecretString(value="123 Main St"),
                line2=payment_methods_pb2.SecretString(value="Suite 4"),
                city=payment_methods_pb2.SecretString(value="Seattle"),
                state=payment_methods_pb2.SecretString(value="WA"),
                zip_code=payment_methods_pb2.SecretString(value="98101"),
                country_alpha2_code=payment_methods_pb2.CountryAlpha2.Value("US"),
                phone_number=payment_methods_pb2.SecretString(value="4155552671"),
            ),
            billing_address=common_types_pb2.Address(),
        ),
        auth_type=payment_pb2.AuthenticationType.Value("NO_THREE_DS"),  # Authentication Details.
        return_url="https://example.com/return",  # URLs for Redirection and Webhooks.
        billing_descriptor=payment_pb2.BillingDescriptor(  # Statement Descriptor.
            name=payment_methods_pb2.SecretString(value="PROBE STORE"),  # Customer's billing name.
            statement_descriptor="PROBE STORE",  # Complete description that appears on customers' statements.
        ),
        l2_l3_data=payment_pb2.L2L3Data(  # Level 2 / Level 3 data for enhanced payment processing.
            order_info=payment_pb2.OrderInfo(  # Order-level information.
                # order_details: [{"product_name": "Probe item", "quantity": 1, "amount": 800, "total_tax_amount": 50, "category": "default", "unit_of_measure": "EA", "sku": "PROBE-SKU-1", "commodity_code": "4421"}]  # Line items for the order.
                merchant_order_reference_id="PO-PROBE-001",  # Merchant's internal reference ID for the order.
                discount_amount=0,  # Discount applied to the order (in minor currency units).
                shipping_cost=100,  # Shipping cost for the order (in minor currency units).
                duty_amount=50,  # Duty/customs amount for the order (in minor currency units).
            ),
            tax_info=payment_pb2.TaxInfo(  # Tax information.
                order_tax_amount=50,  # Total tax amount for the order (in minor currency units).
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

def _build_incremental_authorization_request():
    return payment_pb2.PaymentServiceIncrementalAuthorizationRequest(
        merchant_authorization_id="probe_auth_001",  # Identification.
        connector_transaction_id="probe_connector_txn_001",
        amount=common_types_pb2.Money(  # New total authorized amount, including this increase.
            minor_amount=1100,
            currency=common_types_pb2.Currency.Value("USD"),
        ),
        reason="incremental_auth_probe",  # Optional Fields.
    )

def _build_parse_event_request():
    return events_pb2.EventServiceParseRequest(
        request_details=payment_pb2.RequestDetails(
            method=payment_pb2.HttpMethod.Value("HTTP_METHOD_POST"),  # HTTP method of the request (e.g., GET, POST).
            uri="https://example.com/webhook",  # URI of the request.
            headers={},  # Headers of the HTTP request.
            body="{}".encode(),  # Body of the HTTP request.
        ),
    )

def _build_post_authenticate_request():
    return payment_pb2.PaymentMethodAuthenticationServicePostAuthenticateRequest(
        amount=common_types_pb2.Money(  # Amount Information.
            minor_amount=1000,
            currency=common_types_pb2.Currency.Value("USD"),
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
            billing_address=common_types_pb2.Address(),
        ),
        redirection_response=payment_pb2.RedirectionResponse(  # Redirection Information after DDC step.
            params="probe_redirect_params",
            payload={"transaction_id": "probe_txn_123"},
        ),
    )

def _build_pre_authenticate_request():
    return payment_pb2.PaymentMethodAuthenticationServicePreAuthenticateRequest(
        amount=common_types_pb2.Money(  # Amount Information.
            minor_amount=1000,
            currency=common_types_pb2.Currency.Value("USD"),
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
            billing_address=common_types_pb2.Address(),
        ),
        enrolled_for_3ds=False,  # Authentication Details.
        return_url="https://example.com/3ds-return",  # URLs for Redirection.
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
            email=payment_methods_pb2.SecretString(value="test@example.com"),
        ),
        address=payment_pb2.PaymentAddress(
            billing_address=common_types_pb2.Address(),
        ),
        capture_method=payment_pb2.CaptureMethod.Value("AUTOMATIC"),
        auth_type=payment_pb2.AuthenticationType.Value("NO_THREE_DS"),
        return_url="https://example.com/return",
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
            email=payment_methods_pb2.SecretString(value="test@example.com"),
        ),
        address=payment_pb2.PaymentAddress(
            billing_address=common_types_pb2.Address(),
        ),
        customer_acceptance=payment_pb2.CustomerAcceptance(
            acceptance_type=payment_pb2.AcceptanceType.Value("OFFLINE"),  # Type of acceptance (e.g., online, offline).
            accepted_at=0,  # Timestamp when the acceptance was made (Unix timestamp, seconds since epoch).
        ),
        auth_type=payment_pb2.AuthenticationType.Value("NO_THREE_DS"),
        setup_future_usage=payment_pb2.FutureUsage.Value("OFF_SESSION"),
    )

def _build_recurring_charge_request():
    return payment_pb2.RecurringPaymentServiceChargeRequest(
        connector_recurring_payment_id=payment_pb2.MandateReference(  # Reference to existing mandate.
            connector_mandate_id=payment_pb2.ConnectorMandateReferenceId(  # mandate_id sent by the connector.
                connector_mandate_id="probe-mandate-123",
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
        payment_method_type=payment_pb2.PaymentMethodType.Value("PAY_PAL"),
        off_session=True,  # Behavioral Flags and Preferences.
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

def _build_reverse_request(connector_transaction_id: str):
    return payment_pb2.PaymentServiceReverseRequest(
        merchant_reverse_id="probe_reverse_001",  # Identification.
        connector_transaction_id=connector_transaction_id,
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
            email=payment_methods_pb2.SecretString(value="test@example.com"),
        ),
        address=payment_pb2.PaymentAddress(  # Address Information.
            billing_address=common_types_pb2.Address(),
        ),
        auth_type=payment_pb2.AuthenticationType.Value("NO_THREE_DS"),  # Type of authentication to be used.
        enrolled_for_3ds=False,  # Indicates if the customer is enrolled for 3D Secure.
        return_url="https://example.com/mandate-return",  # URL to redirect after setup.
        setup_future_usage=payment_pb2.FutureUsage.Value("OFF_SESSION"),  # Indicates future usage intention.
        request_incremental_authorization=False,  # Indicates if incremental authorization is requested.
        customer_acceptance=payment_pb2.CustomerAcceptance(  # Details of customer acceptance.
            acceptance_type=payment_pb2.AcceptanceType.Value("OFFLINE"),  # Type of acceptance (e.g., online, offline).
            accepted_at=0,  # Timestamp when the acceptance was made (Unix timestamp, seconds since epoch).
        ),
    )

def _build_token_authorize_request():
    return payment_pb2.PaymentServiceTokenAuthorizeRequest(
        merchant_transaction_id="probe_tokenized_txn_001",
        amount=common_types_pb2.Money(
            minor_amount=1000,
            currency=common_types_pb2.Currency.Value("USD"),
        ),
        connector_token=payment_methods_pb2.SecretString(value="pm_1AbcXyzStripeTestToken"),  # Connector-issued token. Replaces PaymentMethod entirely. Examples: Stripe pm_xxx, Adyen recurringDetailReference, Braintree nonce.
        customer=common_types_pb2.Customer(
            email=payment_methods_pb2.SecretString(value="test@example.com"),
        ),
        address=payment_pb2.PaymentAddress(
            billing_address=common_types_pb2.Address(),
        ),
        capture_method=payment_pb2.CaptureMethod.Value("AUTOMATIC"),
        return_url="https://example.com/return",
    )

def _build_void_request(connector_transaction_id: str):
    return payment_pb2.PaymentServiceVoidRequest(
        merchant_void_id="probe_void_001",  # Identification.
        connector_transaction_id=connector_transaction_id,
        cancellation_reason="requested_by_customer",  # Void Details.
        amount=common_types_pb2.Money(  # Amount Information.
            minor_amount=1000,
            currency=common_types_pb2.Currency.Value("USD"),
        ),
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


async def process_authenticate(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentMethodAuthenticationService.Authenticate"""
    paymentmethodauthentication_client = PaymentMethodAuthenticationClient(config)

    authenticate_response = await paymentmethodauthentication_client.authenticate(_build_authenticate_request())

    return {"status": authenticate_response.status}


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


async def process_incremental_authorization(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.IncrementalAuthorization"""
    payment_client = PaymentClient(config)

    incremental_response = await payment_client.incremental_authorization(_build_incremental_authorization_request())

    return {"status": incremental_response.status}


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


async def process_recurring_revoke(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: RecurringPaymentService.Revoke"""
    recurringpayment_client = RecurringPaymentClient(config)

    recurring_response = await recurringpayment_client.recurring_revoke(_build_recurring_revoke_request())

    return {"status": recurring_response.status}


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


async def process_setup_recurring(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.SetupRecurring"""
    payment_client = PaymentClient(config)

    setup_response = await payment_client.setup_recurring(_build_setup_recurring_request())

    return {"status": setup_response.status, "mandate_id": setup_response.connector_recurring_payment_id}


async def process_token_authorize(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentService.TokenAuthorize"""
    payment_client = PaymentClient(config)

    token_response = await payment_client.token_authorize(_build_token_authorize_request())

    return {"status": token_response.status}


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
