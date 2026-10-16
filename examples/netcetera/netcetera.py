# This file is auto-generated. Do not edit manually.
# Replace YOUR_API_KEY and placeholder values with real data.
# Regenerate: python3 scripts/generate-connector-docs.py netcetera
#
# Netcetera — all integration scenarios and flows in one file.
# Run a scenario:  python3 netcetera.py checkout_card

import asyncio
import sys
from payments import PaymentMethodAuthenticationClient
from payments.generated import sdk_config_pb2, payment_pb2, events_pb2, payment_methods_pb2

SUPPORTED_FLOWS = ["authenticate", "post_authenticate", "pre_authenticate"]

_default_config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
    connector_config=payment_pb2.ConnectorSpecificConfig(
        netcetera=payment_pb2.NetceteraConfig(
            certificate=payment_methods_pb2.SecretString(value="YOUR_CERTIFICATE"),
            private_key=payment_methods_pb2.SecretString(value="YOUR_PRIVATE_KEY"),
            three_ds_requestor_id="YOUR_THREE_DS_REQUESTOR_ID",
            three_ds_requestor_name="YOUR_THREE_DS_REQUESTOR_NAME",
            merchant_configuration_id="YOUR_MERCHANT_CONFIGURATION_ID",
            base_url="YOUR_BASE_URL",
        ),
    ),
)




def _build_authenticate_request():
    return payment_pb2.PaymentMethodAuthenticationServiceAuthenticateRequest(
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
        return_url="https://example.com/3ds-return",  # URLs for Redirection. For 3DS this is the browser challenge return URL (EMVCo notificationURL / threeDSRequestorURL).
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
async def process_authenticate(merchant_transaction_id: str, config: sdk_config_pb2.ConnectorConfig = _default_config):
    """Flow: PaymentMethodAuthenticationService.Authenticate"""
    paymentmethodauthentication_client = PaymentMethodAuthenticationClient(config)

    authenticate_response = await paymentmethodauthentication_client.authenticate(_build_authenticate_request())

    return {"status": authenticate_response.status}


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

if __name__ == "__main__":
    scenario = sys.argv[1] if len(sys.argv) > 1 else "authenticate"
    fn = globals().get(f"process_{scenario}")
    if not fn:
        available = [k[8:] for k in globals() if k.startswith("process_")]
        print(f"Unknown scenario: {scenario}. Available: {available}", file=sys.stderr)
        sys.exit(1)
    asyncio.run(fn("order_001"))
