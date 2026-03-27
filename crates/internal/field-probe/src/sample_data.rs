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
    use proto::samsung_wallet::{payment_credential::TokenData, PaymentCredential};
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
                    data: Some(Secret::new("probe_samsung_token_data".to_string())),
                }),
            }),
        })),
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
