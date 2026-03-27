// AUTO-GENERATED — do not edit manually.
// Regenerate: python3 scripts/generate-connector-docs.py --all
//
// Maps connector name (from creds.json) to ConnectorSpecificConfig proto type.

use grpc_api_types::payments::{connector_specific_config, ConnectorSpecificConfig, *};
use hyperswitch_masking::Secret;

fn get_val(
    creds: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Result<String, String> {
    match creds.get(key) {
        Some(serde_json::Value::String(s)) => Ok(s.clone()),
        Some(serde_json::Value::Object(obj)) => obj
            .get("value")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .ok_or_else(|| format!("field {key}: no .value")),
        _ => Err(format!("missing or invalid field: {key}")),
    }
}

fn get_opt_secret(
    creds: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Option<Secret<String>> {
    get_val(creds, key).ok().map(Secret::new)
}

pub fn build_connector_config(
    connector: &str,
    creds: &serde_json::Map<String, serde_json::Value>,
) -> Result<ConnectorSpecificConfig, String> {
    #[allow(clippy::match_single_binding)]
    match connector {
        "adyen" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Adyen(AdyenConfig {
                api_key: Some(Secret::new(get_val(creds, "api_key")?)),
                merchant_account: Some(Secret::new(get_val(creds, "merchant_account")?)),
                review_key: get_opt_secret(creds, "review_key"),
                ..Default::default()
            })),
        }),
        "airwallex" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Airwallex(
                AirwallexConfig {
                    api_key: Some(Secret::new(get_val(creds, "api_key")?)),
                    client_id: Some(Secret::new(get_val(creds, "client_id")?)),
                    ..Default::default()
                },
            )),
        }),
        "bambora" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Bambora(BamboraConfig {
                merchant_id: Some(Secret::new(get_val(creds, "merchant_id")?)),
                api_key: Some(Secret::new(get_val(creds, "api_key")?)),
                ..Default::default()
            })),
        }),
        "bankofamerica" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Bankofamerica(
                BankOfAmericaConfig {
                    api_key: Some(Secret::new(get_val(creds, "api_key")?)),
                    merchant_account: Some(Secret::new(get_val(creds, "merchant_account")?)),
                    api_secret: Some(Secret::new(get_val(creds, "api_secret")?)),
                    ..Default::default()
                },
            )),
        }),
        "billwerk" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Billwerk(
                BillwerkConfig {
                    api_key: Some(Secret::new(get_val(creds, "api_key")?)),
                    public_api_key: Some(Secret::new(get_val(creds, "public_api_key")?)),
                    ..Default::default()
                },
            )),
        }),
        "bluesnap" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Bluesnap(
                BluesnapConfig {
                    username: Some(Secret::new(get_val(creds, "username")?)),
                    password: Some(Secret::new(get_val(creds, "password")?)),
                    ..Default::default()
                },
            )),
        }),
        "braintree" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Braintree(
                BraintreeConfig {
                    public_key: Some(Secret::new(get_val(creds, "public_key")?)),
                    private_key: Some(Secret::new(get_val(creds, "private_key")?)),
                    merchant_account_id: get_opt_secret(creds, "merchant_account_id"),
                    ..Default::default()
                },
            )),
        }),
        "cashtocode" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Cashtocode(
                CashtocodeConfig {
                    ..Default::default()
                },
            )),
        }),
        "cryptopay" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Cryptopay(
                CryptopayConfig {
                    api_key: Some(Secret::new(get_val(creds, "api_key")?)),
                    api_secret: Some(Secret::new(get_val(creds, "api_secret")?)),
                    ..Default::default()
                },
            )),
        }),
        "cybersource" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Cybersource(
                CybersourceConfig {
                    api_key: Some(Secret::new(get_val(creds, "api_key")?)),
                    merchant_account: Some(Secret::new(get_val(creds, "merchant_account")?)),
                    api_secret: Some(Secret::new(get_val(creds, "api_secret")?)),
                    disable_avs: creds.get("disable_avs").and_then(|v| v.as_bool()),
                    disable_cvn: creds.get("disable_cvn").and_then(|v| v.as_bool()),
                    ..Default::default()
                },
            )),
        }),
        "datatrans" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Datatrans(
                DatatransConfig {
                    merchant_id: Some(Secret::new(get_val(creds, "merchant_id")?)),
                    password: Some(Secret::new(get_val(creds, "password")?)),
                    ..Default::default()
                },
            )),
        }),
        "dlocal" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Dlocal(DlocalConfig {
                x_login: Some(Secret::new(get_val(creds, "x_login")?)),
                x_trans_key: Some(Secret::new(get_val(creds, "x_trans_key")?)),
                secret: Some(Secret::new(get_val(creds, "secret")?)),
                ..Default::default()
            })),
        }),
        "elavon" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Elavon(ElavonConfig {
                ssl_merchant_id: Some(Secret::new(get_val(creds, "ssl_merchant_id")?)),
                ssl_user_id: Some(Secret::new(get_val(creds, "ssl_user_id")?)),
                ssl_pin: Some(Secret::new(get_val(creds, "ssl_pin")?)),
                ..Default::default()
            })),
        }),
        "fiserv" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Fiserv(FiservConfig {
                api_key: Some(Secret::new(get_val(creds, "api_key")?)),
                merchant_account: Some(Secret::new(get_val(creds, "merchant_account")?)),
                api_secret: Some(Secret::new(get_val(creds, "api_secret")?)),
                terminal_id: get_opt_secret(creds, "terminal_id"),
                ..Default::default()
            })),
        }),
        "fiservemea" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Fiservemea(
                FiservemeaConfig {
                    api_key: Some(Secret::new(get_val(creds, "api_key")?)),
                    api_secret: Some(Secret::new(get_val(creds, "api_secret")?)),
                    ..Default::default()
                },
            )),
        }),
        "forte" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Forte(ForteConfig {
                api_access_id: Some(Secret::new(get_val(creds, "api_access_id")?)),
                organization_id: Some(Secret::new(get_val(creds, "organization_id")?)),
                location_id: Some(Secret::new(get_val(creds, "location_id")?)),
                api_secret_key: Some(Secret::new(get_val(creds, "api_secret_key")?)),
                ..Default::default()
            })),
        }),
        "getnet" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Getnet(GetnetConfig {
                api_key: Some(Secret::new(get_val(creds, "api_key")?)),
                api_secret: Some(Secret::new(get_val(creds, "api_secret")?)),
                seller_id: Some(Secret::new(get_val(creds, "seller_id")?)),
                ..Default::default()
            })),
        }),
        "globalpay" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Globalpay(
                GlobalpayConfig {
                    app_id: Some(Secret::new(get_val(creds, "app_id")?)),
                    app_key: Some(Secret::new(get_val(creds, "app_key")?)),
                    ..Default::default()
                },
            )),
        }),
        "hipay" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Hipay(HipayConfig {
                api_key: Some(Secret::new(get_val(creds, "api_key")?)),
                api_secret: Some(Secret::new(get_val(creds, "api_secret")?)),
                ..Default::default()
            })),
        }),
        "helcim" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Helcim(HelcimConfig {
                api_key: Some(Secret::new(get_val(creds, "api_key")?)),
                ..Default::default()
            })),
        }),
        "iatapay" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Iatapay(IatapayConfig {
                client_id: Some(Secret::new(get_val(creds, "client_id")?)),
                merchant_id: Some(Secret::new(get_val(creds, "merchant_id")?)),
                client_secret: Some(Secret::new(get_val(creds, "client_secret")?)),
                ..Default::default()
            })),
        }),
        "jpmorgan" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Jpmorgan(
                JpmorganConfig {
                    client_id: Some(Secret::new(get_val(creds, "client_id")?)),
                    client_secret: Some(Secret::new(get_val(creds, "client_secret")?)),
                    company_name: get_opt_secret(creds, "company_name"),
                    product_name: get_opt_secret(creds, "product_name"),
                    merchant_purchase_description: get_opt_secret(
                        creds,
                        "merchant_purchase_description",
                    ),
                    statement_descriptor: get_opt_secret(creds, "statement_descriptor"),
                    ..Default::default()
                },
            )),
        }),
        "mifinity" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Mifinity(
                MifinityConfig {
                    key: Some(Secret::new(get_val(creds, "key")?)),
                    ..Default::default()
                },
            )),
        }),
        "mollie" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Mollie(MollieConfig {
                api_key: Some(Secret::new(get_val(creds, "api_key")?)),
                profile_token: get_opt_secret(creds, "profile_token"),
                ..Default::default()
            })),
        }),
        "multisafepay" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Multisafepay(
                MultisafepayConfig {
                    api_key: Some(Secret::new(get_val(creds, "api_key")?)),
                    ..Default::default()
                },
            )),
        }),
        "nexinets" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Nexinets(
                NexinetsConfig {
                    merchant_id: Some(Secret::new(get_val(creds, "merchant_id")?)),
                    api_key: Some(Secret::new(get_val(creds, "api_key")?)),
                    ..Default::default()
                },
            )),
        }),
        "nexixpay" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Nexixpay(
                NexixpayConfig {
                    api_key: Some(Secret::new(get_val(creds, "api_key")?)),
                    ..Default::default()
                },
            )),
        }),
        "nmi" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Nmi(NmiConfig {
                api_key: Some(Secret::new(get_val(creds, "api_key")?)),
                public_key: get_opt_secret(creds, "public_key"),
                ..Default::default()
            })),
        }),
        "noon" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Noon(NoonConfig {
                api_key: Some(Secret::new(get_val(creds, "api_key")?)),
                application_identifier: Some(Secret::new(get_val(
                    creds,
                    "application_identifier",
                )?)),
                business_identifier: Some(Secret::new(get_val(creds, "business_identifier")?)),
                ..Default::default()
            })),
        }),
        "novalnet" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Novalnet(
                NovalnetConfig {
                    product_activation_key: Some(Secret::new(get_val(
                        creds,
                        "product_activation_key",
                    )?)),
                    payment_access_key: Some(Secret::new(get_val(creds, "payment_access_key")?)),
                    tariff_id: Some(Secret::new(get_val(creds, "tariff_id")?)),
                    ..Default::default()
                },
            )),
        }),
        "nuvei" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Nuvei(NuveiConfig {
                merchant_id: Some(Secret::new(get_val(creds, "merchant_id")?)),
                merchant_site_id: Some(Secret::new(get_val(creds, "merchant_site_id")?)),
                merchant_secret: Some(Secret::new(get_val(creds, "merchant_secret")?)),
                ..Default::default()
            })),
        }),
        "paybox" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Paybox(PayboxConfig {
                site: Some(Secret::new(get_val(creds, "site")?)),
                rank: Some(Secret::new(get_val(creds, "rank")?)),
                key: Some(Secret::new(get_val(creds, "key")?)),
                merchant_id: Some(Secret::new(get_val(creds, "merchant_id")?)),
                ..Default::default()
            })),
        }),
        "payme" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Payme(PaymeConfig {
                seller_payme_id: Some(Secret::new(get_val(creds, "seller_payme_id")?)),
                payme_client_key: get_opt_secret(creds, "payme_client_key"),
                ..Default::default()
            })),
        }),
        "payu" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Payu(PayuConfig {
                api_key: Some(Secret::new(get_val(creds, "api_key")?)),
                api_secret: Some(Secret::new(get_val(creds, "api_secret")?)),
                ..Default::default()
            })),
        }),
        "powertranz" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Powertranz(
                PowertranzConfig {
                    power_tranz_id: Some(Secret::new(get_val(creds, "power_tranz_id")?)),
                    power_tranz_password: Some(Secret::new(get_val(
                        creds,
                        "power_tranz_password",
                    )?)),
                    ..Default::default()
                },
            )),
        }),
        "rapyd" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Rapyd(RapydConfig {
                access_key: Some(Secret::new(get_val(creds, "access_key")?)),
                secret_key: Some(Secret::new(get_val(creds, "secret_key")?)),
                ..Default::default()
            })),
        }),
        "redsys" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Redsys(RedsysConfig {
                merchant_id: Some(Secret::new(get_val(creds, "merchant_id")?)),
                terminal_id: Some(Secret::new(get_val(creds, "terminal_id")?)),
                sha256_pwd: Some(Secret::new(get_val(creds, "sha256_pwd")?)),
                ..Default::default()
            })),
        }),
        "shift4" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Shift4(Shift4Config {
                api_key: Some(Secret::new(get_val(creds, "api_key")?)),
                ..Default::default()
            })),
        }),
        "stax" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Stax(StaxConfig {
                api_key: Some(Secret::new(get_val(creds, "api_key")?)),
                ..Default::default()
            })),
        }),
        "stripe" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Stripe(StripeConfig {
                api_key: Some(Secret::new(get_val(creds, "api_key")?)),
                ..Default::default()
            })),
        }),
        "trustpay" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Trustpay(
                TrustpayConfig {
                    api_key: Some(Secret::new(get_val(creds, "api_key")?)),
                    project_id: Some(Secret::new(get_val(creds, "project_id")?)),
                    secret_key: Some(Secret::new(get_val(creds, "secret_key")?)),
                    ..Default::default()
                },
            )),
        }),
        "tsys" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Tsys(TsysConfig {
                device_id: Some(Secret::new(get_val(creds, "device_id")?)),
                transaction_key: Some(Secret::new(get_val(creds, "transaction_key")?)),
                developer_id: Some(Secret::new(get_val(creds, "developer_id")?)),
                ..Default::default()
            })),
        }),
        "volt" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Volt(VoltConfig {
                username: Some(Secret::new(get_val(creds, "username")?)),
                password: Some(Secret::new(get_val(creds, "password")?)),
                client_id: Some(Secret::new(get_val(creds, "client_id")?)),
                client_secret: Some(Secret::new(get_val(creds, "client_secret")?)),
                ..Default::default()
            })),
        }),
        "wellsfargo" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Wellsfargo(
                WellsfargoConfig {
                    api_key: Some(Secret::new(get_val(creds, "api_key")?)),
                    merchant_account: Some(Secret::new(get_val(creds, "merchant_account")?)),
                    api_secret: Some(Secret::new(get_val(creds, "api_secret")?)),
                    ..Default::default()
                },
            )),
        }),
        "worldpay" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Worldpay(
                WorldpayConfig {
                    username: Some(Secret::new(get_val(creds, "username")?)),
                    password: Some(Secret::new(get_val(creds, "password")?)),
                    entity_id: Some(Secret::new(get_val(creds, "entity_id")?)),
                    merchant_name: get_opt_secret(creds, "merchant_name"),
                    ..Default::default()
                },
            )),
        }),
        "worldpayvantiv" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Worldpayvantiv(
                WorldpayvantivConfig {
                    user: Some(Secret::new(get_val(creds, "user")?)),
                    password: Some(Secret::new(get_val(creds, "password")?)),
                    merchant_id: Some(Secret::new(get_val(creds, "merchant_id")?)),
                    ..Default::default()
                },
            )),
        }),
        "xendit" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Xendit(XenditConfig {
                api_key: Some(Secret::new(get_val(creds, "api_key")?)),
                ..Default::default()
            })),
        }),
        "phonepe" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Phonepe(PhonepeConfig {
                merchant_id: Some(Secret::new(get_val(creds, "merchant_id")?)),
                salt_key: Some(Secret::new(get_val(creds, "salt_key")?)),
                salt_index: Some(Secret::new(get_val(creds, "salt_index")?)),
                ..Default::default()
            })),
        }),
        "cashfree" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Cashfree(
                CashfreeConfig {
                    app_id: Some(Secret::new(get_val(creds, "app_id")?)),
                    secret_key: Some(Secret::new(get_val(creds, "secret_key")?)),
                    ..Default::default()
                },
            )),
        }),
        "paytm" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Paytm(PaytmConfig {
                merchant_id: Some(Secret::new(get_val(creds, "merchant_id")?)),
                merchant_key: Some(Secret::new(get_val(creds, "merchant_key")?)),
                website: Some(Secret::new(get_val(creds, "website")?)),
                client_id: get_opt_secret(creds, "client_id"),
                ..Default::default()
            })),
        }),
        "calida" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Calida(CalidaConfig {
                api_key: Some(Secret::new(get_val(creds, "api_key")?)),
                ..Default::default()
            })),
        }),
        "payload" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Payload(PayloadConfig {
                ..Default::default()
            })),
        }),
        "authipay" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Authipay(
                AuthipayConfig {
                    api_key: Some(Secret::new(get_val(creds, "api_key")?)),
                    api_secret: Some(Secret::new(get_val(creds, "api_secret")?)),
                    ..Default::default()
                },
            )),
        }),
        "silverflow" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Silverflow(
                SilverflowConfig {
                    api_key: Some(Secret::new(get_val(creds, "api_key")?)),
                    api_secret: Some(Secret::new(get_val(creds, "api_secret")?)),
                    merchant_acceptor_key: Some(Secret::new(get_val(
                        creds,
                        "merchant_acceptor_key",
                    )?)),
                    ..Default::default()
                },
            )),
        }),
        "celero" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Celero(CeleroConfig {
                api_key: Some(Secret::new(get_val(creds, "api_key")?)),
                ..Default::default()
            })),
        }),
        "trustpayments" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Trustpayments(
                TrustpaymentsConfig {
                    username: Some(Secret::new(get_val(creds, "username")?)),
                    password: Some(Secret::new(get_val(creds, "password")?)),
                    site_reference: Some(Secret::new(get_val(creds, "site_reference")?)),
                    ..Default::default()
                },
            )),
        }),
        "paysafe" => {
            Ok(ConnectorSpecificConfig {
                config: Some(connector_specific_config::Config::Paysafe(PaysafeConfig {
                    username: Some(Secret::new(get_val(creds, "username")?)),
                    password: Some(Secret::new(get_val(creds, "password")?)),
                    // account_id: ..., // complex type: PaysafePaymentMethodDetails
                    ..Default::default()
                })),
            })
        }
        "barclaycard" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Barclaycard(
                BarclaycardConfig {
                    api_key: Some(Secret::new(get_val(creds, "api_key")?)),
                    merchant_account: Some(Secret::new(get_val(creds, "merchant_account")?)),
                    api_secret: Some(Secret::new(get_val(creds, "api_secret")?)),
                    ..Default::default()
                },
            )),
        }),
        "worldpayxml" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Worldpayxml(
                WorldpayxmlConfig {
                    api_username: Some(Secret::new(get_val(creds, "api_username")?)),
                    api_password: Some(Secret::new(get_val(creds, "api_password")?)),
                    merchant_code: Some(Secret::new(get_val(creds, "merchant_code")?)),
                    ..Default::default()
                },
            )),
        }),
        "revolut" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Revolut(RevolutConfig {
                secret_api_key: Some(Secret::new(get_val(creds, "secret_api_key")?)),
                signing_secret: get_opt_secret(creds, "signing_secret"),
                ..Default::default()
            })),
        }),
        "loonio" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Loonio(LoonioConfig {
                merchant_id: Some(Secret::new(get_val(creds, "merchant_id")?)),
                merchant_token: Some(Secret::new(get_val(creds, "merchant_token")?)),
                ..Default::default()
            })),
        }),
        "gigadat" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Gigadat(GigadatConfig {
                campaign_id: Some(Secret::new(get_val(creds, "campaign_id")?)),
                access_token: Some(Secret::new(get_val(creds, "access_token")?)),
                security_token: Some(Secret::new(get_val(creds, "security_token")?)),
                ..Default::default()
            })),
        }),
        "hyperpg" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Hyperpg(HyperpgConfig {
                username: Some(Secret::new(get_val(creds, "username")?)),
                password: Some(Secret::new(get_val(creds, "password")?)),
                merchant_id: Some(Secret::new(get_val(creds, "merchant_id")?)),
                ..Default::default()
            })),
        }),
        "zift" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Zift(ZiftConfig {
                user_name: Some(Secret::new(get_val(creds, "user_name")?)),
                password: Some(Secret::new(get_val(creds, "password")?)),
                account_id: Some(Secret::new(get_val(creds, "account_id")?)),
                ..Default::default()
            })),
        }),
        "screenstream" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Screenstream(
                ScreenstreamConfig {
                    api_key: Some(Secret::new(get_val(creds, "api_key")?)),
                    ..Default::default()
                },
            )),
        }),
        "ebanx" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Ebanx(EbanxConfig {
                api_key: Some(Secret::new(get_val(creds, "api_key")?)),
                ..Default::default()
            })),
        }),
        "fiuu" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Fiuu(FiuuConfig {
                merchant_id: Some(Secret::new(get_val(creds, "merchant_id")?)),
                verify_key: Some(Secret::new(get_val(creds, "verify_key")?)),
                secret_key: Some(Secret::new(get_val(creds, "secret_key")?)),
                ..Default::default()
            })),
        }),
        "globepay" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Globepay(
                GlobepayConfig {
                    api_key: Some(Secret::new(get_val(creds, "api_key")?)),
                    ..Default::default()
                },
            )),
        }),
        "coinbase" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Coinbase(
                CoinbaseConfig {
                    api_key: Some(Secret::new(get_val(creds, "api_key")?)),
                    ..Default::default()
                },
            )),
        }),
        "coingate" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Coingate(
                CoingateConfig {
                    api_key: Some(Secret::new(get_val(creds, "api_key")?)),
                    ..Default::default()
                },
            )),
        }),
        "revolv3" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Revolv3(Revolv3Config {
                api_key: Some(Secret::new(get_val(creds, "api_key")?)),
                ..Default::default()
            })),
        }),
        "authorizedotnet" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Authorizedotnet(
                AuthorizedotnetConfig {
                    name: Some(Secret::new(get_val(creds, "name")?)),
                    transaction_key: Some(Secret::new(get_val(creds, "transaction_key")?)),
                    ..Default::default()
                },
            )),
        }),
        "peachpayments" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Peachpayments(
                PeachpaymentsConfig {
                    api_key: Some(Secret::new(get_val(creds, "api_key")?)),
                    tenant_id: Some(Secret::new(get_val(creds, "tenant_id")?)),
                    ..Default::default()
                },
            )),
        }),
        "paypal" => Ok(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Paypal(PaypalConfig {
                client_id: Some(Secret::new(get_val(creds, "client_id")?)),
                client_secret: Some(Secret::new(get_val(creds, "client_secret")?)),
                payer_id: get_opt_secret(creds, "payer_id"),
                ..Default::default()
            })),
        }),
        _ => Err(format!(
            "unsupported connector for Rust smoke test: {connector}"
        )),
    }
}
