use std::collections::HashMap;

use common_enums::{AttemptStatus, CountryAlpha2, Currency, RefundStatus};
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    crypto::jose::JoseConfig,
    request::Method,
    types::{FloatMajorUnit, FloatMajorUnitForConnector, MinorUnit},
};
use domain_types::{
    connector_flow::{Authorize, Capture, PSync, RSync, Refund, Void, VoidPC},
    connector_types::{
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCancelPostCaptureData,
        PaymentsCaptureData, PaymentsResponseData, PaymentsSyncData, RefundFlowData,
        RefundSyncData, RefundsData, RefundsResponseData, ResponseId,
    },
    errors,
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes, WalletData},
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
};
use error_stack::ResultExt;
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use crate::{connectors::twoc_twop_paco::TwocTwopPacoRouterData, types::ResponseRouterData};

const PACO_LANGUAGE: &str = "en-US";
const PACO_CARD_TYPE_CREDIT: &str = "credit";
const PACO_CARD_TYPE_DEBIT: &str = "debit";
const PACO_REFUND_MAKER_ID: &str = "merchant";
const PACO_KID_HEX_LEN: usize = 32;
const PACO_OFFICE_ID_MAX_LEN: usize = 20;
pub const PACO_AUDIENCE: &str = "PacoAudience";
const PACO_JWT_TTL_SECONDS: i64 = 300;
const PACO_INTEGRATION_DOC_URL: &str =
    "https://developer.2c2p.com/docs/getting-started-with-payment-air-controller-paco";

pub const PACO_RESPONSE_CODE_SUCCESS: &str = "PC-B050000";

#[derive(Debug, Clone, Copy, Serialize)]
pub enum PacoPaymentType {
    CC,
    #[serde(rename = "WALLET-GCASH")]
    WalletGcash,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub enum PacoRequest3dsFlag {
    Y,
    N,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub enum PacoDeviceCategory {
    M,
    P,
}

#[derive(Debug, Clone)]
pub struct TwocTwopPacoAuthType {
    pub access_token: Secret<String>,
    pub office_id: Secret<String>,
    pub response_audience: Secret<String>,
    pub jose_cfg: JoseConfig,
}

impl TryFrom<&ConnectorSpecificConfig> for TwocTwopPacoAuthType {
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(value: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match value {
            ConnectorSpecificConfig::TwocTwopPaco {
                access_token,
                office_id,
                paco_kid,
                merchant_signing_private_key,
                merchant_encryption_private_key,
                paco_signing_public_key,
                paco_encryption_public_key,
                response_audience,
                base_url: _,
            } => {
                let kid = paco_kid.peek().clone();
                if kid.len() != PACO_KID_HEX_LEN || !kid.chars().all(|c| c.is_ascii_hexdigit()) {
                    return Err(errors::IntegrationError::InvalidDataFormat {
                        field_name: "paco_kid",
                        context: errors::IntegrationErrorContext {
                            suggested_action: Some(
                                "Provide a 32-character lowercase hex string for paco_kid."
                                    .to_string(),
                            ),
                            doc_url: Some(PACO_INTEGRATION_DOC_URL.to_string()),
                            additional_context: Some(
                                "paco_kid must be exactly 32 hexadecimal characters.".to_string(),
                            ),
                        },
                    }
                    .into());
                }

                let office = office_id.peek();
                if office.is_empty() || office.len() > PACO_OFFICE_ID_MAX_LEN {
                    return Err(errors::IntegrationError::InvalidDataFormat {
                        field_name: "office_id",
                        context: errors::IntegrationErrorContext {
                            suggested_action: Some(
                                "office_id must be 1..=20 characters.".to_string(),
                            ),
                            doc_url: Some(PACO_INTEGRATION_DOC_URL.to_string()),
                            additional_context: Some(format!(
                                "Received office_id length {}.",
                                office.len()
                            )),
                        },
                    }
                    .into());
                }

                let jose_cfg = JoseConfig::new(
                    kid,
                    merchant_signing_private_key.clone(),
                    merchant_encryption_private_key.clone(),
                    paco_signing_public_key.clone(),
                    paco_encryption_public_key.clone(),
                )
                .map_err(|err| {
                    errors::IntegrationError::FailedToObtainAuthType {
                        context: errors::IntegrationErrorContext {
                            suggested_action: Some(
                                "Verify the four PEMs supplied for the PACO connector parse with OpenSSL."
                                    .to_string(),
                            ),
                            doc_url: Some(PACO_INTEGRATION_DOC_URL.to_string()),
                            additional_context: Some(format!("JoseConfig validation failed: {err}")),
                        },
                    }
                })?;

                Ok(Self {
                    access_token: access_token.clone(),
                    office_id: office_id.clone(),
                    response_audience: response_audience
                        .clone()
                        .unwrap_or_else(|| access_token.clone()),
                    jose_cfg,
                })
            }
            _ => Err(errors::IntegrationError::FailedToObtainAuthType {
                context: errors::IntegrationErrorContext {
                    suggested_action: Some(
                        "Configure the connector with the TwocTwopPaco auth variant.".to_string(),
                    ),
                    doc_url: Some(PACO_INTEGRATION_DOC_URL.to_string()),
                    additional_context: Some(
                        "Expected ConnectorSpecificConfig::TwocTwopPaco.".to_string(),
                    ),
                },
            }
            .into()),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ApiRequestEnvelope {
    #[serde(rename = "requestMessageID")]
    pub request_message_id: String,
    #[serde(rename = "requestDateTime")]
    pub request_date_time: String,
    pub language: &'static str,
}

fn paco_require_merchant_request_id(
    result: Result<String, error_stack::Report<errors::IntegrationError>>,
) -> Result<String, error_stack::Report<errors::IntegrationError>> {
    result.map_err(|_| {
        error_stack::report!(errors::IntegrationError::MissingRequiredField {
            field_name: "merchant_request_id",
            context: errors::IntegrationErrorContext {
                suggested_action: Some(
                    "Pass a unique `merchant_request_id` (UUID) on the gRPC request — 2C2P PACO requires it as the `apiRequest.requestMessageID` on every call."
                        .to_string(),
                ),
                doc_url: Some(PACO_INTEGRATION_DOC_URL.to_string()),
                additional_context: Some(
                    "PACO does not accept calls without `requestMessageID`.".to_string(),
                ),
            },
        })
    })
}

impl ApiRequestEnvelope {
    fn new(request_message_id: String) -> Self {
        let now = time::OffsetDateTime::now_utc();
        let formatted = now
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_else(|_| String::from("1970-01-01T00:00:00Z"));
        Self {
            request_message_id,
            request_date_time: formatted,
            language: PACO_LANGUAGE,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PacoTransactionAmount {
    pub amount_text: String,
    pub currency_code: Currency,
    pub decimal_places: u8,
    pub amount: FloatMajorUnit,
}

impl PacoTransactionAmount {
    fn new(minor_amount: MinorUnit, currency: Currency) -> Result<Self, errors::IntegrationError> {
        let decimals = currency
            .number_of_digits_after_decimal_point()
            .map_err(|_| errors::IntegrationError::InvalidDataFormat {
                field_name: "currency",
                context: errors::IntegrationErrorContext {
                    suggested_action: Some(
                        "Use an ISO 4217 currency PACO accepts (e.g. PHP, USD).".to_string(),
                    ),
                    doc_url: Some(PACO_INTEGRATION_DOC_URL.to_string()),
                    additional_context: Some(format!(
                        "Currency {currency:?} not supported for amount conversion"
                    )),
                },
            })?;
        let raw = minor_amount.get_amount_as_i64();
        let amount_text = format!("{raw:0>12}");
        let amount = <FloatMajorUnitForConnector as common_utils::types::AmountConvertor>::convert(
            &FloatMajorUnitForConnector,
            minor_amount,
            currency,
        )
        .map_err(|err| errors::IntegrationError::InvalidDataFormat {
            field_name: "amount",
            context: errors::IntegrationErrorContext {
                suggested_action: Some(
                    "Verify the request `amount` is a positive integer minor-unit value."
                        .to_string(),
                ),
                doc_url: Some(PACO_INTEGRATION_DOC_URL.to_string()),
                additional_context: Some(format!(
                    "Failed to convert minor amount to FloatMajorUnit: {err}"
                )),
            },
        })?;
        Ok(Self {
            amount_text,
            currency_code: currency,
            decimal_places: decimals,
            amount,
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PacoNotificationUrls {
    #[serde(rename = "confirmationURL", skip_serializing_if = "Option::is_none")]
    pub confirmation_url: Option<String>,
    #[serde(rename = "failedURL", skip_serializing_if = "Option::is_none")]
    pub failed_url: Option<String>,
    #[serde(rename = "cancellationURL", skip_serializing_if = "Option::is_none")]
    pub cancellation_url: Option<String>,
    #[serde(rename = "backendURL", skip_serializing_if = "Option::is_none")]
    pub backend_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PacoBillingAddress {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bill_addr_city: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bill_addr_country: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bill_addr_line1: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bill_addr_line2: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bill_addr_line3: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bill_addr_post_code: Option<Secret<String>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PacoShippingAddress {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ship_addr_city: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ship_addr_country: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ship_addr_line1: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ship_addr_line2: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ship_addr_line3: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ship_addr_post_code: Option<Secret<String>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PacoCreditCardDetails {
    pub card_number: Secret<String>,
    #[serde(rename = "cardExpiryMMYY")]
    pub card_expiry_mmyy: Secret<String>,
    pub cvv_code: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_holder_name: Option<Secret<String>>,
    pub card_type: &'static str,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PacoBrowserInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accept_header: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub javascript_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub java_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_depth: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screen_height: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screen_width: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_zone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<String>,
}

impl PacoBrowserInfo {
    pub fn from_browser_info(bi: &domain_types::router_request_types::BrowserInformation) -> Self {
        Self {
            accept_header: bi.accept_header.clone(),
            ip: bi.ip_address.map(|ip| ip.to_string()),
            javascript_enabled: bi.java_script_enabled,
            java_enabled: bi.java_enabled,
            language: bi.language.clone(),
            color_depth: bi.color_depth.map(|d| d.to_string()),
            screen_height: bi.screen_height.map(|h| h.to_string()),
            screen_width: bi.screen_width.map(|w| w.to_string()),
            time_zone: bi.time_zone.map(|tz| tz.to_string()),
            user_agent: bi.user_agent.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TwocTwopPacoCardAuthorizeRequest {
    pub api_request: ApiRequestEnvelope,
    pub office_id: Secret<String>,
    pub order_no: String,
    pub product_description: String,
    pub payment_type: PacoPaymentType,
    pub transaction_amount: PacoTransactionAmount,
    #[serde(rename = "notificationURLs")]
    pub notification_urls: PacoNotificationUrls,
    pub credit_card_details: PacoCreditCardDetails,
    #[serde(rename = "request3dsFlag")]
    pub request3ds_flag: PacoRequest3dsFlag,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser_info: Option<PacoBrowserInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_details: Option<PacoDeviceDetails>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billing_address: Option<PacoBillingAddress>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shipping_address: Option<PacoShippingAddress>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PacoDeviceDetails {
    pub device_category: PacoDeviceCategory,
    pub user_agent: String,
}

impl PacoDeviceDetails {
    fn default_browser() -> Self {
        Self {
            device_category: PacoDeviceCategory::P,
            user_agent: "Mozilla/5.0 hyperswitch-prism".to_string(),
        }
    }

    pub fn from_user_agent(user_agent: String) -> Self {
        let lower = user_agent.to_ascii_lowercase();
        let is_mobile = lower.contains("mobile")
            || lower.contains("android")
            || lower.contains("iphone")
            || lower.contains("ipad");
        Self {
            device_category: if is_mobile {
                PacoDeviceCategory::M
            } else {
                PacoDeviceCategory::P
            },
            user_agent,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TwocTwopPacoWalletAuthorizeRequest {
    pub api_request: ApiRequestEnvelope,
    pub office_id: Secret<String>,
    pub order_no: String,
    pub product_description: String,
    pub payment_type: PacoPaymentType,
    pub transaction_amount: PacoTransactionAmount,
    #[serde(rename = "notificationURLs")]
    pub notification_urls: PacoNotificationUrls,
    pub device_details: PacoDeviceDetails,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billing_address: Option<PacoBillingAddress>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shipping_address: Option<PacoShippingAddress>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
#[allow(clippy::large_enum_variant)]
pub enum TwocTwopPacoAuthorizeRequest {
    Card(TwocTwopPacoCardAuthorizeRequest),
    Wallet(TwocTwopPacoWalletAuthorizeRequest),
}

#[derive(Debug, Clone, Serialize)]
#[serde(transparent)]
pub struct TwocTwopPacoVoidPcRequest(pub TwocTwopPacoVoidRequest);

/// Pairs a parsed PACO response with the exact JSON it was parsed from.
///
/// PACO bodies are JOSE-encrypted on the wire, so the raw HTTP body is
/// ciphertext. Re-serialising the typed struct for `raw_connector_response`
/// silently drops every field the struct doesn't model; this keeps the full
/// decrypted payload instead.
#[derive(Debug, Clone)]
pub struct PacoResponseWithRaw<T> {
    pub parsed_response: T,
    pub raw_response: serde_json::Value,
}

impl<'de, T: serde::de::DeserializeOwned> Deserialize<'de> for PacoResponseWithRaw<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw_response = serde_json::Value::deserialize(deserializer)?;
        let parsed_response = T::deserialize(&raw_response).map_err(serde::de::Error::custom)?;
        Ok(Self {
            parsed_response,
            raw_response,
        })
    }
}

impl<T> Serialize for PacoResponseWithRaw<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.raw_response.serialize(serializer)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TwocTwopPacoAuthorizeResponse(pub PacoResponseWithRaw<TwocTwopPacoNonUiResponse>);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TwocTwopPacoCaptureResponse(pub PacoResponseWithRaw<TwocTwopPacoNonUiResponse>);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TwocTwopPacoVoidResponse(pub PacoResponseWithRaw<TwocTwopPacoNonUiResponse>);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TwocTwopPacoVoidPcResponse(pub PacoResponseWithRaw<TwocTwopPacoNonUiResponse>);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TwocTwopPacoRefundResponse(pub PacoResponseWithRaw<TwocTwopPacoNonUiResponse>);

pub fn build_authorize_request<T>(
    item: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
    auth: &TwocTwopPacoAuthType,
) -> Result<TwocTwopPacoAuthorizeRequest, error_stack::Report<errors::IntegrationError>>
where
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
{
    let office_id = auth.office_id.clone();
    let order_no = item
        .resource_common_data
        .connector_request_reference_id
        .clone();
    let description = item
        .resource_common_data
        .description
        .clone()
        .unwrap_or_else(|| order_no.clone());
    let request_message_id =
        paco_require_merchant_request_id(item.resource_common_data.get_merchant_request_id())?;
    let amount = PacoTransactionAmount::new(item.request.minor_amount, item.request.currency)?;
    let notification_urls = PacoNotificationUrls {
        confirmation_url: item.request.router_return_url.clone(),
        failed_url: item.request.router_return_url.clone(),
        cancellation_url: item.request.router_return_url.clone(),
        backend_url: item.request.webhook_url.clone(),
    };

    let common = &item.resource_common_data;
    let paco_billing_address = common
        .get_optional_billing()
        .and_then(|billing| billing.address.as_ref())
        .map(|_| PacoBillingAddress {
            bill_addr_city: common.get_optional_billing_city(),
            bill_addr_country: common
                .get_optional_billing_country()
                .map(|c| CountryAlpha2::to_numeric(c).to_string()),
            bill_addr_line1: common.get_optional_billing_line1(),
            bill_addr_line2: common.get_optional_billing_line2(),
            bill_addr_line3: common.get_optional_billing_line3(),
            bill_addr_post_code: common.get_optional_billing_zip(),
        });

    let paco_shipping_address = common
        .get_optional_shipping()
        .and_then(|shipping| shipping.address.as_ref())
        .map(|_| PacoShippingAddress {
            ship_addr_city: common.get_optional_shipping_city(),
            ship_addr_country: common
                .get_optional_shipping_country()
                .map(|c| CountryAlpha2::to_numeric(c).to_string()),
            ship_addr_line1: common.get_optional_shipping_line1(),
            ship_addr_line2: common.get_optional_shipping_line2(),
            ship_addr_line3: common.get_optional_shipping_line3(),
            ship_addr_post_code: common.get_optional_shipping_zip(),
        });

    match &item.request.payment_method_data {
        PaymentMethodData::Card(card) => {
            let card_type = match card.card_type.as_deref() {
                Some(t) if t.eq_ignore_ascii_case("debit") => PACO_CARD_TYPE_DEBIT,
                _ => PACO_CARD_TYPE_CREDIT,
            };
            let mmyy = card.get_card_expiry_month_year_2_digit_with_delimiter(String::new())?;
            let request3ds_flag = match item.resource_common_data.auth_type {
                common_enums::AuthenticationType::ThreeDs => PacoRequest3dsFlag::Y,
                common_enums::AuthenticationType::NoThreeDs => PacoRequest3dsFlag::N,
            };
            let browser_info = item
                .request
                .browser_info
                .as_ref()
                .map(PacoBrowserInfo::from_browser_info);
            let device_details = item
                .request
                .browser_info
                .as_ref()
                .and_then(|bi| bi.user_agent.clone())
                .map(PacoDeviceDetails::from_user_agent);
            let body = TwocTwopPacoCardAuthorizeRequest {
                api_request: ApiRequestEnvelope::new(request_message_id),
                office_id,
                order_no,
                product_description: description,
                payment_type: PacoPaymentType::CC,
                transaction_amount: amount,
                notification_urls,
                credit_card_details: PacoCreditCardDetails {
                    card_number: Secret::new(card.card_number.peek().to_string()),
                    card_expiry_mmyy: mmyy,
                    cvv_code: card.card_cvc.clone(),
                    card_holder_name: card.get_optional_cardholder_name(),
                    card_type,
                },
                request3ds_flag,
                browser_info,
                device_details,
                billing_address: paco_billing_address,
                shipping_address: paco_shipping_address,
            };
            Ok(TwocTwopPacoAuthorizeRequest::Card(body))
        }
        PaymentMethodData::Wallet(WalletData::GcashRedirect(_)) => {
            let device_details = item
                .request
                .browser_info
                .as_ref()
                .and_then(|bi| bi.user_agent.clone())
                .map(|ua| PacoDeviceDetails {
                    device_category: PacoDeviceCategory::P,
                    user_agent: ua,
                })
                .unwrap_or_else(PacoDeviceDetails::default_browser);
            let body = TwocTwopPacoWalletAuthorizeRequest {
                api_request: ApiRequestEnvelope::new(request_message_id),
                office_id,
                order_no,
                product_description: description,
                payment_type: PacoPaymentType::WalletGcash,
                transaction_amount: amount,
                notification_urls,
                device_details,
                billing_address: paco_billing_address,
                shipping_address: paco_shipping_address,
            };
            Ok(TwocTwopPacoAuthorizeRequest::Wallet(body))
        }
        _ => Err(errors::IntegrationError::NotImplemented(
            "Selected payment method through TwocTwopPaco".to_string(),
            errors::IntegrationErrorContext {
                suggested_action: Some(
                    "Use Card or GcashRedirect; PACO does not support other payment methods today."
                        .to_string(),
                ),
                doc_url: Some(PACO_INTEGRATION_DOC_URL.to_string()),
                additional_context: Some(
                    "Authorize accepts card S2S or GCash wallet redirect.".to_string(),
                ),
            },
        )
        .into()),
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PacoSettlementAmount {
    pub amount_text: String,
    pub currency_code: Currency,
    pub decimal_places: u8,
    pub amount: FloatMajorUnit,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TwocTwopPacoCaptureRequest {
    pub api_request: ApiRequestEnvelope,
    pub office_id: Secret<String>,
    pub order_no: String,
    #[serde(rename = "invoiceNo2C2P")]
    pub invoice_no2c2p: String,
    pub settlement_amount: PacoSettlementAmount,
}

pub fn build_capture_request(
    item: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
    auth: &TwocTwopPacoAuthType,
) -> Result<TwocTwopPacoCaptureRequest, error_stack::Report<errors::IntegrationError>> {
    let office_id = auth.office_id.clone();
    let invoice_no = item.request.get_connector_transaction_id()?;
    let amount =
        PacoTransactionAmount::new(item.request.minor_amount_to_capture, item.request.currency)?;
    let request_message_id =
        paco_require_merchant_request_id(item.resource_common_data.get_merchant_request_id())?;
    Ok(TwocTwopPacoCaptureRequest {
        api_request: ApiRequestEnvelope::new(request_message_id),
        office_id,
        order_no: item
            .resource_common_data
            .connector_request_reference_id
            .clone(),
        invoice_no2c2p: invoice_no,
        settlement_amount: PacoSettlementAmount {
            amount_text: amount.amount_text,
            currency_code: amount.currency_code,
            decimal_places: amount.decimal_places,
            amount: amount.amount,
        },
    })
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TwocTwopPacoVoidRequest {
    pub api_request: ApiRequestEnvelope,
    pub office_id: Secret<String>,
    pub order_no: String,
    #[serde(rename = "invoiceNo2C2P")]
    pub invoice_no2c2p: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cancellation_reason: Option<String>,
}

pub fn build_void_request(
    item: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
    auth: &TwocTwopPacoAuthType,
) -> Result<TwocTwopPacoVoidRequest, error_stack::Report<errors::IntegrationError>> {
    let office_id = auth.office_id.clone();
    let request_message_id =
        paco_require_merchant_request_id(item.resource_common_data.get_merchant_request_id())?;
    Ok(TwocTwopPacoVoidRequest {
        api_request: ApiRequestEnvelope::new(request_message_id),
        office_id,
        order_no: item
            .resource_common_data
            .connector_request_reference_id
            .clone(),
        invoice_no2c2p: item.request.connector_transaction_id.clone(),
        cancellation_reason: item.request.cancellation_reason.clone(),
    })
}

pub fn build_void_pc_request(
    item: &RouterDataV2<
        VoidPC,
        PaymentFlowData,
        PaymentsCancelPostCaptureData,
        PaymentsResponseData,
    >,
    auth: &TwocTwopPacoAuthType,
) -> Result<TwocTwopPacoVoidRequest, error_stack::Report<errors::IntegrationError>> {
    let office_id = auth.office_id.clone();
    let request_message_id =
        paco_require_merchant_request_id(item.resource_common_data.get_merchant_request_id())?;
    Ok(TwocTwopPacoVoidRequest {
        api_request: ApiRequestEnvelope::new(request_message_id),
        office_id,
        order_no: item
            .resource_common_data
            .connector_request_reference_id
            .clone(),
        invoice_no2c2p: item.request.connector_transaction_id.clone(),
        cancellation_reason: item.request.cancellation_reason.clone(),
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct PacoHumanActor {
    pub username: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PacoMakerChecker {
    pub maker: PacoHumanActor,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TwocTwopPacoRefundRequest {
    pub api_request: ApiRequestEnvelope,
    pub office_id: Secret<String>,
    pub order_no: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_description: Option<String>,
    pub refund_amount: PacoTransactionAmount,
    pub local_maker_checker: PacoMakerChecker,
}

pub fn build_refund_request(
    item: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
    auth: &TwocTwopPacoAuthType,
) -> Result<TwocTwopPacoRefundRequest, error_stack::Report<errors::IntegrationError>> {
    let office_id = auth.office_id.clone();
    let amount =
        PacoTransactionAmount::new(item.request.minor_refund_amount, item.request.currency)?;
    let original_order_no = item.request.get_connector_order_id().change_context(
        errors::IntegrationError::MissingRequiredField {
            field_name: "connector_order_id",
            context: errors::IntegrationErrorContext {
                suggested_action: Some(
                    "Pass the original Authorize's `orderNo` (== the \
                             `merchant_transaction_id` you sent on Authorize) as \
                             `connector_order_id` on the Refund request."
                        .to_string(),
                ),
                doc_url: Some("https://devzone.2c2p.com/reference/refund".to_string()),
                additional_context: Some(
                    "PACO matches refunds against the original transaction's `orderNo`, \
                             which is not derivable from `connector_transaction_id` (PACO's \
                             `invoiceNo2C2P`)."
                        .to_string(),
                ),
            },
        },
    )?;
    let maker_id = item
        .request
        .refund_connector_metadata
        .as_ref()
        .and_then(extract_paco_maker_id)
        .or_else(|| {
            item.request
                .connector_feature_data
                .as_ref()
                .and_then(extract_paco_maker_id)
        })
        .unwrap_or_else(|| PACO_REFUND_MAKER_ID.to_string());
    let request_message_id =
        paco_require_merchant_request_id(item.resource_common_data.get_merchant_request_id())?;
    Ok(TwocTwopPacoRefundRequest {
        api_request: ApiRequestEnvelope::new(request_message_id),
        office_id,
        order_no: original_order_no,
        product_description: item.request.reason.clone(),
        refund_amount: amount,
        local_maker_checker: PacoMakerChecker {
            maker: PacoHumanActor { username: maker_id },
        },
    })
}

fn extract_paco_maker_id(meta: &common_utils::SecretSerdeValue) -> Option<String> {
    let value = meta.peek();
    let obj = value.as_object()?;
    let s = obj.get("maker_id").and_then(|v| v.as_str())?;
    (!s.is_empty()).then(|| s.to_string())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PacoPaymentStatus {
    /// Authorized.
    A,
    /// Settled / Charged.
    S,
    /// Voided.
    V,
    /// Refunded.
    R,
    /// Incomplete (3DS challenge in flight or pending).
    I,
    /// Pending.
    P,
    /// Payment Created, Page Generated (hosted-page wallet / redirect).
    #[serde(rename = "PCPS")]
    Pcps,
    /// Failure.
    F,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PacoPaymentStep {
    /// Pre-authorisation.
    PA,
    /// Settlement.
    ST,
    /// Voided.
    VD,
    /// Refunded (final).
    RF,
    /// Refund Requested (in flight).
    RR,
    /// Awaiting Challenge.
    AC,
    /// Initiated / Pending.
    IN,
    /// Pending refund.
    RP,
    /// Hosted page generated.
    GP,
    /// Pending Response from acquirer.
    PR,
    #[serde(other)]
    Unknown,
}

fn map_attempt_status(status: &PacoPaymentStatus, step: &PacoPaymentStep) -> AttemptStatus {
    match (status, step) {
        (PacoPaymentStatus::A, PacoPaymentStep::PA) => AttemptStatus::Charged,
        (PacoPaymentStatus::S, PacoPaymentStep::ST) => AttemptStatus::Charged,
        (PacoPaymentStatus::V, PacoPaymentStep::VD) => AttemptStatus::Voided,
        (PacoPaymentStatus::R, PacoPaymentStep::RF) => AttemptStatus::Charged,
        (PacoPaymentStatus::R, PacoPaymentStep::RR) => AttemptStatus::Charged,
        (PacoPaymentStatus::I, _) => AttemptStatus::AuthenticationPending,
        (PacoPaymentStatus::Pcps, PacoPaymentStep::GP) => AttemptStatus::AuthenticationPending,
        (PacoPaymentStatus::P, PacoPaymentStep::IN) => AttemptStatus::Authorizing,
        (PacoPaymentStatus::P, PacoPaymentStep::RP) => AttemptStatus::Authorizing,
        (PacoPaymentStatus::F, _) => AttemptStatus::Failure,
        (s, st) => {
            tracing::warn!(
                target: "twoc_twop_paco",
                paymentStatus = ?s,
                paymentStep = ?st,
                "twoc_twop_paco: unknown (paymentStatus, paymentStep) pair — mapped to AttemptStatus::Unknown"
            );
            AttemptStatus::Unknown
        }
    }
}

fn map_refund_status(status: &PacoPaymentStatus, step: &PacoPaymentStep) -> RefundStatus {
    match (status, step) {
        (PacoPaymentStatus::R, PacoPaymentStep::RF) => RefundStatus::Success,
        (PacoPaymentStatus::R, PacoPaymentStep::RR) => RefundStatus::Pending,
        (PacoPaymentStatus::P, PacoPaymentStep::RP) => RefundStatus::Pending,
        (PacoPaymentStatus::V, PacoPaymentStep::VD) => RefundStatus::Success,
        (PacoPaymentStatus::F, _) => RefundStatus::Failure,
        (s, st) => {
            tracing::warn!(
                target: "twoc_twop_paco",
                paymentStatus = ?s,
                paymentStep = ?st,
                "twoc_twop_paco: unknown (paymentStatus, paymentStep) pair — defaulting refund to Failure"
            );
            RefundStatus::Failure
        }
    }
}

/// PACO refund response codes, classified by terminal/in-flight state.
///
/// Source: https://devzone.2c2p.com/docs/api-response-code (sections relevant
/// to /Refund/refund). Codes outside this enum fall into the `Unknown` arm and
/// are classified as `Pending` (see `From<PacoRefundResponseCode> for
/// RefundStatus` for why — duplicate-refund safety).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PacoRefundResponseCode {
    // --- Terminal Success ---
    #[serde(rename = "PC-B052407")]
    Refunded,
    #[serde(rename = "PC-B053501")]
    RefundDisbursementSuccess,

    // --- In-flight / Pending (refund accepted, downstream not yet final) ---
    #[serde(rename = "PC-B053502")]
    RefundRequestAccepted,
    #[serde(rename = "PC-B053557")]
    RefundPendingReview,
    #[serde(rename = "PC-B053563")]
    PendingExternalPartyReview,
    #[serde(rename = "PC-B054042")]
    RefundPending,
    #[serde(rename = "PC-B054046")]
    InsufficientFundsForRefund,
    #[serde(rename = "PC-B054048")]
    SubMerchantInsufficientFunds,

    // --- Terminal Failure (request validation + downstream rejection) ---
    #[serde(rename = "PC-B050040")]
    InvalidRefundAmount,
    #[serde(rename = "PC-B050041")]
    InvalidRefundItemReference,
    #[serde(rename = "PC-B050042")]
    ItemizedRefundUnavailable,
    #[serde(rename = "PC-B050043")]
    RefundItemsExceedRefundable,
    #[serde(rename = "PC-B050053")]
    TransactionCannotBeRefunded,
    #[serde(rename = "PC-B050054")]
    InvalidRefundNumber,
    #[serde(rename = "PC-B050055")]
    RefundApiFeatureUnavailable,
    #[serde(rename = "PC-B050056")]
    RefundAmountInvalid,
    #[serde(rename = "PC-B050057")]
    CannotRefundMoreThanTransaction,
    #[serde(rename = "PC-B050058")]
    RefundExceedsTransactionAmount,
    #[serde(rename = "PC-B050059")]
    RefundNotAllowed,
    #[serde(rename = "PC-B050060")]
    PartialRefundNotAllowed,
    #[serde(rename = "PC-B050061")]
    SubMerchantRefundExceedsTransaction,
    #[serde(rename = "PC-B050062")]
    RefundExceededAllowableTimeframe,
    #[serde(rename = "PC-B053503")]
    RefundRejected,
    #[serde(rename = "PC-B053504")]
    RefundFailed,
    #[serde(rename = "PC-B053505")]
    RefundRejectedByBank,
    #[serde(rename = "PC-B053506")]
    RefundEmailDeliveryFailed,
    #[serde(rename = "PC-B053507")]
    RefundCancelled,
    #[serde(rename = "PC-B053508")]
    RefundLinkExpired,
    #[serde(rename = "PC-B054043")]
    RefundRejectedByReviewer,
    #[serde(rename = "PC-B054044")]
    RefundRejectedGeneric,
    #[serde(rename = "PC-B054045")]
    RefundFailedGeneric,

    /// Catch-all for unenumerated PC-Bxxxxxx codes. Resolves to Pending so we
    /// don't tell a merchant a refund failed when PACO may actually have
    /// processed it — see the `From` impl below for rationale.
    #[serde(other)]
    Unknown,
}

impl From<PacoRefundResponseCode> for RefundStatus {
    fn from(code: PacoRefundResponseCode) -> Self {
        use PacoRefundResponseCode::*;
        match code {
            Refunded | RefundDisbursementSuccess => Self::Success,

            // Why Unknown → Pending (not Failure): returning Failure on an
            // unknown code is dangerous for refunds. If PACO actually
            // processed the refund but returned a code we haven't enumerated
            // yet, the merchant sees "failed" → retries → gets a duplicate
            // refund → real money loss. Pending is recoverable: RSync will
            // poll, return a known code, and reclassify correctly. The raw
            // PC-Bxxxxxx string is still surfaced for ops grep-ability.
            RefundRequestAccepted
            | RefundPendingReview
            | PendingExternalPartyReview
            | RefundPending
            | InsufficientFundsForRefund
            | SubMerchantInsufficientFunds
            | Unknown => Self::Pending,

            InvalidRefundAmount
            | InvalidRefundItemReference
            | ItemizedRefundUnavailable
            | RefundItemsExceedRefundable
            | TransactionCannotBeRefunded
            | InvalidRefundNumber
            | RefundApiFeatureUnavailable
            | RefundAmountInvalid
            | CannotRefundMoreThanTransaction
            | RefundExceedsTransactionAmount
            | RefundNotAllowed
            | PartialRefundNotAllowed
            | SubMerchantRefundExceedsTransaction
            | RefundExceededAllowableTimeframe
            | RefundRejected
            | RefundFailed
            | RefundRejectedByBank
            | RefundEmailDeliveryFailed
            | RefundCancelled
            | RefundLinkExpired
            | RefundRejectedByReviewer
            | RefundRejectedGeneric
            | RefundFailedGeneric => Self::Failure,
        }
    }
}

pub fn classify_refund_response_code(code: Option<&str>) -> Option<RefundStatus> {
    let code = code?.trim();
    if code.is_empty() {
        return None;
    }
    let parsed: PacoRefundResponseCode =
        serde_json::from_value(serde_json::Value::String(code.to_string())).ok()?;
    Some(parsed.into())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PacoApiResponse {
    pub response_code: Option<String>,
    pub response_description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PacoPaymentStatusInfo {
    pub payment_status: PacoPaymentStatus,
    pub payment_step: PacoPaymentStep,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PacoPriorPaymentResponseDetails {
    #[serde(default)]
    pub response_code: Option<String>,
    #[serde(default)]
    pub response_description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PacoPaymentPage {
    #[serde(default, alias = "paymentPageURL")]
    pub payment_page_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PacoPaymentResultBlock {
    #[serde(default, rename = "invoiceNo2C2P")]
    pub invoice_no2c2p: Option<String>,
    #[serde(default)]
    pub order_no: Option<String>,
    #[serde(default)]
    pub controller_internal_id: Option<String>,
    #[serde(default)]
    pub payment_status_info: Option<PacoPaymentStatusInfo>,
    #[serde(default)]
    pub prior_payment_response_details: Option<PacoPriorPaymentResponseDetails>,
    #[serde(default)]
    pub payment_page: Option<PacoPaymentPage>,
    #[serde(default, alias = "paymentPageURL")]
    pub payment_page_url: Option<String>,
    #[serde(default)]
    pub web_payment_url: Option<String>,
    #[serde(default, rename = "aresACSChallenge")]
    pub ares_acs_challenge: Option<AresAcsChallenge>,
    #[serde(default)]
    pub credit_card_authenticated_details: Option<PacoCreditCardAuthenticatedDetails>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AresAcsChallenge {
    #[serde(default, rename = "acsURL", alias = "acsUrl")]
    pub acs_url: Option<String>,
    #[serde(default, rename = "rawCreq", alias = "raw_creq")]
    pub raw_creq: Option<Secret<String>>,
    #[serde(default, rename = "threeDSSessionData")]
    pub three_ds_session_data: Option<Secret<String>>,
    #[serde(default, rename = "authentication3DSVersion")]
    pub authentication_3ds_version: Option<String>,
    #[serde(default, rename = "challengeHTML")]
    pub challenge_html: Option<Secret<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PacoCreditCardAuthenticatedDetails {
    pub cavv: Option<Secret<String>>,
    #[serde(rename = "eciValue")]
    pub eci_value: Option<String>,
    #[serde(rename = "threeDsTransactionId")]
    pub three_ds_transaction_id: Option<Secret<String>>,
    #[serde(rename = "authentication3DSVersion")]
    pub authentication_3ds_version: Option<String>,
    pub authentication_status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PacoData {
    #[serde(default)]
    pub payment_result: Option<PacoPaymentResultBlock>,
    #[serde(default)]
    pub payment_incomplete_result: Option<PacoPaymentResultBlock>,
    #[serde(default)]
    pub web_payment_result: Option<PacoPaymentResultBlock>,
    #[serde(default)]
    pub payment_page: Option<PacoPaymentPage>,
    #[serde(default)]
    pub payment_status_info: Option<PacoPaymentStatusInfo>,
    #[serde(default, rename = "invoiceNo2C2P")]
    pub invoice_no2c2p: Option<String>,
    #[serde(default)]
    pub order_no: Option<String>,
    #[serde(default)]
    pub refund_no: Option<String>,
    #[serde(default)]
    pub psp_response: Option<PacoPriorPaymentResponseDetails>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TwocTwopPacoNonUiResponse {
    #[serde(default)]
    pub data: Option<PacoData>,
    #[serde(default)]
    pub api_response: Option<PacoApiResponse>,
    #[serde(default)]
    pub version: Option<String>,
}

impl TwocTwopPacoNonUiResponse {
    pub fn merged_result(&self) -> Option<&PacoPaymentResultBlock> {
        self.data.as_ref().and_then(|d| {
            d.payment_result
                .as_ref()
                .or(d.payment_incomplete_result.as_ref())
                .or(d.web_payment_result.as_ref())
        })
    }

    pub fn flat_data_block(&self) -> Option<PacoPaymentResultBlock> {
        let data = self.data.as_ref()?;
        if let Some(b) = data
            .payment_result
            .as_ref()
            .or(data.payment_incomplete_result.as_ref())
        {
            return Some(b.clone());
        }
        if data.payment_status_info.is_some() {
            return Some(PacoPaymentResultBlock {
                invoice_no2c2p: data.invoice_no2c2p.clone(),
                order_no: data.order_no.clone(),
                controller_internal_id: None,
                payment_status_info: data.payment_status_info.clone(),
                prior_payment_response_details: data.psp_response.clone(),
                payment_page: None,
                payment_page_url: None,
                web_payment_url: None,
                ares_acs_challenge: None,
                credit_card_authenticated_details: None,
            });
        }
        None
    }
}

impl<F, T> TryFrom<ResponseRouterData<TwocTwopPacoNonUiResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
where
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TwocTwopPacoNonUiResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;
        let result = response.merged_result();
        let api_response = response.api_response.clone();

        let (status, redirection_data, connector_txn_id, connector_response_reference_id, prior) =
            match result {
                Some(block) => {
                    let info = block
                        .payment_status_info
                        .as_ref()
                        .ok_or_else(|| {
                            error_stack::report!(
                                errors::ConnectorError::response_deserialization_failed_with_context(
                                    http_code,
                                    Some(
                                        "twoc_twop_paco: paymentStatusInfo missing on Authorize response"
                                            .to_string(),
                                    ),
                                )
                            )
                        })?;
                    let status = map_attempt_status(&info.payment_status, &info.payment_step);
                    let redirection_data =
                        if let Some(challenge) = block.ares_acs_challenge.as_ref() {
                            let acs_url = challenge.acs_url.clone().unwrap_or_default();
                            let mut form_fields: HashMap<String, String> = HashMap::new();
                            if let Some(creq) = &challenge.raw_creq {
                                form_fields.insert("creq".to_string(), creq.peek().clone());
                            }
                            if let Some(session_data) = &challenge.three_ds_session_data {
                                form_fields.insert(
                                    "threeDSSessionData".to_string(),
                                    session_data.peek().clone(),
                                );
                            }
                            Some(Box::new(RedirectForm::Form {
                                endpoint: acs_url,
                                method: Method::Post,
                                form_fields,
                            }))
                        } else {
                            let url = block
                                .web_payment_url
                                .clone()
                                .or_else(|| {
                                    response
                                        .data
                                        .as_ref()
                                        .and_then(|d| d.payment_page.as_ref())
                                        .and_then(|p| p.payment_page_url.clone())
                                })
                                .or_else(|| {
                                    block
                                        .payment_page
                                        .as_ref()
                                        .and_then(|p| p.payment_page_url.clone())
                                })
                                .or_else(|| block.payment_page_url.clone());
                            url.map(|endpoint| {
                                Box::new(RedirectForm::Form {
                                    endpoint,
                                    method: Method::Get,
                                    form_fields: HashMap::new(),
                                })
                            })
                        };
                    (
                        status,
                        redirection_data,
                        block.invoice_no2c2p.clone(),
                        block.order_no.clone(),
                        block.prior_payment_response_details.clone(),
                    )
                }
                None => (AttemptStatus::Pending, None, None, None, None),
            };

        if matches!(status, AttemptStatus::Failure) {
            let (code, message) = error_code_message(&api_response, &prior);
            let error = ErrorResponse {
                code,
                message: message.clone(),
                reason: Some(message),
                status_code: http_code,
                attempt_status: Some(FlowStatus::Payment(status)),
                connector_transaction_id: connector_txn_id,
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
            };
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    raw_connector_response: serde_json::to_string(&response).ok().map(Secret::new),
                    ..router_data.resource_common_data
                },
                response: Err(error),
                ..router_data
            });
        }

        let resource_id = connector_txn_id
            .clone()
            .map(ResponseId::ConnectorTransactionId)
            .unwrap_or(ResponseId::NoResponseId);

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                raw_connector_response: serde_json::to_string(&response).ok().map(Secret::new),
                ..router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id,
                redirection_data,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id,
                incremental_authorization_allowed: None,
                status_code: http_code,
                splits: None,
            }),
            ..router_data
        })
    }
}

impl TryFrom<ResponseRouterData<TwocTwopPacoNonUiResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TwocTwopPacoNonUiResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;
        let result = response.merged_result();
        let api_response = response.api_response.clone();
        let (status, txn_id, ref_id, prior) =
            extract_status(result, AttemptStatus::CaptureInitiated);

        if matches!(status, AttemptStatus::Failure) {
            let (code, message) = error_code_message(&api_response, &prior);
            let error = ErrorResponse {
                code,
                message: message.clone(),
                reason: Some(message),
                status_code: http_code,
                attempt_status: Some(FlowStatus::Payment(status)),
                connector_transaction_id: txn_id,
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
            };
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    raw_connector_response: serde_json::to_string(&response).ok().map(Secret::new),
                    ..router_data.resource_common_data
                },
                response: Err(error),
                ..router_data
            });
        }

        let resource_id = txn_id
            .map(ResponseId::ConnectorTransactionId)
            .unwrap_or(ResponseId::NoResponseId);

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                raw_connector_response: serde_json::to_string(&response).ok().map(Secret::new),
                ..router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id,
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: ref_id,
                incremental_authorization_allowed: None,
                status_code: http_code,
                splits: None,
            }),
            ..router_data
        })
    }
}

impl TryFrom<ResponseRouterData<TwocTwopPacoNonUiResponse, Self>>
    for RouterDataV2<VoidPC, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TwocTwopPacoNonUiResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;
        let result = response.flat_data_block();
        let api_response = response.api_response.clone();
        let (status, txn_id, ref_id, prior) =
            extract_status(result.as_ref(), AttemptStatus::VoidInitiated);

        if matches!(status, AttemptStatus::Failure) {
            let (code, message) = error_code_message(&api_response, &prior);
            let error = ErrorResponse {
                code,
                message: message.clone(),
                reason: Some(message),
                status_code: http_code,
                attempt_status: Some(FlowStatus::Payment(status)),
                connector_transaction_id: txn_id,
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
            };
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    raw_connector_response: serde_json::to_string(&response).ok().map(Secret::new),
                    ..router_data.resource_common_data
                },
                response: Err(error),
                ..router_data
            });
        }

        let resource_id = txn_id
            .map(ResponseId::ConnectorTransactionId)
            .unwrap_or(ResponseId::NoResponseId);

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                raw_connector_response: serde_json::to_string(&response).ok().map(Secret::new),
                ..router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id,
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: ref_id,
                incremental_authorization_allowed: None,
                status_code: http_code,
                splits: None,
            }),
            ..router_data
        })
    }
}

impl TryFrom<ResponseRouterData<TwocTwopPacoNonUiResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TwocTwopPacoNonUiResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;
        let result = response.flat_data_block();
        let api_response = response.api_response.clone();
        let (status, txn_id, ref_id, prior) =
            extract_status(result.as_ref(), AttemptStatus::VoidInitiated);

        if matches!(status, AttemptStatus::Failure) {
            let (code, message) = error_code_message(&api_response, &prior);
            let error = ErrorResponse {
                code,
                message: message.clone(),
                reason: Some(message),
                status_code: http_code,
                attempt_status: Some(FlowStatus::Payment(status)),
                connector_transaction_id: txn_id,
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
            };
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    raw_connector_response: serde_json::to_string(&response).ok().map(Secret::new),
                    ..router_data.resource_common_data
                },
                response: Err(error),
                ..router_data
            });
        }

        let resource_id = txn_id
            .map(ResponseId::ConnectorTransactionId)
            .unwrap_or(ResponseId::NoResponseId);

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                raw_connector_response: serde_json::to_string(&response).ok().map(Secret::new),
                ..router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id,
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: ref_id,
                incremental_authorization_allowed: None,
                status_code: http_code,
                splits: None,
            }),
            ..router_data
        })
    }
}

impl TryFrom<ResponseRouterData<TwocTwopPacoNonUiResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TwocTwopPacoNonUiResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;
        let result = response.flat_data_block();

        let refund_status = match result.as_ref().and_then(|b| b.payment_status_info.as_ref()) {
            Some(info) => map_refund_status(&info.payment_status, &info.payment_step),
            None => RefundStatus::Pending,
        };

        let connector_refund_id = result
            .as_ref()
            .and_then(|b| b.invoice_no2c2p.clone())
            .unwrap_or_else(|| router_data.request.refund_id.clone());

        if refund_status == RefundStatus::Failure {
            let (code, message) = error_code_message(
                &response.api_response,
                &result.and_then(|b| b.prior_payment_response_details),
            );
            let error = ErrorResponse {
                code,
                message: message.clone(),
                reason: Some(message),
                status_code: http_code,
                attempt_status: None,
                connector_transaction_id: Some(connector_refund_id),
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
            };
            return Ok(Self {
                resource_common_data: RefundFlowData {
                    status: refund_status,
                    raw_connector_response: serde_json::to_string(&response).ok().map(Secret::new),
                    ..router_data.resource_common_data
                },
                response: Err(error),
                ..router_data
            });
        }

        Ok(Self {
            resource_common_data: RefundFlowData {
                status: refund_status,
                raw_connector_response: serde_json::to_string(&response).ok().map(Secret::new),
                ..router_data.resource_common_data
            },
            response: Ok(RefundsResponseData {
                connector_refund_id,
                refund_status,
                status_code: http_code,
            }),
            ..router_data
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TwocTwopPacoInquiryResponse {
    #[serde(default)]
    pub api_response: Option<PacoApiResponse>,
    #[serde(default, deserialize_with = "deserialize_inquiry_data")]
    pub data: Option<PacoInquiryData>,
}

fn deserialize_inquiry_data<'de, D>(deserializer: D) -> Result<Option<PacoInquiryData>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize as _;
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Repr {
        List(Vec<PacoInquiryData>),
        Single(PacoInquiryData),
        Null,
    }
    match Option::<Repr>::deserialize(deserializer)? {
        Some(Repr::List(mut v)) => Ok(v.drain(..).next()),
        Some(Repr::Single(d)) => Ok(Some(d)),
        Some(Repr::Null) | None => Ok(None),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PacoInquiryData {
    #[serde(default)]
    pub payment_status_info: Option<PacoPaymentStatusInfo>,
    #[serde(default, rename = "invoiceNo2C2P")]
    pub invoice_no2c2p: Option<String>,
    #[serde(default)]
    pub order_no: Option<String>,
    #[serde(default)]
    pub credit_card_authenticated_details: Option<PacoCreditCardAuthenticatedDetails>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TwocTwopPacoPSyncInquiryResponse(pub PacoResponseWithRaw<TwocTwopPacoInquiryResponse>);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TwocTwopPacoRSyncInquiryResponse(pub PacoResponseWithRaw<TwocTwopPacoInquiryResponse>);

impl TryFrom<ResponseRouterData<TwocTwopPacoPSyncInquiryResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TwocTwopPacoPSyncInquiryResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let PacoResponseWithRaw {
            parsed_response,
            raw_response,
        } = item.response.0;
        // Derive settlement_status from PACO's paymentStatus before we move parsed_response into
        // the inner TryFrom. PACO splits success into A (Authorized — awaiting settlement,
        // Void is the valid reversal) and S (Settled — Refund is the valid reversal); UCS's
        // AttemptStatus collapses both into Charged, so euler needs this field to route
        // refund vs void.
        let settlement_status = parsed_response
            .data
            .as_ref()
            .and_then(|d| d.payment_status_info.as_ref())
            .map(|psi| paco_status_to_settlement_status(&psi.payment_status));
        let router_data = Self::try_from(ResponseRouterData {
            response: parsed_response,
            router_data: item.router_data,
            http_code: item.http_code,
        })?;
        Ok(Self {
            resource_common_data: PaymentFlowData {
                raw_connector_response: Some(Secret::new(raw_response.to_string())),
                settlement_status,
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}

fn paco_status_to_settlement_status(
    status: &PacoPaymentStatus,
) -> domain_types::connector_types::SettlementStatus {
    use domain_types::connector_types::SettlementStatus;
    match status {
        PacoPaymentStatus::S => SettlementStatus::Settled,
        PacoPaymentStatus::A => SettlementStatus::NotSettled,
        _ => SettlementStatus::Unspecified,
    }
}

impl TryFrom<ResponseRouterData<TwocTwopPacoInquiryResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TwocTwopPacoInquiryResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;

        let info = response
            .data
            .as_ref()
            .and_then(|d| d.payment_status_info.as_ref());
        let status = match info {
            Some(info) => map_attempt_status(&info.payment_status, &info.payment_step),
            None => AttemptStatus::Pending,
        };
        let invoice = response
            .data
            .as_ref()
            .and_then(|d| d.invoice_no2c2p.clone());
        let order = response.data.as_ref().and_then(|d| d.order_no.clone());

        if matches!(status, AttemptStatus::Failure) {
            let (code, message) = error_code_message(&response.api_response, &None);
            let error = ErrorResponse {
                code,
                message: message.clone(),
                reason: Some(message),
                status_code: http_code,
                attempt_status: Some(FlowStatus::Payment(status)),
                connector_transaction_id: invoice.clone(),
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
            };
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    raw_connector_response: serde_json::to_string(&response).ok().map(Secret::new),
                    ..router_data.resource_common_data
                },
                response: Err(error),
                ..router_data
            });
        }

        let resource_id = invoice
            .clone()
            .map(ResponseId::ConnectorTransactionId)
            .unwrap_or(ResponseId::NoResponseId);

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                raw_connector_response: serde_json::to_string(&response).ok().map(Secret::new),
                ..router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id,
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: order,
                incremental_authorization_allowed: None,
                status_code: http_code,
                splits: None,
            }),
            ..router_data
        })
    }
}

impl TryFrom<ResponseRouterData<TwocTwopPacoRSyncInquiryResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TwocTwopPacoRSyncInquiryResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let PacoResponseWithRaw {
            parsed_response,
            raw_response,
        } = item.response.0;
        let router_data = Self::try_from(ResponseRouterData {
            response: parsed_response,
            router_data: item.router_data,
            http_code: item.http_code,
        })?;
        Ok(Self {
            resource_common_data: RefundFlowData {
                raw_connector_response: Some(Secret::new(raw_response.to_string())),
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}

impl TryFrom<ResponseRouterData<TwocTwopPacoInquiryResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TwocTwopPacoInquiryResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;

        let info = response
            .data
            .as_ref()
            .and_then(|d| d.payment_status_info.as_ref());
        let refund_status = match info {
            Some(info) => map_refund_status(&info.payment_status, &info.payment_step),
            None => RefundStatus::Pending,
        };
        let connector_refund_id = router_data.request.connector_refund_id.clone();

        if refund_status == RefundStatus::Failure {
            let (code, message) = error_code_message(&response.api_response, &None);
            let error = ErrorResponse {
                code,
                message: message.clone(),
                reason: Some(message),
                status_code: http_code,
                attempt_status: None,
                connector_transaction_id: Some(connector_refund_id),
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
            };
            return Ok(Self {
                resource_common_data: RefundFlowData {
                    status: refund_status,
                    raw_connector_response: serde_json::to_string(&response).ok().map(Secret::new),
                    ..router_data.resource_common_data
                },
                response: Err(error),
                ..router_data
            });
        }

        Ok(Self {
            resource_common_data: RefundFlowData {
                status: refund_status,
                raw_connector_response: serde_json::to_string(&response).ok().map(Secret::new),
                ..router_data.resource_common_data
            },
            response: Ok(RefundsResponseData {
                connector_refund_id,
                refund_status,
                status_code: http_code,
            }),
            ..router_data
        })
    }
}

fn extract_status(
    block: Option<&PacoPaymentResultBlock>,
    fallback: AttemptStatus,
) -> (
    AttemptStatus,
    Option<String>,
    Option<String>,
    Option<PacoPriorPaymentResponseDetails>,
) {
    match block {
        Some(b) => {
            let status = b
                .payment_status_info
                .as_ref()
                .map(|i| map_attempt_status(&i.payment_status, &i.payment_step))
                .unwrap_or(fallback);
            (
                status,
                b.invoice_no2c2p.clone(),
                b.order_no.clone(),
                b.prior_payment_response_details.clone(),
            )
        }
        None => (fallback, None, None, None),
    }
}

pub fn error_code_message(
    api_response: &Option<PacoApiResponse>,
    prior: &Option<PacoPriorPaymentResponseDetails>,
) -> (String, String) {
    let prior_code = prior.as_ref().and_then(|p| p.response_code.clone());
    let prior_msg = prior.as_ref().and_then(|p| p.response_description.clone());
    let api_code = api_response.as_ref().and_then(|a| a.response_code.clone());
    let api_msg = api_response
        .as_ref()
        .and_then(|a| a.response_description.clone());
    let code = prior_code
        .or(api_code)
        .unwrap_or_else(|| NO_ERROR_CODE.to_string());
    let message = prior_msg
        .or(api_msg)
        .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string());
    (code, message)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TwocTwopPacoErrorResponse {
    #[serde(default)]
    pub api_response: Option<PacoApiResponse>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
}

impl TwocTwopPacoErrorResponse {
    pub fn flatten(self) -> (String, String) {
        let api_code = self
            .api_response
            .as_ref()
            .and_then(|a| a.response_code.clone());
        let api_msg = self
            .api_response
            .as_ref()
            .and_then(|a| a.response_description.clone());
        let code = api_code
            .or(self.error)
            .unwrap_or_else(|| NO_ERROR_CODE.to_string());
        let message = api_msg
            .or(self.message)
            .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string());
        (code, message)
    }
}

#[derive(Debug, Serialize)]
pub struct PacoJoseClaims<'a> {
    pub iss: &'a str,
    pub aud: &'static str,
    #[serde(rename = "CompanyApiKey")]
    pub company_api_key: &'a str,
    pub iat: i64,
    pub nbf: i64,
    pub exp: i64,
    pub request: serde_json::Value,
}

impl<'a> PacoJoseClaims<'a> {
    pub fn new(access_token: &'a str, request: serde_json::Value) -> Self {
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        Self {
            iss: access_token,
            aud: PACO_AUDIENCE,
            company_api_key: access_token,
            iat: now,
            nbf: now,
            exp: now + PACO_JWT_TTL_SECONDS,
            request,
        }
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TwocTwopPacoRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for TwocTwopPacoAuthorizeRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: TwocTwopPacoRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = TwocTwopPacoAuthType::try_from(&item.router_data.connector_config)?;
        build_authorize_request(&item.router_data, &auth)
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TwocTwopPacoRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for TwocTwopPacoCaptureRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: TwocTwopPacoRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = TwocTwopPacoAuthType::try_from(&item.router_data.connector_config)?;
        build_capture_request(&item.router_data, &auth)
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TwocTwopPacoRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for TwocTwopPacoVoidRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: TwocTwopPacoRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = TwocTwopPacoAuthType::try_from(&item.router_data.connector_config)?;
        build_void_request(&item.router_data, &auth)
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TwocTwopPacoRouterData<
            RouterDataV2<
                VoidPC,
                PaymentFlowData,
                PaymentsCancelPostCaptureData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for TwocTwopPacoVoidPcRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: TwocTwopPacoRouterData<
            RouterDataV2<
                VoidPC,
                PaymentFlowData,
                PaymentsCancelPostCaptureData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = TwocTwopPacoAuthType::try_from(&item.router_data.connector_config)?;
        Ok(Self(build_void_pc_request(&item.router_data, &auth)?))
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TwocTwopPacoRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for TwocTwopPacoRefundRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: TwocTwopPacoRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = TwocTwopPacoAuthType::try_from(&item.router_data.connector_config)?;
        build_refund_request(&item.router_data, &auth)
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<TwocTwopPacoAuthorizeResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TwocTwopPacoAuthorizeResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let PacoResponseWithRaw {
            parsed_response,
            raw_response,
        } = item.response.0;
        let router_data = Self::try_from(ResponseRouterData {
            response: parsed_response,
            router_data: item.router_data,
            http_code: item.http_code,
        })?;
        Ok(Self {
            resource_common_data: PaymentFlowData {
                raw_connector_response: Some(Secret::new(raw_response.to_string())),
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}

impl TryFrom<ResponseRouterData<TwocTwopPacoCaptureResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TwocTwopPacoCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let PacoResponseWithRaw {
            parsed_response,
            raw_response,
        } = item.response.0;
        let router_data = Self::try_from(ResponseRouterData {
            response: parsed_response,
            router_data: item.router_data,
            http_code: item.http_code,
        })?;
        Ok(Self {
            resource_common_data: PaymentFlowData {
                raw_connector_response: Some(Secret::new(raw_response.to_string())),
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}

impl TryFrom<ResponseRouterData<TwocTwopPacoVoidResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TwocTwopPacoVoidResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let PacoResponseWithRaw {
            parsed_response,
            raw_response,
        } = item.response.0;
        let router_data = Self::try_from(ResponseRouterData {
            response: parsed_response,
            router_data: item.router_data,
            http_code: item.http_code,
        })?;
        Ok(Self {
            resource_common_data: PaymentFlowData {
                raw_connector_response: Some(Secret::new(raw_response.to_string())),
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}

impl TryFrom<ResponseRouterData<TwocTwopPacoVoidPcResponse, Self>>
    for RouterDataV2<VoidPC, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TwocTwopPacoVoidPcResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let PacoResponseWithRaw {
            parsed_response,
            raw_response,
        } = item.response.0;
        let router_data = Self::try_from(ResponseRouterData {
            response: parsed_response,
            router_data: item.router_data,
            http_code: item.http_code,
        })?;
        Ok(Self {
            resource_common_data: PaymentFlowData {
                raw_connector_response: Some(Secret::new(raw_response.to_string())),
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}

impl TryFrom<ResponseRouterData<TwocTwopPacoRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TwocTwopPacoRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let PacoResponseWithRaw {
            parsed_response,
            raw_response,
        } = item.response.0;
        let router_data = Self::try_from(ResponseRouterData {
            response: parsed_response,
            router_data: item.router_data,
            http_code: item.http_code,
        })?;
        Ok(Self {
            resource_common_data: RefundFlowData {
                raw_connector_response: Some(Secret::new(raw_response.to_string())),
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}
