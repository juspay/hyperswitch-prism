use std::collections::HashMap;
use std::sync::Arc;

use common_utils::metadata::{HeaderMaskingConfig, MaskedMetadata};
use domain_types::{
    connector_types::ConnectorEnum,
    router_data::{
        ConnectorSpecificConfig, PaysafeAchAccountId, PaysafeCardAccountId,
        PaysafePaymentMethodDetails,
    },
};
use hyperswitch_masking::Secret;

pub(crate) fn load_config() -> Arc<ucs_env::configs::Config> {
    ffi::utils::load_config(ffi::handlers::payments::EMBEDDED_DEVELOPMENT_CONFIG)
        .expect("Failed to load dev config")
}

pub(crate) fn make_masked_metadata() -> MaskedMetadata {
    MaskedMetadata::new(
        tonic::metadata::MetadataMap::new(),
        HeaderMaskingConfig::default(),
    )
}

pub(crate) fn dummy_auth(connector: &ConnectorEnum) -> ConnectorSpecificConfig {
    let k = || Secret::new("probe_key".to_string());
    let s = || Secret::new("probe_secret".to_string());
    let m = || Secret::new("probe_merchant".to_string());
    let u = || Secret::new("probe_user".to_string());
    let p = || Secret::new("probe_pass".to_string());
    let id = || Secret::new("probe_id".to_string());

    match connector {
        ConnectorEnum::Stripe => ConnectorSpecificConfig::Stripe {
            api_key: k(),
            base_url: None,
        },
        ConnectorEnum::Calida => ConnectorSpecificConfig::Calida {
            api_key: k(),
            base_url: None,
        },
        ConnectorEnum::Celero => ConnectorSpecificConfig::Celero {
            api_key: k(),
            base_url: None,
        },
        ConnectorEnum::Helcim => ConnectorSpecificConfig::Helcim {
            api_key: k(),
            base_url: None,
        },
        ConnectorEnum::Mifinity => {
            // Load mifinity-specific metadata from connector_metadata
            let mifinity_meta = crate::config::connector_feature_data_json(connector);
            let (brand_id, dest_acct) = if let Some(meta_str) = mifinity_meta {
                // Try to parse the JSON to extract brand_id and destination_account_number
                if let Ok(meta_json) = serde_json::from_str::<serde_json::Value>(&meta_str) {
                    let brand = meta_json
                        .get("brand_id")
                        .and_then(|v| v.as_str())
                        .map(|s| Secret::new(s.to_string()));
                    let dest = meta_json
                        .get("destination_account_number")
                        .and_then(|v| v.as_str())
                        .map(|s| Secret::new(s.to_string()));
                    (brand, dest)
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            };
            ConnectorSpecificConfig::Mifinity {
                key: k(),
                base_url: None,
                brand_id,
                destination_account_number: dest_acct,
            }
        }
        ConnectorEnum::Multisafepay => ConnectorSpecificConfig::Multisafepay {
            api_key: k(),
            base_url: None,
        },
        ConnectorEnum::Nexixpay => ConnectorSpecificConfig::Nexixpay {
            api_key: k(),
            base_url: None,
        },
        ConnectorEnum::Shift4 => ConnectorSpecificConfig::Shift4 {
            api_key: k(),
            base_url: None,
        },
        ConnectorEnum::Stax => ConnectorSpecificConfig::Stax {
            api_key: k(),
            base_url: None,
        },
        ConnectorEnum::Xendit => ConnectorSpecificConfig::Xendit {
            api_key: k(),
            base_url: None,
        },
        ConnectorEnum::Revolut => ConnectorSpecificConfig::Revolut {
            secret_api_key: k(),
            signing_secret: None,
            base_url: None,
        },
        ConnectorEnum::Bambora => ConnectorSpecificConfig::Bambora {
            merchant_id: m(),
            api_key: k(),
            base_url: None,
        },
        ConnectorEnum::Nexinets => ConnectorSpecificConfig::Nexinets {
            merchant_id: m(),
            api_key: k(),
            base_url: None,
        },
        ConnectorEnum::Razorpay => ConnectorSpecificConfig::Razorpay {
            api_key: k(),
            api_secret: Some(s()),
            base_url: None,
        },
        ConnectorEnum::RazorpayV2 => ConnectorSpecificConfig::RazorpayV2 {
            api_key: k(),
            api_secret: Some(s()),
            base_url: None,
        },
        ConnectorEnum::Aci => ConnectorSpecificConfig::Aci {
            api_key: k(),
            entity_id: id(),
            base_url: None,
        },
        ConnectorEnum::Airwallex => ConnectorSpecificConfig::Airwallex {
            api_key: k(),
            client_id: id(),
            base_url: None,
        },
        ConnectorEnum::Authorizedotnet => ConnectorSpecificConfig::Authorizedotnet {
            name: u(),
            transaction_key: k(),
            base_url: None,
        },
        ConnectorEnum::Billwerk => ConnectorSpecificConfig::Billwerk {
            api_key: k(),
            public_api_key: Secret::new("probe_pub_key".to_string()),
            base_url: None,
            secondary_base_url: None,
        },
        ConnectorEnum::Bluesnap => ConnectorSpecificConfig::Bluesnap {
            username: u(),
            password: p(),
            base_url: None,
        },
        ConnectorEnum::Cashfree => ConnectorSpecificConfig::Cashfree {
            app_id: id(),
            secret_key: k(),
            base_url: None,
        },
        ConnectorEnum::Cryptopay => ConnectorSpecificConfig::Cryptopay {
            api_key: k(),
            api_secret: s(),
            base_url: None,
        },
        ConnectorEnum::Datatrans => ConnectorSpecificConfig::Datatrans {
            merchant_id: m(),
            password: p(),
            base_url: None,
        },
        ConnectorEnum::Globalpay => ConnectorSpecificConfig::Globalpay {
            app_id: id(),
            app_key: k(),
            base_url: None,
        },
        ConnectorEnum::Hipay => ConnectorSpecificConfig::Hipay {
            api_key: k(),
            api_secret: s(),
            base_url: None,
            secondary_base_url: None,
            third_base_url: None,
        },
        ConnectorEnum::Jpmorgan => ConnectorSpecificConfig::Jpmorgan {
            client_id: id(),
            client_secret: s(),
            base_url: None,
            secondary_base_url: None,
            company_name: Some(Secret::new("ProbeCompany".to_string())),
            product_name: Some(Secret::new("ProbeProduct".to_string())),
            merchant_purchase_description: Some(Secret::new("Probe Purchase".to_string())),
            statement_descriptor: Some(Secret::new("PROBE".to_string())),
        },
        ConnectorEnum::Loonio => ConnectorSpecificConfig::Loonio {
            merchant_id: m(),
            merchant_token: k(),
            base_url: None,
        },
        ConnectorEnum::Paysafe => ConnectorSpecificConfig::Paysafe {
            username: u(),
            password: p(),
            base_url: None,
            account_id: Some(PaysafePaymentMethodDetails {
                card: Some(HashMap::from([(
                    common_enums::enums::Currency::USD,
                    PaysafeCardAccountId {
                        no_three_ds: Some(Secret::new("probe_acct_no3ds".to_string())),
                        three_ds: Some(Secret::new("probe_acct_3ds".to_string())),
                    },
                )])),
                ach: Some(HashMap::from([(
                    common_enums::enums::Currency::USD,
                    PaysafeAchAccountId {
                        account_id: Some(Secret::new("probe_ach_acct".to_string())),
                    },
                )])),
            }),
        },
        ConnectorEnum::Payu => ConnectorSpecificConfig::Payu {
            api_key: k(),
            api_secret: s(),
            base_url: None,
        },
        ConnectorEnum::Placetopay => ConnectorSpecificConfig::Placetopay {
            login: u(),
            tran_key: k(),
            base_url: None,
        },
        ConnectorEnum::Peachpayments => {
            // Load peachpayments-specific metadata from connector_metadata
            let peach_meta = crate::config::connector_feature_data_json(connector);
            let (client_merchant_ref_id, merchant_route_id) = if let Some(meta_str) = peach_meta {
                // Try to parse the JSON to extract fields
                if let Ok(meta_json) = serde_json::from_str::<serde_json::Value>(&meta_str) {
                    let client_ref = meta_json
                        .get("client_merchant_reference_id")
                        .and_then(|v| v.as_str())
                        .map(|s| Secret::new(s.to_string()));
                    let route_id = meta_json
                        .get("merchant_payment_method_route_id")
                        .and_then(|v| v.as_str())
                        .map(|s| Secret::new(s.to_string()));
                    (client_ref, route_id)
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            };
            ConnectorSpecificConfig::Peachpayments {
                api_key: k(),
                tenant_id: s(),
                base_url: None,
                client_merchant_reference_id: client_merchant_ref_id,
                merchant_payment_method_route_id: merchant_route_id,
            }
        }
        ConnectorEnum::Ppro => ConnectorSpecificConfig::Ppro {
            api_key: k(),
            merchant_id: m(),
            base_url: None,
        },
        ConnectorEnum::Powertranz => ConnectorSpecificConfig::Powertranz {
            power_tranz_id: id(),
            power_tranz_password: p(),
            base_url: None,
        },
        ConnectorEnum::Rapyd => ConnectorSpecificConfig::Rapyd {
            access_key: k(),
            secret_key: s(),
            base_url: None,
        },
        ConnectorEnum::Authipay => ConnectorSpecificConfig::Authipay {
            api_key: k(),
            api_secret: s(),
            base_url: None,
        },
        ConnectorEnum::Fiservemea => ConnectorSpecificConfig::Fiservemea {
            api_key: k(),
            api_secret: s(),
            base_url: None,
        },
        ConnectorEnum::Mollie => ConnectorSpecificConfig::Mollie {
            api_key: k(),
            profile_token: None,
            base_url: None,
            secondary_base_url: None,
        },
        ConnectorEnum::Nmi => ConnectorSpecificConfig::Nmi {
            api_key: k(),
            public_key: None,
            base_url: None,
        },
        ConnectorEnum::Sanlam => ConnectorSpecificConfig::Sanlam {
            api_key: k(),
            merchant_id: m(),
            base_url: None,
        },
        ConnectorEnum::Payme => ConnectorSpecificConfig::Payme {
            seller_payme_id: id(),
            payme_client_key: None,
            base_url: None,
        },
        ConnectorEnum::Braintree => {
            // Load braintree-specific metadata from connector_feature_data
            let braintree_meta = crate::config::connector_feature_data_json(connector);
            let (merchant_acct_id, merchant_config_currency) =
                if let Some(meta_str) = braintree_meta {
                    // Try to parse the JSON to extract fields
                    if let Ok(meta_json) = serde_json::from_str::<serde_json::Value>(&meta_str) {
                        let acct_id = meta_json
                            .get("merchant_account_id")
                            .and_then(|v| v.as_str())
                            .map(|s| Secret::new(s.to_string()));
                        let currency = meta_json
                            .get("merchant_config_currency")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                        (acct_id, currency)
                    } else {
                        (None, None)
                    }
                } else {
                    (None, None)
                };
            ConnectorSpecificConfig::Braintree {
                public_key: k(),
                private_key: s(),
                base_url: None,
                merchant_account_id: merchant_acct_id
                    .or(Some(Secret::new("probe_merchant_account".to_string()))),
                merchant_config_currency: merchant_config_currency.or(Some("USD".to_string())),
                apple_pay_supported_networks: vec![],
                apple_pay_merchant_capabilities: vec![],
                apple_pay_label: None,
                gpay_merchant_name: None,
                gpay_merchant_id: None,
                gpay_allowed_auth_methods: vec![],
                gpay_allowed_card_networks: vec![],
                paypal_client_id: None,
                gpay_gateway_merchant_id: None,
            }
        }
        ConnectorEnum::Truelayer => {
            // Load truelayer-specific metadata from connector_metadata
            let truelayer_meta = crate::config::connector_feature_data_json(connector);
            let (merchant_acct_id, acct_holder_name, private_key_val, kid_val) =
                if let Some(meta_str) = truelayer_meta {
                    // Try to parse the JSON to extract fields
                    if let Ok(meta_json) = serde_json::from_str::<serde_json::Value>(&meta_str) {
                        let merchant_id = meta_json
                            .get("merchant_account_id")
                            .and_then(|v| v.as_str())
                            .map(|s| Secret::new(s.to_string()));
                        let acct_name = meta_json
                            .get("account_holder_name")
                            .and_then(|v| v.as_str())
                            .map(|s| Secret::new(s.to_string()));
                        let pkey = meta_json
                            .get("private_key")
                            .and_then(|v| v.as_str())
                            .map(|s| Secret::new(s.to_string()));
                        let kid_value = meta_json
                            .get("kid")
                            .and_then(|v| v.as_str())
                            .map(|s| Secret::new(s.to_string()));
                        (merchant_id, acct_name, pkey, kid_value)
                    } else {
                        (None, None, None, None)
                    }
                } else {
                    (None, None, None, None)
                };
            ConnectorSpecificConfig::Truelayer {
                client_id: id(),
                client_secret: s(),
                base_url: None,
                secondary_base_url: None,
                merchant_account_id: merchant_acct_id,
                account_holder_name: acct_holder_name,
                private_key: private_key_val,
                kid: kid_val,
            }
        }
        ConnectorEnum::Worldpay => ConnectorSpecificConfig::Worldpay {
            username: u(),
            password: p(),
            entity_id: id(),
            base_url: None,
            merchant_name: Some(Secret::new("Probe Merchant".to_string())),
        },
        ConnectorEnum::Adyen => ConnectorSpecificConfig::Adyen {
            api_key: k(),
            merchant_account: m(),
            review_key: None,
            base_url: None,
            dispute_base_url: None,
            endpoint_prefix: None,
        },
        ConnectorEnum::Bankofamerica => ConnectorSpecificConfig::BankOfAmerica {
            api_key: k(),
            merchant_account: m(),
            // Must be valid base64 — used for HMAC-SHA256 signing
            api_secret: Secret::new("cHJvYmVfc2VjcmV0".to_string()),
            base_url: None,
        },
        ConnectorEnum::Bamboraapac => ConnectorSpecificConfig::Bamboraapac {
            username: u(),
            password: p(),
            account_number: Secret::new("probe_acct_num".to_string()),
            base_url: None,
        },
        ConnectorEnum::Barclaycard => ConnectorSpecificConfig::Barclaycard {
            api_key: k(),
            merchant_account: m(),
            // Must be valid base64 — used for HMAC-SHA256 signing
            api_secret: Secret::new("cHJvYmVfc2VjcmV0".to_string()),
            base_url: None,
        },
        ConnectorEnum::Checkout => ConnectorSpecificConfig::Checkout {
            api_key: k(),
            api_secret: s(),
            processing_channel_id: id(),
            base_url: None,
        },
        ConnectorEnum::Cybersource => ConnectorSpecificConfig::Cybersource {
            api_key: k(),
            merchant_account: m(),
            // Must be valid base64 — used for HMAC-SHA256 signing in header generation
            api_secret: Secret::new("cHJvYmVfc2VjcmV0".to_string()),
            base_url: None,
            disable_avs: None,
            disable_cvn: None,
        },
        ConnectorEnum::Dlocal => ConnectorSpecificConfig::Dlocal {
            x_login: u(),
            x_trans_key: k(),
            secret: s(),
            base_url: None,
        },
        ConnectorEnum::Elavon => ConnectorSpecificConfig::Elavon {
            ssl_merchant_id: m(),
            ssl_user_id: u(),
            ssl_pin: Secret::new("probe_pin".to_string()),
            base_url: None,
        },
        ConnectorEnum::Fiserv => ConnectorSpecificConfig::Fiserv {
            api_key: k(),
            merchant_account: m(),
            api_secret: s(),
            base_url: None,
            terminal_id: None,
        },
        ConnectorEnum::Fiuu => ConnectorSpecificConfig::Fiuu {
            merchant_id: m(),
            verify_key: k(),
            secret_key: s(),
            base_url: None,
            secondary_base_url: None,
        },
        ConnectorEnum::Getnet => ConnectorSpecificConfig::Getnet {
            api_key: k(),
            api_secret: s(),
            seller_id: id(),
            base_url: None,
        },
        ConnectorEnum::Gigadat => ConnectorSpecificConfig::Gigadat {
            security_token: k(),
            access_token: Secret::new("probe_access_token".to_string()),
            campaign_id: id(),
            base_url: None,
            site: Some("probe_site".to_string()),
        },
        ConnectorEnum::Hyperpg => ConnectorSpecificConfig::Hyperpg {
            username: u(),
            password: p(),
            merchant_id: m(),
            base_url: None,
        },
        ConnectorEnum::Iatapay => ConnectorSpecificConfig::Iatapay {
            client_id: id(),
            merchant_id: m(),
            client_secret: s(),
            base_url: None,
        },
        ConnectorEnum::Noon => ConnectorSpecificConfig::Noon {
            api_key: k(),
            business_identifier: id(),
            application_identifier: Secret::new("probe_app_id".to_string()),
            base_url: None,
        },
        ConnectorEnum::Novalnet => ConnectorSpecificConfig::Novalnet {
            product_activation_key: k(),
            payment_access_key: Secret::new("probe_payment_access".to_string()),
            tariff_id: id(),
            base_url: None,
        },
        ConnectorEnum::Nuvei => ConnectorSpecificConfig::Nuvei {
            merchant_id: m(),
            merchant_site_id: id(),
            merchant_secret: s(),
            base_url: None,
        },
        ConnectorEnum::Phonepe => ConnectorSpecificConfig::Phonepe {
            merchant_id: m(),
            salt_key: k(),
            salt_index: Secret::new("1".to_string()),
            base_url: None,
        },
        ConnectorEnum::Redsys => ConnectorSpecificConfig::Redsys {
            merchant_id: m(),
            terminal_id: id(),
            // Must be valid base64 decoding to 24 bytes (3-key 3DES key) —
            // used in des_encrypt() for HMAC signing. "probe_secret" contains '_'
            // which is invalid in standard base64 and causes RequestEncodingFailed.
            sha256_pwd: Secret::new("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".to_string()),
            base_url: None,
        },
        ConnectorEnum::Silverflow => ConnectorSpecificConfig::Silverflow {
            api_key: k(),
            api_secret: s(),
            merchant_acceptor_key: m(),
            base_url: None,
        },
        ConnectorEnum::Trustpay => ConnectorSpecificConfig::Trustpay {
            api_key: k(),
            project_id: id(),
            secret_key: s(),
            base_url: None,
            base_url_bank_redirects: None,
        },
        ConnectorEnum::Trustpayments => ConnectorSpecificConfig::Trustpayments {
            username: u(),
            password: p(),
            site_reference: Secret::new("probe_site_ref".to_string()),
            base_url: None,
        },
        ConnectorEnum::Tsys => ConnectorSpecificConfig::Tsys {
            device_id: id(),
            transaction_key: k(),
            developer_id: Secret::new("probe_dev_id".to_string()),
            base_url: None,
        },
        ConnectorEnum::Wellsfargo => ConnectorSpecificConfig::Wellsfargo {
            api_key: k(),
            merchant_account: m(),
            // Must be valid base64 — used for HMAC-SHA256 signing
            api_secret: Secret::new("cHJvYmVfc2VjcmV0".to_string()),
            base_url: None,
        },
        ConnectorEnum::Worldpayvantiv => ConnectorSpecificConfig::Worldpayvantiv {
            user: u(),
            password: p(),
            merchant_id: m(),
            base_url: None,
            report_group: None,
            merchant_config_currency: None,
            secondary_base_url: None,
        },
        ConnectorEnum::Worldpayxml => ConnectorSpecificConfig::Worldpayxml {
            api_username: u(),
            api_password: p(),
            merchant_code: Secret::new("probe_merchant_code".to_string()),
            base_url: None,
        },
        ConnectorEnum::Zift => ConnectorSpecificConfig::Zift {
            user_name: u(),
            password: p(),
            account_id: id(),
            base_url: None,
        },
        ConnectorEnum::Paypal => ConnectorSpecificConfig::Paypal {
            client_id: id(),
            client_secret: s(),
            payer_id: None,
            base_url: None,
        },
        ConnectorEnum::Forte => ConnectorSpecificConfig::Forte {
            api_access_id: id(),
            organization_id: Secret::new("probe_org_id".to_string()),
            location_id: Secret::new("probe_loc_id".to_string()),
            api_secret_key: k(),
            base_url: None,
        },
        ConnectorEnum::Paybox => ConnectorSpecificConfig::Paybox {
            site: Secret::new("probe_site".to_string()),
            rank: Secret::new("probe_rank".to_string()),
            key: k(),
            merchant_id: m(),
            base_url: None,
        },
        ConnectorEnum::Paytm => ConnectorSpecificConfig::Paytm {
            merchant_id: m(),
            merchant_key: k(),
            website: Secret::new("probe_website".to_string()),
            client_id: None,
            base_url: None,
        },
        ConnectorEnum::Volt => ConnectorSpecificConfig::Volt {
            username: u(),
            password: p(),
            client_id: id(),
            client_secret: s(),
            base_url: None,
            secondary_base_url: None,
        },
        ConnectorEnum::Cashtocode => ConnectorSpecificConfig::Cashtocode {
            auth_key_map: HashMap::new(),
            base_url: None,
        },
        ConnectorEnum::Payload => ConnectorSpecificConfig::Payload {
            auth_key_map: HashMap::from([(
                common_enums::enums::Currency::USD,
                Secret::new(serde_json::json!({
                    "api_key": "probe_key",
                    "processing_account_id": null
                })),
            )]),
            base_url: None,
        },
        ConnectorEnum::Revolv3 => ConnectorSpecificConfig::Revolv3 {
            api_key: k(),
            base_url: None,
        },
        ConnectorEnum::Finix => ConnectorSpecificConfig::Finix {
            finix_user_name: u(),
            finix_password: p(),
            merchant_identity_id: id(),
            merchant_id: m(),
            base_url: None,
        },
        ConnectorEnum::Trustly => ConnectorSpecificConfig::Trustly {
            username: u(),
            password: p(),
            private_key: s(),
            base_url: None,
        },
        ConnectorEnum::Fiservcommercehub => ConnectorSpecificConfig::Fiservcommercehub {
            api_key: k(),
            secret: s(),
            merchant_id: m(),
            terminal_id: id(),
            base_url: None,
        },
        ConnectorEnum::Itaubank => ConnectorSpecificConfig::Itaubank {
            client_id: id(),
            client_secret: s(),
            base_url: None,
        },
        ConnectorEnum::PinelabsOnline => ConnectorSpecificConfig::PinelabsOnline {
            client_id: id(),
            client_secret: s(),
            base_url: None,
        },
    }
}
