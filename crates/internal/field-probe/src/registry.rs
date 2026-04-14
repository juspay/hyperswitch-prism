use domain_types::connector_types::ConnectorEnum;
use grpc_api_types::payments::PaymentMethod;
use hyperswitch_masking::Secret;
use strum::IntoEnumIterator;

use crate::config::get_config;
use crate::sample_data::*;

pub(crate) fn authorize_pm_variants() -> Vec<(&'static str, fn() -> PaymentMethod)> {
    vec![
        // Card
        ("Card", card_payment_method as fn() -> PaymentMethod),
        (
            "BancontactCard",
            bancontact_card_method as fn() -> PaymentMethod,
        ),
        // Wallet
        (
            "ApplePay",
            apple_pay_encrypted_method as fn() -> PaymentMethod,
        ),
        (
            "ApplePayDecrypted",
            apple_pay_method as fn() -> PaymentMethod,
        ),
        (
            "ApplePayThirdPartySdk",
            apple_pay_third_party_sdk_method as fn() -> PaymentMethod,
        ),
        (
            "GooglePay",
            google_pay_encrypted_method as fn() -> PaymentMethod,
        ),
        (
            "GooglePayDecrypted",
            google_pay_decrypted_method as fn() -> PaymentMethod,
        ),
        (
            "GooglePayThirdPartySdk",
            google_pay_third_party_sdk_method as fn() -> PaymentMethod,
        ),
        ("PaypalSdk", paypal_sdk_method as fn() -> PaymentMethod),
        (
            "AmazonPayRedirect",
            amazon_pay_redirect_method as fn() -> PaymentMethod,
        ),
        ("CashappQr", cashapp_qr_method as fn() -> PaymentMethod),
        (
            "PaypalRedirect",
            paypal_redirect_method as fn() -> PaymentMethod,
        ),
        (
            "WeChatPayQr",
            we_chat_pay_qr_method as fn() -> PaymentMethod,
        ),
        (
            "AliPayRedirect",
            ali_pay_redirect_method as fn() -> PaymentMethod,
        ),
        ("RevolutPay", revolut_pay_method as fn() -> PaymentMethod),
        ("Mifinity", mifinity_method as fn() -> PaymentMethod),
        ("Bluecode", bluecode_method as fn() -> PaymentMethod),
        ("Paze", paze_method as fn() -> PaymentMethod),
        (
            "SamsungPay",
            samsung_pay_payment_method as fn() -> PaymentMethod,
        ),
        ("MbWay", mb_way_method as fn() -> PaymentMethod),
        ("Satispay", satispay_method as fn() -> PaymentMethod),
        ("Wero", wero_method as fn() -> PaymentMethod),
        // UPI
        (
            "UpiCollect",
            upi_collect_payment_method as fn() -> PaymentMethod,
        ),
        ("UpiIntent", upi_intent_method as fn() -> PaymentMethod),
        ("UpiQr", upi_qr_method as fn() -> PaymentMethod),
        // Online Banking
        (
            "OnlineBankingThailand",
            online_banking_thailand_method as fn() -> PaymentMethod,
        ),
        (
            "OnlineBankingCzechRepublic",
            online_banking_czech_republic_method as fn() -> PaymentMethod,
        ),
        (
            "OnlineBankingFinland",
            online_banking_finland_method as fn() -> PaymentMethod,
        ),
        (
            "OnlineBankingFpx",
            online_banking_fpx_method as fn() -> PaymentMethod,
        ),
        (
            "OnlineBankingPoland",
            online_banking_poland_method as fn() -> PaymentMethod,
        ),
        (
            "OnlineBankingSlovakia",
            online_banking_slovakia_method as fn() -> PaymentMethod,
        ),
        // Open Banking
        (
            "OpenBankingUk",
            open_banking_uk_method as fn() -> PaymentMethod,
        ),
        (
            "OpenBankingPis",
            open_banking_pis_method as fn() -> PaymentMethod,
        ),
        ("OpenBanking", open_banking_method as fn() -> PaymentMethod),
        // Bank Redirect
        (
            "LocalBankRedirect",
            local_bank_redirect_method as fn() -> PaymentMethod,
        ),
        ("Ideal", ideal_payment_method as fn() -> PaymentMethod),
        ("Sofort", sofort_method as fn() -> PaymentMethod),
        ("Trustly", trustly_method as fn() -> PaymentMethod),
        ("Giropay", giropay_method as fn() -> PaymentMethod),
        ("Eps", eps_method as fn() -> PaymentMethod),
        ("Przelewy24", przelewy24_method as fn() -> PaymentMethod),
        ("Pse", pse_method as fn() -> PaymentMethod),
        ("Blik", blik_payment_method as fn() -> PaymentMethod),
        ("Interac", interac_method as fn() -> PaymentMethod),
        ("Bizum", bizum_method as fn() -> PaymentMethod),
        ("Eft", eft_method as fn() -> PaymentMethod),
        ("DuitNow", duit_now_method as fn() -> PaymentMethod),
        // Bank Transfer
        (
            "AchBankTransfer",
            ach_bank_transfer_method as fn() -> PaymentMethod,
        ),
        (
            "SepaBankTransfer",
            sepa_bank_transfer_method as fn() -> PaymentMethod,
        ),
        (
            "BacsBankTransfer",
            bacs_bank_transfer_method as fn() -> PaymentMethod,
        ),
        (
            "MultibancoBankTransfer",
            multibanco_bank_transfer_method as fn() -> PaymentMethod,
        ),
        (
            "InstantBankTransfer",
            instant_bank_transfer_method as fn() -> PaymentMethod,
        ),
        (
            "InstantBankTransferFinland",
            instant_bank_transfer_finland_method as fn() -> PaymentMethod,
        ),
        (
            "InstantBankTransferPoland",
            instant_bank_transfer_poland_method as fn() -> PaymentMethod,
        ),
        ("Pix", pix_method as fn() -> PaymentMethod),
        (
            "PermataBankTransfer",
            permata_bank_transfer_method as fn() -> PaymentMethod,
        ),
        (
            "BcaBankTransfer",
            bca_bank_transfer_method as fn() -> PaymentMethod,
        ),
        (
            "BniVaBankTransfer",
            bni_va_bank_transfer_method as fn() -> PaymentMethod,
        ),
        (
            "BriVaBankTransfer",
            bri_va_bank_transfer_method as fn() -> PaymentMethod,
        ),
        (
            "CimbVaBankTransfer",
            cimb_va_bank_transfer_method as fn() -> PaymentMethod,
        ),
        (
            "DanamonVaBankTransfer",
            danamon_va_bank_transfer_method as fn() -> PaymentMethod,
        ),
        (
            "MandiriVaBankTransfer",
            mandiri_va_bank_transfer_method as fn() -> PaymentMethod,
        ),
        (
            "LocalBankTransfer",
            local_bank_transfer_method as fn() -> PaymentMethod,
        ),
        (
            "IndonesianBankTransfer",
            indonesian_bank_transfer_method as fn() -> PaymentMethod,
        ),
        // Bank Debit
        ("Ach", ach_payment_method as fn() -> PaymentMethod),
        ("Sepa", sepa_payment_method as fn() -> PaymentMethod),
        ("Bacs", bacs_payment_method as fn() -> PaymentMethod),
        ("Becs", becs_payment_method as fn() -> PaymentMethod),
        (
            "SepaGuaranteedDebit",
            sepa_guaranteed_debit_method as fn() -> PaymentMethod,
        ),
        // BNPL
        ("Affirm", affirm_payment_method as fn() -> PaymentMethod),
        ("Afterpay", afterpay_payment_method as fn() -> PaymentMethod),
        ("Klarna", klarna_payment_method as fn() -> PaymentMethod),
        // Crypto
        ("Crypto", crypto_method as fn() -> PaymentMethod),
        // Reward
        (
            "ClassicReward",
            classic_reward_method as fn() -> PaymentMethod,
        ),
        // Gift Cards / Prepaid
        ("Givex", givex_method as fn() -> PaymentMethod),
        ("PaySafeCard", pay_safe_card_method as fn() -> PaymentMethod),
        // Vouchers
        ("EVoucher", e_voucher_method as fn() -> PaymentMethod),
        ("Boleto", boleto_method as fn() -> PaymentMethod),
        ("Efecty", efecty_method as fn() -> PaymentMethod),
        (
            "PagoEfectivo",
            pago_efectivo_method as fn() -> PaymentMethod,
        ),
        ("RedCompra", red_compra_method as fn() -> PaymentMethod),
        ("RedPagos", red_pagos_method as fn() -> PaymentMethod),
        ("Alfamart", alfamart_method as fn() -> PaymentMethod),
        ("Indomaret", indomaret_method as fn() -> PaymentMethod),
        ("Oxxo", oxxo_method as fn() -> PaymentMethod),
        ("SevenEleven", seven_eleven_method as fn() -> PaymentMethod),
        ("Lawson", lawson_method as fn() -> PaymentMethod),
        ("MiniStop", mini_stop_method as fn() -> PaymentMethod),
        ("FamilyMart", family_mart_method as fn() -> PaymentMethod),
        ("Seicomart", seicomart_method as fn() -> PaymentMethod),
        ("PayEasy", pay_easy_method as fn() -> PaymentMethod),
    ]
}

/// Static variant for config filtering (same as authorize_pm_variants but usable at config load time)
pub(crate) fn authorize_pm_variants_static() -> Vec<(&'static str, fn() -> PaymentMethod)> {
    authorize_pm_variants()
}

/// Build a mock ConnectorState with an access token for OAuth connectors.
/// Checks for connector-specific token overrides first.
pub(crate) fn mock_connector_state(
    connector: Option<&ConnectorEnum>,
) -> grpc_api_types::payments::ConnectorState {
    let config = get_config();

    // Check for connector-specific token override
    let token = if let Some(conn) = connector {
        crate::config::connector_access_token_override(conn)
            .unwrap_or_else(|| config.access_token.token.clone())
    } else {
        config.access_token.token.clone()
    };

    grpc_api_types::payments::ConnectorState {
        access_token: Some(grpc_api_types::payments::AccessToken {
            token: Some(Secret::new(token)),
            token_type: Some(config.access_token.token_type.clone()),
            expires_in_seconds: Some(config.access_token.expires_in_seconds),
        }),
        connector_customer_id: None,
    }
}

/// Returns every connector known to the system by iterating ConnectorEnum.
/// No hardcoding — adding a new variant to ConnectorEnum automatically
/// includes it in probe runs.
pub(crate) fn all_connectors() -> Vec<ConnectorEnum> {
    ConnectorEnum::iter().collect()
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
