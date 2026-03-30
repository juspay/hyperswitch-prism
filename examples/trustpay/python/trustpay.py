# Auto-generated for trustpay
import asyncio
from payments import PaymentClient
from payments.generated.payment_methods_pb2 import (
    CardDetails,
    CardNumberType,
    CountryAlpha2,
    PaymentMethod,
    SecretString,
)
from payments.generated.payment_pb2 import (
    AccessToken,
    Address,
    AuthenticationType,
    BrowserInformation,
    CaptureMethod,
    ConnectorState,
    Currency,
    Customer,
    Money,
    PaymentAddress,
    PaymentServiceAuthorizeRequest,
    PaymentServiceGetRequest,
    PaymentServiceRefundRequest,
)

async def process_checkout_autocapture(merchant_id, config):
    """Card Payment (Automatic Capture)"""
    client = PaymentClient(config)
    # Step 1: Authorize — reserve funds on the payment method
    result = await client.authorize(PaymentServiceAuthorizeRequest(
            merchant_transaction_id='probe_txn_001',
            amount=Money(
                minor_amount=1000,
                currency=Currency.Value('USD'),
            ),
            payment_method=PaymentMethod(
                card=CardDetails(
                    card_number=CardNumberType(value='4111111111111111'),
                    card_exp_month=SecretString(value='03'),
                    card_exp_year=SecretString(value='2030'),
                    card_cvc=SecretString(value='737'),
                    card_holder_name=SecretString(value='John Doe'),
                ),
            ),
            capture_method=CaptureMethod.Value('AUTOMATIC'),
            customer=Customer(
                email=SecretString(value='test@example.com'),
            ),
            address=PaymentAddress(
                billing_address=Address(
                    first_name=SecretString(value='John'),
                    line1=SecretString(value='123 Main St'),
                    city=SecretString(value='Seattle'),
                    zip_code=SecretString(value='98101'),
                    country_alpha2_code=CountryAlpha2.Value('US'),
                ),
            ),
            auth_type=AuthenticationType.Value('NO_THREE_DS'),
            return_url='https://example.com/return',
            browser_info=BrowserInformation(
                user_agent='Mozilla/5.0 (probe-bot)',
                ip_address='1.2.3.4',
            ),
            state=ConnectorState(
                access_token=AccessToken(
                    token=SecretString(value='probe_access_token'),
                    expires_in_seconds=3600,
                    token_type='Bearer',
                ),
            ),
        ))
    return result
async def process_refund(merchant_id, config):
    """Refund a Payment"""
    client = PaymentClient(config)
    # Step 1: Authorize — reserve funds on the payment method
    result = await client.authorize(PaymentServiceAuthorizeRequest(
            merchant_transaction_id='probe_txn_001',
            amount=Money(
                minor_amount=1000,
                currency=Currency.Value('USD'),
            ),
            payment_method=PaymentMethod(
                card=CardDetails(
                    card_number=CardNumberType(value='4111111111111111'),
                    card_exp_month=SecretString(value='03'),
                    card_exp_year=SecretString(value='2030'),
                    card_cvc=SecretString(value='737'),
                    card_holder_name=SecretString(value='John Doe'),
                ),
            ),
            capture_method=CaptureMethod.Value('AUTOMATIC'),
            customer=Customer(
                email=SecretString(value='test@example.com'),
            ),
            address=PaymentAddress(
                billing_address=Address(
                    first_name=SecretString(value='John'),
                    line1=SecretString(value='123 Main St'),
                    city=SecretString(value='Seattle'),
                    zip_code=SecretString(value='98101'),
                    country_alpha2_code=CountryAlpha2.Value('US'),
                ),
            ),
            auth_type=AuthenticationType.Value('NO_THREE_DS'),
            return_url='https://example.com/return',
            browser_info=BrowserInformation(
                user_agent='Mozilla/5.0 (probe-bot)',
                ip_address='1.2.3.4',
            ),
            state=ConnectorState(
                access_token=AccessToken(
                    token=SecretString(value='probe_access_token'),
                    expires_in_seconds=3600,
                    token_type='Bearer',
                ),
            ),
        ))
    # Step 2: Refund — return funds to the customer
    result = await client.refund(PaymentServiceRefundRequest(
            merchant_refund_id='probe_refund_001',
            connector_transaction_id='probe_connector_txn_001',
            payment_amount=1000,
            refund_amount=Money(
                minor_amount=1000,
                currency=Currency.Value('USD'),
            ),
            reason='customer_request',
            state=ConnectorState(
                access_token=AccessToken(
                    token=SecretString(value='probe_access_token'),
                    expires_in_seconds=3600,
                    token_type='Bearer',
                ),
            ),
        ))
    return result
async def process_get_payment(merchant_id, config):
    """Get Payment Status"""
    client = PaymentClient(config)
    # Step 1: Authorize — reserve funds on the payment method
    result = await client.authorize(PaymentServiceAuthorizeRequest(
            merchant_transaction_id='probe_txn_001',
            amount=Money(
                minor_amount=1000,
                currency=Currency.Value('USD'),
            ),
            payment_method=PaymentMethod(
                card=CardDetails(
                    card_number=CardNumberType(value='4111111111111111'),
                    card_exp_month=SecretString(value='03'),
                    card_exp_year=SecretString(value='2030'),
                    card_cvc=SecretString(value='737'),
                    card_holder_name=SecretString(value='John Doe'),
                ),
            ),
            capture_method=CaptureMethod.Value('MANUAL'),
            customer=Customer(
                email=SecretString(value='test@example.com'),
            ),
            address=PaymentAddress(
                billing_address=Address(
                    first_name=SecretString(value='John'),
                    line1=SecretString(value='123 Main St'),
                    city=SecretString(value='Seattle'),
                    zip_code=SecretString(value='98101'),
                    country_alpha2_code=CountryAlpha2.Value('US'),
                ),
            ),
            auth_type=AuthenticationType.Value('NO_THREE_DS'),
            return_url='https://example.com/return',
            browser_info=BrowserInformation(
                user_agent='Mozilla/5.0 (probe-bot)',
                ip_address='1.2.3.4',
            ),
            state=ConnectorState(
                access_token=AccessToken(
                    token=SecretString(value='probe_access_token'),
                    expires_in_seconds=3600,
                    token_type='Bearer',
                ),
            ),
        ))
    # Step 2: Get — retrieve current payment status from the connector
    result = await client.get(PaymentServiceGetRequest(
            merchant_transaction_id='probe_merchant_txn_001',
            connector_transaction_id='probe_connector_txn_001',
            amount=Money(
                minor_amount=1000,
                currency=Currency.Value('USD'),
            ),
            state=ConnectorState(
                access_token=AccessToken(
                    token=SecretString(value='probe_access_token'),
                    expires_in_seconds=3600,
                    token_type='Bearer',
                ),
            ),
        ))
    return result

if __name__ == "__main__":
    asyncio.run(process_checkout_card("order_001"))