# Auto-generated for billwerk
import asyncio
from payments import PaymentMethodClient
from payments import PaymentClient
from payments.generated.payment_methods_pb2 import (
    CardDetails,
    CardNumberType,
    GooglePayEncryptedTokenizationData,
    GoogleWallet,
    PaymentMethod,
    SecretString,
    Sepa,
)
from payments.generated.payment_pb2 import (
    Address,
    AuthenticationType,
    CaptureMethod,
    Currency,
    Money,
    PaymentAddress,
    PaymentMethodServiceTokenizeRequest,
    PaymentServiceAuthorizeRequest,
    PaymentServiceCaptureRequest,
    PaymentServiceGetRequest,
    PaymentServiceRefundRequest,
    PaymentServiceVoidRequest,
)

async def process_checkout_card(merchant_id, config):
    """Card Payment (Authorize + Capture)"""
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
            address=PaymentAddress(
                billing_address=Address(),
            ),
            auth_type=AuthenticationType.Value('NO_THREE_DS'),
            return_url='https://example.com/return',
            payment_method_token=SecretString(value='probe_pm_token'),
        ))
    # Step 2: Capture — settle the reserved funds
    result = await client.capture(PaymentServiceCaptureRequest(
            merchant_capture_id='probe_capture_001',
            connector_transaction_id='probe_connector_txn_001',
            amount_to_capture=Money(
                minor_amount=1000,
                currency=Currency.Value('USD'),
            ),
        ))
    return result
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
            address=PaymentAddress(
                billing_address=Address(),
            ),
            auth_type=AuthenticationType.Value('NO_THREE_DS'),
            return_url='https://example.com/return',
            payment_method_token=SecretString(value='probe_pm_token'),
        ))
    return result
async def process_checkout_wallet(merchant_id, config):
    """Wallet Payment (Google Pay / Apple Pay)"""
    client = PaymentClient(config)
    # Step 1: Authorize — reserve funds on the payment method
    result = await client.authorize(PaymentServiceAuthorizeRequest(
            merchant_transaction_id='probe_txn_001',
            amount=Money(
                minor_amount=1000,
                currency=Currency.Value('USD'),
            ),
            payment_method=PaymentMethod(
                google_pay=GoogleWallet(
                    type='CARD',
                    description='Visa 1111',
                    info=GoogleWallet.PaymentMethodInfo(
                        card_network='VISA',
                        card_details='1111',
                    ),
                    tokenization_data=GoogleWallet.TokenizationData(
                        encrypted_data=GooglePayEncryptedTokenizationData(
                            token_type='PAYMENT_GATEWAY',
                            token='{"id":"tok_probe_gpay","object":"token","type":"card"}',
                        ),
                    ),
                ),
            ),
            capture_method=CaptureMethod.Value('AUTOMATIC'),
            address=PaymentAddress(
                billing_address=Address(),
            ),
            auth_type=AuthenticationType.Value('NO_THREE_DS'),
            return_url='https://example.com/return',
            payment_method_token=SecretString(value='probe_pm_token'),
        ))
    return result
async def process_checkout_bank(merchant_id, config):
    """Bank Transfer (SEPA / ACH / BACS)"""
    client = PaymentClient(config)
    # Step 1: Authorize — reserve funds on the payment method
    result = await client.authorize(PaymentServiceAuthorizeRequest(
            merchant_transaction_id='probe_txn_001',
            amount=Money(
                minor_amount=1000,
                currency=Currency.Value('EUR'),
            ),
            payment_method=PaymentMethod(
                sepa=Sepa(
                    iban=SecretString(value='DE89370400440532013000'),
                    bank_account_holder_name=SecretString(value='John Doe'),
                ),
            ),
            capture_method=CaptureMethod.Value('AUTOMATIC'),
            address=PaymentAddress(
                billing_address=Address(),
            ),
            auth_type=AuthenticationType.Value('NO_THREE_DS'),
            return_url='https://example.com/return',
            payment_method_token=SecretString(value='probe_pm_token'),
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
            address=PaymentAddress(
                billing_address=Address(),
            ),
            auth_type=AuthenticationType.Value('NO_THREE_DS'),
            return_url='https://example.com/return',
            payment_method_token=SecretString(value='probe_pm_token'),
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
        ))
    return result
async def process_void_payment(merchant_id, config):
    """Void a Payment"""
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
            address=PaymentAddress(
                billing_address=Address(),
            ),
            auth_type=AuthenticationType.Value('NO_THREE_DS'),
            return_url='https://example.com/return',
            payment_method_token=SecretString(value='probe_pm_token'),
        ))
    # Step 2: Void — release reserved funds (cancel authorization)
    result = await client.void(PaymentServiceVoidRequest(
            merchant_void_id='probe_void_001',
            connector_transaction_id='probe_connector_txn_001',
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
            address=PaymentAddress(
                billing_address=Address(),
            ),
            auth_type=AuthenticationType.Value('NO_THREE_DS'),
            return_url='https://example.com/return',
            payment_method_token=SecretString(value='probe_pm_token'),
        ))
    # Step 2: Get — retrieve current payment status from the connector
    result = await client.get(PaymentServiceGetRequest(
            merchant_transaction_id='probe_merchant_txn_001',
            connector_transaction_id='probe_connector_txn_001',
            amount=Money(
                minor_amount=1000,
                currency=Currency.Value('USD'),
            ),
            connector_order_reference_id='probe_order_ref_001',
        ))
    return result
async def process_tokenize(merchant_id, config):
    """Tokenize Payment Method"""
    pm_client = PaymentMethodClient(config)
    # Step 1: Tokenize — store card details and return a reusable token
    result = await pm_client.tokenize(PaymentMethodServiceTokenizeRequest(
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
            address=PaymentAddress(
                billing_address=Address(),
            ),
        ))
    return result

if __name__ == "__main__":
    asyncio.run(process_checkout_card("order_001"))