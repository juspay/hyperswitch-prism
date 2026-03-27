use domain_types::connector_types::ConnectorEnum;
use grpc_api_types::payments::PaymentMethod;
use hyperswitch_masking::Secret;

use crate::config::get_config;
use crate::sample_data::*;

pub(crate) fn authorize_pm_variants() -> Vec<(&'static str, fn() -> PaymentMethod)> {
    vec![
        ("Card", card_payment_method as fn() -> PaymentMethod),
        ("Sepa", sepa_payment_method as fn() -> PaymentMethod),
        ("Bacs", bacs_payment_method as fn() -> PaymentMethod),
        ("Ach", ach_payment_method as fn() -> PaymentMethod),
        ("Becs", becs_payment_method as fn() -> PaymentMethod),
        (
            "GooglePay",
            google_pay_encrypted_method as fn() -> PaymentMethod,
        ),
        (
            "GooglePayDecrypted",
            google_pay_decrypted_method as fn() -> PaymentMethod,
        ),
        (
            "ApplePay",
            apple_pay_encrypted_method as fn() -> PaymentMethod,
        ),
        (
            "ApplePayDecrypted",
            apple_pay_method as fn() -> PaymentMethod,
        ),
        ("Ideal", ideal_payment_method as fn() -> PaymentMethod),
        (
            "PaypalRedirect",
            paypal_redirect_method as fn() -> PaymentMethod,
        ),
        ("Blik", blik_payment_method as fn() -> PaymentMethod),
        ("Klarna", klarna_payment_method as fn() -> PaymentMethod),
        ("Afterpay", afterpay_payment_method as fn() -> PaymentMethod),
        (
            "UpiCollect",
            upi_collect_payment_method as fn() -> PaymentMethod,
        ),
        ("Affirm", affirm_payment_method as fn() -> PaymentMethod),
        (
            "SamsungPay",
            samsung_pay_payment_method as fn() -> PaymentMethod,
        ),
    ]
}

/// Static variant for config filtering (same as authorize_pm_variants but usable at config load time)
pub(crate) fn authorize_pm_variants_static() -> Vec<(&'static str, fn() -> PaymentMethod)> {
    authorize_pm_variants()
}

/// Build a mock ConnectorState with an access token for OAuth connectors.
pub(crate) fn mock_connector_state() -> grpc_api_types::payments::ConnectorState {
    let config = get_config();
    grpc_api_types::payments::ConnectorState {
        access_token: Some(grpc_api_types::payments::AccessToken {
            token: Some(Secret::new(config.access_token.token.clone())),
            token_type: Some(config.access_token.token_type.clone()),
            expires_in_seconds: Some(config.access_token.expires_in_seconds),
        }),
        connector_customer_id: None,
    }
}

pub(crate) fn all_connectors() -> Vec<ConnectorEnum> {
    vec![
        ConnectorEnum::Adyen,
        ConnectorEnum::Forte,
        ConnectorEnum::Razorpay,
        ConnectorEnum::RazorpayV2,
        ConnectorEnum::Fiserv,
        ConnectorEnum::Elavon,
        ConnectorEnum::Xendit,
        ConnectorEnum::Checkout,
        ConnectorEnum::Authorizedotnet,
        ConnectorEnum::Bamboraapac,
        ConnectorEnum::Mifinity,
        ConnectorEnum::Phonepe,
        ConnectorEnum::Cashfree,
        ConnectorEnum::Paytm,
        ConnectorEnum::Fiuu,
        ConnectorEnum::Payu,
        ConnectorEnum::Cashtocode,
        ConnectorEnum::Novalnet,
        ConnectorEnum::Nexinets,
        ConnectorEnum::Noon,
        ConnectorEnum::Braintree,
        ConnectorEnum::Volt,
        ConnectorEnum::Calida,
        ConnectorEnum::Cryptopay,
        ConnectorEnum::Helcim,
        ConnectorEnum::Dlocal,
        ConnectorEnum::Placetopay,
        ConnectorEnum::Rapyd,
        ConnectorEnum::Aci,
        ConnectorEnum::Trustpay,
        ConnectorEnum::Stripe,
        ConnectorEnum::Cybersource,
        ConnectorEnum::Worldpay,
        ConnectorEnum::Worldpayvantiv,
        ConnectorEnum::Worldpayxml,
        ConnectorEnum::Multisafepay,
        ConnectorEnum::Payload,
        ConnectorEnum::Fiservemea,
        ConnectorEnum::Paysafe,
        ConnectorEnum::Datatrans,
        ConnectorEnum::Bluesnap,
        ConnectorEnum::Authipay,
        ConnectorEnum::Silverflow,
        ConnectorEnum::Celero,
        ConnectorEnum::Paypal,
        ConnectorEnum::Stax,
        ConnectorEnum::Billwerk,
        ConnectorEnum::Hipay,
        ConnectorEnum::Trustpayments,
        ConnectorEnum::Redsys,
        ConnectorEnum::Globalpay,
        ConnectorEnum::Nuvei,
        ConnectorEnum::Iatapay,
        ConnectorEnum::Nmi,
        ConnectorEnum::Shift4,
        ConnectorEnum::Paybox,
        ConnectorEnum::Barclaycard,
        ConnectorEnum::Nexixpay,
        ConnectorEnum::Mollie,
        ConnectorEnum::Airwallex,
        ConnectorEnum::Tsys,
        ConnectorEnum::Bankofamerica,
        ConnectorEnum::Powertranz,
        ConnectorEnum::Getnet,
        ConnectorEnum::Jpmorgan,
        ConnectorEnum::Bambora,
        ConnectorEnum::Payme,
        ConnectorEnum::Revolut,
        ConnectorEnum::Gigadat,
        ConnectorEnum::Loonio,
        ConnectorEnum::Wellsfargo,
        ConnectorEnum::Hyperpg,
        ConnectorEnum::Zift,
        ConnectorEnum::Revolv3,
        ConnectorEnum::Truelayer,
        ConnectorEnum::Finix,
        ConnectorEnum::PinelabsOnline,
    ]
}

// ---------------------------------------------------------------------------
// Doc-format overrides for wallet payment methods
// ---------------------------------------------------------------------------
//
// The probe uses internal workaround formats (pre-decrypted Apple Pay data,
// fake Stripe GPay tokens) to make probe runs succeed, but users integrating
// the SDK always receive ENCRYPTED tokens from the device wallets. These
// functions return the correct real-world `payment_method` JSON that should
// appear in the published documentation proto_request.

#[allow(dead_code)]
pub(crate) fn doc_payment_method_override(pm_name: &str) -> Option<serde_json::Value> {
    // Produce the correct proto3 JSON format for wallet payment methods.
    // In proto3 JSON, oneof variants are inlined at the containing-message level
    // (no extra wrapper with the oneof field name). So the value stored in
    // proto_req["payment_method"] should already be the variant, not
    // {"payment_method": {"apple_pay": {...}}}.
    match pm_name {
        "ApplePay" | "ApplePayDecrypted" => Some(serde_json::json!({
            // payment_data is inlined — no "payment_data" oneof wrapper
            "apple_pay": {
                "payment_data": {
                    "encrypted_data": "<base64_encoded_apple_pay_payment_token>"
                },
                "payment_method": {
                    "display_name": "Visa 1111",
                    "network": "Visa",
                    "type": "debit"
                },
                "transaction_identifier": "<apple_pay_transaction_identifier>"
            }
        })),
        "GooglePay" | "GooglePayDecrypted" => Some(serde_json::json!({
            // tokenization_data is inlined — no "tokenization_data" oneof wrapper.
            // "token" is the full JSON string returned by the Google Pay API.
            "google_pay": {
                "type": "CARD",
                "description": "Visa 1111",
                "info": {
                    "card_network": "VISA",
                    "card_details": "1111"
                },
                "tokenization_data": {
                    "encrypted_data": {
                        "token": "{\"version\":\"ECv2\",\"signature\":\"<sig>\",\"intermediateSigningKey\":{\"signedKey\":\"<signed_key>\",\"signatures\":[\"<sig>\"]},\"signedMessage\":\"<signed_message>\"}",
                        "token_type": "PAYMENT_GATEWAY"
                    }
                }
            }
        })),
        _ => None,
    }
}
