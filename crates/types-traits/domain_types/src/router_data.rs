use std::collections::HashMap;

use cards::NetworkToken;
use common_utils::{
    ext_traits::{OptionExt, ValueExt},
    types::Money,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, Secret};

use crate::{
    connector_types, errors, payment_method_data,
    utils::{missing_field_err, ForeignTryFrom},
};

pub type Error = error_stack::Report<errors::IntegrationError>;

#[derive(Default, Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(tag = "auth_type")]
pub enum ConnectorAuthType {
    TemporaryAuth,
    HeaderKey {
        api_key: Secret<String>,
    },
    BodyKey {
        api_key: Secret<String>,
        key1: Secret<String>,
    },
    SignatureKey {
        api_key: Secret<String>,
        key1: Secret<String>,
        api_secret: Secret<String>,
    },
    MultiAuthKey {
        api_key: Secret<String>,
        key1: Secret<String>,
        api_secret: Secret<String>,
        key2: Secret<String>,
    },
    CurrencyAuthKey {
        auth_key_map: HashMap<common_enums::enums::Currency, common_utils::pii::SecretSerdeValue>,
    },
    CertificateAuth {
        certificate: Secret<String>,
        private_key: Secret<String>,
    },
    #[default]
    NoKey,
}

impl ConnectorAuthType {
    pub fn from_option_secret_value(
        value: Option<common_utils::pii::SecretSerdeValue>,
    ) -> common_utils::errors::CustomResult<Self, common_utils::errors::ParsingError> {
        value
            .parse_value::<Self>("ConnectorAuthType")
            .change_context(common_utils::errors::ParsingError::StructParseFailure(
                "ConnectorAuthType",
            ))
    }

    pub fn from_secret_value(
        value: common_utils::pii::SecretSerdeValue,
    ) -> common_utils::errors::CustomResult<Self, common_utils::errors::ParsingError> {
        value
            .parse_value::<Self>("ConnectorAuthType")
            .change_context(common_utils::errors::ParsingError::StructParseFailure(
                "ConnectorAuthType",
            ))
    }

    // show only first and last two digits of the key and mask others with *
    // mask the entire key if it's length is less than or equal to 4
    fn mask_key(&self, key: String) -> Secret<String> {
        let key_len = key.len();
        let masked_key = if key_len <= 4 {
            "*".repeat(key_len)
        } else {
            // Show the first two and last two characters, mask the rest with '*'
            let mut masked_key = String::new();
            let key_len = key.len();
            // Iterate through characters by their index
            for (index, character) in key.chars().enumerate() {
                if index < 2 || index >= key_len - 2 {
                    masked_key.push(character); // Keep the first two and last two characters
                } else {
                    masked_key.push('*'); // Mask the middle characters
                }
            }
            masked_key
        };
        Secret::new(masked_key)
    }

    // Mask the keys in the auth_type
    pub fn get_masked_keys(&self) -> Self {
        match self {
            Self::TemporaryAuth => Self::TemporaryAuth,
            Self::NoKey => Self::NoKey,
            Self::HeaderKey { api_key } => Self::HeaderKey {
                api_key: self.mask_key(api_key.clone().expose()),
            },
            Self::BodyKey { api_key, key1 } => Self::BodyKey {
                api_key: self.mask_key(api_key.clone().expose()),
                key1: self.mask_key(key1.clone().expose()),
            },
            Self::SignatureKey {
                api_key,
                key1,
                api_secret,
            } => Self::SignatureKey {
                api_key: self.mask_key(api_key.clone().expose()),
                key1: self.mask_key(key1.clone().expose()),
                api_secret: self.mask_key(api_secret.clone().expose()),
            },
            Self::MultiAuthKey {
                api_key,
                key1,
                api_secret,
                key2,
            } => Self::MultiAuthKey {
                api_key: self.mask_key(api_key.clone().expose()),
                key1: self.mask_key(key1.clone().expose()),
                api_secret: self.mask_key(api_secret.clone().expose()),
                key2: self.mask_key(key2.clone().expose()),
            },
            Self::CurrencyAuthKey { auth_key_map } => Self::CurrencyAuthKey {
                auth_key_map: auth_key_map.clone(),
            },
            Self::CertificateAuth {
                certificate,
                private_key,
            } => Self::CertificateAuth {
                certificate: self.mask_key(certificate.clone().expose()),
                private_key: self.mask_key(private_key.clone().expose()),
            },
        }
    }
}

/// Connector-specific authentication types.
///
/// Each variant holds the exact credentials a specific connector needs,
/// as opposed to the generic `ConnectorAuthType` which uses positional fields.
#[derive(Debug, Default, Clone, serde::Deserialize, serde::Serialize)]
pub struct PaysafePaymentMethodDetails {
    pub card: Option<HashMap<common_enums::enums::Currency, PaysafeCardAccountId>>,
    pub ach: Option<HashMap<common_enums::enums::Currency, PaysafeAchAccountId>>,
}

#[derive(Debug, Default, Clone, serde::Deserialize, serde::Serialize)]
pub struct PaysafeCardAccountId {
    pub no_three_ds: Option<Secret<String>>,
    pub three_ds: Option<Secret<String>>,
}

#[derive(Debug, Default, Clone, serde::Deserialize, serde::Serialize)]
pub struct PaysafeAchAccountId {
    pub account_id: Option<Secret<String>>,
}

impl PaysafePaymentMethodDetails {
    pub fn get_no_three_ds_account_id(
        &self,
        currency: common_enums::enums::Currency,
    ) -> Result<Secret<String>, errors::IntegrationError> {
        self.card
            .as_ref()
            .and_then(|cards| cards.get(&currency))
            .and_then(|card| card.no_three_ds.clone())
            .ok_or(errors::IntegrationError::InvalidConnectorConfig {
                config: "Missing no_3ds account_id",
                context: Default::default(),
            })
    }

    pub fn get_three_ds_account_id(
        &self,
        currency: common_enums::enums::Currency,
    ) -> Result<Secret<String>, errors::IntegrationError> {
        self.card
            .as_ref()
            .and_then(|cards| cards.get(&currency))
            .and_then(|card| card.three_ds.clone())
            .ok_or(errors::IntegrationError::InvalidConnectorConfig {
                config: "Missing 3ds account_id",
                context: Default::default(),
            })
    }

    pub fn get_ach_account_id(
        &self,
        currency: common_enums::enums::Currency,
    ) -> Result<Secret<String>, errors::IntegrationError> {
        self.ach
            .as_ref()
            .and_then(|ach| ach.get(&currency))
            .and_then(|ach| ach.account_id.clone())
            .ok_or(errors::IntegrationError::InvalidConnectorConfig {
                config: "Missing ach account_id",
                context: Default::default(),
            })
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub enum ConnectorSpecificConfig {
    // --- Single-field (HeaderKey) connectors ---
    Stripe {
        api_key: Secret<String>,
        base_url: Option<String>,
    },
    Calida {
        api_key: Secret<String>,
        base_url: Option<String>,
    },
    Celero {
        api_key: Secret<String>,
        base_url: Option<String>,
    },
    Helcim {
        api_key: Secret<String>,
        base_url: Option<String>,
    },
    Mifinity {
        key: Secret<String>,
        base_url: Option<String>,
        brand_id: Option<Secret<String>>,
        destination_account_number: Option<Secret<String>>,
    },
    Multisafepay {
        api_key: Secret<String>,
        base_url: Option<String>,
    },
    Nexixpay {
        api_key: Secret<String>,
        base_url: Option<String>,
    },
    Revolut {
        secret_api_key: Secret<String>,
        signing_secret: Option<Secret<String>>,
        base_url: Option<String>,
    },
    Shift4 {
        api_key: Secret<String>,
        base_url: Option<String>,
    },
    Stax {
        api_key: Secret<String>,
        base_url: Option<String>,
    },
    Xendit {
        api_key: Secret<String>,
        base_url: Option<String>,
    },
    Imerchantsolutions {
        api_key: Secret<String>,
        base_url: Option<String>,
    },
    Bambora {
        merchant_id: Secret<String>,
        api_key: Secret<String>,
        base_url: Option<String>,
    },
    Nexinets {
        merchant_id: Secret<String>,
        api_key: Secret<String>,
        base_url: Option<String>,
    },
    Ppro {
        api_key: Secret<String>,
        merchant_id: Secret<String>,
        base_url: Option<String>,
    },

    // --- Two-field connectors ---
    Razorpay {
        api_key: Secret<String>,
        api_secret: Option<Secret<String>>,
        base_url: Option<String>,
    },
    RazorpayV2 {
        api_key: Secret<String>,
        api_secret: Option<Secret<String>>,
        base_url: Option<String>,
    },
    Aci {
        api_key: Secret<String>,
        entity_id: Secret<String>,
        base_url: Option<String>,
    },
    Airwallex {
        api_key: Secret<String>,
        client_id: Secret<String>,
        base_url: Option<String>,
    },
    Authorizedotnet {
        name: Secret<String>,
        transaction_key: Secret<String>,
        base_url: Option<String>,
    },
    Billwerk {
        api_key: Secret<String>,
        public_api_key: Secret<String>,
        base_url: Option<String>,
        secondary_base_url: Option<String>,
    },
    Bluesnap {
        username: Secret<String>,
        password: Secret<String>,
        base_url: Option<String>,
    },
    Cashfree {
        app_id: Secret<String>,
        secret_key: Secret<String>,
        base_url: Option<String>,
    },
    Cryptopay {
        api_key: Secret<String>,
        api_secret: Secret<String>,
        base_url: Option<String>,
    },
    Datatrans {
        merchant_id: Secret<String>,
        password: Secret<String>,
        base_url: Option<String>,
    },
    Globalpay {
        app_id: Secret<String>,
        app_key: Secret<String>,
        base_url: Option<String>,
    },
    Hipay {
        api_key: Secret<String>,
        api_secret: Secret<String>,
        base_url: Option<String>,
        secondary_base_url: Option<String>,
        third_base_url: Option<String>,
    },
    Jpmorgan {
        client_id: Secret<String>,
        client_secret: Secret<String>,
        base_url: Option<String>,
        secondary_base_url: Option<String>,
        company_name: Option<Secret<String>>,
        product_name: Option<Secret<String>>,
        merchant_purchase_description: Option<Secret<String>>,
        statement_descriptor: Option<Secret<String>>,
    },
    Loonio {
        merchant_id: Secret<String>,
        merchant_token: Secret<String>,
        base_url: Option<String>,
    },
    Paysafe {
        username: Secret<String>,
        password: Secret<String>,
        base_url: Option<String>,
        account_id: Option<PaysafePaymentMethodDetails>,
    },
    Payu {
        api_key: Secret<String>,
        api_secret: Secret<String>,
        base_url: Option<String>,
    },
    Placetopay {
        login: Secret<String>,
        tran_key: Secret<String>,
        base_url: Option<String>,
    },
    Powertranz {
        power_tranz_id: Secret<String>,
        power_tranz_password: Secret<String>,
        base_url: Option<String>,
    },
    Rapyd {
        access_key: Secret<String>,
        secret_key: Secret<String>,
        base_url: Option<String>,
    },
    Authipay {
        api_key: Secret<String>,
        api_secret: Secret<String>,
        base_url: Option<String>,
    },
    Fiservemea {
        api_key: Secret<String>,
        api_secret: Secret<String>,
        base_url: Option<String>,
    },
    Mollie {
        api_key: Secret<String>,
        profile_token: Option<Secret<String>>,
        base_url: Option<String>,
        secondary_base_url: Option<String>,
    },
    Nmi {
        api_key: Secret<String>,
        public_key: Option<Secret<String>>,
        base_url: Option<String>,
    },
    Payme {
        seller_payme_id: Secret<String>,
        payme_client_key: Option<Secret<String>>,
        base_url: Option<String>,
    },
    Peachpayments {
        api_key: Secret<String>,
        tenant_id: Secret<String>,
        base_url: Option<String>,
        client_merchant_reference_id: Option<Secret<String>>,
        merchant_payment_method_route_id: Option<Secret<String>>,
    },
    Braintree {
        public_key: Secret<String>,
        private_key: Secret<String>,
        base_url: Option<String>,
        merchant_account_id: Option<Secret<String>>,
        merchant_config_currency: Option<String>,
        apple_pay_supported_networks: Vec<String>,
        apple_pay_merchant_capabilities: Vec<String>,
        apple_pay_label: Option<String>,
        gpay_merchant_name: Option<String>,
        gpay_merchant_id: Option<String>,
        gpay_allowed_auth_methods: Vec<String>,
        gpay_allowed_card_networks: Vec<String>,
        paypal_client_id: Option<String>,
        gpay_gateway_merchant_id: Option<String>,
    },
    Truelayer {
        client_id: Secret<String>,
        client_secret: Secret<String>,
        merchant_account_id: Option<Secret<String>>,
        account_holder_name: Option<Secret<String>>,
        private_key: Option<Secret<String>>,
        kid: Option<Secret<String>>,
        base_url: Option<String>,
        secondary_base_url: Option<String>,
    },
    Worldpay {
        username: Secret<String>,
        password: Secret<String>,
        entity_id: Secret<String>,
        base_url: Option<String>,
        merchant_name: Option<Secret<String>>,
    },
    Trustly {
        username: Secret<String>,
        password: Secret<String>,
        private_key: Secret<String>,
        base_url: Option<String>,
    },

    // --- Three-field connectors ---
    Adyen {
        api_key: Secret<String>,
        merchant_account: Secret<String>,
        review_key: Option<Secret<String>>,
        base_url: Option<String>,
        dispute_base_url: Option<String>,
        endpoint_prefix: Option<String>,
    },
    BankOfAmerica {
        api_key: Secret<String>,
        merchant_account: Secret<String>,
        api_secret: Secret<String>,
        base_url: Option<String>,
    },
    Bamboraapac {
        username: Secret<String>,
        password: Secret<String>,
        account_number: Secret<String>,
        base_url: Option<String>,
    },
    Barclaycard {
        api_key: Secret<String>,
        merchant_account: Secret<String>,
        api_secret: Secret<String>,
        base_url: Option<String>,
    },
    Checkout {
        api_key: Secret<String>,
        api_secret: Secret<String>,
        processing_channel_id: Secret<String>,
        base_url: Option<String>,
    },
    Cybersource {
        api_key: Secret<String>,
        merchant_account: Secret<String>,
        api_secret: Secret<String>,
        base_url: Option<String>,
        disable_avs: Option<bool>,
        disable_cvn: Option<bool>,
    },
    Dlocal {
        x_login: Secret<String>,
        x_trans_key: Secret<String>,
        secret: Secret<String>,
        base_url: Option<String>,
    },
    Elavon {
        ssl_merchant_id: Secret<String>,
        ssl_user_id: Secret<String>,
        ssl_pin: Secret<String>,
        base_url: Option<String>,
    },
    Fiserv {
        api_key: Secret<String>,
        merchant_account: Secret<String>,
        api_secret: Secret<String>,
        base_url: Option<String>,
        terminal_id: Option<Secret<String>>,
    },
    Fiuu {
        merchant_id: Secret<String>,
        verify_key: Secret<String>,
        secret_key: Secret<String>,
        base_url: Option<String>,
        secondary_base_url: Option<String>,
    },
    Getnet {
        api_key: Secret<String>,
        api_secret: Secret<String>,
        seller_id: Secret<String>,
        base_url: Option<String>,
    },
    Gigadat {
        security_token: Secret<String>,
        access_token: Secret<String>,
        campaign_id: Secret<String>,
        base_url: Option<String>,
        site: Option<String>,
    },
    Hyperpg {
        username: Secret<String>,
        password: Secret<String>,
        merchant_id: Secret<String>,
        base_url: Option<String>,
    },
    Iatapay {
        client_id: Secret<String>,
        merchant_id: Secret<String>,
        client_secret: Secret<String>,
        base_url: Option<String>,
    },
    Noon {
        api_key: Secret<String>,
        business_identifier: Secret<String>,
        application_identifier: Secret<String>,
        base_url: Option<String>,
    },
    Novalnet {
        product_activation_key: Secret<String>,
        payment_access_key: Secret<String>,
        tariff_id: Secret<String>,
        base_url: Option<String>,
    },
    Nuvei {
        merchant_id: Secret<String>,
        merchant_site_id: Secret<String>,
        merchant_secret: Secret<String>,
        base_url: Option<String>,
    },
    Phonepe {
        merchant_id: Secret<String>,
        salt_key: Secret<String>,
        salt_index: Secret<String>,
        base_url: Option<String>,
    },
    Redsys {
        merchant_id: Secret<String>,
        terminal_id: Secret<String>,
        sha256_pwd: Secret<String>,
        base_url: Option<String>,
    },
    Silverflow {
        api_key: Secret<String>,
        api_secret: Secret<String>,
        merchant_acceptor_key: Secret<String>,
        base_url: Option<String>,
    },
    Trustpay {
        api_key: Secret<String>,
        project_id: Secret<String>,
        secret_key: Secret<String>,
        base_url: Option<String>,
        base_url_bank_redirects: Option<String>,
    },
    Trustpayments {
        username: Secret<String>,
        password: Secret<String>,
        site_reference: Secret<String>,
        base_url: Option<String>,
    },
    Tsys {
        device_id: Secret<String>,
        transaction_key: Secret<String>,
        developer_id: Secret<String>,
        base_url: Option<String>,
    },
    Wellsfargo {
        api_key: Secret<String>,
        merchant_account: Secret<String>,
        api_secret: Secret<String>,
        base_url: Option<String>,
    },
    Worldpayvantiv {
        user: Secret<String>,
        password: Secret<String>,
        merchant_id: Secret<String>,
        base_url: Option<String>,
        secondary_base_url: Option<String>,
        report_group: Option<String>,
        merchant_config_currency: Option<String>,
    },
    Worldpayxml {
        api_username: Secret<String>,
        api_password: Secret<String>,
        merchant_code: Secret<String>,
        base_url: Option<String>,
    },
    Zift {
        user_name: Secret<String>,
        password: Secret<String>,
        account_id: Secret<String>,
        base_url: Option<String>,
    },
    Paypal {
        client_id: Secret<String>,
        client_secret: Secret<String>,
        payer_id: Option<Secret<String>>,
        base_url: Option<String>,
    },

    // --- Four+ field connectors ---
    Forte {
        api_access_id: Secret<String>,
        organization_id: Secret<String>,
        location_id: Secret<String>,
        api_secret_key: Secret<String>,
        base_url: Option<String>,
    },
    Paybox {
        site: Secret<String>,
        rank: Secret<String>,
        key: Secret<String>,
        merchant_id: Secret<String>,
        base_url: Option<String>,
    },
    Paytm {
        merchant_id: Secret<String>,
        merchant_key: Secret<String>,
        website: Secret<String>,
        client_id: Option<Secret<String>>,
        base_url: Option<String>,
    },
    Volt {
        username: Secret<String>,
        password: Secret<String>,
        client_id: Secret<String>,
        client_secret: Secret<String>,
        base_url: Option<String>,
        secondary_base_url: Option<String>,
    },
    Cashtocode {
        auth_key_map: HashMap<common_enums::enums::Currency, common_utils::pii::SecretSerdeValue>,
        base_url: Option<String>,
    },
    Payload {
        auth_key_map: HashMap<common_enums::enums::Currency, common_utils::pii::SecretSerdeValue>,
        base_url: Option<String>,
    },

    // --- Proto-only connectors (not in ConnectorEnum, reachable via proto auth path) ---
    Screenstream {
        api_key: Secret<String>,
        base_url: Option<String>,
    },
    Ebanx {
        api_key: Secret<String>,
        base_url: Option<String>,
    },
    Globepay {
        api_key: Secret<String>,
        base_url: Option<String>,
    },
    Coinbase {
        api_key: Secret<String>,
        base_url: Option<String>,
    },
    Coingate {
        api_key: Secret<String>,
        base_url: Option<String>,
    },
    Revolv3 {
        api_key: Secret<String>,
        base_url: Option<String>,
    },
    Finix {
        finix_user_name: Secret<String>,
        finix_password: Secret<String>,
        merchant_identity_id: Secret<String>,
        merchant_id: Secret<String>,
        base_url: Option<String>,
    },
    Fiservcommercehub {
        api_key: Secret<String>,
        secret: Secret<String>,
        merchant_id: Secret<String>,
        terminal_id: Secret<String>,
        base_url: Option<String>,
    },
    Itaubank {
        client_id: Secret<String>,
        client_secret: Secret<String>,
        base_url: Option<String>,
    },
}

impl ConnectorSpecificConfig {
    /// Returns the base_url override if set, allowing runtime override of connector base URLs.
    pub fn base_url_override(&self) -> Option<&str> {
        macro_rules! extract_base_url {
            ($($variant:ident { $($field:ident),* $(,)? }),* $(,)?) => {
                match self {
                    $(Self::$variant { base_url, .. } => base_url.as_deref(),)*
                }
            };
        }
        extract_base_url!(
            Stripe { api_key },
            Calida { api_key },
            Celero { api_key },
            Helcim { api_key },
            Mifinity { key },
            Multisafepay { api_key },
            Nexixpay { api_key },
            Revolut { secret_api_key },
            Shift4 { api_key },
            Stax { api_key },
            Xendit { api_key },
            Bambora {
                merchant_id,
                api_key
            },
            Nexinets {
                merchant_id,
                api_key
            },
            Razorpay { api_key },
            RazorpayV2 { api_key },
            Aci { api_key, entity_id },
            Airwallex { api_key, client_id },
            Authorizedotnet {
                name,
                transaction_key
            },
            Billwerk {
                api_key,
                public_api_key
            },
            Bluesnap { username, password },
            Cashfree { app_id, secret_key },
            Cryptopay {
                api_key,
                api_secret
            },
            Datatrans {
                merchant_id,
                password
            },
            Globalpay { app_id, app_key },
            Hipay {
                api_key,
                api_secret
            },
            Jpmorgan {
                client_id,
                client_secret
            },
            Loonio {
                merchant_id,
                merchant_token
            },
            Paysafe { username, password },
            Payu {
                api_key,
                api_secret
            },
            Placetopay { login, tran_key },
            Powertranz {
                power_tranz_id,
                power_tranz_password
            },
            Rapyd {
                access_key,
                secret_key
            },
            Authipay {
                api_key,
                api_secret
            },
            Fiservemea {
                api_key,
                api_secret
            },
            Mollie { api_key },
            Nmi { api_key },
            Payme { seller_payme_id },
            Braintree {
                public_key,
                private_key
            },
            Truelayer {
                client_id,
                client_secret
            },
            Worldpay {
                username,
                password,
                entity_id
            },
            Adyen {
                api_key,
                merchant_account
            },
            BankOfAmerica {
                api_key,
                merchant_account,
                api_secret
            },
            Bamboraapac {
                username,
                password,
                account_number
            },
            Barclaycard {
                api_key,
                merchant_account,
                api_secret
            },
            Checkout {
                api_key,
                api_secret,
                processing_channel_id
            },
            Cybersource {
                api_key,
                merchant_account,
                api_secret
            },
            Dlocal {
                x_login,
                x_trans_key,
                secret
            },
            Elavon {
                ssl_merchant_id,
                ssl_user_id,
                ssl_pin
            },
            Fiserv {
                api_key,
                merchant_account,
                api_secret
            },
            Fiuu {
                merchant_id,
                verify_key,
                secret_key
            },
            Getnet {
                api_key,
                api_secret,
                seller_id
            },
            Gigadat {
                security_token,
                access_token,
                campaign_id
            },
            Hyperpg {
                username,
                password,
                merchant_id
            },
            Iatapay {
                client_id,
                merchant_id,
                client_secret
            },
            Noon {
                api_key,
                business_identifier,
                application_identifier
            },
            Novalnet {
                product_activation_key,
                payment_access_key,
                tariff_id
            },
            Nuvei {
                merchant_id,
                merchant_site_id,
                merchant_secret
            },
            Phonepe {
                merchant_id,
                salt_key,
                salt_index
            },
            Peachpayments { api_key, tenant_id },
            Redsys {
                merchant_id,
                terminal_id,
                sha256_pwd
            },
            Silverflow {
                api_key,
                api_secret,
                merchant_acceptor_key
            },
            Trustpay {
                api_key,
                project_id,
                secret_key
            },
            Trustpayments {
                username,
                password,
                site_reference
            },
            Tsys {
                device_id,
                transaction_key,
                developer_id
            },
            Wellsfargo {
                api_key,
                merchant_account,
                api_secret
            },
            Worldpayvantiv {
                user,
                password,
                merchant_id
            },
            Worldpayxml {
                api_username,
                api_password,
                merchant_code
            },
            Zift {
                user_name,
                password,
                account_id
            },
            Paypal {
                client_id,
                client_secret
            },
            Forte {
                api_access_id,
                organization_id,
                location_id,
                api_secret_key
            },
            Paybox {
                site,
                rank,
                key,
                merchant_id
            },
            Paytm {
                merchant_id,
                merchant_key,
                website
            },
            Volt {
                username,
                password,
                client_id,
                client_secret
            },
            Cashtocode { auth_key_map },
            Payload { auth_key_map },
            Screenstream { api_key },
            Ebanx { api_key },
            Globepay { api_key },
            Coinbase { api_key },
            Coingate { api_key },
            Revolv3 { api_key },
            Finix {
                finix_user_name,
                finix_password,
                merchant_identity_id,
                merchant_id
            },
            Ppro {
                api_key,
                merchant_id
            },
            Fiservcommercehub {
                api_key,
                secret,
                merchant_id,
                terminal_id
            },
            Trustly {
                username,
                password,
                private_key
            },
            Itaubank {
                client_id,
                client_secret
            },
            Imerchantsolutions { api_key },
        )
    }

    /// Builds a connector patch for runtime config merging.
    ///
    /// This is the only path by which URL overrides in `ConnectorSpecificConfig` should influence
    /// request execution. Connector transformers should continue reading URLs from the effective
    /// merged connector config in `resource_common_data.connectors`.
    pub fn connector_config_override_patch(&self) -> Option<serde_json::Value> {
        let mut connector_patch = serde_json::Map::new();

        if let Some(base_url) = self.base_url_override() {
            connector_patch.insert(
                "base_url".to_string(),
                serde_json::Value::String(base_url.to_string()),
            );
        }

        match self {
            Self::Adyen {
                dispute_base_url: Some(dispute_base_url),
                ..
            } => {
                connector_patch.insert(
                    "dispute_base_url".to_string(),
                    serde_json::Value::String(dispute_base_url.clone()),
                );
            }
            Self::Billwerk {
                secondary_base_url, ..
            }
            | Self::Fiuu {
                secondary_base_url, ..
            }
            | Self::Jpmorgan {
                secondary_base_url, ..
            }
            | Self::Mollie {
                secondary_base_url, ..
            }
            | Self::Truelayer {
                secondary_base_url, ..
            }
            | Self::Volt {
                secondary_base_url, ..
            }
            | Self::Worldpayvantiv {
                secondary_base_url, ..
            } => {
                if let Some(secondary_base_url) = secondary_base_url {
                    connector_patch.insert(
                        "secondary_base_url".to_string(),
                        serde_json::Value::String(secondary_base_url.clone()),
                    );
                }
            }
            Self::Hipay {
                secondary_base_url,
                third_base_url,
                ..
            } => {
                if let Some(secondary_base_url) = secondary_base_url {
                    connector_patch.insert(
                        "secondary_base_url".to_string(),
                        serde_json::Value::String(secondary_base_url.clone()),
                    );
                }
                if let Some(third_base_url) = third_base_url {
                    connector_patch.insert(
                        "third_base_url".to_string(),
                        serde_json::Value::String(third_base_url.clone()),
                    );
                }
            }
            Self::Trustpay {
                base_url_bank_redirects: Some(base_url_bank_redirects),
                ..
            } => {
                connector_patch.insert(
                    "base_url_bank_redirects".to_string(),
                    serde_json::Value::String(base_url_bank_redirects.clone()),
                );
            }
            _ => {}
        }

        if connector_patch.is_empty() {
            return None;
        }

        macro_rules! connector_key {
            ($($variant:ident { $($field:ident),* $(,)? }),* $(,)?) => {
                match self {
                    $(Self::$variant { .. } => stringify!($variant).to_ascii_lowercase(),)*
                }
            };
        }

        let mut connectors = serde_json::Map::new();
        connectors.insert(
            connector_key!(
                Stripe { api_key },
                Calida { api_key },
                Celero { api_key },
                Helcim { api_key },
                Mifinity { key },
                Multisafepay { api_key },
                Nexixpay { api_key },
                Revolut { secret_api_key },
                Shift4 { api_key },
                Stax { api_key },
                Xendit { api_key },
                Bambora {
                    merchant_id,
                    api_key
                },
                Nexinets {
                    merchant_id,
                    api_key
                },
                Razorpay { api_key },
                RazorpayV2 { api_key },
                Aci { api_key, entity_id },
                Airwallex { api_key, client_id },
                Authorizedotnet {
                    name,
                    transaction_key
                },
                Billwerk {
                    api_key,
                    public_api_key
                },
                Bluesnap { username, password },
                Cashfree { app_id, secret_key },
                Cryptopay {
                    api_key,
                    api_secret
                },
                Datatrans {
                    merchant_id,
                    password
                },
                Globalpay { app_id, app_key },
                Hipay {
                    api_key,
                    api_secret
                },
                Jpmorgan {
                    client_id,
                    client_secret
                },
                Loonio {
                    merchant_id,
                    merchant_token
                },
                Paysafe { username, password },
                Payu {
                    api_key,
                    api_secret
                },
                Placetopay { login, tran_key },
                Powertranz {
                    power_tranz_id,
                    power_tranz_password
                },
                Rapyd {
                    access_key,
                    secret_key
                },
                Authipay {
                    api_key,
                    api_secret
                },
                Fiservemea {
                    api_key,
                    api_secret
                },
                Mollie { api_key },
                Nmi { api_key },
                Payme { seller_payme_id },
                Braintree {
                    public_key,
                    private_key
                },
                Truelayer {
                    client_id,
                    client_secret
                },
                Worldpay {
                    username,
                    password,
                    entity_id
                },
                Adyen {
                    api_key,
                    merchant_account
                },
                BankOfAmerica {
                    api_key,
                    merchant_account,
                    api_secret
                },
                Bamboraapac {
                    username,
                    password,
                    account_number
                },
                Barclaycard {
                    api_key,
                    merchant_account,
                    api_secret
                },
                Checkout {
                    api_key,
                    api_secret,
                    processing_channel_id
                },
                Cybersource {
                    api_key,
                    merchant_account,
                    api_secret
                },
                Dlocal {
                    x_login,
                    x_trans_key,
                    secret
                },
                Elavon {
                    ssl_merchant_id,
                    ssl_user_id,
                    ssl_pin
                },
                Fiserv {
                    api_key,
                    merchant_account,
                    api_secret
                },
                Fiuu {
                    merchant_id,
                    verify_key,
                    secret_key
                },
                Getnet {
                    api_key,
                    api_secret,
                    seller_id
                },
                Gigadat {
                    security_token,
                    access_token,
                    campaign_id
                },
                Hyperpg {
                    username,
                    password,
                    merchant_id
                },
                Iatapay {
                    client_id,
                    merchant_id,
                    client_secret
                },
                Noon {
                    api_key,
                    business_identifier,
                    application_identifier
                },
                Novalnet {
                    product_activation_key,
                    payment_access_key,
                    tariff_id
                },
                Nuvei {
                    merchant_id,
                    merchant_site_id,
                    merchant_secret
                },
                Phonepe {
                    merchant_id,
                    salt_key,
                    salt_index
                },
                Peachpayments { api_key, tenant_id },
                Redsys {
                    merchant_id,
                    terminal_id,
                    sha256_pwd
                },
                Silverflow {
                    api_key,
                    api_secret,
                    merchant_acceptor_key
                },
                Trustpay {
                    api_key,
                    project_id,
                    secret_key
                },
                Trustpayments {
                    username,
                    password,
                    site_reference
                },
                Tsys {
                    device_id,
                    transaction_key,
                    developer_id
                },
                Wellsfargo {
                    api_key,
                    merchant_account,
                    api_secret
                },
                Worldpayvantiv {
                    user,
                    password,
                    merchant_id
                },
                Worldpayxml {
                    api_username,
                    api_password,
                    merchant_code
                },
                Zift {
                    user_name,
                    password,
                    account_id
                },
                Paypal {
                    client_id,
                    client_secret
                },
                Forte {
                    api_access_id,
                    organization_id,
                    location_id,
                    api_secret_key
                },
                Paybox {
                    site,
                    rank,
                    key,
                    merchant_id
                },
                Paytm {
                    merchant_id,
                    merchant_key,
                    website
                },
                Volt {
                    username,
                    password,
                    client_id,
                    client_secret
                },
                Cashtocode { auth_key_map },
                Payload { auth_key_map },
                Screenstream { api_key },
                Ebanx { api_key },
                Globepay { api_key },
                Coinbase { api_key },
                Coingate { api_key },
                Revolv3 { api_key },
                Finix {
                    finix_user_name,
                    finix_password,
                    merchant_identity_id,
                    merchant_id
                },
                Ppro {
                    api_key,
                    merchant_id
                },
                Fiservcommercehub {
                    api_key,
                    secret,
                    merchant_id,
                    terminal_id
                },
                Trustly {
                    username,
                    password,
                    private_key
                },
                Itaubank {
                    client_id,
                    client_secret
                },
                Imerchantsolutions { api_key },
            ),
            serde_json::Value::Object(connector_patch),
        );

        let mut top_level = serde_json::Map::new();
        top_level.insert(
            "connectors".to_string(),
            serde_json::Value::Object(connectors),
        );

        Some(serde_json::Value::Object(top_level))
    }
}

impl ForeignTryFrom<grpc_api_types::payments::ConnectorSpecificConfig> for ConnectorSpecificConfig {
    type Error = errors::IntegrationError;

    fn foreign_try_from(
        auth: grpc_api_types::payments::ConnectorSpecificConfig,
    ) -> Result<Self, Error> {
        use grpc_api_types::payments::connector_specific_config::Config as AuthType;

        let err = || errors::IntegrationError::FailedToObtainAuthType {
            context: Default::default(),
        };
        let auth_type = auth.config.ok_or_else(err)?;

        match auth_type {
            AuthType::Adyen(adyen) => Ok(Self::Adyen {
                api_key: adyen.api_key.ok_or_else(err)?,
                merchant_account: adyen.merchant_account.ok_or_else(err)?,
                review_key: adyen.review_key,
                base_url: adyen.base_url,
                dispute_base_url: adyen.dispute_base_url,
                endpoint_prefix: adyen.endpoint_prefix,
            }),
            AuthType::Airwallex(airwallex) => Ok(Self::Airwallex {
                api_key: airwallex.api_key.ok_or_else(err)?,
                client_id: airwallex.client_id.ok_or_else(err)?,
                base_url: airwallex.base_url,
            }),
            AuthType::Bambora(bambora) => Ok(Self::Bambora {
                merchant_id: bambora.merchant_id.ok_or_else(err)?,
                api_key: bambora.api_key.ok_or_else(err)?,
                base_url: bambora.base_url,
            }),
            AuthType::Bankofamerica(bankofamerica) => Ok(Self::BankOfAmerica {
                api_key: bankofamerica.api_key.ok_or_else(err)?,
                merchant_account: bankofamerica.merchant_account.ok_or_else(err)?,
                api_secret: bankofamerica.api_secret.ok_or_else(err)?,
                base_url: bankofamerica.base_url,
            }),
            AuthType::Billwerk(billwerk) => Ok(Self::Billwerk {
                api_key: billwerk.api_key.ok_or_else(err)?,
                public_api_key: billwerk.public_api_key.ok_or_else(err)?,
                base_url: billwerk.base_url,
                secondary_base_url: billwerk.secondary_base_url,
            }),
            AuthType::Bluesnap(bluesnap) => Ok(Self::Bluesnap {
                username: bluesnap.username.ok_or_else(err)?,
                password: bluesnap.password.ok_or_else(err)?,
                base_url: bluesnap.base_url,
            }),
            AuthType::Braintree(braintree) => Ok(Self::Braintree {
                public_key: braintree.public_key.ok_or_else(err)?,
                private_key: braintree.private_key.ok_or_else(err)?,
                base_url: braintree.base_url,
                merchant_account_id: braintree.merchant_account_id,
                merchant_config_currency: braintree.merchant_config_currency,
                apple_pay_supported_networks: braintree.apple_pay_supported_networks,
                apple_pay_merchant_capabilities: braintree.apple_pay_merchant_capabilities,
                apple_pay_label: braintree.apple_pay_label,
                gpay_merchant_name: braintree.gpay_merchant_name,
                gpay_merchant_id: braintree.gpay_merchant_id,
                gpay_allowed_auth_methods: braintree.gpay_allowed_auth_methods,
                gpay_allowed_card_networks: braintree.gpay_allowed_card_networks,
                paypal_client_id: braintree.paypal_client_id,
                gpay_gateway_merchant_id: braintree.gpay_gateway_merchant_id,
            }),
            AuthType::Cashtocode(cashtocode) => Ok(Self::Cashtocode {
                auth_key_map: serde_json::to_value(cashtocode.auth_key_map)
                    .and_then(serde_json::from_value)
                    .map_err(|_| errors::IntegrationError::FailedToObtainAuthType {
                        context: Default::default(),
                    })?,
                base_url: cashtocode.base_url,
            }),
            AuthType::Cryptopay(cryptopay) => Ok(Self::Cryptopay {
                api_key: cryptopay.api_key.ok_or_else(err)?,
                api_secret: cryptopay.api_secret.ok_or_else(err)?,
                base_url: cryptopay.base_url,
            }),
            AuthType::Cybersource(cybersource) => Ok(Self::Cybersource {
                api_key: cybersource.api_key.ok_or_else(err)?,
                merchant_account: cybersource.merchant_account.ok_or_else(err)?,
                api_secret: cybersource.api_secret.ok_or_else(err)?,
                base_url: cybersource.base_url,
                disable_avs: cybersource.disable_avs,
                disable_cvn: cybersource.disable_cvn,
            }),
            AuthType::Datatrans(datatrans) => Ok(Self::Datatrans {
                merchant_id: datatrans.merchant_id.ok_or_else(err)?,
                password: datatrans.password.ok_or_else(err)?,
                base_url: datatrans.base_url,
            }),
            AuthType::Dlocal(dlocal) => Ok(Self::Dlocal {
                x_login: dlocal.x_login.ok_or_else(err)?,
                x_trans_key: dlocal.x_trans_key.ok_or_else(err)?,
                secret: dlocal.secret.ok_or_else(err)?,
                base_url: dlocal.base_url,
            }),
            AuthType::Elavon(elavon) => Ok(Self::Elavon {
                ssl_merchant_id: elavon.ssl_merchant_id.ok_or_else(err)?,
                ssl_user_id: elavon.ssl_user_id.ok_or_else(err)?,
                ssl_pin: elavon.ssl_pin.ok_or_else(err)?,
                base_url: elavon.base_url,
            }),
            AuthType::Fiserv(fiserv) => Ok(Self::Fiserv {
                api_key: fiserv.api_key.ok_or_else(err)?,
                merchant_account: fiserv.merchant_account.ok_or_else(err)?,
                api_secret: fiserv.api_secret.ok_or_else(err)?,
                base_url: fiserv.base_url,
                terminal_id: fiserv.terminal_id,
            }),
            AuthType::Fiservemea(fiservemea) => Ok(Self::Fiservemea {
                api_key: fiservemea.api_key.ok_or_else(err)?,
                api_secret: fiservemea.api_secret.ok_or_else(err)?,
                base_url: fiservemea.base_url,
            }),
            AuthType::Forte(forte) => Ok(Self::Forte {
                api_access_id: forte.api_access_id.ok_or_else(err)?,
                organization_id: forte.organization_id.ok_or_else(err)?,
                location_id: forte.location_id.ok_or_else(err)?,
                api_secret_key: forte.api_secret_key.ok_or_else(err)?,
                base_url: forte.base_url,
            }),
            AuthType::Getnet(getnet) => Ok(Self::Getnet {
                api_key: getnet.api_key.ok_or_else(err)?,
                api_secret: getnet.api_secret.ok_or_else(err)?,
                seller_id: getnet.seller_id.ok_or_else(err)?,
                base_url: getnet.base_url,
            }),
            AuthType::Globalpay(globalpay) => Ok(Self::Globalpay {
                app_id: globalpay.app_id.ok_or_else(err)?,
                app_key: globalpay.app_key.ok_or_else(err)?,
                base_url: globalpay.base_url,
            }),
            AuthType::Hipay(hipay) => Ok(Self::Hipay {
                api_key: hipay.api_key.ok_or_else(err)?,
                api_secret: hipay.api_secret.ok_or_else(err)?,
                base_url: hipay.base_url,
                secondary_base_url: hipay.secondary_base_url,
                third_base_url: hipay.third_base_url,
            }),
            AuthType::Helcim(helcim) => Ok(Self::Helcim {
                api_key: helcim.api_key.ok_or_else(err)?,
                base_url: helcim.base_url,
            }),
            AuthType::Iatapay(iatapay) => Ok(Self::Iatapay {
                client_id: iatapay.client_id.ok_or_else(err)?,
                merchant_id: iatapay.merchant_id.ok_or_else(err)?,
                client_secret: iatapay.client_secret.ok_or_else(err)?,
                base_url: iatapay.base_url,
            }),
            AuthType::Jpmorgan(jpmorgan) => Ok(Self::Jpmorgan {
                client_id: jpmorgan.client_id.ok_or_else(err)?,
                client_secret: jpmorgan.client_secret.ok_or_else(err)?,
                base_url: jpmorgan.base_url,
                secondary_base_url: jpmorgan.secondary_base_url,
                company_name: jpmorgan.company_name,
                product_name: jpmorgan.product_name,
                merchant_purchase_description: jpmorgan.merchant_purchase_description,
                statement_descriptor: jpmorgan.statement_descriptor,
            }),
            AuthType::Mifinity(mifinity) => Ok(Self::Mifinity {
                key: mifinity.key.ok_or_else(err)?,
                base_url: mifinity.base_url,
                brand_id: mifinity.brand_id,
                destination_account_number: mifinity.destination_account_number,
            }),
            AuthType::Mollie(mollie) => Ok(Self::Mollie {
                api_key: mollie.api_key.ok_or_else(err)?,
                profile_token: mollie.profile_token,
                base_url: mollie.base_url,
                secondary_base_url: mollie.secondary_base_url,
            }),
            AuthType::Multisafepay(multisafepay) => Ok(Self::Multisafepay {
                api_key: multisafepay.api_key.ok_or_else(err)?,
                base_url: multisafepay.base_url,
            }),
            AuthType::Nexinets(nexinets) => Ok(Self::Nexinets {
                merchant_id: nexinets.merchant_id.ok_or_else(err)?,
                api_key: nexinets.api_key.ok_or_else(err)?,
                base_url: nexinets.base_url,
            }),
            AuthType::Nexixpay(nexixpay) => Ok(Self::Nexixpay {
                api_key: nexixpay.api_key.ok_or_else(err)?,
                base_url: nexixpay.base_url,
            }),
            AuthType::Nmi(nmi) => Ok(Self::Nmi {
                api_key: nmi.api_key.ok_or_else(err)?,
                public_key: nmi.public_key,
                base_url: nmi.base_url,
            }),
            AuthType::Noon(noon) => Ok(Self::Noon {
                api_key: noon.api_key.ok_or_else(err)?,
                business_identifier: noon.business_identifier.ok_or_else(err)?,
                application_identifier: noon.application_identifier.ok_or_else(err)?,
                base_url: noon.base_url,
            }),
            AuthType::Novalnet(novalnet) => Ok(Self::Novalnet {
                product_activation_key: novalnet.product_activation_key.ok_or_else(err)?,
                payment_access_key: novalnet.payment_access_key.ok_or_else(err)?,
                tariff_id: novalnet.tariff_id.ok_or_else(err)?,
                base_url: novalnet.base_url,
            }),
            AuthType::Nuvei(nuvei) => Ok(Self::Nuvei {
                merchant_id: nuvei.merchant_id.ok_or_else(err)?,
                merchant_site_id: nuvei.merchant_site_id.ok_or_else(err)?,
                merchant_secret: nuvei.merchant_secret.ok_or_else(err)?,
                base_url: nuvei.base_url,
            }),
            AuthType::Paybox(paybox) => Ok(Self::Paybox {
                site: paybox.site.ok_or_else(err)?,
                rank: paybox.rank.ok_or_else(err)?,
                key: paybox.key.ok_or_else(err)?,
                merchant_id: paybox.merchant_id.ok_or_else(err)?,
                base_url: paybox.base_url,
            }),
            AuthType::Payme(payme) => Ok(Self::Payme {
                seller_payme_id: payme.seller_payme_id.ok_or_else(err)?,
                payme_client_key: payme.payme_client_key,
                base_url: payme.base_url,
            }),
            AuthType::Payu(payu) => Ok(Self::Payu {
                api_key: payu.api_key.ok_or_else(err)?,
                api_secret: payu.api_secret.ok_or_else(err)?,
                base_url: payu.base_url,
            }),
            AuthType::Powertranz(powertranz) => Ok(Self::Powertranz {
                power_tranz_id: powertranz.power_tranz_id.ok_or_else(err)?,
                power_tranz_password: powertranz.power_tranz_password.ok_or_else(err)?,
                base_url: powertranz.base_url,
            }),
            AuthType::Rapyd(rapyd) => Ok(Self::Rapyd {
                access_key: rapyd.access_key.ok_or_else(err)?,
                secret_key: rapyd.secret_key.ok_or_else(err)?,
                base_url: rapyd.base_url,
            }),
            AuthType::Redsys(redsys) => Ok(Self::Redsys {
                merchant_id: redsys.merchant_id.ok_or_else(err)?,
                terminal_id: redsys.terminal_id.ok_or_else(err)?,
                sha256_pwd: redsys.sha256_pwd.ok_or_else(err)?,
                base_url: redsys.base_url,
            }),
            AuthType::Shift4(shift4) => Ok(Self::Shift4 {
                api_key: shift4.api_key.ok_or_else(err)?,
                base_url: shift4.base_url,
            }),
            AuthType::Stax(stax) => Ok(Self::Stax {
                api_key: stax.api_key.ok_or_else(err)?,
                base_url: stax.base_url,
            }),
            AuthType::Stripe(stripe) => Ok(Self::Stripe {
                api_key: stripe.api_key.ok_or_else(err)?,
                base_url: stripe.base_url,
            }),
            AuthType::Trustpay(trustpay) => Ok(Self::Trustpay {
                api_key: trustpay.api_key.ok_or_else(err)?,
                project_id: trustpay.project_id.ok_or_else(err)?,
                secret_key: trustpay.secret_key.ok_or_else(err)?,
                base_url: trustpay.base_url,
                base_url_bank_redirects: trustpay.base_url_bank_redirects,
            }),
            AuthType::Tsys(tsys) => Ok(Self::Tsys {
                device_id: tsys.device_id.ok_or_else(err)?,
                transaction_key: tsys.transaction_key.ok_or_else(err)?,
                developer_id: tsys.developer_id.ok_or_else(err)?,
                base_url: tsys.base_url,
            }),
            AuthType::Volt(volt) => Ok(Self::Volt {
                username: volt.username.ok_or_else(err)?,
                password: volt.password.ok_or_else(err)?,
                client_id: volt.client_id.ok_or_else(err)?,
                client_secret: volt.client_secret.ok_or_else(err)?,
                base_url: volt.base_url,
                secondary_base_url: volt.secondary_base_url,
            }),
            AuthType::Wellsfargo(wellsfargo) => Ok(Self::Wellsfargo {
                api_key: wellsfargo.api_key.ok_or_else(err)?,
                merchant_account: wellsfargo.merchant_account.ok_or_else(err)?,
                api_secret: wellsfargo.api_secret.ok_or_else(err)?,
                base_url: wellsfargo.base_url,
            }),
            AuthType::Worldpay(worldpay) => Ok(Self::Worldpay {
                username: worldpay.username.ok_or_else(err)?,
                password: worldpay.password.ok_or_else(err)?,
                entity_id: worldpay.entity_id.ok_or_else(err)?,
                base_url: worldpay.base_url,
                merchant_name: worldpay.merchant_name,
            }),
            AuthType::Worldpayvantiv(worldpayvantiv) => Ok(Self::Worldpayvantiv {
                user: worldpayvantiv.user.ok_or_else(err)?,
                password: worldpayvantiv.password.ok_or_else(err)?,
                merchant_id: worldpayvantiv.merchant_id.ok_or_else(err)?,
                base_url: worldpayvantiv.base_url,
                secondary_base_url: worldpayvantiv.secondary_base_url,
                report_group: worldpayvantiv.report_group,
                merchant_config_currency: worldpayvantiv.merchant_config_currency,
            }),
            AuthType::Xendit(xendit) => Ok(Self::Xendit {
                api_key: xendit.api_key.ok_or_else(err)?,
                base_url: xendit.base_url,
            }),
            AuthType::Phonepe(phonepe) => Ok(Self::Phonepe {
                merchant_id: phonepe.merchant_id.ok_or_else(err)?,
                salt_key: phonepe.salt_key.ok_or_else(err)?,
                salt_index: phonepe.salt_index.ok_or_else(err)?,
                base_url: phonepe.base_url,
            }),
            AuthType::Cashfree(cashfree) => Ok(Self::Cashfree {
                app_id: cashfree.app_id.ok_or_else(err)?,
                secret_key: cashfree.secret_key.ok_or_else(err)?,
                base_url: cashfree.base_url,
            }),
            AuthType::Paytm(paytm) => Ok(Self::Paytm {
                merchant_id: paytm.merchant_id.ok_or_else(err)?,
                merchant_key: paytm.merchant_key.ok_or_else(err)?,
                website: paytm.website.ok_or_else(err)?,
                client_id: paytm.client_id,
                base_url: paytm.base_url,
            }),
            AuthType::Calida(calida) => Ok(Self::Calida {
                api_key: calida.api_key.ok_or_else(err)?,
                base_url: calida.base_url,
            }),
            AuthType::Payload(payload) => Ok(Self::Payload {
                auth_key_map: serde_json::to_value(payload.auth_key_map)
                    .and_then(serde_json::from_value)
                    .map_err(|_| errors::IntegrationError::FailedToObtainAuthType {
                        context: Default::default(),
                    })?,
                base_url: payload.base_url,
            }),
            AuthType::Authipay(authipay) => Ok(Self::Authipay {
                api_key: authipay.api_key.ok_or_else(err)?,
                api_secret: authipay.api_secret.ok_or_else(err)?,
                base_url: authipay.base_url,
            }),
            AuthType::Silverflow(silverflow) => Ok(Self::Silverflow {
                api_key: silverflow.api_key.ok_or_else(err)?,
                api_secret: silverflow.api_secret.ok_or_else(err)?,
                merchant_acceptor_key: silverflow.merchant_acceptor_key.ok_or_else(err)?,
                base_url: silverflow.base_url,
            }),
            AuthType::Celero(celero) => Ok(Self::Celero {
                api_key: celero.api_key.ok_or_else(err)?,
                base_url: celero.base_url,
            }),
            AuthType::Trustpayments(trustpayments) => Ok(Self::Trustpayments {
                username: trustpayments.username.ok_or_else(err)?,
                password: trustpayments.password.ok_or_else(err)?,
                site_reference: trustpayments.site_reference.ok_or_else(err)?,
                base_url: trustpayments.base_url,
            }),
            AuthType::Paysafe(paysafe) => Ok(Self::Paysafe {
                username: paysafe.username.ok_or_else(err)?,
                password: paysafe.password.ok_or_else(err)?,
                base_url: paysafe.base_url,
                account_id: paysafe
                    .account_id
                    .map(|account_id| {
                        serde_json::to_value(account_id)
                            .and_then(serde_json::from_value)
                            .map_err(|_| errors::IntegrationError::FailedToObtainAuthType {
                                context: Default::default(),
                            })
                    })
                    .transpose()?,
            }),
            AuthType::Barclaycard(barclaycard) => Ok(Self::Barclaycard {
                api_key: barclaycard.api_key.ok_or_else(err)?,
                merchant_account: barclaycard.merchant_account.ok_or_else(err)?,
                api_secret: barclaycard.api_secret.ok_or_else(err)?,
                base_url: barclaycard.base_url,
            }),
            AuthType::Worldpayxml(worldpayxml) => Ok(Self::Worldpayxml {
                api_username: worldpayxml.api_username.ok_or_else(err)?,
                api_password: worldpayxml.api_password.ok_or_else(err)?,
                merchant_code: worldpayxml.merchant_code.ok_or_else(err)?,
                base_url: worldpayxml.base_url,
            }),
            AuthType::Revolut(revolut) => Ok(Self::Revolut {
                secret_api_key: revolut.secret_api_key.ok_or_else(err)?,
                signing_secret: revolut.signing_secret,
                base_url: revolut.base_url,
            }),
            AuthType::Loonio(loonio) => Ok(Self::Loonio {
                merchant_id: loonio.merchant_id.ok_or_else(err)?,
                merchant_token: loonio.merchant_token.ok_or_else(err)?,
                base_url: loonio.base_url,
            }),
            AuthType::Gigadat(gigadat) => Ok(Self::Gigadat {
                security_token: gigadat.security_token.ok_or_else(err)?,
                access_token: gigadat.access_token.ok_or_else(err)?,
                campaign_id: gigadat.campaign_id.ok_or_else(err)?,
                base_url: gigadat.base_url,
                site: gigadat.site,
            }),
            AuthType::Hyperpg(hyperpg) => Ok(Self::Hyperpg {
                username: hyperpg.username.ok_or_else(err)?,
                password: hyperpg.password.ok_or_else(err)?,
                merchant_id: hyperpg.merchant_id.ok_or_else(err)?,
                base_url: hyperpg.base_url,
            }),
            AuthType::Zift(zift) => Ok(Self::Zift {
                user_name: zift.user_name.ok_or_else(err)?,
                password: zift.password.ok_or_else(err)?,
                account_id: zift.account_id.ok_or_else(err)?,
                base_url: zift.base_url,
            }),
            AuthType::Screenstream(screenstream) => Ok(Self::Screenstream {
                api_key: screenstream.api_key.ok_or_else(err)?,
                base_url: screenstream.base_url,
            }),
            AuthType::Ebanx(ebanx) => Ok(Self::Ebanx {
                api_key: ebanx.api_key.ok_or_else(err)?,
                base_url: ebanx.base_url,
            }),
            AuthType::Fiuu(fiuu) => Ok(Self::Fiuu {
                merchant_id: fiuu.merchant_id.ok_or_else(err)?,
                verify_key: fiuu.verify_key.ok_or_else(err)?,
                secret_key: fiuu.secret_key.ok_or_else(err)?,
                base_url: fiuu.base_url,
                secondary_base_url: fiuu.secondary_base_url,
            }),
            AuthType::Globepay(globepay) => Ok(Self::Globepay {
                api_key: globepay.api_key.ok_or_else(err)?,
                base_url: globepay.base_url,
            }),
            AuthType::Coinbase(coinbase) => Ok(Self::Coinbase {
                api_key: coinbase.api_key.ok_or_else(err)?,
                base_url: coinbase.base_url,
            }),
            AuthType::Coingate(coingate) => Ok(Self::Coingate {
                api_key: coingate.api_key.ok_or_else(err)?,
                base_url: coingate.base_url,
            }),
            AuthType::Revolv3(revolv3) => Ok(Self::Revolv3 {
                api_key: revolv3.api_key.ok_or_else(err)?,
                base_url: revolv3.base_url,
            }),
            AuthType::Authorizedotnet(authorizedotnet) => Ok(Self::Authorizedotnet {
                name: authorizedotnet.name.ok_or_else(err)?,
                transaction_key: authorizedotnet.transaction_key.ok_or_else(err)?,
                base_url: authorizedotnet.base_url,
            }),
            AuthType::Peachpayments(peachpayments) => Ok(Self::Peachpayments {
                api_key: peachpayments.api_key.ok_or_else(err)?,
                tenant_id: peachpayments.tenant_id.ok_or_else(err)?,
                base_url: peachpayments.base_url,
                client_merchant_reference_id: peachpayments.client_merchant_reference_id,
                merchant_payment_method_route_id: peachpayments.merchant_payment_method_route_id,
            }),
            AuthType::Paypal(paypal) => Ok(Self::Paypal {
                client_id: paypal.client_id.ok_or_else(err)?,
                client_secret: paypal.client_secret.ok_or_else(err)?,
                payer_id: paypal.payer_id,
                base_url: paypal.base_url,
            }),
            AuthType::Trustly(trustly) => Ok(Self::Trustly {
                username: trustly.username.ok_or_else(err)?,
                password: trustly.password.ok_or_else(err)?,
                private_key: trustly.private_key.ok_or_else(err)?,
                base_url: trustly.base_url,
            }),
            AuthType::Truelayer(truelayer) => Ok(Self::Truelayer {
                client_id: truelayer.client_id.ok_or_else(err)?,
                client_secret: truelayer.client_secret.ok_or_else(err)?,
                merchant_account_id: truelayer.merchant_account_id,
                account_holder_name: truelayer.account_holder_name,
                private_key: truelayer.private_key,
                kid: truelayer.kid,
                base_url: truelayer.base_url,
                secondary_base_url: truelayer.secondary_base_url,
            }),
            AuthType::Fiservcommercehub(fiservcommercehub) => Ok(Self::Fiservcommercehub {
                api_key: fiservcommercehub.api_key.ok_or_else(err)?,
                secret: fiservcommercehub.secret.ok_or_else(err)?,
                merchant_id: fiservcommercehub.merchant_id.ok_or_else(err)?,
                terminal_id: fiservcommercehub.terminal_id.ok_or_else(err)?,
                base_url: fiservcommercehub.base_url,
            }),
            AuthType::Itaubank(itaubank) => Ok(Self::Itaubank {
                client_secret: itaubank.client_secret.ok_or_else(err)?,
                client_id: itaubank.client_id.ok_or_else(err)?,
                base_url: itaubank.base_url,
            }),
            AuthType::Ppro(ppro) => Ok(Self::Ppro {
                api_key: ppro.api_key.ok_or_else(err)?,
                merchant_id: ppro.merchant_id.ok_or_else(err)?,
                base_url: ppro.base_url,
            }),
            AuthType::Imerchantsolutions(imerchantsolutions) => Ok(Self::Imerchantsolutions {
                api_key: imerchantsolutions.api_key.ok_or_else(err)?,
                base_url: imerchantsolutions.base_url,
            }),
        }
    }
}

impl ForeignTryFrom<(&ConnectorAuthType, &connector_types::ConnectorEnum)>
    for ConnectorSpecificConfig
{
    type Error = errors::IntegrationError;

    fn foreign_try_from(
        (auth, connector): (&ConnectorAuthType, &connector_types::ConnectorEnum),
    ) -> Result<Self, Error> {
        use connector_types::ConnectorEnum;

        let err = || errors::IntegrationError::FailedToObtainAuthType {
            context: Default::default(),
        };

        match connector {
            // --- HeaderKey connectors ---
            ConnectorEnum::Stripe => match auth {
                ConnectorAuthType::HeaderKey { api_key } => Ok(Self::Stripe {
                    api_key: api_key.clone(),
                    base_url: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Calida => match auth {
                ConnectorAuthType::HeaderKey { api_key } => Ok(Self::Calida {
                    api_key: api_key.clone(),
                    base_url: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Celero => match auth {
                ConnectorAuthType::HeaderKey { api_key } => Ok(Self::Celero {
                    api_key: api_key.clone(),
                    base_url: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Helcim => match auth {
                ConnectorAuthType::HeaderKey { api_key } => Ok(Self::Helcim {
                    api_key: api_key.clone(),
                    base_url: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Mifinity => match auth {
                ConnectorAuthType::HeaderKey { api_key } => Ok(Self::Mifinity {
                    key: api_key.clone(),
                    base_url: None,
                    brand_id: None,
                    destination_account_number: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Multisafepay => match auth {
                ConnectorAuthType::HeaderKey { api_key } => Ok(Self::Multisafepay {
                    api_key: api_key.clone(),
                    base_url: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Nexixpay => match auth {
                ConnectorAuthType::HeaderKey { api_key } => Ok(Self::Nexixpay {
                    api_key: api_key.clone(),
                    base_url: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Revolut => match auth {
                ConnectorAuthType::HeaderKey { api_key } => Ok(Self::Revolut {
                    secret_api_key: api_key.clone(),
                    signing_secret: None,
                    base_url: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Shift4 => match auth {
                ConnectorAuthType::HeaderKey { api_key } => Ok(Self::Shift4 {
                    api_key: api_key.clone(),
                    base_url: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Stax => match auth {
                ConnectorAuthType::HeaderKey { api_key } => Ok(Self::Stax {
                    api_key: api_key.clone(),
                    base_url: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Xendit => match auth {
                ConnectorAuthType::HeaderKey { api_key } => Ok(Self::Xendit {
                    api_key: api_key.clone(),
                    base_url: None,
                }),
                _ => Err(err().into()),
            },
            // Razorpay supports both HeaderKey and BodyKey
            ConnectorEnum::Razorpay => match auth {
                ConnectorAuthType::HeaderKey { api_key } => Ok(Self::Razorpay {
                    api_key: api_key.clone(),
                    api_secret: None,
                    base_url: None,
                }),
                ConnectorAuthType::BodyKey { api_key, key1 } => Ok(Self::Razorpay {
                    api_key: api_key.clone(),
                    api_secret: Some(key1.clone()),
                    base_url: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::RazorpayV2 => match auth {
                ConnectorAuthType::HeaderKey { api_key } => Ok(Self::RazorpayV2 {
                    api_key: api_key.clone(),
                    api_secret: None,
                    base_url: None,
                }),
                ConnectorAuthType::BodyKey { api_key, key1 } => Ok(Self::RazorpayV2 {
                    api_key: api_key.clone(),
                    api_secret: Some(key1.clone()),
                    base_url: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Imerchantsolutions => match auth {
                ConnectorAuthType::HeaderKey { api_key } => Ok(Self::Imerchantsolutions {
                    api_key: api_key.clone(),
                    base_url: None,
                }),
                _ => Err(err().into()),
            },

            // --- BodyKey connectors ---
            ConnectorEnum::Aci => match auth {
                ConnectorAuthType::BodyKey { api_key, key1 } => Ok(Self::Aci {
                    api_key: api_key.clone(),
                    entity_id: key1.clone(),
                    base_url: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Airwallex => match auth {
                ConnectorAuthType::BodyKey { api_key, key1 } => Ok(Self::Airwallex {
                    api_key: api_key.clone(),
                    client_id: key1.clone(),
                    base_url: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Authorizedotnet => match auth {
                ConnectorAuthType::BodyKey { api_key, key1 } => Ok(Self::Authorizedotnet {
                    name: api_key.clone(),
                    transaction_key: key1.clone(),
                    base_url: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Bambora => match auth {
                ConnectorAuthType::BodyKey { api_key, key1 } => Ok(Self::Bambora {
                    merchant_id: key1.clone(),
                    api_key: api_key.clone(),
                    base_url: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Billwerk => match auth {
                ConnectorAuthType::BodyKey { api_key, key1 } => Ok(Self::Billwerk {
                    api_key: api_key.clone(),
                    public_api_key: key1.clone(),
                    base_url: None,
                    secondary_base_url: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Bluesnap => match auth {
                ConnectorAuthType::BodyKey { api_key, key1 } => Ok(Self::Bluesnap {
                    username: key1.clone(),
                    password: api_key.clone(),
                    base_url: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Cashfree => match auth {
                ConnectorAuthType::BodyKey { api_key, key1 } => Ok(Self::Cashfree {
                    app_id: key1.clone(),
                    secret_key: api_key.clone(),
                    base_url: None,
                }),
                ConnectorAuthType::SignatureKey {
                    api_key: _,
                    key1,
                    api_secret,
                } => Ok(Self::Cashfree {
                    app_id: key1.clone(),
                    secret_key: api_secret.clone(),
                    base_url: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Cryptopay => match auth {
                ConnectorAuthType::BodyKey { api_key, key1 } => Ok(Self::Cryptopay {
                    api_key: api_key.clone(),
                    api_secret: key1.clone(),
                    base_url: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Datatrans => match auth {
                ConnectorAuthType::BodyKey { api_key, key1 } => Ok(Self::Datatrans {
                    merchant_id: key1.clone(),
                    password: api_key.clone(),
                    base_url: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Globalpay => match auth {
                ConnectorAuthType::BodyKey { api_key, key1 } => Ok(Self::Globalpay {
                    app_id: key1.clone(),
                    app_key: api_key.clone(),
                    base_url: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Hipay => match auth {
                ConnectorAuthType::BodyKey { api_key, key1 } => Ok(Self::Hipay {
                    api_key: api_key.clone(),
                    api_secret: key1.clone(),
                    base_url: None,
                    secondary_base_url: None,
                    third_base_url: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Jpmorgan => match auth {
                ConnectorAuthType::BodyKey { api_key, key1 } => Ok(Self::Jpmorgan {
                    client_id: api_key.clone(),
                    client_secret: key1.clone(),
                    base_url: None,
                    secondary_base_url: None,
                    company_name: None,
                    product_name: None,
                    merchant_purchase_description: None,
                    statement_descriptor: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Loonio => match auth {
                ConnectorAuthType::BodyKey { api_key, key1 } => Ok(Self::Loonio {
                    merchant_id: api_key.clone(),
                    merchant_token: key1.clone(),
                    base_url: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Paysafe => match auth {
                ConnectorAuthType::BodyKey { api_key, key1 } => Ok(Self::Paysafe {
                    username: api_key.clone(),
                    password: key1.clone(),
                    base_url: None,
                    account_id: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Payu => match auth {
                ConnectorAuthType::BodyKey { api_key, key1 } => Ok(Self::Payu {
                    api_key: api_key.clone(),
                    api_secret: key1.clone(),
                    base_url: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Placetopay => match auth {
                ConnectorAuthType::BodyKey { api_key, key1 } => Ok(Self::Placetopay {
                    login: api_key.clone(),
                    tran_key: key1.clone(),
                    base_url: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Powertranz => match auth {
                ConnectorAuthType::BodyKey { api_key, key1 } => Ok(Self::Powertranz {
                    power_tranz_id: key1.clone(),
                    power_tranz_password: api_key.clone(),
                    base_url: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Rapyd => match auth {
                ConnectorAuthType::BodyKey { api_key, key1 } => Ok(Self::Rapyd {
                    access_key: api_key.clone(),
                    secret_key: key1.clone(),
                    base_url: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Truelayer => match auth {
                ConnectorAuthType::BodyKey { api_key, key1 } => Ok(Self::Truelayer {
                    client_id: api_key.clone(),
                    client_secret: key1.clone(),
                    account_holder_name: None,
                    merchant_account_id: None,
                    private_key: None,
                    kid: None,
                    base_url: None,
                    secondary_base_url: None,
                }),
                _ => Err(err().into()),
            },

            // --- Connectors supporting both BodyKey and SignatureKey ---
            ConnectorEnum::Adyen => match auth {
                ConnectorAuthType::BodyKey { api_key, key1 } => Ok(Self::Adyen {
                    api_key: api_key.clone(),
                    merchant_account: key1.clone(),
                    review_key: None,
                    base_url: None,
                    dispute_base_url: None,
                    endpoint_prefix: None,
                }),
                ConnectorAuthType::SignatureKey {
                    api_key,
                    key1,
                    api_secret,
                } => Ok(Self::Adyen {
                    api_key: api_key.clone(),
                    merchant_account: key1.clone(),
                    review_key: Some(api_secret.clone()),
                    base_url: None,
                    dispute_base_url: None,
                    endpoint_prefix: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Authipay => match auth {
                ConnectorAuthType::BodyKey { api_key, key1 } => Ok(Self::Authipay {
                    api_key: api_key.clone(),
                    api_secret: key1.clone(),
                    base_url: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Fiservemea => match auth {
                ConnectorAuthType::BodyKey { api_key, key1 } => Ok(Self::Fiservemea {
                    api_key: api_key.clone(),
                    api_secret: key1.clone(),
                    base_url: None,
                }),
                ConnectorAuthType::SignatureKey {
                    api_key,
                    key1: _,
                    api_secret,
                } => Ok(Self::Fiservemea {
                    api_key: api_key.clone(),
                    api_secret: api_secret.clone(),
                    base_url: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Mollie => match auth {
                ConnectorAuthType::BodyKey { api_key, key1 } => Ok(Self::Mollie {
                    api_key: api_key.clone(),
                    profile_token: Some(key1.clone()),
                    base_url: None,
                    secondary_base_url: None,
                }),
                ConnectorAuthType::HeaderKey { api_key } => Ok(Self::Mollie {
                    api_key: api_key.clone(),
                    profile_token: None,
                    base_url: None,
                    secondary_base_url: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Nmi => match auth {
                ConnectorAuthType::HeaderKey { api_key } => Ok(Self::Nmi {
                    api_key: api_key.clone(),
                    public_key: None,
                    base_url: None,
                }),
                ConnectorAuthType::BodyKey { api_key, key1 } => Ok(Self::Nmi {
                    api_key: api_key.clone(),
                    public_key: Some(key1.clone()),
                    base_url: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Payme => match auth {
                ConnectorAuthType::BodyKey { api_key, key1 } => Ok(Self::Payme {
                    seller_payme_id: api_key.clone(),
                    payme_client_key: Some(key1.clone()),
                    base_url: None,
                }),
                ConnectorAuthType::SignatureKey {
                    api_key,
                    key1,
                    api_secret: _,
                } => Ok(Self::Payme {
                    seller_payme_id: api_key.clone(),
                    payme_client_key: Some(key1.clone()),
                    base_url: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Nexinets => match auth {
                ConnectorAuthType::BodyKey { api_key, key1 } => Ok(Self::Nexinets {
                    merchant_id: key1.clone(),
                    api_key: api_key.clone(),
                    base_url: None,
                }),
                _ => Err(err().into()),
            },

            // --- SignatureKey connectors ---
            ConnectorEnum::Bankofamerica => match auth {
                ConnectorAuthType::SignatureKey {
                    api_key,
                    key1,
                    api_secret,
                } => Ok(Self::BankOfAmerica {
                    api_key: api_key.clone(),
                    merchant_account: key1.clone(),
                    api_secret: api_secret.clone(),
                    base_url: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Bamboraapac => match auth {
                ConnectorAuthType::SignatureKey {
                    api_key,
                    key1,
                    api_secret,
                } => Ok(Self::Bamboraapac {
                    username: api_key.clone(),
                    password: api_secret.clone(),
                    account_number: key1.clone(),
                    base_url: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Barclaycard => match auth {
                ConnectorAuthType::SignatureKey {
                    api_key,
                    key1,
                    api_secret,
                } => Ok(Self::Barclaycard {
                    api_key: api_key.clone(),
                    merchant_account: key1.clone(),
                    api_secret: api_secret.clone(),
                    base_url: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Braintree => match auth {
                ConnectorAuthType::SignatureKey {
                    api_key,
                    key1: _,
                    api_secret,
                } => Ok(Self::Braintree {
                    public_key: api_key.clone(),
                    private_key: api_secret.clone(),
                    base_url: None,
                    merchant_account_id: None,
                    merchant_config_currency: None,
                    apple_pay_supported_networks: vec![],
                    apple_pay_merchant_capabilities: vec![],
                    apple_pay_label: None,
                    gpay_merchant_name: None,
                    gpay_merchant_id: None,
                    gpay_allowed_auth_methods: vec![],
                    gpay_allowed_card_networks: vec![],
                    paypal_client_id: None,
                    gpay_gateway_merchant_id: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Checkout => match auth {
                ConnectorAuthType::SignatureKey {
                    api_key,
                    key1,
                    api_secret,
                } => Ok(Self::Checkout {
                    api_key: api_key.clone(),
                    api_secret: api_secret.clone(),
                    processing_channel_id: key1.clone(),
                    base_url: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Cybersource => match auth {
                ConnectorAuthType::SignatureKey {
                    api_key,
                    key1,
                    api_secret,
                } => Ok(Self::Cybersource {
                    api_key: api_key.clone(),
                    merchant_account: key1.clone(),
                    api_secret: api_secret.clone(),
                    base_url: None,
                    disable_avs: None,
                    disable_cvn: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Dlocal => match auth {
                ConnectorAuthType::SignatureKey {
                    api_key,
                    key1,
                    api_secret,
                } => Ok(Self::Dlocal {
                    x_login: api_key.clone(),
                    x_trans_key: key1.clone(),
                    secret: api_secret.clone(),
                    base_url: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Elavon => match auth {
                ConnectorAuthType::SignatureKey {
                    api_key,
                    key1,
                    api_secret,
                } => Ok(Self::Elavon {
                    ssl_merchant_id: api_key.clone(),
                    ssl_user_id: key1.clone(),
                    ssl_pin: api_secret.clone(),
                    base_url: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Fiserv => match auth {
                ConnectorAuthType::SignatureKey {
                    api_key,
                    key1,
                    api_secret,
                } => Ok(Self::Fiserv {
                    api_key: api_key.clone(),
                    merchant_account: key1.clone(),
                    api_secret: api_secret.clone(),
                    base_url: None,
                    terminal_id: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Fiuu => match auth {
                ConnectorAuthType::SignatureKey {
                    api_key,
                    key1,
                    api_secret,
                } => Ok(Self::Fiuu {
                    merchant_id: key1.clone(),
                    verify_key: api_key.clone(),
                    secret_key: api_secret.clone(),
                    base_url: None,
                    secondary_base_url: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Getnet => match auth {
                ConnectorAuthType::SignatureKey {
                    api_key,
                    key1,
                    api_secret,
                } => Ok(Self::Getnet {
                    api_key: api_key.clone(),
                    api_secret: api_secret.clone(),
                    seller_id: key1.clone(),
                    base_url: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Gigadat => match auth {
                ConnectorAuthType::SignatureKey {
                    api_key,
                    key1,
                    api_secret,
                } => Ok(Self::Gigadat {
                    security_token: api_secret.clone(),
                    access_token: api_key.clone(),
                    campaign_id: key1.clone(),
                    base_url: None,
                    site: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Hyperpg => match auth {
                ConnectorAuthType::SignatureKey {
                    api_key,
                    key1,
                    api_secret,
                } => Ok(Self::Hyperpg {
                    username: api_key.clone(),
                    password: key1.clone(),
                    merchant_id: api_secret.clone(),
                    base_url: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Iatapay => match auth {
                ConnectorAuthType::SignatureKey {
                    api_key,
                    key1,
                    api_secret,
                } => Ok(Self::Iatapay {
                    client_id: api_key.clone(),
                    merchant_id: key1.clone(),
                    client_secret: api_secret.clone(),
                    base_url: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Noon => match auth {
                ConnectorAuthType::SignatureKey {
                    api_key,
                    key1,
                    api_secret,
                } => Ok(Self::Noon {
                    api_key: api_key.clone(),
                    business_identifier: key1.clone(),
                    application_identifier: api_secret.clone(),
                    base_url: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Novalnet => match auth {
                ConnectorAuthType::SignatureKey {
                    api_key,
                    key1,
                    api_secret,
                } => Ok(Self::Novalnet {
                    product_activation_key: api_key.clone(),
                    payment_access_key: key1.clone(),
                    tariff_id: api_secret.clone(),
                    base_url: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Nuvei => match auth {
                ConnectorAuthType::SignatureKey {
                    api_key,
                    key1,
                    api_secret,
                } => Ok(Self::Nuvei {
                    merchant_id: api_key.clone(),
                    merchant_site_id: key1.clone(),
                    merchant_secret: api_secret.clone(),
                    base_url: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Phonepe => match auth {
                ConnectorAuthType::SignatureKey {
                    api_key,
                    key1,
                    api_secret,
                } => Ok(Self::Phonepe {
                    merchant_id: api_key.clone(),
                    salt_key: key1.clone(),
                    salt_index: api_secret.clone(),
                    base_url: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Redsys => match auth {
                ConnectorAuthType::SignatureKey {
                    api_key,
                    key1,
                    api_secret,
                } => Ok(Self::Redsys {
                    merchant_id: api_key.clone(),
                    terminal_id: key1.clone(),
                    sha256_pwd: api_secret.clone(),
                    base_url: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Silverflow => match auth {
                ConnectorAuthType::SignatureKey {
                    api_key,
                    key1,
                    api_secret,
                } => Ok(Self::Silverflow {
                    api_key: api_key.clone(),
                    api_secret: api_secret.clone(),
                    merchant_acceptor_key: key1.clone(),
                    base_url: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Trustpay => match auth {
                ConnectorAuthType::SignatureKey {
                    api_key,
                    key1,
                    api_secret,
                } => Ok(Self::Trustpay {
                    api_key: api_key.clone(),
                    project_id: key1.clone(),
                    secret_key: api_secret.clone(),
                    base_url: None,
                    base_url_bank_redirects: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Trustpayments => match auth {
                ConnectorAuthType::SignatureKey {
                    api_key,
                    key1,
                    api_secret,
                } => Ok(Self::Trustpayments {
                    username: api_key.clone(),
                    password: key1.clone(),
                    site_reference: api_secret.clone(),
                    base_url: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Tsys => match auth {
                ConnectorAuthType::SignatureKey {
                    api_key,
                    key1,
                    api_secret,
                } => Ok(Self::Tsys {
                    device_id: api_key.clone(),
                    transaction_key: key1.clone(),
                    developer_id: api_secret.clone(),
                    base_url: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Wellsfargo => match auth {
                ConnectorAuthType::SignatureKey {
                    api_key,
                    key1,
                    api_secret,
                } => Ok(Self::Wellsfargo {
                    api_key: api_key.clone(),
                    merchant_account: key1.clone(),
                    api_secret: api_secret.clone(),
                    base_url: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Worldpay => match auth {
                ConnectorAuthType::SignatureKey {
                    api_key,
                    key1,
                    api_secret,
                } => Ok(Self::Worldpay {
                    username: key1.clone(),
                    password: api_key.clone(),
                    entity_id: api_secret.clone(),
                    base_url: None,
                    merchant_name: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Worldpayvantiv => match auth {
                ConnectorAuthType::SignatureKey {
                    api_key,
                    key1,
                    api_secret,
                } => Ok(Self::Worldpayvantiv {
                    user: api_key.clone(),
                    password: api_secret.clone(),
                    merchant_id: key1.clone(),
                    base_url: None,
                    secondary_base_url: None,
                    report_group: None,
                    merchant_config_currency: None,
                }),
                ConnectorAuthType::MultiAuthKey {
                    api_key,
                    key1,
                    api_secret,
                    key2: _,
                } => Ok(Self::Worldpayvantiv {
                    user: api_key.clone(),
                    password: api_secret.clone(),
                    merchant_id: key1.clone(),
                    base_url: None,
                    secondary_base_url: None,
                    report_group: None,
                    merchant_config_currency: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Worldpayxml => match auth {
                ConnectorAuthType::SignatureKey {
                    api_key,
                    key1,
                    api_secret,
                } => Ok(Self::Worldpayxml {
                    api_username: api_key.clone(),
                    api_password: key1.clone(),
                    merchant_code: api_secret.clone(),
                    base_url: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Zift => match auth {
                ConnectorAuthType::SignatureKey {
                    api_key,
                    key1,
                    api_secret,
                } => Ok(Self::Zift {
                    user_name: api_key.clone(),
                    password: api_secret.clone(),
                    account_id: key1.clone(),
                    base_url: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Trustly => match auth {
                ConnectorAuthType::SignatureKey {
                    api_key,
                    key1,
                    api_secret,
                } => Ok(Self::Trustly {
                    username: api_key.clone(),
                    password: key1.clone(),
                    private_key: api_secret.clone(),
                    base_url: None,
                }),
                _ => Err(err().into()),
            },

            // --- Paypal (BodyKey or SignatureKey) ---
            ConnectorEnum::Paypal => match auth {
                ConnectorAuthType::BodyKey { api_key, key1 } => Ok(Self::Paypal {
                    client_id: key1.clone(),
                    client_secret: api_key.clone(),
                    payer_id: None,
                    base_url: None,
                }),
                ConnectorAuthType::SignatureKey {
                    api_key,
                    key1,
                    api_secret,
                } => Ok(Self::Paypal {
                    client_id: key1.clone(),
                    client_secret: api_key.clone(),
                    payer_id: Some(api_secret.clone()),
                    base_url: None,
                }),
                _ => Err(err().into()),
            },

            // --- MultiAuthKey connectors ---
            ConnectorEnum::Forte => match auth {
                ConnectorAuthType::MultiAuthKey {
                    api_key,
                    key1,
                    api_secret,
                    key2,
                } => Ok(Self::Forte {
                    api_access_id: api_key.clone(),
                    organization_id: key1.clone(),
                    location_id: key2.clone(),
                    api_secret_key: api_secret.clone(),
                    base_url: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Paybox => match auth {
                ConnectorAuthType::MultiAuthKey {
                    api_key,
                    key1,
                    api_secret,
                    key2,
                } => Ok(Self::Paybox {
                    site: api_key.clone(),
                    rank: key1.clone(),
                    key: api_secret.clone(),
                    merchant_id: key2.clone(),
                    base_url: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Paytm => match auth {
                ConnectorAuthType::SignatureKey {
                    api_key,
                    key1,
                    api_secret,
                } => Ok(Self::Paytm {
                    merchant_id: api_key.clone(),
                    merchant_key: key1.clone(),
                    website: api_secret.clone(),
                    client_id: None,
                    base_url: None,
                }),
                ConnectorAuthType::MultiAuthKey {
                    api_key,
                    key1,
                    api_secret,
                    key2,
                } => Ok(Self::Paytm {
                    merchant_id: api_key.clone(),
                    merchant_key: key1.clone(),
                    website: api_secret.clone(),
                    client_id: Some(key2.clone()),
                    base_url: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Volt => match auth {
                ConnectorAuthType::MultiAuthKey {
                    api_key,
                    key1,
                    api_secret,
                    key2,
                } => Ok(Self::Volt {
                    username: api_key.clone(),
                    password: api_secret.clone(),
                    client_id: key1.clone(),
                    client_secret: key2.clone(),
                    base_url: None,
                    secondary_base_url: None,
                }),
                _ => Err(err().into()),
            },

            // --- CurrencyAuthKey connectors ---
            ConnectorEnum::Cashtocode => match auth {
                ConnectorAuthType::CurrencyAuthKey { auth_key_map } => Ok(Self::Cashtocode {
                    auth_key_map: auth_key_map.clone(),
                    base_url: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Payload => match auth {
                ConnectorAuthType::CurrencyAuthKey { auth_key_map } => Ok(Self::Payload {
                    auth_key_map: auth_key_map.clone(),
                    base_url: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Revolv3 => match auth {
                ConnectorAuthType::HeaderKey { api_key } => Ok(Self::Revolv3 {
                    api_key: api_key.clone(),
                    base_url: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Peachpayments => match auth {
                ConnectorAuthType::BodyKey { api_key, key1 } => Ok(Self::Peachpayments {
                    api_key: api_key.clone(),
                    tenant_id: key1.clone(),
                    base_url: None,
                    client_merchant_reference_id: None,
                    merchant_payment_method_route_id: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Finix => match auth {
                ConnectorAuthType::MultiAuthKey {
                    api_key,
                    key1,
                    api_secret,
                    key2,
                } => Ok(Self::Finix {
                    finix_user_name: api_key.clone(),
                    finix_password: api_secret.clone(),
                    merchant_id: key1.clone(),
                    merchant_identity_id: key2.clone(),
                    base_url: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Ppro => match auth {
                ConnectorAuthType::BodyKey { api_key, key1 } => Ok(ConnectorSpecificConfig::Ppro {
                    api_key: api_key.clone(),
                    merchant_id: key1.clone(),
                    base_url: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Fiservcommercehub => match auth {
                ConnectorAuthType::MultiAuthKey {
                    api_key,
                    key1,
                    api_secret,
                    key2,
                } => Ok(Self::Fiservcommercehub {
                    api_key: api_key.clone(),
                    secret: api_secret.clone(),
                    merchant_id: key1.clone(),
                    terminal_id: key2.clone(),
                    base_url: None,
                }),
                _ => Err(err().into()),
            },
            ConnectorEnum::Itaubank => match auth {
                ConnectorAuthType::BodyKey { api_key, key1 } => Ok(Self::Itaubank {
                    client_id: api_key.clone(),
                    client_secret: key1.clone(),
                    base_url: None,
                }),
                _ => Err(err().into()),
            },
        }
    }
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct ErrorResponse {
    pub code: String,
    pub message: String,
    pub reason: Option<String>,
    pub status_code: u16,
    pub attempt_status: Option<common_enums::enums::AttemptStatus>,
    pub connector_transaction_id: Option<String>,
    pub network_decline_code: Option<String>,
    pub network_advice_code: Option<String>,
    pub network_error_message: Option<String>,
}

impl Default for ErrorResponse {
    fn default() -> Self {
        Self {
            code: "HE_00".to_string(),
            message: "Something went wrong".to_string(),
            reason: None,
            status_code: http::StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
            attempt_status: None,
            connector_transaction_id: None,
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
        }
    }
}

impl ErrorResponse {
    /// Returns attempt status for gRPC response
    ///
    /// For 2xx: If attempt_status is None, use fallback (router_data.status set by connector)
    /// For 4xx/5xx: If attempt_status is None, return None
    pub fn get_attempt_status_for_grpc(
        &self,
        http_status_code: u16,
        fallback_status: common_enums::enums::AttemptStatus,
    ) -> Option<common_enums::enums::AttemptStatus> {
        self.attempt_status.or_else(|| {
            if (200..300).contains(&http_status_code) {
                Some(fallback_status)
            } else {
                None
            }
        })
    }

    pub fn get_not_implemented() -> Self {
        Self {
            code: "IR_00".to_string(),
            message: "This API is under development and will be made available soon.".to_string(),
            reason: None,
            status_code: http::StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
            attempt_status: None,
            connector_transaction_id: None,
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ApplePayCryptogramData {
    pub online_payment_cryptogram: Secret<String>,
    pub eci_indicator: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PazeDecryptedData {
    pub client_id: Secret<String>,
    pub profile_id: String,
    pub token: PazeToken,
    pub payment_card_network: common_enums::enums::CardNetwork,
    pub dynamic_data: Vec<PazeDynamicData>,
    pub billing_address: PazeAddress,
    pub consumer: PazeConsumer,
    pub eci: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PazeToken {
    pub payment_token: cards::NetworkToken,
    pub token_expiration_month: Secret<String>,
    pub token_expiration_year: Secret<String>,
    pub payment_account_reference: Secret<String>,
}

pub type NetworkTokenNumber = NetworkToken;

#[derive(Debug, Clone, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PazeConsumer {
    // This is consumer data not customer data.
    pub first_name: Option<Secret<String>>,
    pub last_name: Option<Secret<String>>,
    pub full_name: Secret<String>,
    pub email_address: common_utils::pii::Email,
    pub mobile_number: Option<PazePhoneNumber>,
    pub country_code: Option<common_enums::enums::CountryAlpha2>,
    pub language_code: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PazePhoneNumber {
    pub country_code: Secret<String>,
    pub phone_number: Secret<String>,
}

#[derive(Debug, Clone, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PazeAddress {
    pub name: Option<Secret<String>>,
    pub line1: Option<Secret<String>>,
    pub line2: Option<Secret<String>>,
    pub line3: Option<Secret<String>>,
    pub city: Option<Secret<String>>,
    pub state: Option<Secret<String>>,
    pub zip: Option<Secret<String>>,
    pub country_code: Option<common_enums::enums::CountryAlpha2>,
}

#[derive(Debug, Clone, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PazeDynamicData {
    pub dynamic_data_value: Option<Secret<String>>,
    pub dynamic_data_type: Option<String>,
    pub dynamic_data_expiration: Option<String>,
}

// Dead code: nothing populates this after PaymentFlowData.payment_method_token was removed.
// #[derive(Debug, Clone, serde::Deserialize)]
// pub enum PaymentMethodToken {
//     Token(Secret<String>),
// }

#[derive(Debug, Default, Clone)]
pub struct RecurringMandatePaymentData {
    pub payment_method_type: Option<common_enums::enums::PaymentMethodType>, //required for making recurring payment using saved payment method through stripe
    pub original_payment_authorized_amount: Option<Money>,
    pub mandate_metadata: Option<common_utils::pii::SecretSerdeValue>,
}

impl RecurringMandatePaymentData {
    pub fn get_original_payment_amount(&self) -> Result<Money, Error> {
        self.original_payment_authorized_amount
            .clone()
            .ok_or_else(missing_field_err("original_payment_authorized_amount"))
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConnectorResponseData {
    pub additional_payment_method_data: Option<AdditionalPaymentMethodConnectorResponse>,
    extended_authorization_response_data: Option<ExtendedAuthorizationResponseData>,
    is_overcapture_enabled: Option<bool>,
}

impl ConnectorResponseData {
    pub fn with_auth_code(auth_code: String, pmt: common_enums::PaymentMethodType) -> Self {
        let additional_payment_method_data = match pmt {
            common_enums::PaymentMethodType::GooglePay => {
                AdditionalPaymentMethodConnectorResponse::GooglePay {
                    auth_code: Some(auth_code),
                }
            }
            common_enums::PaymentMethodType::ApplePay => {
                AdditionalPaymentMethodConnectorResponse::ApplePay {
                    auth_code: Some(auth_code),
                }
            }
            _ => AdditionalPaymentMethodConnectorResponse::Card {
                authentication_data: None,
                payment_checks: None,
                card_network: None,
                domestic_network: None,
                auth_code: Some(auth_code),
            },
        };
        Self {
            additional_payment_method_data: Some(additional_payment_method_data),
            extended_authorization_response_data: None,
            is_overcapture_enabled: None,
        }
    }
    pub fn with_additional_payment_method_data(
        additional_payment_method_data: AdditionalPaymentMethodConnectorResponse,
    ) -> Self {
        Self {
            additional_payment_method_data: Some(additional_payment_method_data),
            extended_authorization_response_data: None,
            is_overcapture_enabled: None,
        }
    }
    pub fn new(
        additional_payment_method_data: Option<AdditionalPaymentMethodConnectorResponse>,
        is_overcapture_enabled: Option<bool>,
        extended_authorization_response_data: Option<ExtendedAuthorizationResponseData>,
    ) -> Self {
        Self {
            additional_payment_method_data,
            extended_authorization_response_data,
            is_overcapture_enabled,
        }
    }

    pub fn get_extended_authorization_response_data(
        &self,
    ) -> Option<&ExtendedAuthorizationResponseData> {
        self.extended_authorization_response_data.as_ref()
    }

    pub fn is_overcapture_enabled(&self) -> Option<bool> {
        self.is_overcapture_enabled
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum AdditionalPaymentMethodConnectorResponse {
    Card {
        /// Details regarding the authentication details of the connector, if this is a 3ds payment.
        authentication_data: Option<serde_json::Value>,
        /// Various payment checks that are done for a payment
        payment_checks: Option<serde_json::Value>,
        /// Card Network returned by the processor
        card_network: Option<String>,
        /// Domestic(Co-Branded) Card network returned by the processor
        domestic_network: Option<String>,
        /// auth code returned by the processor
        auth_code: Option<String>,
    },
    Upi {
        /// UPI source detected from the connector response
        upi_mode: Option<payment_method_data::UpiSource>,
    },
    GooglePay {
        auth_code: Option<String>,
    },
    ApplePay {
        auth_code: Option<String>,
    },
    BankRedirect {
        interac: Option<InteracCustomerInfo>,
    },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExtendedAuthorizationResponseData {
    pub extended_authentication_applied: Option<bool>,
    pub extended_authorization_last_applied_at: Option<time::PrimitiveDateTime>,
    pub capture_before: Option<time::PrimitiveDateTime>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct InteracCustomerInfo {
    pub customer_info: Option<payment_method_data::CustomerInfoDetails>,
}
