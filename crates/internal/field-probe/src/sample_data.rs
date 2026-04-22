use std::str::FromStr;

use grpc_api_types::payments::{
    self as proto, payment_method::PaymentMethod as PmVariant, BrowserInformation, CardDetails,
    Money, PaymentMethod,
};
use hyperswitch_masking::Secret;

pub(crate) fn usd_money(minor: i64) -> Money {
    Money {
        minor_amount: minor,
        currency: proto::Currency::Usd as i32,
    }
}

pub(crate) fn full_browser_info() -> BrowserInformation {
    BrowserInformation {
        color_depth: Some(24),
        screen_height: Some(900),
        screen_width: Some(1440),
        java_enabled: Some(false),
        java_script_enabled: Some(true),
        language: Some("en-US".to_string()),
        time_zone_offset_minutes: Some(-480),
        accept_header: Some("application/json".to_string()),
        user_agent: Some("Mozilla/5.0 (probe-bot)".to_string()),
        accept_language: Some("en-US,en;q=0.9".to_string()),
        referer: None,
        ip_address: Some("1.2.3.4".to_string()),
        os_type: None,
        os_version: None,
        device_model: None,
    }
}

// ---------------------------------------------------------------------------
// Payment method builders
// ---------------------------------------------------------------------------

pub(crate) fn card_payment_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::Card(CardDetails {
            card_number: Some(
                cards::CardNumber::from_str("4111111111111111").expect("static test card"),
            ),
            card_exp_month: Some(Secret::new("03".to_string())),
            card_exp_year: Some(Secret::new("2030".to_string())),
            card_cvc: Some(Secret::new("737".to_string())),
            card_holder_name: Some(Secret::new("John Doe".to_string())),
            ..Default::default()
        })),
    }
}

pub(crate) fn sepa_payment_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::Sepa(proto::Sepa {
            iban: Some(Secret::new("DE89370400440532013000".to_string())),
            bank_account_holder_name: Some(Secret::new("John Doe".to_string())),
        })),
    }
}

pub(crate) fn bacs_payment_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::Bacs(proto::Bacs {
            sort_code: Some(Secret::new("200000".to_string())),
            account_number: Some(Secret::new("55779911".to_string())),
            bank_account_holder_name: Some(Secret::new("John Doe".to_string())),
        })),
    }
}

pub(crate) fn ach_payment_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::Ach(proto::Ach {
            routing_number: Some(Secret::new("110000000".to_string())),
            account_number: Some(Secret::new("000123456789".to_string())),
            bank_account_holder_name: Some(Secret::new("John Doe".to_string())),
            ..Default::default()
        })),
    }
}

pub(crate) fn becs_payment_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::Becs(proto::Becs {
            bsb_number: Some(Secret::new("000000".to_string())),
            account_number: Some(Secret::new("000123456".to_string())),
            bank_account_holder_name: Some(Secret::new("John Doe".to_string())),
        })),
    }
}

pub(crate) fn google_pay_decrypted_method() -> PaymentMethod {
    use proto::google_wallet::{tokenization_data::TokenizationData as TD, TokenizationData};
    // Decrypted format - provides card data directly for connectors that support it
    PaymentMethod {
        payment_method: Some(PmVariant::GooglePay(proto::GoogleWallet {
            r#type: "CARD".to_string(),
            description: "Visa 1111".to_string(),
            info: Some(proto::google_wallet::PaymentMethodInfo {
                card_network: "VISA".to_string(),
                card_details: "1111".to_string(),
                assurance_details: None,
            }),
            tokenization_data: Some(TokenizationData {
                tokenization_data: Some(TD::DecryptedData(proto::GooglePayDecryptedData {
                    card_exp_month: Some(Secret::new("03".to_string())),
                    card_exp_year: Some(Secret::new("2030".to_string())),
                    application_primary_account_number: Some(
                        cards::CardNumber::from_str("4111111111111111").expect("static test card"),
                    ),
                    cryptogram: Some(Secret::new("AAAAAA==".to_string())),
                    eci_indicator: Some("05".to_string()),
                })),
            }),
        })),
    }
}

/// Google Pay using encrypted token format (needed by connectors like Stripe that call
/// `get_encrypted_google_pay_token()` and parse it as a JSON struct with a token `id`).
pub(crate) fn google_pay_encrypted_method() -> PaymentMethod {
    use proto::google_wallet::{tokenization_data::TokenizationData as TD, TokenizationData};
    // Stripe parses this as StripeGpayToken { id: String } — provide a minimal JSON.
    let encrypted_token = r#"{"id":"tok_probe_gpay","object":"token","type":"card"}"#;
    PaymentMethod {
        payment_method: Some(PmVariant::GooglePay(proto::GoogleWallet {
            r#type: "CARD".to_string(),
            description: "Visa 1111".to_string(),
            info: Some(proto::google_wallet::PaymentMethodInfo {
                card_network: "VISA".to_string(),
                card_details: "1111".to_string(),
                assurance_details: None,
            }),
            tokenization_data: Some(TokenizationData {
                tokenization_data: Some(TD::EncryptedData(
                    proto::GooglePayEncryptedTokenizationData {
                        token: encrypted_token.to_string(),
                        token_type: "PAYMENT_GATEWAY".to_string(),
                    },
                )),
            }),
        })),
    }
}

#[allow(dead_code)]
pub(crate) fn google_pay_method() -> PaymentMethod {
    google_pay_decrypted_method()
}

/// Apple Pay using the encrypted token format (required by connectors like Nexinets/Novalnet
/// that call `get_apple_pay_encrypted_data()` rather than using decrypted card data).
pub(crate) fn apple_pay_encrypted_method() -> PaymentMethod {
    use proto::apple_wallet::{payment_data::PaymentData as PD, PaymentData};
    PaymentMethod {
        payment_method: Some(PmVariant::ApplePay(proto::AppleWallet {
            payment_data: Some(PaymentData {
                payment_data: Some(PD::EncryptedData(
                    // Valid base64 encoding of a minimal Apple Pay token JSON stub.
                    // Decodes to: {"version":"EC_v1","data":"probe","signature":"probe"}
                    "eyJ2ZXJzaW9uIjoiRUNfdjEiLCJkYXRhIjoicHJvYmUiLCJzaWduYXR1cmUiOiJwcm9iZSJ9"
                        .to_string(),
                )),
            }),
            payment_method: Some(proto::apple_wallet::PaymentMethod {
                display_name: "Visa 1111".to_string(),
                network: "Visa".to_string(),
                r#type: "debit".to_string(),
            }),
            transaction_identifier: "probe_txn_id".to_string(),
        })),
    }
}

pub(crate) fn ideal_payment_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::Ideal(proto::Ideal { bank_name: None })),
    }
}

pub(crate) fn paypal_redirect_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::PaypalRedirect(proto::PaypalRedirectWallet {
            email: Some(Secret::new("test@example.com".to_string())),
        })),
    }
}

pub(crate) fn blik_payment_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::Blik(proto::Blik {
            blik_code: Some("777124".to_string()),
        })),
    }
}

pub(crate) fn klarna_payment_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::Klarna(proto::Klarna {})),
    }
}

pub(crate) fn afterpay_payment_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::AfterpayClearpay(proto::AfterpayClearpay {})),
    }
}

pub(crate) fn upi_collect_payment_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::UpiCollect(proto::UpiCollect {
            vpa_id: Some(Secret::new("test@upi".to_string())),
            upi_source: None,
        })),
    }
}

pub(crate) fn affirm_payment_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::Affirm(proto::Affirm {})),
    }
}

pub(crate) fn samsung_pay_payment_method() -> PaymentMethod {
    use base64::Engine;
    use proto::samsung_wallet::{payment_credential::TokenData, PaymentCredential};

    // SamsungPay token data must be a valid JWT with header containing "kid" field
    // Format: base64url(header).base64url(payload).signature (no padding)
    // Header: {"alg":"RS256","typ":"JWT","kid":"samsung_probe_key_123"}
    let jwt_header = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(r#"{"alg":"RS256","typ":"JWT","kid":"samsung_probe_key_123"}"#);
    let jwt_payload = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(r#"{"paymentMethodToken":"probe_samsung_token"}"#);
    let jwt_signature = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode("dummy_signature");
    let jwt_token = format!("{}.{}.{}", jwt_header, jwt_payload, jwt_signature);

    PaymentMethod {
        payment_method: Some(PmVariant::SamsungPay(proto::SamsungWallet {
            payment_credential: Some(PaymentCredential {
                method: Some("3DS".to_string()),
                recurring_payment: Some(false),
                card_brand: grpc_api_types::payments::CardNetwork::Visa.into(),
                dpan_last_four_digits: None,
                card_last_four_digits: Some(Secret::new("1234".to_string())),
                token_data: Some(TokenData {
                    r#type: Some("S".to_string()),
                    version: "100".to_string(),
                    data: Some(Secret::new(jwt_token)),
                }),
            }),
        })),
    }
}

pub(crate) fn bancontact_card_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::BancontactCard(proto::BancontactCard {
            card_number: Some(
                cards::CardNumber::from_str("4111111111111111").expect("static test card"),
            ),
            card_exp_month: Some(Secret::new("03".to_string())),
            card_exp_year: Some(Secret::new("2030".to_string())),
            card_holder_name: Some(Secret::new("John Doe".to_string())),
        })),
    }
}

pub(crate) fn apple_pay_third_party_sdk_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::ApplePayThirdPartySdk(
            proto::ApplePayThirdPartySdkWallet {
                token: Some(Secret::new("probe_apple_pay_third_party_token".to_string())),
            },
        )),
    }
}

pub(crate) fn google_pay_third_party_sdk_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::GooglePayThirdPartySdk(
            proto::GooglePayThirdPartySdkWallet {
                token: Some(Secret::new(
                    "probe_google_pay_third_party_token".to_string(),
                )),
            },
        )),
    }
}

pub(crate) fn paypal_sdk_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::PaypalSdk(proto::PaypalSdkWallet {
            token: Some(Secret::new("probe_paypal_sdk_token".to_string())),
        })),
    }
}

pub(crate) fn amazon_pay_redirect_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::AmazonPayRedirect(
            proto::AmazonPayRedirectWallet::default(),
        )),
    }
}

pub(crate) fn cashapp_qr_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::CashappQr(proto::CashappQrWallet::default())),
    }
}

pub(crate) fn we_chat_pay_qr_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::WeChatPayQr(proto::WeChatPayQrWallet::default())),
    }
}

pub(crate) fn ali_pay_redirect_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::AliPayRedirect(
            proto::AliPayRedirectWallet::default(),
        )),
    }
}

pub(crate) fn revolut_pay_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::RevolutPay(proto::RevolutPayWallet::default())),
    }
}

pub(crate) fn mifinity_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::Mifinity(proto::MifinityWallet {
            date_of_birth: Some(Secret::new("1990-01-01".to_string())),
            language_preference: Some("en".to_string()),
        })),
    }
}

pub(crate) fn bluecode_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::Bluecode(proto::Bluecode::default())),
    }
}

pub(crate) fn paze_method() -> PaymentMethod {
    use proto::paze_wallet::PazeData;
    PaymentMethod {
        payment_method: Some(PmVariant::Paze(proto::PazeWallet {
            paze_data: Some(PazeData::CompleteResponse(Secret::new(
                "probe_paze_complete_response".to_string(),
            ))),
        })),
    }
}

pub(crate) fn mb_way_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::MbWay(proto::MbWay::default())),
    }
}

pub(crate) fn satispay_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::Satispay(proto::Satispay::default())),
    }
}

pub(crate) fn wero_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::Wero(proto::Wero::default())),
    }
}

pub(crate) fn upi_intent_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::UpiIntent(proto::UpiIntent::default())),
    }
}

pub(crate) fn upi_qr_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::UpiQr(proto::UpiQr::default())),
    }
}

pub(crate) fn online_banking_thailand_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::OnlineBankingThailand(
            proto::OnlineBankingThailand {
                issuer: proto::BankNames::BangkokBank as i32,
            },
        )),
    }
}

pub(crate) fn online_banking_czech_republic_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::OnlineBankingCzechRepublic(
            proto::OnlineBankingCzechRepublic {
                issuer: proto::BankNames::CeskaSporitelna as i32,
            },
        )),
    }
}

pub(crate) fn online_banking_finland_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::OnlineBankingFinland(
            proto::OnlineBankingFinland {
                email: Some(Secret::new("test@example.com".to_string())),
            },
        )),
    }
}

pub(crate) fn online_banking_fpx_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::OnlineBankingFpx(proto::OnlineBankingFpx {
            issuer: proto::BankNames::Maybank as i32,
        })),
    }
}

pub(crate) fn online_banking_poland_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::OnlineBankingPoland(proto::OnlineBankingPoland {
            issuer: proto::BankNames::BankPekaoSa as i32,
        })),
    }
}

pub(crate) fn online_banking_slovakia_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::OnlineBankingSlovakia(
            proto::OnlineBankingSlovakia {
                issuer: proto::BankNames::TatraPay as i32,
            },
        )),
    }
}

pub(crate) fn open_banking_uk_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::OpenBankingUk(proto::OpenBankingUk::default())),
    }
}

pub(crate) fn open_banking_pis_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::OpenBankingPis(proto::OpenBankingPis::default())),
    }
}

pub(crate) fn open_banking_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::OpenBanking(proto::OpenBanking::default())),
    }
}

pub(crate) fn local_bank_redirect_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::LocalBankRedirect(
            proto::LocalBankRedirect::default(),
        )),
    }
}

pub(crate) fn sofort_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::Sofort(proto::Sofort::default())),
    }
}

pub(crate) fn trustly_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::Trustly(proto::Trustly::default())),
    }
}

pub(crate) fn giropay_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::Giropay(proto::Giropay::default())),
    }
}

pub(crate) fn eps_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::Eps(proto::Eps::default())),
    }
}

pub(crate) fn przelewy24_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::Przelewy24(proto::Przelewy24::default())),
    }
}

pub(crate) fn pse_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::Pse(proto::Pse::default())),
    }
}

pub(crate) fn interac_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::Interac(proto::Interac::default())),
    }
}

pub(crate) fn bizum_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::Bizum(proto::Bizum::default())),
    }
}

pub(crate) fn eft_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::EftBankRedirect(proto::EftBankRedirect {
            provider: "ozow".to_string(),
        })),
    }
}

pub(crate) fn duit_now_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::DuitNow(proto::DuitNow::default())),
    }
}

pub(crate) fn ach_bank_transfer_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::AchBankTransfer(proto::AchBankTransfer::default())),
    }
}

pub(crate) fn sepa_bank_transfer_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::SepaBankTransfer(
            proto::SepaBankTransfer::default(),
        )),
    }
}

pub(crate) fn bacs_bank_transfer_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::BacsBankTransfer(
            proto::BacsBankTransfer::default(),
        )),
    }
}

pub(crate) fn multibanco_bank_transfer_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::MultibancoBankTransfer(
            proto::MultibancoBankTransfer::default(),
        )),
    }
}

pub(crate) fn instant_bank_transfer_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::InstantBankTransfer(
            proto::InstantBankTransfer::default(),
        )),
    }
}

pub(crate) fn instant_bank_transfer_finland_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::InstantBankTransferFinland(
            proto::InstantBankTransferFinland::default(),
        )),
    }
}

pub(crate) fn instant_bank_transfer_poland_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::InstantBankTransferPoland(
            proto::InstantBankTransferPoland::default(),
        )),
    }
}

pub(crate) fn pix_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::Pix(proto::PixPayment::default())),
    }
}

pub(crate) fn permata_bank_transfer_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::PermataBankTransfer(
            proto::PermataBankTransfer::default(),
        )),
    }
}

pub(crate) fn bca_bank_transfer_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::BcaBankTransfer(proto::BcaBankTransfer::default())),
    }
}

pub(crate) fn bni_va_bank_transfer_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::BniVaBankTransfer(
            proto::BniVaBankTransfer::default(),
        )),
    }
}

pub(crate) fn bri_va_bank_transfer_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::BriVaBankTransfer(
            proto::BriVaBankTransfer::default(),
        )),
    }
}

pub(crate) fn cimb_va_bank_transfer_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::CimbVaBankTransfer(
            proto::CimbVaBankTransfer::default(),
        )),
    }
}

pub(crate) fn danamon_va_bank_transfer_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::DanamonVaBankTransfer(
            proto::DanamonVaBankTransfer::default(),
        )),
    }
}

pub(crate) fn mandiri_va_bank_transfer_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::MandiriVaBankTransfer(
            proto::MandiriVaBankTransfer::default(),
        )),
    }
}

pub(crate) fn local_bank_transfer_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::LocalBankTransfer(
            proto::LocalBankTransfer::default(),
        )),
    }
}

pub(crate) fn indonesian_bank_transfer_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::IndonesianBankTransfer(
            proto::IndonesianBankTransfer::default(),
        )),
    }
}

pub(crate) fn sepa_guaranteed_debit_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::SepaGuaranteedDebit(proto::SepaGuaranteedDebit {
            iban: Some(Secret::new("DE89370400440532013000".to_string())),
            bank_account_holder_name: Some(Secret::new("John Doe".to_string())),
        })),
    }
}

pub(crate) fn crypto_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::Crypto(proto::CryptoCurrency {
            pay_currency: Some("LTC".to_string()),
            network: Some("litecoin".to_string()),
        })),
    }
}

pub(crate) fn classic_reward_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::ClassicReward(proto::ClassicReward::default())),
    }
}

pub(crate) fn givex_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::Givex(proto::Givex {
            number: Some(Secret::new("6006491000011234".to_string())),
            cvc: Some(Secret::new("7100".to_string())),
        })),
    }
}

pub(crate) fn pay_safe_card_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::PaySafeCard(proto::PaySafeCard::default())),
    }
}

pub(crate) fn e_voucher_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::EVoucher(proto::EVoucher::default())),
    }
}

pub(crate) fn boleto_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::Boleto(proto::Boleto::default())),
    }
}

pub(crate) fn efecty_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::Efecty(proto::Efecty::default())),
    }
}

pub(crate) fn pago_efectivo_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::PagoEfectivo(proto::PagoEfectivo::default())),
    }
}

pub(crate) fn red_compra_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::RedCompra(proto::RedCompra::default())),
    }
}

pub(crate) fn red_pagos_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::RedPagos(proto::RedPagos::default())),
    }
}

pub(crate) fn alfamart_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::Alfamart(proto::Alfamart::default())),
    }
}

pub(crate) fn indomaret_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::Indomaret(proto::Indomaret::default())),
    }
}

pub(crate) fn oxxo_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::Oxxo(proto::Oxxo::default())),
    }
}

pub(crate) fn seven_eleven_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::SevenEleven(proto::SevenEleven::default())),
    }
}

pub(crate) fn lawson_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::Lawson(proto::Lawson::default())),
    }
}

pub(crate) fn mini_stop_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::MiniStop(proto::MiniStop::default())),
    }
}

pub(crate) fn family_mart_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::FamilyMart(proto::FamilyMart::default())),
    }
}

pub(crate) fn seicomart_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::Seicomart(proto::Seicomart::default())),
    }
}

pub(crate) fn pay_easy_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(PmVariant::PayEasy(proto::PayEasy::default())),
    }
}

pub(crate) fn apple_pay_method() -> PaymentMethod {
    use proto::apple_wallet::{payment_data::PaymentData as PD, PaymentData};
    // Use pre-decrypted format so connectors that support it (e.g. Stripe) can build
    // the request using card-like data without needing real decryption.
    // Connectors that require the encrypted path will fall through to their own error.
    PaymentMethod {
        payment_method: Some(PmVariant::ApplePay(proto::AppleWallet {
            payment_data: Some(PaymentData {
                payment_data: Some(PD::DecryptedData(proto::ApplePayDecryptedData {
                    application_primary_account_number: Some(
                        cards::CardNumber::from_str("4111111111111111").expect("static test card"),
                    ),
                    application_expiration_month: Some(Secret::new("03".to_string())),
                    application_expiration_year: Some(Secret::new("2030".to_string())),
                    payment_data: Some(proto::ApplePayCryptogramData {
                        online_payment_cryptogram: Some(Secret::new("AAAAAA==".to_string())),
                        eci_indicator: Some("05".to_string()),
                    }),
                })),
            }),
            payment_method: Some(proto::apple_wallet::PaymentMethod {
                display_name: "Visa 1111".to_string(),
                network: "Visa".to_string(),
                r#type: "debit".to_string(),
            }),
            transaction_identifier: "probe_txn_id".to_string(),
        })),
    }
}
