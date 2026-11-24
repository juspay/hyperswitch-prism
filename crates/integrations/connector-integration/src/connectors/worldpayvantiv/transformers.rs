use std::{borrow::Cow, collections::HashMap};

use common_enums::{self, CountryAlpha2, Currency};
use common_utils::{id_type::CustomerId, types::MinorUnit, StringMajorUnit};
use domain_types::{
    connector_flow::{
        Authorize, Capture, IncrementalAuthorization, PSync, RSync, Refund, RepeatPayment, Void,
        VoidPC,
    },
    connector_types::{
        MandateReference, MandateReferenceId, PaymentFlowData, PaymentVoidData,
        PaymentsAuthorizeData, PaymentsCancelPostCaptureData, PaymentsCaptureData,
        PaymentsIncrementalAuthorizationData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData, RepeatPaymentData,
        ResponseId,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    payment_method_data::{
        AdditionalPaymentData, PaymentMethodData, PaymentMethodDataTypes, RawCardNumber, WalletData,
    },
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
    ResponseTransformationErrorContext,
};
use error_stack::{Report, ResultExt};
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use crate::{
    connectors::worldpayvantiv::WorldpayvantivRouterData, types::ResponseRouterData, utils,
};

// Helper function to extract report group from connector config
fn extract_report_group(connector_config: &ConnectorSpecificConfig) -> Option<String> {
    WorldpayvantivAuthType::try_from(connector_config)
        .ok()
        .and_then(|auth| auth.report_group)
}

fn extract_customer_id(customer_id: &Option<CustomerId>) -> Option<String> {
    customer_id.as_ref().and_then(|id| {
        let customer_id_str = id.get_string_repr().to_string();
        if customer_id_str.len() <= worldpayvantiv_constants::CUSTOMER_ID_MAX_LENGTH {
            Some(customer_id_str)
        } else {
            None
        }
    })
}

pub const BASE64_ENGINE: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

// WorldpayVantiv Payments Request - wrapper for all payment flows with custom XML serialization
#[derive(Debug)]
pub struct WorldpayvantivPaymentsRequest<T: PaymentMethodDataTypes> {
    pub cnp_request: CnpOnlineRequest<T>,
}

// The macro's SoapXml path sends these pre-serialized bytes while logging the
// structured request with secrets masked. Serializing here so failures propagate.
#[derive(Debug, Serialize)]
pub struct WorldpayvantivXmlRequest<T: PaymentMethodDataTypes> {
    #[serde(flatten)]
    cnp_request: CnpOnlineRequest<T>,
    #[serde(skip)]
    xml: Secret<String>,
}

impl<T: PaymentMethodDataTypes + Serialize> TryFrom<CnpOnlineRequest<T>>
    for WorldpayvantivXmlRequest<T>
{
    type Error = Report<IntegrationError>;

    fn try_from(cnp_request: CnpOnlineRequest<T>) -> Result<Self, Self::Error> {
        let xml = utils::serialize_to_xml_string_with_root("cnpOnlineRequest", &cnp_request)?;
        Ok(Self {
            cnp_request,
            xml: Secret::new(xml),
        })
    }
}

impl<T: PaymentMethodDataTypes> super::super::macros::GetSoapXml for WorldpayvantivXmlRequest<T> {
    fn to_soap_xml(&self) -> String {
        self.xml.peek().clone()
    }
}

pub type WorldpayvantivAuthorizeRequest<T> = WorldpayvantivXmlRequest<T>;
pub type WorldpayvantivRepeatPaymentRequest<T> = WorldpayvantivXmlRequest<T>;
pub type WorldpayvantivRepeatPaymentResponse = CnpOnlineResponse;

// Serialize implementation
impl<T: PaymentMethodDataTypes + Serialize> Serialize for WorldpayvantivPaymentsRequest<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let full_xml =
            utils::serialize_to_xml_string_with_root("cnpOnlineRequest", &self.cnp_request)
                .map_err(serde::ser::Error::custom)?;

        // Serialize the complete XML string
        full_xml.serialize(serializer)
    }
}

// TryFrom implementations for macro integration
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        WorldpayvantivRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for WorldpayvantivAuthorizeRequest<T>
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: WorldpayvantivRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let payment_method_data = &item.router_data.request.payment_method_data;
        let order_source = OrderSource::from(payment_method_data.clone());

        // Handle payment info directly without generic constraints
        let payment_info = match payment_method_data {
            PaymentMethodData::Card(card_data) => {
                let card_type = match card_data.card_network.clone() {
                    Some(network) => WorldpayvativCardType::try_from(network)?,
                    None => {
                        // Fallback to BIN-based card issuer detection
                        let card_issuer =
                            domain_types::utils::get_card_issuer(card_data.card_number.peek())?;
                        WorldpayvativCardType::try_from(&card_issuer)?
                    }
                };

                let year_str = card_data.card_exp_year.peek();
                let formatted_year = if year_str.len() == 4 {
                    &year_str[2..]
                } else {
                    year_str
                };
                let exp_date = format!("{}{}", card_data.card_exp_month.peek(), formatted_year);

                let worldpay_card = WorldpayvantivCardData {
                    card_type,
                    number: card_data.card_number.clone(),
                    exp_date: exp_date.into(),
                    card_validation_num: Some(card_data.card_cvc.clone()),
                };

                PaymentInfo::Card(CardData {
                    card: worldpay_card,
                    processing_type: None,
                    network_transaction_id: None,
                })
            }
            _ => {
                return Err(IntegrationError::NotSupported {
                    message: "Payment method".to_string(),
                    connector: "worldpayvantiv",
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "Worldpay Vantiv authorization supports card payment data only."
                                .to_owned(),
                        ),
                        ..Default::default()
                    },
                }
                .into());
            }
        };

        let request = build_payment_request(
            &item.router_data,
            item.router_data.request.currency,
            item.router_data.request.money.amount(),
            item.router_data.request.is_auto_capture(),
            order_source,
            payment_info,
        )?;
        Self::try_from(request.cnp_request)
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        WorldpayvantivRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for WorldpayvantivRepeatPaymentRequest<T>
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: WorldpayvantivRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let request = &item.router_data.request;
        let payment_info = get_repeat_payment_info(request)?;
        let order_source = match request.payment_channel.as_ref() {
            Some(common_enums::PaymentChannel::MailOrder) => OrderSource::MailOrder,
            Some(common_enums::PaymentChannel::TelephoneOrder) => OrderSource::Telephone,
            _ => OrderSource::Ecommerce,
        };
        let request = build_payment_request(
            &item.router_data,
            request.currency,
            request.money.amount(),
            request.is_auto_capture(),
            order_source,
            payment_info,
        )?;
        Self::try_from(request.cnp_request)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorldpayvantivMandateMetadata {
    network_transaction_id: Option<Secret<String>>,
}

fn get_repeat_payment_info<T: PaymentMethodDataTypes>(
    request: &RepeatPaymentData<T>,
) -> Result<PaymentInfo<T>, Report<IntegrationError>> {
    match &request.mandate_reference {
        MandateReferenceId::ConnectorMandateId(mandate) => {
            let cnp_token = mandate.get_connector_mandate_id().ok_or(
                IntegrationError::MissingConnectorMandateID {
                    context: IntegrationErrorContext {
                        additional_context: Some("Connector mandate payments require the Worldpay Vantiv cnpToken as the connector mandate ID.".to_owned()),
                        ..Default::default()
                    },
                },
            )?;
            let card = match request.additional_payment_data.as_ref() {
                Some(AdditionalPaymentData::Card(card)) => card,
                _ => return Err(utils::missing_field_err("additional_payment_data.card")()),
            };
            let month = card
                .card_exp_month
                .as_ref()
                .ok_or_else(utils::missing_field_err(
                    "additional_payment_data.card.card_exp_month",
                ))?;
            let year = card
                .card_exp_year
                .as_ref()
                .ok_or_else(utils::missing_field_err(
                    "additional_payment_data.card.card_exp_year",
                ))?;
            let exp_date = format_token_expiry(month, year)?;
            let original_network_transaction_id = mandate
                .get_network_transaction_id()
                .map(Secret::new)
                .or_else(|| {
                    mandate.get_mandate_metadata().and_then(|metadata| {
                        serde_json::from_value::<WorldpayvantivMandateMetadata>(metadata.expose())
                            .ok()
                            .and_then(|metadata| metadata.network_transaction_id)
                    })
                });
            Ok(PaymentInfo::Token(TokenData {
                token: TokenizationData {
                    cnp_token: Secret::new(cnp_token),
                    exp_date,
                },
                original_network_transaction_id,
            }))
        }
        MandateReferenceId::NetworkMandateId(mandate) => {
            let (number, month, year, network) = match &request.payment_method_data {
                PaymentMethodData::Card(card) => (
                    &card.card_number,
                    &card.card_exp_month,
                    &card.card_exp_year,
                    &card.card_network,
                ),
                PaymentMethodData::CardDetailsForNetworkTransactionId(card) => (
                    &card.card_number,
                    &card.card_exp_month,
                    &card.card_exp_year,
                    &card.card_network,
                ),
                _ => {
                    return Err(IntegrationError::NotSupported {
                        message: "Network mandate payments require card details".into(),
                        connector: "worldpayvantiv",
                        context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Provide the card number and expiry with the network transaction ID, or use a connector mandate ID with token expiry in additional_payment_data.card."
                            .to_owned(),
                    ),
                    ..Default::default()
                },
                    }
                    .into())
                }
            };
            let card_type = match network.clone() {
                Some(network) => WorldpayvativCardType::try_from(network)?,
                None => WorldpayvativCardType::try_from(&domain_types::utils::get_card_issuer(
                    number.peek(),
                )?)?,
            };
            Ok(PaymentInfo::Card(CardData {
                card: WorldpayvantivCardData {
                    card_type,
                    number: number.clone(),
                    exp_date: format_token_expiry(month, year)?,
                    card_validation_num: None,
                },
                processing_type: Some(VantivProcessingType::MerchantInitiatedCOF),
                network_transaction_id: Some(Secret::new(mandate.network_transaction_id.clone())),
            }))
        }
        MandateReferenceId::NetworkTokenWithNTI(_) => Err(IntegrationError::NotSupported {
            message: "Network token mandate payments".into(),
            connector: "worldpayvantiv",
            context: IntegrationErrorContext {
                additional_context: Some("Worldpay Vantiv repeat payments do not support network tokens with a network transaction ID.".to_owned()),
                ..Default::default()
            },
        }
        .into()),
    }
}

fn format_token_expiry(
    month: &Secret<String>,
    year: &Secret<String>,
) -> Result<Secret<String>, Report<IntegrationError>> {
    let year = utils::get_expiry_year_2_digit(year.peek(), "card_exp_year")?;
    Ok(Secret::new(format!("{:0>2}{year}", month.peek())))
}

fn build_payment_request<T: PaymentMethodDataTypes, F, Req>(
    router_data: &RouterDataV2<F, PaymentFlowData, Req, PaymentsResponseData>,
    currency: Currency,
    amount: MinorUnit,
    is_auto_capture: bool,
    order_source: OrderSource,
    payment_info: PaymentInfo<T>,
) -> Result<WorldpayvantivPaymentsRequest<T>, Report<IntegrationError>> {
    let auth = WorldpayvantivAuthType::try_from(&router_data.connector_config)?;
    let merchant_id = auth.merchant_id_for_currency(Some(currency))?;

    let authentication = Authentication {
        user: auth.user,
        password: auth.password,
    };

    let merchant_txn_id = get_valid_transaction_id(
        router_data
            .resource_common_data
            .connector_request_reference_id
            .clone(),
        "transaction_id",
    )?;

    // Extract report group from metadata or use default
    let report_group =
        extract_report_group(&router_data.connector_config).unwrap_or_else(|| "rtpGrp".to_string());

    let bill_to_address = get_billing_address(&router_data.resource_common_data);
    let ship_to_address = get_shipping_address(&router_data.resource_common_data);

    let (authorization, sale) = if is_auto_capture && amount != MinorUnit::zero() {
        let sale = Sale {
            id: format!("{}_{}", OperationId::Sale, merchant_txn_id),
            report_group: report_group.clone(),
            customer_id: extract_customer_id(&router_data.resource_common_data.customer_id)
                .map(Secret::new),
            order_id: merchant_txn_id.clone(),
            amount,
            order_source,
            bill_to_address,
            ship_to_address,
            payment_info,
            enhanced_data: None,
            processing_instructions: None,
            cardholder_authentication: None,
        };
        (None, Some(sale))
    } else {
        let operation_id = if is_auto_capture {
            OperationId::Sale
        } else {
            OperationId::Auth
        };
        let authorization = Authorization {
            id: format!("{operation_id}_{merchant_txn_id}"),
            report_group: report_group.clone(),
            customer_id: extract_customer_id(&router_data.resource_common_data.customer_id)
                .map(Secret::new),
            cnp_txn_id: None,
            order_id: Some(merchant_txn_id.clone()),
            amount,
            order_source: Some(order_source),
            bill_to_address,
            ship_to_address,
            payment_info: Some(payment_info),
            enhanced_data: None,
            processing_instructions: None,
            cardholder_authentication: None,
            auth_indicator: None,
        };
        (Some(authorization), None)
    };

    let cnp_request = CnpOnlineRequest {
        version: worldpayvantiv_constants::WORLDPAYVANTIV_VERSION.to_string(),
        xmlns: worldpayvantiv_constants::XMLNS.to_string(),
        merchant_id,
        authentication,
        authorization,
        sale,
        capture: None,
        auth_reversal: None,
        void: None,
        credit: None,
    };

    Ok(WorldpayvantivPaymentsRequest { cnp_request })
}

pub(super) mod worldpayvantiv_constants {
    pub const WORLDPAYVANTIV_VERSION: &str = "12.30";
    #[allow(dead_code)]
    pub const XML_VERSION: &str = "1.0";
    #[allow(dead_code)]
    pub const XML_ENCODING: &str = "UTF-8";
    #[allow(dead_code)]
    pub const XML_STANDALONE: &str = "yes";
    pub const XMLNS: &str = "http://www.vantivcnp.com/schema";
    pub const MAX_PAYMENT_REFERENCE_ID_LENGTH: usize = 28;
    #[allow(dead_code)]
    pub const XML_CHARGEBACK: &str = "http://www.vantivcnp.com/chargebacks";
    #[allow(dead_code)]
    pub const MAC_FIELD_NUMBER: &str = "39";
    #[allow(dead_code)]
    pub const CUSTOMER_ID_MAX_LENGTH: usize = 50;
    #[allow(dead_code)]
    pub const CUSTOMER_REFERENCE_MAX_LENGTH: usize = 17;
}

#[derive(Debug, Clone)]
pub struct WorldpayvantivAuthType {
    pub user: Secret<String>,
    pub password: Secret<String>,
    /// Single merchant ID used for every currency (SignatureKey / MultiAuthKey).
    pub merchant_id: Option<Secret<String>>,
    /// Merchant IDs keyed by currency (BodyKey); used when `merchant_id` is not set.
    pub merchant_id_by_currency: Option<HashMap<Currency, Secret<String>>>,
    pub report_group: Option<String>,
    pub merchant_config_currency: Option<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for WorldpayvantivAuthType {
    type Error = Report<IntegrationError>;
    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Worldpayvantiv {
                user,
                password,
                merchant_id,
                merchant_id_by_currency,
                report_group,
                merchant_config_currency,
                ..
            } => {
                if merchant_id_by_currency
                    .as_ref()
                    .is_some_and(|merchant_ids| merchant_ids.is_empty())
                {
                    return Err(IntegrationError::InvalidConnectorConfig {
                        config: "merchant_id_by_currency",
                        context: IntegrationErrorContext {
                            suggested_action: Some(
                                "Set at least one currency in the Worldpay Vantiv merchant_id_by_currency."
                                    .to_string(),
                            ),
                            ..Default::default()
                        },
                    }
                    .into());
                }
                if merchant_id.is_none() && merchant_id_by_currency.is_none() {
                    return Err(IntegrationError::FailedToObtainAuthType {
                        context: IntegrationErrorContext {
                            suggested_action: Some(
                                "Set either merchant_id or merchant_id_by_currency in the Worldpay Vantiv connector configuration."
                                    .to_string(),
                            ),
                            ..Default::default()
                        },
                    }
                    .into());
                }
                Ok(Self {
                    user: user.to_owned(),
                    password: password.to_owned(),
                    merchant_id: merchant_id.to_owned(),
                    merchant_id_by_currency: merchant_id_by_currency.to_owned(),
                    report_group: report_group.clone(),
                    merchant_config_currency: merchant_config_currency.clone(),
                })
            }
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: IntegrationErrorContext {
                    additional_context: Some("Expected Worldpay Vantiv connector configuration containing user, password, and merchant ID settings.".to_owned()),
                    ..Default::default()
                },
            }
            .into()),
        }
    }
}

impl WorldpayvantivAuthType {
    pub(super) fn merchant_id_for_currency(
        &self,
        currency: Option<Currency>,
    ) -> Result<Secret<String>, Report<IntegrationError>> {
        match &self.merchant_id {
            Some(merchant_id) => Ok(merchant_id.clone()),
            None => {
                let currency = currency.ok_or(IntegrationError::MissingRequiredField {
                    field_name: "currency",
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "Worldpay Vantiv merchant ID is configured per currency".to_string(),
                        ),
                        ..Default::default()
                    },
                })?;
                self.merchant_id_by_currency
                    .as_ref()
                    .and_then(|merchant_ids| merchant_ids.get(&currency))
                    .cloned()
                    .ok_or_else(|| {
                        IntegrationError::CurrencyNotSupported {
                            message: currency.to_string(),
                            connector: "Worldpayvantiv",
                            context: IntegrationErrorContext {
                                suggested_action: Some(format!(
                                    "Set a Worldpay Vantiv merchant_id_by_currency.{currency} in the connector configuration."
                                )),
                                ..Default::default()
                            },
                        }
                        .into()
                    })
            }
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename = "cnpOnlineRequest", rename_all = "camelCase")]
pub struct CnpOnlineRequest<T: PaymentMethodDataTypes> {
    #[serde(rename = "@version")]
    pub version: String,
    #[serde(rename = "@xmlns")]
    pub xmlns: String,
    #[serde(rename = "@merchantId")]
    pub merchant_id: Secret<String>,
    pub authentication: Authentication,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorization: Option<Authorization<T>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sale: Option<Sale<T>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capture: Option<CaptureRequest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_reversal: Option<AuthReversal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub void: Option<VoidRequest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credit: Option<RefundRequest>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Authentication {
    pub user: Secret<String>,
    pub password: Secret<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Authorization<T: PaymentMethodDataTypes> {
    #[serde(rename = "@id")]
    pub id: String,
    #[serde(rename = "@reportGroup")]
    pub report_group: String,
    #[serde(rename = "@customerId", skip_serializing_if = "Option::is_none")]
    pub customer_id: Option<Secret<String>>,
    // For incremental auth, use cnp_txn_id referencing the original auth.
    // Vantiv requires this to come BEFORE order_id in XML serialization order.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cnp_txn_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order_id: Option<String>,
    pub amount: MinorUnit,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order_source: Option<OrderSource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bill_to_address: Option<BillToAddress>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ship_to_address: Option<ShipToAddress>,
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub payment_info: Option<PaymentInfo<T>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enhanced_data: Option<EnhancedData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub processing_instructions: Option<ProcessingInstructions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cardholder_authentication: Option<CardholderAuthentication>,
    /// Indicates the type of authorization: "Estimated" or "Incremental".
    /// Required for incremental auth flow.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_indicator: Option<VantivAuthIndicator>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq)]
pub enum VantivAuthIndicator {
    Estimated,
    Incremental,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sale<T: PaymentMethodDataTypes> {
    #[serde(rename = "@id")]
    pub id: String,
    #[serde(rename = "@reportGroup")]
    pub report_group: String,
    #[serde(rename = "@customerId", skip_serializing_if = "Option::is_none")]
    pub customer_id: Option<Secret<String>>,
    pub order_id: String,
    pub amount: MinorUnit,
    pub order_source: OrderSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bill_to_address: Option<BillToAddress>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ship_to_address: Option<ShipToAddress>,
    #[serde(flatten)]
    pub payment_info: PaymentInfo<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enhanced_data: Option<EnhancedData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub processing_instructions: Option<ProcessingInstructions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cardholder_authentication: Option<CardholderAuthentication>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureRequest {
    #[serde(rename = "@id")]
    pub id: String,
    #[serde(rename = "@reportGroup")]
    pub report_group: String,
    pub cnp_txn_id: String,
    pub amount: MinorUnit,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enhanced_data: Option<EnhancedData>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthReversal {
    #[serde(rename = "@id")]
    pub id: String,
    #[serde(rename = "@reportGroup")]
    pub report_group: String,
    pub cnp_txn_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<MinorUnit>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoidRequest {
    #[serde(rename = "@id")]
    pub id: String,
    #[serde(rename = "@reportGroup")]
    pub report_group: String,
    pub cnp_txn_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefundRequest {
    #[serde(rename = "@id")]
    pub id: String,
    #[serde(rename = "@reportGroup")]
    pub report_group: String,
    #[serde(rename = "@customerId", skip_serializing_if = "Option::is_none")]
    pub customer_id: Option<String>,
    pub cnp_txn_id: String,
    pub amount: MinorUnit,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum PaymentInfo<T: PaymentMethodDataTypes> {
    Card(CardData<T>),
    Token(TokenData),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardData<T: PaymentMethodDataTypes> {
    pub card: WorldpayvantivCardData<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub processing_type: Option<VantivProcessingType>,
    #[serde(
        rename = "originalNetworkTransactionId",
        skip_serializing_if = "Option::is_none"
    )]
    pub network_transaction_id: Option<Secret<String>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenData {
    pub token: TokenizationData,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_network_transaction_id: Option<Secret<String>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldpayvantivCardData<T: PaymentMethodDataTypes> {
    #[serde(rename = "type")]
    pub card_type: WorldpayvativCardType,
    pub number: RawCardNumber<T>,
    pub exp_date: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_validation_num: Option<Secret<String>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenizationData {
    pub cnp_token: Secret<String>,
    pub exp_date: Secret<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum VantivProcessingType {
    InitialCOF,
    MerchantInitiatedCOF,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub enum WorldpayvativCardType {
    #[serde(rename = "VI")]
    Visa,
    #[serde(rename = "MC")]
    MasterCard,
    #[serde(rename = "AX")]
    AmericanExpress,
    #[serde(rename = "DI")]
    Discover,
    #[serde(rename = "DC")]
    DinersClub,
    #[serde(rename = "JC")]
    JCB,
    #[serde(rename = "UP")]
    UnionPay,
}

impl TryFrom<common_enums::CardNetwork> for WorldpayvativCardType {
    type Error = Report<IntegrationError>;
    fn try_from(card_network: common_enums::CardNetwork) -> Result<Self, Self::Error> {
        match card_network {
            common_enums::CardNetwork::Visa => Ok(Self::Visa),
            common_enums::CardNetwork::Mastercard => Ok(Self::MasterCard),
            common_enums::CardNetwork::AmericanExpress => Ok(Self::AmericanExpress),
            common_enums::CardNetwork::Discover => Ok(Self::Discover),
            common_enums::CardNetwork::DinersClub => Ok(Self::DinersClub),
            common_enums::CardNetwork::JCB => Ok(Self::JCB),
            common_enums::CardNetwork::UnionPay => Ok(Self::UnionPay),
            _ => Err(IntegrationError::NotSupported {
                message: "Card network".to_string(),
                connector: "worldpayvantiv",
                context: IntegrationErrorContext {
                    additional_context: Some("Supported card networks are Visa, Mastercard, American Express, Discover, Diners Club, JCB, and UnionPay.".to_owned()),
                    ..Default::default()
                },
            }
            .into()),
        }
    }
}

impl TryFrom<&domain_types::utils::CardIssuer> for WorldpayvativCardType {
    type Error = Report<IntegrationError>;
    fn try_from(card_issuer: &domain_types::utils::CardIssuer) -> Result<Self, Self::Error> {
        match card_issuer {
            domain_types::utils::CardIssuer::Visa => Ok(Self::Visa),
            domain_types::utils::CardIssuer::Master => Ok(Self::MasterCard),
            domain_types::utils::CardIssuer::AmericanExpress => Ok(Self::AmericanExpress),
            domain_types::utils::CardIssuer::Discover => Ok(Self::Discover),
            domain_types::utils::CardIssuer::DinersClub => Ok(Self::DinersClub),
            domain_types::utils::CardIssuer::JCB => Ok(Self::JCB),
            _ => Err(IntegrationError::NotSupported {
                message: "Card network".to_string(),
                connector: "worldpayvantiv",
                context: IntegrationErrorContext {
                    additional_context: Some("Supported card issuers for automatic detection are Visa, Mastercard, American Express, Discover, Diners Club, and JCB.".to_owned()),
                    ..Default::default()
                },
            }
            .into()),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum OrderSource {
    #[serde(rename = "ecommerce")]
    Ecommerce,
    #[serde(rename = "installment")]
    Installment,
    #[serde(rename = "mailorder")]
    MailOrder,
    #[serde(rename = "recurring")]
    Recurring,
    #[serde(rename = "telephone")]
    Telephone,
    #[serde(rename = "applepay")]
    ApplePay,
    #[serde(rename = "androidpay")]
    AndroidPay,
}

impl<T: PaymentMethodDataTypes> From<PaymentMethodData<T>> for OrderSource {
    fn from(payment_method_data: PaymentMethodData<T>) -> Self {
        match payment_method_data {
            PaymentMethodData::Wallet(WalletData::ApplePay(_)) => Self::ApplePay,
            PaymentMethodData::Wallet(WalletData::GooglePay(_)) => Self::AndroidPay,
            _ => Self::Ecommerce,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BillToAddress {
    pub name: Option<Secret<String>>,
    #[serde(rename = "companyName")]
    pub company: Option<String>,
    pub address_line1: Option<Secret<String>>,
    pub address_line2: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address_line3: Option<Secret<String>>,
    pub city: Option<Secret<String>>,
    pub state: Option<Secret<String>>,
    pub zip: Option<Secret<String>>,
    pub country: Option<CountryAlpha2>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<common_utils::pii::Email>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone: Option<Secret<String>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShipToAddress {
    pub name: Option<Secret<String>>,
    #[serde(rename = "companyName")]
    pub company: Option<String>,
    pub address_line1: Option<Secret<String>>,
    pub address_line2: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address_line3: Option<Secret<String>>,
    pub city: Option<Secret<String>>,
    pub state: Option<Secret<String>>,
    pub zip: Option<Secret<String>>,
    pub country: Option<CountryAlpha2>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<common_utils::pii::Email>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone: Option<Secret<String>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnhancedData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer_reference: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sales_tax: Option<MinorUnit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tax_exempt: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discount_amount: Option<MinorUnit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shipping_amount: Option<MinorUnit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duty_amount: Option<MinorUnit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_item_data: Option<Vec<LineItemData>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LineItemData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item_sequence_number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item_description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quantity: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit_of_measure: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit_cost: Option<MinorUnit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item_total: Option<MinorUnit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item_discount_amount: Option<MinorUnit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commodity_code: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessingInstructions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bypass_velocity_check: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardholderAuthentication {
    pub authentication_value: Secret<String>,
}

// Response structures
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename = "cnpOnlineResponse", rename_all = "camelCase")]
pub struct CnpOnlineResponse {
    #[serde(rename = "@version")]
    pub version: String,
    #[serde(rename = "@response")]
    pub response_code: String,
    #[serde(rename = "@message")]
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorization_response: Option<PaymentResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sale_response: Option<PaymentResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capture_response: Option<CaptureResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_reversal_response: Option<AuthReversalResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub void_response: Option<VoidResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credit_response: Option<CreditResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PaymentResponse {
    #[serde(rename = "@id")]
    pub id: String,
    #[serde(rename = "@reportGroup")]
    pub report_group: String,
    #[serde(rename = "@customerId", skip_serializing_if = "Option::is_none")]
    pub customer_id: Option<Secret<String>>,
    pub cnp_txn_id: String,
    pub order_id: String,
    pub response: WorldpayvantivResponseCode,
    pub message: String,
    pub response_time: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_code: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fraud_result: Option<FraudResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_response: Option<TokenResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_transaction_id: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approved_amount: Option<MinorUnit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enhanced_auth_response: Option<EnhancedAuthResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_suffix: Option<String>,
}

impl PaymentResponse {
    fn mandate_reference(&self) -> Result<Option<Box<MandateReference>>, serde_json::Error> {
        let Some(token) = self.token_response.as_ref() else {
            return Ok(None);
        };
        let mandate_metadata = self
            .network_transaction_id
            .as_ref()
            .map(|ntid| {
                serde_json::to_value(WorldpayvantivMandateMetadata {
                    network_transaction_id: Some(ntid.clone()),
                })
                .map(Secret::new)
            })
            .transpose()?;

        Ok(Some(Box::new(MandateReference {
            connector_mandate_id: Some(token.cnp_token.peek().clone()),
            payment_method_id: None,
            connector_mandate_request_reference_id: None,
            mandate_metadata,
        })))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CaptureResponse {
    #[serde(rename = "@id")]
    pub id: String,
    #[serde(rename = "@reportGroup")]
    pub report_group: String,
    pub cnp_txn_id: String,
    pub response: WorldpayvantivResponseCode,
    pub message: String,
    pub response_time: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AuthReversalResponse {
    #[serde(rename = "@id")]
    pub id: String,
    #[serde(rename = "@reportGroup")]
    pub report_group: String,
    pub cnp_txn_id: String,
    pub response: WorldpayvantivResponseCode,
    pub message: String,
    pub response_time: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VoidResponse {
    #[serde(rename = "@id")]
    pub id: String,
    #[serde(rename = "@reportGroup")]
    pub report_group: String,
    pub cnp_txn_id: String,
    pub response: WorldpayvantivResponseCode,
    pub message: String,
    pub response_time: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CreditResponse {
    #[serde(rename = "@id")]
    pub id: String,
    #[serde(rename = "@reportGroup")]
    pub report_group: String,
    pub cnp_txn_id: String,
    pub response: WorldpayvantivResponseCode,
    pub message: String,
    pub response_time: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FraudResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avs_result: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_validation_result: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authentication_result: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub advanced_a_v_s_result: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub advanced_fraud_results: Option<AdvancedFraudResults>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AdvancedFraudResults {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_review_status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TokenResponse {
    pub cnp_token: Secret<String>,
    pub token_response_code: String,
    pub token_message: String,
    #[serde(rename = "type")]
    pub token_type: String,
    pub bin: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EnhancedAuthResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub funding_source: Option<FundingSource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub virtual_account_number: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_response: Option<NetworkResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NetworkResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    #[serde(
        rename = "networkField",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub network_field: Vec<NetworkField>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NetworkField {
    #[serde(rename = "@fieldNumber")]
    pub field_number: String,
    #[serde(rename = "@fieldName", skip_serializing_if = "Option::is_none")]
    pub field_name: Option<String>,
    #[serde(rename = "fieldValue", skip_serializing_if = "Option::is_none")]
    pub field_value: Option<String>,
    #[serde(
        rename = "networkSubField",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub network_sub_field: Vec<NetworkSubField>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NetworkSubField {
    #[serde(rename = "@fieldNumber")]
    pub field_number: String,
    #[serde(rename = "fieldValue", skip_serializing_if = "Option::is_none")]
    pub field_value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FundingSource {
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub funding_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub available_balance: Option<String>,
}

#[derive(Debug, strum::Display, Serialize, Deserialize, PartialEq, Clone, Copy)]
pub enum WorldpayvantivResponseCode {
    #[serde(rename = "001")]
    #[strum(serialize = "001")]
    TransactionReceived,
    #[serde(rename = "000")]
    #[strum(serialize = "000")]
    Approved,
    #[serde(rename = "010")]
    #[strum(serialize = "010")]
    PartiallyApproved,
    #[serde(rename = "011")]
    #[strum(serialize = "011")]
    OfflineApproval,
    #[serde(rename = "013")]
    #[strum(serialize = "013")]
    OfflineApprovalUnableToGoOnline,
    #[serde(rename = "015")]
    #[strum(serialize = "015")]
    PendingShopperCheckoutCompletion,
    #[serde(rename = "016")]
    #[strum(serialize = "016")]
    ShopperCheckoutExpired,
    #[serde(rename = "100")]
    #[strum(serialize = "100")]
    ProcessingNetworkUnavailable,
    #[serde(rename = "101")]
    #[strum(serialize = "101")]
    IssuerUnavailable,
    #[serde(rename = "102")]
    #[strum(serialize = "102")]
    ReSubmitTransaction,
    #[serde(rename = "103")]
    #[strum(serialize = "103")]
    MerchantNotConfiguredForProcessingAtThisSite,
    #[serde(rename = "108")]
    #[strum(serialize = "108")]
    TryAgainLater,
    #[serde(rename = "110")]
    #[strum(serialize = "110")]
    InsufficientFunds,
    #[serde(rename = "111")]
    #[strum(serialize = "111")]
    AuthorizationAmountHasAlreadyBeenDepleted,
    #[serde(rename = "112")]
    #[strum(serialize = "112")]
    InsufficientFundsRetryAfter1Hour,
    #[serde(rename = "113")]
    #[strum(serialize = "113")]
    InsufficientFundsRetryAfter24Hour,
    #[serde(rename = "114")]
    #[strum(serialize = "114")]
    InsufficientFundsRetryAfter2Days,
    #[serde(rename = "115")]
    #[strum(serialize = "115")]
    InsufficientFundsRetryAfter4Days,
    #[serde(rename = "116")]
    #[strum(serialize = "116")]
    InsufficientFundsRetryAfter6Days,
    #[serde(rename = "117")]
    #[strum(serialize = "117")]
    InsufficientFundsRetryAfter8Days,
    #[serde(rename = "118")]
    #[strum(serialize = "118")]
    InsufficientFundsRetryAfter10Days,
    #[serde(rename = "120")]
    #[strum(serialize = "120")]
    CallIssuer,
    #[serde(rename = "121")]
    #[strum(serialize = "121")]
    CallAmex,
    #[serde(rename = "122")]
    #[strum(serialize = "122")]
    CallDinersClub,
    #[serde(rename = "123")]
    #[strum(serialize = "123")]
    CallDiscover,
    #[serde(rename = "124")]
    #[strum(serialize = "124")]
    CallJbs,
    #[serde(rename = "125")]
    #[strum(serialize = "125")]
    CallVisaMastercard,
    #[serde(rename = "126")]
    #[strum(serialize = "126")]
    CallIssuerUpdateCardholderData,
    #[serde(rename = "127")]
    #[strum(serialize = "127")]
    ExceedsApprovalAmountLimit,
    #[serde(rename = "130")]
    #[strum(serialize = "130")]
    CallIndicatedNumber,
    #[serde(rename = "131")]
    #[strum(serialize = "131")]
    UnacceptablePinTransactionDeclinedRetry,
    #[serde(rename = "132")]
    #[strum(serialize = "132")]
    PinNotChanged,
    #[serde(rename = "137")]
    #[strum(serialize = "137")]
    ConsumerMultiUseVirtualCardNumberSoftDecline,
    #[serde(rename = "138")]
    #[strum(serialize = "138")]
    ConsumerNonReloadablePrepaidCardSoftDecline,
    #[serde(rename = "139")]
    #[strum(serialize = "139")]
    ConsumerSingleUseVirtualCardNumberSoftDecline,
    #[serde(rename = "140")]
    #[strum(serialize = "140")]
    UpdateCardholderData,
    #[serde(rename = "141")]
    #[strum(serialize = "141")]
    ConsumerNonReloadablePrepaidCardApproved,
    #[serde(rename = "142")]
    #[strum(serialize = "142")]
    ConsumerSingleUseVirtualCardNumberApproved,
    #[serde(rename = "143")]
    #[strum(serialize = "143")]
    MerchantDoesntQualifyForProductCode,
    #[serde(rename = "145")]
    #[strum(serialize = "145")]
    Lifecycle,
    #[serde(rename = "146")]
    #[strum(serialize = "146")]
    Policy,
    #[serde(rename = "147")]
    #[strum(serialize = "147")]
    FraudSecurity,
    #[serde(rename = "148")]
    #[strum(serialize = "148")]
    InvalidOrExpiredCardContactCardholderToUpdate,
    #[serde(rename = "149")]
    #[strum(serialize = "149")]
    InvalidTransactionOrCardRestrictionVerifyInformationAndResubmit,
    #[serde(rename = "154")]
    #[strum(serialize = "154")]
    AtLeastOneOfOrigIdOrOrigCnpTxnIdIsRequired,
    #[serde(rename = "155")]
    #[strum(serialize = "155")]
    OrigCnpTxnIdIsRequiredWhenShowStatusOnlyIsUsed,
    #[serde(rename = "156")]
    #[strum(serialize = "156")]
    IncrementalAuthNotSupported,
    #[serde(rename = "157")]
    #[strum(serialize = "157")]
    SetAuthIndicatorToIncremental,
    #[serde(rename = "158")]
    #[strum(serialize = "158")]
    IncrementalValueForAuthIndicatorNotAllowedInThisAuthStructure,
    #[serde(rename = "159")]
    #[strum(serialize = "159")]
    CannotRequestAnIncrementalAuthIfOriginalAuthNotSetToEstimated,
    #[serde(rename = "161")]
    #[strum(serialize = "161")]
    TransactionMustReferenceTheEstimatedAuth,
    #[serde(rename = "162")]
    #[strum(serialize = "162")]
    IncrementedAuthExceedsMaxTransactionAmount,
    #[serde(rename = "170")]
    #[strum(serialize = "170")]
    SubmittedMccNotAllowed,
    #[serde(rename = "192")]
    #[strum(serialize = "192")]
    MerchantNotCertifiedEnabledForIias,
    #[serde(rename = "206")]
    #[strum(serialize = "206")]
    IssuerGeneratedError,
    #[serde(rename = "207")]
    #[strum(serialize = "207")]
    PickupCardOtherThanLostStolen,
    #[serde(rename = "209")]
    #[strum(serialize = "209")]
    InvalidAmountHardDecline,
    #[serde(rename = "211")]
    #[strum(serialize = "211")]
    ReversalUnsuccessful,
    #[serde(rename = "212")]
    #[strum(serialize = "212")]
    MissingData,
    #[serde(rename = "213")]
    #[strum(serialize = "213")]
    PickupCardLostCard,
    #[serde(rename = "214")]
    #[strum(serialize = "214")]
    PickupCardStolenCard,
    #[serde(rename = "215")]
    #[strum(serialize = "215")]
    RestrictedCard,
    #[serde(rename = "216")]
    #[strum(serialize = "216")]
    InvalidDeactivate,
    #[serde(rename = "217")]
    #[strum(serialize = "217")]
    CardAlreadyActive,
    #[serde(rename = "218")]
    #[strum(serialize = "218")]
    CardNotActive,
    #[serde(rename = "219")]
    #[strum(serialize = "219")]
    CardAlreadyDeactivate,
    #[serde(rename = "221")]
    #[strum(serialize = "221")]
    OverMaxBalance,
    #[serde(rename = "222")]
    #[strum(serialize = "222")]
    InvalidActivate,
    #[serde(rename = "223")]
    #[strum(serialize = "223")]
    NoTransactionFoundForReversal,
    #[serde(rename = "226")]
    #[strum(serialize = "226")]
    IncorrectCvv,
    #[serde(rename = "229")]
    #[strum(serialize = "229")]
    IllegalTransaction,
    #[serde(rename = "251")]
    #[strum(serialize = "251")]
    DuplicateTransaction,
    #[serde(rename = "252")]
    #[strum(serialize = "252")]
    SystemError,
    #[serde(rename = "253")]
    #[strum(serialize = "253")]
    DeconvertedBin,
    #[serde(rename = "254")]
    #[strum(serialize = "254")]
    MerchantDepleted,
    #[serde(rename = "255")]
    #[strum(serialize = "255")]
    GiftCardEscheated,
    #[serde(rename = "256")]
    #[strum(serialize = "256")]
    InvalidReversalTypeForCreditCardTransaction,
    #[serde(rename = "257")]
    #[strum(serialize = "257")]
    SystemErrorMessageFormatError,
    #[serde(rename = "258")]
    #[strum(serialize = "258")]
    SystemErrorCannotProcess,
    #[serde(rename = "271")]
    #[strum(serialize = "271")]
    RefundRejectedDueToPendingDepositStatus,
    #[serde(rename = "272")]
    #[strum(serialize = "272")]
    RefundRejectedDueToDeclinedDepositStatus,
    #[serde(rename = "273")]
    #[strum(serialize = "273")]
    RefundRejectedByTheProcessingNetwork,
    #[serde(rename = "284")]
    #[strum(serialize = "284")]
    CaptureCreditAndAuthReversalTagsCannotBeUsedForGiftCardTransactions,
    #[serde(rename = "301")]
    #[strum(serialize = "301")]
    InvalidAccountNumber,
    #[serde(rename = "302")]
    #[strum(serialize = "302")]
    AccountNumberDoesNotMatchPaymentType,
    #[serde(rename = "303")]
    #[strum(serialize = "303")]
    PickUpCard,
    #[serde(rename = "304")]
    #[strum(serialize = "304")]
    LostStolenCard,
    #[serde(rename = "305")]
    #[strum(serialize = "305")]
    ExpiredCard,
    #[serde(rename = "306")]
    #[strum(serialize = "306")]
    AuthorizationHasExpiredNoNeedToReverse,
    #[serde(rename = "307")]
    #[strum(serialize = "307")]
    RestrictedCardSoftDecline,
    #[serde(rename = "308")]
    #[strum(serialize = "308")]
    RestrictedCardChargeback,
    #[serde(rename = "309")]
    #[strum(serialize = "309")]
    RestrictedCardPrepaidCardFilteringService,
    #[serde(rename = "310")]
    #[strum(serialize = "310")]
    InvalidTrackData,
    #[serde(rename = "311")]
    #[strum(serialize = "311")]
    DepositIsAlreadyReferencedByAChargeback,
    #[serde(rename = "312")]
    #[strum(serialize = "312")]
    RestrictedCardInternationalCardFilteringService,
    #[serde(rename = "313")]
    #[strum(serialize = "313")]
    InternationalFilteringForIssuingCardCountry,
    #[serde(rename = "315")]
    #[strum(serialize = "315")]
    RestrictedCardAuthFraudVelocityFilteringService,
    #[serde(rename = "316")]
    #[strum(serialize = "316")]
    AutomaticRefundAlreadyIssued,
    #[serde(rename = "317")]
    #[strum(serialize = "317")]
    RestrictedCardCardUnderSanction,
    #[serde(rename = "318")]
    #[strum(serialize = "318")]
    RestrictedCardAuthFraudAdviceFilteringService,
    #[serde(rename = "319")]
    #[strum(serialize = "319")]
    RestrictedCardFraudAvsFilteringService,
    #[serde(rename = "320")]
    #[strum(serialize = "320")]
    InvalidExpirationDate,
    #[serde(rename = "321")]
    #[strum(serialize = "321")]
    InvalidMerchant,
    #[serde(rename = "322")]
    #[strum(serialize = "322")]
    InvalidTransaction,
    #[serde(rename = "323")]
    #[strum(serialize = "323")]
    NoSuchIssuer,
    #[serde(rename = "324")]
    #[strum(serialize = "324")]
    InvalidPin,
    #[serde(rename = "325")]
    #[strum(serialize = "325")]
    TransactionNotAllowedAtTerminal,
    #[serde(rename = "326")]
    #[strum(serialize = "326")]
    ExceedsNumberOfPinEntries,
    #[serde(rename = "327")]
    #[strum(serialize = "327")]
    CardholderTransactionNotPermitted,
    #[serde(rename = "328")]
    #[strum(serialize = "328")]
    CardholderRequestedThatRecurringOrInstallmentPaymentBeStopped,
    #[serde(rename = "330")]
    #[strum(serialize = "330")]
    InvalidPaymentType,
    #[serde(rename = "331")]
    #[strum(serialize = "331")]
    InvalidPosCapabilityForCardholderAuthorizedTerminalTransaction,
    #[serde(rename = "332")]
    #[strum(serialize = "332")]
    InvalidPosCardholderIdForCardholderAuthorizedTerminalTransaction,
    #[serde(rename = "335")]
    #[strum(serialize = "335")]
    ThisMethodOfPaymentDoesNotSupportAuthorizationReversals,
    #[serde(rename = "336")]
    #[strum(serialize = "336")]
    ReversalAmountDoesNotMatchAuthorizationAmount,
    #[serde(rename = "337")]
    #[strum(serialize = "337")]
    TransactionDidNotConvertToPinless,
    #[serde(rename = "340")]
    #[strum(serialize = "340")]
    InvalidAmountSoftDecline,
    #[serde(rename = "341")]
    #[strum(serialize = "341")]
    InvalidHealthcareAmounts,
    #[serde(rename = "346")]
    #[strum(serialize = "346")]
    InvalidBillingDescriptorPrefix,
    #[serde(rename = "347")]
    #[strum(serialize = "347")]
    InvalidBillingDescriptor,
    #[serde(rename = "348")]
    #[strum(serialize = "348")]
    InvalidReportGroup,
    #[serde(rename = "349")]
    #[strum(serialize = "349")]
    DoNotHonor,
    #[serde(rename = "350")]
    #[strum(serialize = "350")]
    GenericDecline, // Soft or Hard Decline
    #[serde(rename = "351")]
    #[strum(serialize = "351")]
    DeclineRequestPositiveId,
    #[serde(rename = "352")]
    #[strum(serialize = "352")]
    DeclineCvv2CidFail,
    #[serde(rename = "354")]
    #[strum(serialize = "354")]
    ThreeDSecureTransactionNotSupportedByMerchant,
    #[serde(rename = "356")]
    #[strum(serialize = "356")]
    InvalidPurchaseLevelIiiTheTransactionContainedBadOrMissingData,
    #[serde(rename = "357")]
    #[strum(serialize = "357")]
    MissingHealthcareIiasTagForAnFsaTransaction,
    #[serde(rename = "358")]
    #[strum(serialize = "358")]
    RestrictedByVantivDueToSecurityCodeMismatch,
    #[serde(rename = "360")]
    #[strum(serialize = "360")]
    NoTransactionFoundWithSpecifiedTransactionId,
    #[serde(rename = "361")]
    #[strum(serialize = "361")]
    AuthorizationNoLongerAvailable,
    #[serde(rename = "362")]
    #[strum(serialize = "362")]
    TransactionNotVoidedAlreadySettled,
    #[serde(rename = "363")]
    #[strum(serialize = "363")]
    AutoVoidOnRefund,
    #[serde(rename = "364")]
    #[strum(serialize = "364")]
    InvalidAccountNumberOriginalOrNocUpdatedECheckAccountRequired,
    #[serde(rename = "365")]
    #[strum(serialize = "365")]
    TotalCreditAmountExceedsCaptureAmount,
    #[serde(rename = "366")]
    #[strum(serialize = "366")]
    ExceedTheThresholdForSendingRedeposits,
    #[serde(rename = "367")]
    #[strum(serialize = "367")]
    DepositHasNotBeenReturnedForInsufficientNonSufficientFunds,
    #[serde(rename = "368")]
    #[strum(serialize = "368")]
    InvalidCheckNumber,
    #[serde(rename = "369")]
    #[strum(serialize = "369")]
    RedepositAgainstInvalidTransactionType,
    #[serde(rename = "370")]
    #[strum(serialize = "370")]
    InternalSystemErrorCallVantiv,
    #[serde(rename = "371")]
    #[strum(serialize = "371")]
    OriginalTransactionHasBeenProcessedFutureRedepositsCanceled,
    #[serde(rename = "372")]
    #[strum(serialize = "372")]
    SoftDeclineAutoRecyclingInProgress,
    #[serde(rename = "373")]
    #[strum(serialize = "373")]
    HardDeclineAutoRecyclingComplete,
    #[serde(rename = "375")]
    #[strum(serialize = "375")]
    MerchantIsNotEnabledForSurcharging,
    #[serde(rename = "376")]
    #[strum(serialize = "376")]
    ThisMethodOfPaymentDoesNotSupportSurcharging,
    #[serde(rename = "377")]
    #[strum(serialize = "377")]
    SurchargeIsNotValidForDebitOrPrepaidCards,
    #[serde(rename = "378")]
    #[strum(serialize = "378")]
    SurchargeCannotExceedsTheMaximumAllowedLimit,
    #[serde(rename = "379")]
    #[strum(serialize = "379")]
    TransactionDeclinedByTheProcessingNetwork,
    #[serde(rename = "380")]
    #[strum(serialize = "380")]
    SecondaryAmountCannotExceedTheSaleAmount,
    #[serde(rename = "381")]
    #[strum(serialize = "381")]
    ThisMethodOfPaymentDoesNotSupportSecondaryAmount,
    #[serde(rename = "382")]
    #[strum(serialize = "382")]
    SecondaryAmountCannotBeLessThanZero,
    #[serde(rename = "383")]
    #[strum(serialize = "383")]
    PartialTransactionIsNotSupportedWhenIncludingASecondaryAmount,
    #[serde(rename = "384")]
    #[strum(serialize = "384")]
    SecondaryAmountRequiredOnPartialRefundWhenUsedOnDeposit,
    #[serde(rename = "385")]
    #[strum(serialize = "385")]
    SecondaryAmountNotAllowedOnRefundIfNotIncludedOnDeposit,
    #[serde(rename = "386")]
    #[strum(serialize = "386")]
    ProcessingNetworkError,
    #[serde(rename = "401")]
    #[strum(serialize = "401")]
    InvalidEMail,
    #[serde(rename = "466")]
    #[strum(serialize = "466")]
    InvalidCombinationOfAccountFundingTransactionTypeAndMcc,
    #[serde(rename = "467")]
    #[strum(serialize = "467")]
    InvalidAccountFundingTransactionTypeForThisMethodOfPayment,
    #[serde(rename = "468")]
    #[strum(serialize = "468")]
    MissingOneOrMoreReceiverFieldsForAccountFundingTransaction,
    #[serde(rename = "469")]
    #[strum(serialize = "469")]
    InvalidRecurringRequestSeeRecurringResponseForDetails,
    #[serde(rename = "470")]
    #[strum(serialize = "470")]
    ApprovedRecurringSubscriptionCreated,
    #[serde(rename = "471")]
    #[strum(serialize = "471")]
    ParentTransactionDeclinedRecurringSubscriptionNotCreated,
    #[serde(rename = "472")]
    #[strum(serialize = "472")]
    InvalidPlanCode,
    #[serde(rename = "473")]
    #[strum(serialize = "473")]
    ScheduledRecurringPaymentProcessed,
    #[serde(rename = "475")]
    #[strum(serialize = "475")]
    InvalidSubscriptionId,
    #[serde(rename = "476")]
    #[strum(serialize = "476")]
    AddOnCodeAlreadyExists,
    #[serde(rename = "477")]
    #[strum(serialize = "477")]
    DuplicateAddOnCodesInRequests,
    #[serde(rename = "478")]
    #[strum(serialize = "478")]
    NoMatchingAddOnCodeForTheSubscription,
    #[serde(rename = "480")]
    #[strum(serialize = "480")]
    NoMatchingDiscountCodeForTheSubscription,
    #[serde(rename = "481")]
    #[strum(serialize = "481")]
    DuplicateDiscountCodesInRequest,
    #[serde(rename = "482")]
    #[strum(serialize = "482")]
    InvalidStartDate,
    #[serde(rename = "483")]
    #[strum(serialize = "483")]
    MerchantNotRegisteredForRecurringEngine,
    #[serde(rename = "484")]
    #[strum(serialize = "484")]
    InsufficientDataToUpdateSubscription,
    #[serde(rename = "485")]
    #[strum(serialize = "485")]
    InvalidBillingDate,
    #[serde(rename = "486")]
    #[strum(serialize = "486")]
    DiscountCodeAlreadyExists,
    #[serde(rename = "487")]
    #[strum(serialize = "487")]
    PlanCodeAlreadyExists,
    #[serde(rename = "500")]
    #[strum(serialize = "500")]
    TheAccountNumberWasChanged,
    #[serde(rename = "501")]
    #[strum(serialize = "501")]
    TheAccountWasClosed,
    #[serde(rename = "502")]
    #[strum(serialize = "502")]
    TheExpirationDateWasChanged,
    #[serde(rename = "503")]
    #[strum(serialize = "503")]
    TheIssuingBankDoesNotParticipateInTheUpdateProgram,
    #[serde(rename = "504")]
    #[strum(serialize = "504")]
    ContactTheCardholderForUpdatedInformation,
    #[serde(rename = "507")]
    #[strum(serialize = "507")]
    TheCardholderHasOptedOutOfTheUpdateProgram,
    #[serde(rename = "521")]
    #[strum(serialize = "521")]
    SoftDeclineCardReaderDecryptionServiceIsNotAvailable,
    #[serde(rename = "523")]
    #[strum(serialize = "523")]
    SoftDeclineDecryptionFailed,
    #[serde(rename = "524")]
    #[strum(serialize = "524")]
    HardDeclineInputDataIsInvalid,
    #[serde(rename = "530")]
    #[strum(serialize = "530")]
    ApplePayKeyMismatch,
    #[serde(rename = "531")]
    #[strum(serialize = "531")]
    ApplePayDecryptionFailed,
    #[serde(rename = "540")]
    #[strum(serialize = "540")]
    HardDeclineDecryptionFailed,
    #[serde(rename = "550")]
    #[strum(serialize = "550")]
    AdvancedFraudFilterScoreBelowThreshold,
    #[serde(rename = "555")]
    #[strum(serialize = "555")]
    SuspectedFraud,
    #[serde(rename = "560")]
    #[strum(serialize = "560")]
    SystemErrorContactWorldpayRepresentative,
    #[serde(rename = "561")]
    #[strum(serialize = "561")]
    AmazonPayAmazonUnavailable,
    #[serde(rename = "562")]
    #[strum(serialize = "562")]
    AmazonPayAmazonDeclined,
    #[serde(rename = "563")]
    #[strum(serialize = "563")]
    AmazonPayInvalidToken,
    #[serde(rename = "564")]
    #[strum(serialize = "564")]
    MerchantNotEnabledForAmazonPay,
    #[serde(rename = "565")]
    #[strum(serialize = "565")]
    TransactionNotSupportedBlockedByIssuer,
    #[serde(rename = "566")]
    #[strum(serialize = "566")]
    BlockedByCardholderContactCardholder,
    #[serde(rename = "601")]
    #[strum(serialize = "601")]
    SoftDeclinePrimaryFundingSourceFailed,
    #[serde(rename = "602")]
    #[strum(serialize = "602")]
    SoftDeclineBuyerHasAlternateFundingSource,
    #[serde(rename = "610")]
    #[strum(serialize = "610")]
    HardDeclineInvalidBillingAgreementId,
    #[serde(rename = "611")]
    #[strum(serialize = "611")]
    HardDeclinePrimaryFundingSourceFailed,
    #[serde(rename = "612")]
    #[strum(serialize = "612")]
    HardDeclineIssueWithPaypalAccount,
    #[serde(rename = "613")]
    #[strum(serialize = "613")]
    HardDeclinePayPalAuthorizationIdMissing,
    #[serde(rename = "614")]
    #[strum(serialize = "614")]
    HardDeclineConfirmedEmailAddressIsNotAvailable,
    #[serde(rename = "615")]
    #[strum(serialize = "615")]
    HardDeclinePayPalBuyerAccountDenied,
    #[serde(rename = "616")]
    #[strum(serialize = "616")]
    HardDeclinePayPalBuyerAccountRestricted,
    #[serde(rename = "617")]
    #[strum(serialize = "617")]
    HardDeclinePayPalOrderHasBeenVoidedExpiredOrCompleted,
    #[serde(rename = "618")]
    #[strum(serialize = "618")]
    HardDeclineIssueWithPayPalRefund,
    #[serde(rename = "619")]
    #[strum(serialize = "619")]
    HardDeclinePayPalCredentialsIssue,
    #[serde(rename = "620")]
    #[strum(serialize = "620")]
    HardDeclinePayPalAuthorizationVoidedOrExpired,
    #[serde(rename = "621")]
    #[strum(serialize = "621")]
    HardDeclineRequiredPayPalParameterMissing,
    #[serde(rename = "622")]
    #[strum(serialize = "622")]
    HardDeclinePayPalTransactionIdOrAuthIdIsInvalid,
    #[serde(rename = "623")]
    #[strum(serialize = "623")]
    HardDeclineExceededMaximumNumberOfPayPalAuthorizationAttempts,
    #[serde(rename = "624")]
    #[strum(serialize = "624")]
    HardDeclineTransactionAmountExceedsMerchantsPayPalAccountLimit,
    #[serde(rename = "625")]
    #[strum(serialize = "625")]
    HardDeclinePayPalFundingSourcesUnavailable,
    #[serde(rename = "626")]
    #[strum(serialize = "626")]
    HardDeclineIssueWithPayPalPrimaryFundingSource,
    #[serde(rename = "627")]
    #[strum(serialize = "627")]
    HardDeclinePayPalProfileDoesNotAllowThisTransactionType,
    #[serde(rename = "628")]
    #[strum(serialize = "628")]
    InternalSystemErrorWithPayPalContactVantiv,
    #[serde(rename = "629")]
    #[strum(serialize = "629")]
    HardDeclineContactPayPalConsumerForAnotherPaymentMethod,
    #[serde(rename = "637")]
    #[strum(serialize = "637")]
    InvalidTerminalId,
    #[serde(rename = "640")]
    #[strum(serialize = "640")]
    PinlessDebitProcessingNotSupportedForNonRecurringTransactions,
    #[serde(rename = "641")]
    #[strum(serialize = "641")]
    PinlessDebitProcessingNotSupportedForPartialAuths,
    #[serde(rename = "642")]
    #[strum(serialize = "642")]
    MerchantNotConfiguredForPinlessDebitProcessing,
    #[serde(rename = "651")]
    #[strum(serialize = "651")]
    DeclineCustomerCancellation,
    #[serde(rename = "652")]
    #[strum(serialize = "652")]
    DeclineReTryTransaction,
    #[serde(rename = "653")]
    #[strum(serialize = "653")]
    DeclineUnableToLocateRecordOnFile,
    #[serde(rename = "654")]
    #[strum(serialize = "654")]
    DeclineFileUpdateFieldEditError,
    #[serde(rename = "655")]
    #[strum(serialize = "655")]
    RemoteFunctionUnknown,
    #[serde(rename = "656")]
    #[strum(serialize = "656")]
    DeclinedExceedsWithdrawalFrequencyLimit,
    #[serde(rename = "657")]
    #[strum(serialize = "657")]
    DeclineCardRecordNotAvailable,
    #[serde(rename = "658")]
    #[strum(serialize = "658")]
    InvalidAuthorizationCode,
    #[serde(rename = "659")]
    #[strum(serialize = "659")]
    ReconciliationError,
    #[serde(rename = "660")]
    #[strum(serialize = "660")]
    PreferredDebitRoutingDenialCreditTransactionCanBeDebit,
    #[serde(rename = "661")]
    #[strum(serialize = "661")]
    DeclinedCurrencyConversionCompleteNoAuthPerformed,
    #[serde(rename = "662")]
    #[strum(serialize = "662")]
    DeclinedMultiCurrencyDccFail,
    #[serde(rename = "663")]
    #[strum(serialize = "663")]
    DeclinedMultiCurrencyInvertFail,
    #[serde(rename = "664")]
    #[strum(serialize = "664")]
    Invalid3DSecurePassword,
    #[serde(rename = "665")]
    #[strum(serialize = "665")]
    InvalidSocialSecurityNumber,
    #[serde(rename = "666")]
    #[strum(serialize = "666")]
    InvalidMothersMaidenName,
    #[serde(rename = "667")]
    #[strum(serialize = "667")]
    EnrollmentInquiryDeclined,
    #[serde(rename = "668")]
    #[strum(serialize = "668")]
    SocialSecurityNumberNotAvailable,
    #[serde(rename = "669")]
    #[strum(serialize = "669")]
    MothersMaidenNameNotAvailable,
    #[serde(rename = "670")]
    #[strum(serialize = "670")]
    PinAlreadyExistsOnDatabase,
    #[serde(rename = "701")]
    #[strum(serialize = "701")]
    Under18YearsOld,
    #[serde(rename = "702")]
    #[strum(serialize = "702")]
    BillToOutsideUsa,
    #[serde(rename = "703")]
    #[strum(serialize = "703")]
    BillToAddressIsNotEqualToShipToAddress,
    #[serde(rename = "704")]
    #[strum(serialize = "704")]
    DeclinedForeignCurrencyMustBeUsd,
    #[serde(rename = "705")]
    #[strum(serialize = "705")]
    OnNegativeFile,
    #[serde(rename = "706")]
    #[strum(serialize = "706")]
    BlockedAgreement,
    #[serde(rename = "707")]
    #[strum(serialize = "707")]
    InsufficientBuyingPower,
    #[serde(rename = "708")]
    #[strum(serialize = "708")]
    InvalidData,
    #[serde(rename = "709")]
    #[strum(serialize = "709")]
    InvalidDataDataElementsMissing,
    #[serde(rename = "710")]
    #[strum(serialize = "710")]
    InvalidDataDataFormatError,
    #[serde(rename = "711")]
    #[strum(serialize = "711")]
    InvalidDataInvalidTCVersion,
    #[serde(rename = "712")]
    #[strum(serialize = "712")]
    DuplicateTransactionPaypalCredit,
    #[serde(rename = "713")]
    #[strum(serialize = "713")]
    VerifyBillingAddress,
    #[serde(rename = "714")]
    #[strum(serialize = "714")]
    InactiveAccount,
    #[serde(rename = "716")]
    #[strum(serialize = "716")]
    InvalidAuth,
    #[serde(rename = "717")]
    #[strum(serialize = "717")]
    AuthorizationAlreadyExistsForTheOrder,
    #[serde(rename = "730")]
    #[strum(serialize = "730")]
    LodgingTransactionsAreNotAllowedForThisMcc,
    #[serde(rename = "731")]
    #[strum(serialize = "731")]
    DurationCannotBeNegative,
    #[serde(rename = "732")]
    #[strum(serialize = "732")]
    HotelFolioNumberCannotBeBlank,
    #[serde(rename = "733")]
    #[strum(serialize = "733")]
    InvalidCheckInDate,
    #[serde(rename = "734")]
    #[strum(serialize = "734")]
    InvalidCheckOutDate,
    #[serde(rename = "735")]
    #[strum(serialize = "735")]
    InvalidCheckInOrCheckOutDate,
    #[serde(rename = "736")]
    #[strum(serialize = "736")]
    CheckOutDateCannotBeBeforeCheckInDate,
    #[serde(rename = "737")]
    #[strum(serialize = "737")]
    NumberOfAdultsCannotBeNegative,
    #[serde(rename = "738")]
    #[strum(serialize = "738")]
    RoomRateCannotBeNegative,
    #[serde(rename = "739")]
    #[strum(serialize = "739")]
    RoomTaxCannotBeNegative,
    #[serde(rename = "740")]
    #[strum(serialize = "740")]
    DurationCanOnlyBeFrom0To99ForVisa,
    #[serde(rename = "801")]
    #[strum(serialize = "801")]
    AccountNumberWasSuccessfullyRegistered,
    #[serde(rename = "802")]
    #[strum(serialize = "802")]
    AccountNumberWasPreviouslyRegistered,
    #[serde(rename = "803")]
    #[strum(serialize = "803")]
    ValidToken,
    #[serde(rename = "820")]
    #[strum(serialize = "820")]
    CreditCardNumberWasInvalid,
    #[serde(rename = "821")]
    #[strum(serialize = "821")]
    MerchantIsNotAuthorizedForTokens,
    #[serde(rename = "822")]
    #[strum(serialize = "822")]
    TokenWasNotFound,
    #[serde(rename = "823")]
    #[strum(serialize = "823")]
    TokenInvalid,
    #[serde(rename = "825")]
    #[strum(serialize = "825")]
    MerchantNotAuthorizedForECheckTokens,
    #[serde(rename = "826")]
    #[strum(serialize = "826")]
    CheckoutIdWasInvalid,
    #[serde(rename = "827")]
    #[strum(serialize = "827")]
    CheckoutIdWasNotFound,
    #[serde(rename = "828")]
    #[strum(serialize = "828")]
    GenericCheckoutIdError,
    #[serde(rename = "835")]
    #[strum(serialize = "835")]
    CaptureAmountCanNotBeMoreThanAuthorizedAmount,
    #[serde(rename = "850")]
    #[strum(serialize = "850")]
    TaxBillingOnlyAllowedForMcc9311,
    #[serde(rename = "851")]
    #[strum(serialize = "851")]
    Mcc9311RequiresTaxTypeElement,
    #[serde(rename = "852")]
    #[strum(serialize = "852")]
    DebtRepaymentOnlyAllowedForViTransactionsOnMccs6012And6051,
    #[serde(rename = "861")]
    #[strum(serialize = "861")]
    RoutingNumberDidNotMatchOneOnFileForToken,
    #[serde(rename = "877")]
    #[strum(serialize = "877")]
    InvalidPayPageRegistrationId,
    #[serde(rename = "878")]
    #[strum(serialize = "878")]
    ExpiredPayPageRegistrationId,
    #[serde(rename = "879")]
    #[strum(serialize = "879")]
    MerchantIsNotAuthorizedForPayPage,
    #[serde(rename = "890")]
    #[strum(serialize = "890")]
    MaximumNumberOfUpdatesForThisTokenExceeded,
    #[serde(rename = "891")]
    #[strum(serialize = "891")]
    TooManyTokensCreatedForExistingNamespace,
    #[serde(rename = "895")]
    #[strum(serialize = "895")]
    PinValidationNotPossible,
    #[serde(rename = "898")]
    #[strum(serialize = "898")]
    GenericTokenRegistrationError,
    #[serde(rename = "899")]
    #[strum(serialize = "899")]
    GenericTokenUseError,
    #[serde(rename = "900")]
    #[strum(serialize = "900")]
    InvalidBankRoutingNumber,
    #[serde(rename = "901")]
    #[strum(serialize = "901")]
    MissingName,
    #[serde(rename = "902")]
    #[strum(serialize = "902")]
    InvalidName,
    #[serde(rename = "903")]
    #[strum(serialize = "903")]
    MissingBillingCountryCode,
    #[serde(rename = "904")]
    #[strum(serialize = "904")]
    InvalidIban,
    #[serde(rename = "905")]
    #[strum(serialize = "905")]
    MissingEmailAddress,
    #[serde(rename = "906")]
    #[strum(serialize = "906")]
    MissingMandateReference,
    #[serde(rename = "907")]
    #[strum(serialize = "907")]
    InvalidMandateReference,
    #[serde(rename = "908")]
    #[strum(serialize = "908")]
    MissingMandateUrl,
    #[serde(rename = "909")]
    #[strum(serialize = "909")]
    InvalidMandateUrl,
    #[serde(rename = "911")]
    #[strum(serialize = "911")]
    MissingMandateSignatureDate,
    #[serde(rename = "912")]
    #[strum(serialize = "912")]
    InvalidMandateSignatureDate,
    #[serde(rename = "913")]
    #[strum(serialize = "913")]
    RecurringMandateAlreadyExists,
    #[serde(rename = "914")]
    #[strum(serialize = "914")]
    RecurringMandateWasNotFound,
    #[serde(rename = "915")]
    #[strum(serialize = "915")]
    FinalRecurringWasAlreadyReceivedUsingThisMandate,
    #[serde(rename = "916")]
    #[strum(serialize = "916")]
    IbanDidNotMatchOneOnFileForMandate,
    #[serde(rename = "917")]
    #[strum(serialize = "917")]
    InvalidBillingCountry,
    #[serde(rename = "922")]
    #[strum(serialize = "922")]
    ExpirationDateRequiredForInteracTransaction,
    #[serde(rename = "923")]
    #[strum(serialize = "923")]
    TransactionTypeIsNotSupportedWithThisMethodOfPayment,
    #[serde(rename = "924")]
    #[strum(serialize = "924")]
    UnreferencedOrphanRefundsAreNotAllowed,
    #[serde(rename = "939")]
    #[strum(serialize = "939")]
    UnableToVoidATransactionWithAHeldState,
    #[serde(rename = "940")]
    #[strum(serialize = "940")]
    ThisFundingInstructionResultsInANegativeAccountBalance,
    #[serde(rename = "941")]
    #[strum(serialize = "941")]
    AccountBalanceInformationUnavailableAtThisTime,
    #[serde(rename = "942")]
    #[strum(serialize = "942")]
    TheSubmittedCardIsNotEligibleForFastAccessFunding,
    #[serde(rename = "943")]
    #[strum(serialize = "943")]
    TransactionCannotUseBothCcdPaymentInformationAndCtxPaymentInformation,
    #[serde(rename = "944")]
    #[strum(serialize = "944")]
    ProcessingError,
    #[serde(rename = "945")]
    #[strum(serialize = "945")]
    ThisFundingInstructionTypeIsInvalidForCanadianMerchants,
    #[serde(rename = "946")]
    #[strum(serialize = "946")]
    CtxAndCcdRecordsAreNotAllowedForCanadianMerchants,
    #[serde(rename = "947")]
    #[strum(serialize = "947")]
    CanadianAccountNumberCannotExceed12Digits,
    #[serde(rename = "948")]
    #[strum(serialize = "948")]
    ThisFundingInstructionTypeIsInvalid,
    #[serde(rename = "950")]
    #[strum(serialize = "950")]
    DeclineNegativeInformationOnFile,
    #[serde(rename = "951")]
    #[strum(serialize = "951")]
    AbsoluteDecline,
    #[serde(rename = "952")]
    #[strum(serialize = "952")]
    TheMerchantProfileDoesNotAllowTheRequestedOperation,
    #[serde(rename = "953")]
    #[strum(serialize = "953")]
    TheAccountCannotAcceptAchTransactions,
    #[serde(rename = "954")]
    #[strum(serialize = "954")]
    TheAccountCannotAcceptAchTransactionsOrSiteDrafts,
    #[serde(rename = "955")]
    #[strum(serialize = "955")]
    AmountGreaterThanLimitSpecifiedInTheMerchantProfile,
    #[serde(rename = "956")]
    #[strum(serialize = "956")]
    MerchantIsNotAuthorizedToPerformECheckVerificationTransactions,
    #[serde(rename = "957")]
    #[strum(serialize = "957")]
    FirstNameAndLastNameRequiredForECheckVerifications,
    #[serde(rename = "958")]
    #[strum(serialize = "958")]
    CompanyNameRequiredForCorporateAccountForECheckVerifications,
    #[serde(rename = "959")]
    #[strum(serialize = "959")]
    PhoneNumberRequiredForECheckVerifications,
    #[serde(rename = "961")]
    #[strum(serialize = "961")]
    CardBrandTokenNotSupported,
    #[serde(rename = "962")]
    #[strum(serialize = "962")]
    PrivateLabelCardNotSupported,
    #[serde(rename = "965")]
    #[strum(serialize = "965")]
    AllowedDailyDirectDebitCaptureECheckSaleLimitExceeded,
    #[serde(rename = "966")]
    #[strum(serialize = "966")]
    AllowedDailyDirectDebitCreditECheckCreditLimitExceeded,
    #[serde(rename = "973")]
    #[strum(serialize = "973")]
    AccountNotEligibleForRtp,
    #[serde(rename = "980")]
    #[strum(serialize = "980")]
    SoftDeclineCustomerAuthenticationRequired,
    #[serde(rename = "981")]
    #[strum(serialize = "981")]
    TransactionNotReversedVoidWorkflowNeedToBeInvoked,
    #[serde(rename = "982")]
    #[strum(serialize = "982")]
    TransactionReversalNotSupportedForTheCoreMerchants,
    #[serde(rename = "983")]
    #[strum(serialize = "983")]
    NoValidParentDepositOrParentRefundFound,
    #[serde(rename = "984")]
    #[strum(serialize = "984")]
    TransactionReversalNotEnabledForVisa,
    #[serde(rename = "985")]
    #[strum(serialize = "985")]
    TransactionReversalNotEnabledForMastercard,
    #[serde(rename = "986")]
    #[strum(serialize = "986")]
    TransactionReversalNotEnabledForAmEx,
    #[serde(rename = "987")]
    #[strum(serialize = "987")]
    TransactionReversalNotEnabledForDiscover,
    #[serde(rename = "988")]
    #[strum(serialize = "988")]
    TransactionReversalNotSupported,
    #[serde(rename = "990")]
    #[strum(serialize = "990")]
    FundingInstructionHeldPleaseContactYourRelationshipManager,
    #[serde(rename = "991")]
    #[strum(serialize = "991")]
    MissingAddressInformation,
    #[serde(rename = "992")]
    #[strum(serialize = "992")]
    CryptographicFailure,
    #[serde(rename = "993")]
    #[strum(serialize = "993")]
    InvalidRegionCode,
    #[serde(rename = "994")]
    #[strum(serialize = "994")]
    InvalidCountryCode,
    #[serde(rename = "995")]
    #[strum(serialize = "995")]
    InvalidCreditAccount,
    #[serde(rename = "996")]
    #[strum(serialize = "996")]
    InvalidCheckingAccount,
    #[serde(rename = "997")]
    #[strum(serialize = "997")]
    InvalidSavingsAccount,
    #[serde(rename = "998")]
    #[strum(serialize = "998")]
    InvalidUseOfMccCorrectAndReattempt,
    #[serde(rename = "999")]
    #[strum(serialize = "999")]
    ExceedsRtpTransactionLimit,
}

impl WorldpayvantivResponseCode {
    fn is_approved(self) -> bool {
        matches!(
            self,
            Self::Approved
                | Self::PartiallyApproved
                | Self::OfflineApproval
                | Self::OfflineApprovalUnableToGoOnline
                | Self::ConsumerNonReloadablePrepaidCardApproved
                | Self::ConsumerSingleUseVirtualCardNumberApproved
                | Self::ScheduledRecurringPaymentProcessed
                | Self::ApprovedRecurringSubscriptionCreated
                | Self::PendingShopperCheckoutCompletion
                | Self::TransactionReceived
                | Self::AccountNumberWasSuccessfullyRegistered
                | Self::AccountNumberWasPreviouslyRegistered
                | Self::ValidToken
        )
    }
}

// Sync structures

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VantivSyncResponse {
    #[serde(rename = "paymentId", skip_serializing_if = "Option::is_none")]
    pub payment_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_uuid: Option<String>,
    pub payment_status: PaymentStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_detail: Option<PaymentDetail>,
}

#[derive(Debug, strum::Display, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PaymentStatus {
    NotYetProcessed,
    ProcessedSuccessfully,
    TransactionDeclined,
    StatusUnavailable,
    PaymentStatusNotFound,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentDetail {
    #[serde(rename = "paymentId", skip_serializing_if = "Option::is_none")]
    pub payment_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub batch_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_reason_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_reason_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reject_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dupe_txn_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<StringMajorUnit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub purchase_currency: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_day: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reported_timestamp: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant_order_number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant_txn_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reporting_group: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub txn_type: Option<String>,
    #[serde(rename = "eCommMerchantId", skip_serializing_if = "Option::is_none")]
    pub e_comm_merchant_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub organization_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant_category_code: Option<String>,
}

// Sync error response
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VantivSyncErrorResponse {
    pub error_messages: Vec<String>,
}

// Dispute structures
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename = "chargebackRetrievalResponse", rename_all = "camelCase")]
pub struct ChargebackRetrievalResponse {
    #[serde(rename = "@xmlns")]
    pub xmlns: String,
    pub transaction_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chargeback_case: Option<Vec<ChargebackCase>>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ChargebackCase {
    pub case_id: String,
    pub merchant_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub day_issued_by_bank: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date_received_by_vantiv_cnp: Option<String>,
    pub vantiv_cnp_txn_id: String,
    pub cycle: String,
    pub order_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_number_last4: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_type: Option<String>,
    pub chargeback_amount: MinorUnit,
    pub chargeback_currency_type: Currency,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_txn_day: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chargeback_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason_code_description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_queue: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acquirer_reference_number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chargeback_reference_number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bin: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_amount: Option<MinorUnit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_by_day: Option<String>,
    pub activity: Vec<Activity>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Activity {
    pub activity_date: String,
    pub activity_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_queue: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_queue: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settlement_amount: Option<MinorUnit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename = "chargebackUpdateRequest", rename_all = "camelCase")]
pub struct ChargebackUpdateRequest {
    #[serde(rename = "@xmlns")]
    pub xmlns: String,
    pub activity_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChargebackDocumentUploadResponse {
    pub response_message: String,
    pub response_code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_id: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VantivDisputeErrorResponse {
    pub errors: Vec<ErrorInfo>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorInfo {
    pub error: String,
}

// Payment flow types
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum WorldpayvantivPaymentFlow {
    Sale,
    Auth,
    Capture,
    Void,
    VoidPC, // VoidPostCapture
}

// Helper function to determine payment flow type from merchant transaction ID
fn get_payment_flow_type(
    merchant_txn_id: &str,
) -> Result<WorldpayvantivPaymentFlow, Report<ConnectorError>> {
    let merchant_txn_id_lower = merchant_txn_id.to_lowercase();
    if merchant_txn_id_lower.contains("auth") {
        Ok(WorldpayvantivPaymentFlow::Auth)
    } else if merchant_txn_id_lower.contains("sale") {
        Ok(WorldpayvantivPaymentFlow::Sale)
    } else if merchant_txn_id_lower.contains("voidpc") {
        Ok(WorldpayvantivPaymentFlow::VoidPC)
    } else if merchant_txn_id_lower.contains("void") {
        Ok(WorldpayvantivPaymentFlow::Void)
    } else if merchant_txn_id_lower.contains("capture") {
        Ok(WorldpayvantivPaymentFlow::Capture)
    } else {
        Err(Report::new(ConnectorError::UnexpectedResponseError {
            context: ResponseTransformationErrorContext {
                http_status_code: None,
                additional_context: Some(format!(
                    "Unable to determine payment flow type from merchant transaction ID: {merchant_txn_id}"
                )),
            },
        }))
    }
}

// Helper function to determine attempt status based on payment flow and Vantiv response status
fn determine_attempt_status_for_psync(
    payment_status: PaymentStatus,
    merchant_txn_id: &str,
    current_status: common_enums::AttemptStatus,
) -> Result<common_enums::AttemptStatus, Report<ConnectorError>> {
    let flow_type = get_payment_flow_type(merchant_txn_id)?;

    match payment_status {
        PaymentStatus::ProcessedSuccessfully => match flow_type {
            WorldpayvantivPaymentFlow::Sale | WorldpayvantivPaymentFlow::Capture => {
                Ok(common_enums::AttemptStatus::Charged)
            }
            WorldpayvantivPaymentFlow::Auth => Ok(common_enums::AttemptStatus::Authorized),
            WorldpayvantivPaymentFlow::Void => Ok(common_enums::AttemptStatus::Voided),
            WorldpayvantivPaymentFlow::VoidPC => Ok(common_enums::AttemptStatus::VoidedPostCapture),
        },
        PaymentStatus::TransactionDeclined => match flow_type {
            WorldpayvantivPaymentFlow::Sale | WorldpayvantivPaymentFlow::Capture => {
                Ok(common_enums::AttemptStatus::Failure)
            }
            WorldpayvantivPaymentFlow::Auth => Ok(common_enums::AttemptStatus::AuthorizationFailed),
            WorldpayvantivPaymentFlow::Void | WorldpayvantivPaymentFlow::VoidPC => {
                Ok(common_enums::AttemptStatus::VoidFailed)
            }
        },
        PaymentStatus::PaymentStatusNotFound
        | PaymentStatus::NotYetProcessed
        | PaymentStatus::StatusUnavailable => Ok(current_status),
    }
}

#[derive(Debug, strum::Display, Serialize, Deserialize, PartialEq, Clone, Copy)]
#[strum(serialize_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum OperationId {
    Sale,
    Auth,
    Capture,
    Void,
    #[strum(serialize = "voidPC")]
    VoidPC,
    Refund,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<CnpOnlineResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<CnpOnlineResponse, Self>) -> Result<Self, Self::Error> {
        let is_auto_capture = item.router_data.request.is_auto_capture();
        handle_payment_response(item, is_auto_capture)
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<CnpOnlineResponse, Self>>
    for RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<CnpOnlineResponse, Self>) -> Result<Self, Self::Error> {
        let is_auto_capture = item.router_data.request.is_auto_capture();
        handle_payment_response(item, is_auto_capture)
    }
}

fn handle_payment_response<F, Req>(
    item: ResponseRouterData<
        CnpOnlineResponse,
        RouterDataV2<F, PaymentFlowData, Req, PaymentsResponseData>,
    >,
    is_auto_capture: bool,
) -> Result<RouterDataV2<F, PaymentFlowData, Req, PaymentsResponseData>, Report<ConnectorError>> {
    let (payment_response, flow) = match (
        item.response.sale_response.as_ref(),
        item.response.authorization_response.as_ref(),
    ) {
        (Some(response), None) => (response, WorldpayvantivPaymentFlow::Sale),
        (None, Some(response)) => {
            let flow = if is_auto_capture {
                WorldpayvantivPaymentFlow::Sale
            } else {
                WorldpayvantivPaymentFlow::Auth
            };
            (response, flow)
        }
        (None, None) => {
            let status = common_enums::AttemptStatus::Failure;
            let error_response = ErrorResponse {
                code: item.response.response_code,
                message: item.response.message.clone(),
                reason: Some(item.response.message),
                status_code: item.http_code,
                attempt_status: Some(FlowStatus::Payment(status)),
                connector_transaction_id: None,
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
            };

            return Ok(RouterDataV2 {
                resource_common_data: PaymentFlowData {
                    status,
                    ..item.router_data.resource_common_data
                },
                response: Err(error_response),
                ..item.router_data
            });
        }
        (Some(_), Some(_)) => {
            return Err(Report::from(utils::unexpected_response_fail(
                item.http_code,
                "worldpayvantiv: unexpected response for this operation; retry with idempotency keys and check connector status.",
            ))
            .attach_printable("Only one of 'sale_response' or 'authorization_response' is expected"));
        }
    };

    let status = get_attempt_status(flow, payment_response.response)?;
    let response = if is_payment_failure(status) {
        Err(ErrorResponse {
            code: payment_response.response.to_string(),
            message: payment_response.message.clone(),
            reason: Some(payment_response.message.clone()),
            status_code: item.http_code,
            attempt_status: Some(FlowStatus::Payment(status)),
            connector_transaction_id: Some(payment_response.cnp_txn_id.clone()),
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
            typed_connector_response: None,
            raw_connector_response: None,
            raw_connector_request: None,
            typed_connector_request: None,
        })
    } else {
        Ok(PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(payment_response.cnp_txn_id.clone()),
            redirection_data: None,
            mandate_reference: payment_response.mandate_reference().change_context(
                utils::response_handling_fail_for_connector(item.http_code, "worldpayvantiv"),
            )?,
            connector_metadata: None,
            network_txn_id: payment_response
                .network_transaction_id
                .clone()
                .map(|id| id.expose()),
            network_txn_link_id: None,
            connector_response_reference_id: Some(payment_response.order_id.clone()),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
            splits: None,
            payment_account_reference: None,
        })
    };

    Ok(RouterDataV2 {
        resource_common_data: PaymentFlowData {
            status,
            ..item.router_data.resource_common_data
        },
        response,
        ..item.router_data
    })
}

// Helper functions for creating RawCardNumber from different sources
#[allow(dead_code)]
fn create_raw_card_number_from_string<T: PaymentMethodDataTypes>(
    card_string: String,
) -> Result<RawCardNumber<T>, Report<IntegrationError>>
where
    T::Inner: From<String>,
{
    Ok(RawCardNumber(T::Inner::from(card_string)))
}

#[allow(dead_code)]
fn get_payment_info<T: PaymentMethodDataTypes>(
    payment_method_data: &PaymentMethodData<T>,
) -> Result<PaymentInfo<T>, Report<IntegrationError>>
where
    T::Inner: From<String> + Clone,
{
    match payment_method_data {
        PaymentMethodData::Card(card_data) => {
            let card_type = match card_data.card_network.clone() {
                Some(network) => WorldpayvativCardType::try_from(network)?,
                None => {
                    // Fallback to BIN-based card issuer detection
                    let card_issuer =
                        domain_types::utils::get_card_issuer(card_data.card_number.peek())?;
                    WorldpayvativCardType::try_from(&card_issuer)?
                }
            };

            let year_str = card_data.card_exp_year.peek();
            let formatted_year = if year_str.len() == 4 {
                &year_str[2..]
            } else {
                year_str
            };
            let exp_date = format!("{}{}", card_data.card_exp_month.peek(), formatted_year);

            let worldpay_card = WorldpayvantivCardData {
                card_type,
                number: card_data.card_number.clone(),
                exp_date: exp_date.into(),
                card_validation_num: Some(card_data.card_cvc.clone()),
            };

            Ok(PaymentInfo::Card(CardData {
                card: worldpay_card,
                processing_type: None,
                network_transaction_id: None,
            }))
        }
        PaymentMethodData::Wallet(wallet_data) => {
            match wallet_data {
                WalletData::ApplePay(apple_pay_data) => {
                    match apple_pay_data
                        .payment_data
                        .get_decrypted_apple_pay_payment_data_optional()
                    {
                        Some(apple_pay_decrypted_data) => {
                            let card_type = determine_apple_pay_card_type(
                                &apple_pay_data.payment_method.network,
                            )?;
                            let expiry_month = apple_pay_decrypted_data.get_expiry_month();
                            let expiry_year = apple_pay_decrypted_data.get_four_digit_expiry_year();
                            let formatted_year = &expiry_year.expose()[2..];
                            let exp_date = format!("{}{}", expiry_month.expose(), formatted_year);

                            let card_number_string = apple_pay_decrypted_data
                                .application_primary_account_number
                                .get_card_no();
                            let raw_card_number =
                                create_raw_card_number_from_string::<T>(card_number_string)?;

                            let worldpay_card = WorldpayvantivCardData {
                                card_type,
                                number: raw_card_number,
                                exp_date: exp_date.into(),
                                card_validation_num: None,
                            };

                            Ok(PaymentInfo::Card(CardData {
                                card: worldpay_card,
                                processing_type: None,
                                network_transaction_id: None,
                            }))
                        }
                        None => Err(IntegrationError::MissingRequiredField {
                            field_name: "apple_pay_decrypted_data",
                            context: IntegrationErrorContext {
                                additional_context: Some("Apple Pay card conversion requires decrypted payment data containing the account number and expiry.".to_owned()),
                                ..Default::default()
                            },
                        }
                        .into()),
                    }
                }
                WalletData::GooglePay(google_pay_data) => {
                    match &google_pay_data.tokenization_data {
                        domain_types::payment_method_data::GpayTokenizationData::Decrypted(
                            google_pay_decrypted_data,
                        ) => {
                            let card_type =
                                determine_google_pay_card_type(&google_pay_data.info.card_network)?;
                            let expiry_month = google_pay_decrypted_data
                                .get_expiry_month()
                                .change_context(IntegrationError::InvalidDataFormat {
                                    field_name: "google_pay_decrypted_data.card_exp_month",
                                    context: IntegrationErrorContext {
                                        additional_context: Some("The expiry month in the decrypted Google Pay payment data could not be formatted.".to_owned()),
                                        ..Default::default()
                                    },
                                })?;
                            let expiry_year = google_pay_decrypted_data
                                .get_four_digit_expiry_year()
                                .change_context(IntegrationError::InvalidDataFormat {
                                    field_name: "google_pay_decrypted_data.card_exp_year",
                                    context: IntegrationErrorContext {
                                        additional_context: Some("The expiry year in the decrypted Google Pay payment data could not be formatted as a four-digit year.".to_owned()),
                                        ..Default::default()
                                    },
                                })?;
                            let formatted_year = &expiry_year.expose()[2..];
                            let exp_date = format!("{}{}", expiry_month.expose(), formatted_year);

                            let card_number_string = google_pay_decrypted_data
                                .application_primary_account_number
                                .get_card_no();
                            let raw_card_number =
                                create_raw_card_number_from_string::<T>(card_number_string)?;

                            let worldpay_card = WorldpayvantivCardData {
                                card_type,
                                number: raw_card_number,
                                exp_date: exp_date.into(),
                                card_validation_num: None, // Google Pay doesn't provide CVV
                            };

                            Ok(PaymentInfo::Card(CardData {
                                card: worldpay_card,
                                processing_type: None,
                                network_transaction_id: None,
                            }))
                        }
                        domain_types::payment_method_data::GpayTokenizationData::Encrypted(_) => {
                            Err(IntegrationError::MissingRequiredField {
                                field_name: "google_pay_decrypted_data",
                                context: IntegrationErrorContext {
                                    additional_context: Some("Google Pay card conversion requires decrypted payment data; an encrypted token cannot be converted to card details.".to_owned()),
                                    ..Default::default()
                                },
                            }
                            .into())
                        }
                    }
                }
                _ => Err(IntegrationError::NotSupported {
                    message: "Wallet type".to_string(),
                    connector: "worldpayvantiv",
                    context: IntegrationErrorContext {
                        additional_context: Some("Wallet card conversion supports Apple Pay and Google Pay with decrypted payment data only.".to_owned()),
                        ..Default::default()
                    },
                }
                .into()),
            }
        }
        _ => Err(IntegrationError::NotSupported {
            message: "Payment method".to_string(),
            connector: "worldpayvantiv",
            context: IntegrationErrorContext {
                additional_context: Some("Payment data conversion supports cards and decrypted Apple Pay or Google Pay data only.".to_owned()),
                ..Default::default()
            },
        }
        .into()),
    }
}

#[allow(dead_code)]
fn determine_apple_pay_card_type(
    network: &str,
) -> Result<WorldpayvativCardType, Report<IntegrationError>> {
    match network.to_lowercase().as_str() {
        "visa" => Ok(WorldpayvativCardType::Visa),
        "mastercard" => Ok(WorldpayvativCardType::MasterCard),
        "amex" => Ok(WorldpayvativCardType::AmericanExpress),
        "discover" => Ok(WorldpayvativCardType::Discover),
        _ => Err(IntegrationError::NotSupported {
            message: format!("Apple Pay network: {network}"),
            connector: "worldpayvantiv",
            context: IntegrationErrorContext {
                additional_context: Some("Supported Apple Pay networks are Visa, Mastercard, American Express, and Discover.".to_owned()),
                ..Default::default()
            },
        }
        .into()),
    }
}

#[allow(dead_code)]
fn determine_google_pay_card_type(
    network: &str,
) -> Result<WorldpayvativCardType, Report<IntegrationError>> {
    match network.to_lowercase().as_str() {
        "visa" => Ok(WorldpayvativCardType::Visa),
        "mastercard" => Ok(WorldpayvativCardType::MasterCard),
        "amex" => Ok(WorldpayvativCardType::AmericanExpress),
        "discover" => Ok(WorldpayvativCardType::Discover),
        _ => Err(IntegrationError::NotSupported {
            message: format!("Google Pay network: {network}"),
            connector: "worldpayvantiv",
            context: IntegrationErrorContext {
                additional_context: Some("Supported Google Pay networks are Visa, Mastercard, American Express, and Discover.".to_owned()),
                ..Default::default()
            },
        }
        .into()),
    }
}

fn get_billing_address(resource_data: &PaymentFlowData) -> Option<BillToAddress> {
    resource_data
        .get_optional_billing()
        .and_then(|billing_address| {
            billing_address.address.clone().map(|_| BillToAddress {
                name: resource_data.get_optional_billing_full_name(),
                company: resource_data
                    .get_optional_billing_first_name()
                    .map(|f| f.expose()),
                address_line1: resource_data.get_optional_billing_line1(),
                address_line2: resource_data.get_optional_billing_line2(),
                address_line3: resource_data.get_optional_billing_line3(),
                city: resource_data.get_optional_billing_city(),
                state: resource_data.get_optional_billing_state(),
                zip: resource_data.get_optional_billing_zip(),
                country: resource_data.get_optional_billing_country(),
                email: resource_data.get_optional_billing_email(),
                phone: resource_data.get_optional_billing_phone_number(),
            })
        })
}

fn get_shipping_address(resource_data: &PaymentFlowData) -> Option<ShipToAddress> {
    resource_data
        .get_optional_shipping()
        .and_then(|shipping_address| {
            shipping_address.address.clone().map(|_| ShipToAddress {
                name: resource_data.get_optional_shipping_full_name(),
                company: resource_data
                    .get_optional_shipping_first_name()
                    .map(|f| f.expose()),
                address_line1: resource_data.get_optional_shipping_line1(),
                address_line2: resource_data.get_optional_shipping_line2(),
                address_line3: resource_data.get_optional_shipping_line3(),
                city: resource_data.get_optional_shipping_city(),
                state: resource_data.get_optional_shipping_state(),
                zip: resource_data.get_optional_shipping_zip(),
                country: resource_data.get_optional_shipping_country(),
                email: resource_data.get_optional_shipping_email(),
                phone: resource_data.get_optional_shipping_phone_number(),
            })
        })
}

fn get_valid_transaction_id(
    id: String,
    _error_field_name: &str,
) -> Result<String, Report<IntegrationError>> {
    if id.len() <= worldpayvantiv_constants::MAX_PAYMENT_REFERENCE_ID_LENGTH {
        Ok(id)
    } else {
        Err(IntegrationError::InvalidConnectorConfig {
            config: "Transaction ID length exceeds maximum limit",
            context: IntegrationErrorContext {
                additional_context: Some(format!(
                    "Transaction IDs must not exceed {} bytes.",
                    worldpayvantiv_constants::MAX_PAYMENT_REFERENCE_ID_LENGTH
                )),
                ..Default::default()
            },
        }
        .into())
    }
}

// Step 94-98: TryFrom for PSync response
impl TryFrom<ResponseRouterData<VantivSyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<VantivSyncResponse, Self>) -> Result<Self, Self::Error> {
        let status = if let Some(merchant_txn_id) = item
            .response
            .payment_detail
            .as_ref()
            .and_then(|detail| detail.merchant_txn_id.as_ref())
        {
            determine_attempt_status_for_psync(
                item.response.payment_status,
                merchant_txn_id,
                item.router_data.resource_common_data.status,
            )?
        } else {
            // Without a transaction reference, preserve the status instead of guessing the flow.
            item.router_data.resource_common_data.status
        };

        let payments_response = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(
                item.response
                    .payment_id
                    .map(|id| id.to_string())
                    .unwrap_or_else(|| "unknown".to_string()),
            ),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            network_txn_link_id: None,
            connector_response_reference_id: item
                .response
                .payment_detail
                .as_ref()
                .and_then(|detail| detail.merchant_txn_id.clone()),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
            splits: None,
            payment_account_reference: None,
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            response: Ok(payments_response),
            ..item.router_data
        })
    }
}

// TryFrom for Capture request
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        WorldpayvantivRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for WorldpayvantivPaymentsRequest<T>
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: WorldpayvantivRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = WorldpayvantivAuthType::try_from(&item.router_data.connector_config)?;
        let merchant_id = auth.merchant_id_for_currency(Some(item.router_data.request.currency))?;

        let authentication = Authentication {
            user: auth.user,
            password: auth.password,
        };

        let cnp_txn_id = item
            .router_data
            .request
            .get_connector_transaction_id()
            .change_context(IntegrationError::MissingConnectorTransactionID {
                context: IntegrationErrorContext {
                    additional_context: Some("Capture requires the cnpTxnId returned by Worldpay Vantiv for the original authorization.".to_owned()),
                    ..Default::default()
                },
            })?;
        let merchant_txn_id = item
            .router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();

        // Extract report_group from connector_feature_data
        let report_group = extract_report_group(&item.router_data.connector_config)
            .unwrap_or_else(|| "rtpGrp".to_string());

        let capture = CaptureRequest {
            id: format!("{}_{}", OperationId::Capture, merchant_txn_id),
            report_group,
            cnp_txn_id,
            amount: item.router_data.request.capture_money.amount(),
            enhanced_data: None,
        };

        let cnp_request = CnpOnlineRequest {
            version: worldpayvantiv_constants::WORLDPAYVANTIV_VERSION.to_string(),
            xmlns: worldpayvantiv_constants::XMLNS.to_string(),
            merchant_id,
            authentication,
            authorization: None,
            sale: None,
            capture: Some(capture),
            auth_reversal: None,
            void: None,
            credit: None,
        };

        Ok(Self { cnp_request })
    }
}

// TryFrom for Void request (pre-capture void using AuthReversal)
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        WorldpayvantivRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for WorldpayvantivPaymentsRequest<T>
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: WorldpayvantivRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = WorldpayvantivAuthType::try_from(&item.router_data.connector_config)?;
        let merchant_id = auth.merchant_id_for_currency(item.router_data.request.currency)?;

        let authentication = Authentication {
            user: auth.user,
            password: auth.password,
        };

        let cnp_txn_id = item.router_data.request.connector_transaction_id.clone();
        let merchant_txn_id = item
            .router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();

        // Extract report group from metadata or use default
        let report_group = extract_report_group(&item.router_data.connector_config)
            .unwrap_or_else(|| "rtpGrp".to_string());

        // For pre-capture void, use AuthReversal
        let auth_reversal = AuthReversal {
            id: format!("{}_{}", OperationId::Void, merchant_txn_id),
            report_group,
            cnp_txn_id,
            amount: None, // Amount is optional for AuthReversal
        };

        let cnp_request = CnpOnlineRequest {
            version: worldpayvantiv_constants::WORLDPAYVANTIV_VERSION.to_string(),
            xmlns: worldpayvantiv_constants::XMLNS.to_string(),
            merchant_id,
            authentication,
            authorization: None,
            sale: None,
            capture: None,
            auth_reversal: Some(auth_reversal),
            void: None,
            credit: None,
        };

        Ok(Self { cnp_request })
    }
}

// TryFrom for Refund request
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        WorldpayvantivRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for WorldpayvantivPaymentsRequest<T>
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: WorldpayvantivRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = WorldpayvantivAuthType::try_from(&item.router_data.connector_config)?;
        let merchant_id = auth.merchant_id_for_currency(Some(item.router_data.request.currency))?;

        // Extract report_group from connector config before moving auth fields
        let report_group = auth.report_group.unwrap_or_else(|| "rtpGrp".to_string());

        let authentication = Authentication {
            user: auth.user,
            password: auth.password,
        };

        let cnp_txn_id = item.router_data.request.connector_transaction_id.clone();
        let merchant_txn_id = item
            .router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();

        // Extract customer_id from RefundsData - since RefundsData stores it as String, we convert it to CustomerId to use with extract_customer_id function
        let customer_id = item
            .router_data
            .request
            .customer_id
            .as_ref()
            .and_then(|id_str| CustomerId::try_from(Cow::from(id_str.clone())).ok())
            .as_ref()
            .and_then(|customer_id| extract_customer_id(&Some(customer_id.clone())));

        let credit = RefundRequest {
            report_group,
            id: format!("{}_{}", OperationId::Refund, merchant_txn_id),
            customer_id,
            cnp_txn_id,
            amount: item.router_data.request.refund_money.amount(),
        };

        let cnp_request = CnpOnlineRequest {
            version: worldpayvantiv_constants::WORLDPAYVANTIV_VERSION.to_string(),
            xmlns: worldpayvantiv_constants::XMLNS.to_string(),
            merchant_id,
            authentication,
            authorization: None,
            sale: None,
            capture: None,
            auth_reversal: None,
            void: None,
            credit: Some(credit),
        };

        Ok(Self { cnp_request })
    }
}

// TryFrom for RSync request

// TryFrom for VoidPC (VoidPostCapture) request
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        WorldpayvantivRouterData<
            RouterDataV2<
                VoidPC,
                PaymentFlowData,
                PaymentsCancelPostCaptureData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for WorldpayvantivPaymentsRequest<T>
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: WorldpayvantivRouterData<
            RouterDataV2<
                VoidPC,
                PaymentFlowData,
                PaymentsCancelPostCaptureData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = WorldpayvantivAuthType::try_from(&item.router_data.connector_config)?;
        let merchant_id = auth.merchant_id_for_currency(
            item.router_data
                .request
                .money
                .as_ref()
                .map(|money| money.currency()),
        )?;

        let authentication = Authentication {
            user: auth.user,
            password: auth.password,
        };

        let cnp_txn_id = item.router_data.request.connector_transaction_id.clone();
        let merchant_txn_id = item
            .router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();

        // Extract report group from metadata or use default
        let report_group = extract_report_group(&item.router_data.connector_config)
            .unwrap_or_else(|| "rtpGrp".to_string());

        let void = VoidRequest {
            id: format!("{}_{}", OperationId::VoidPC, merchant_txn_id),
            report_group,
            cnp_txn_id,
        };

        let cnp_request = CnpOnlineRequest {
            version: worldpayvantiv_constants::WORLDPAYVANTIV_VERSION.to_string(),
            xmlns: worldpayvantiv_constants::XMLNS.to_string(),
            merchant_id,
            authentication,
            authorization: None,
            sale: None,
            capture: None,
            auth_reversal: None,
            void: Some(void),
            credit: None,
        };

        Ok(Self { cnp_request })
    }
}

impl TryFrom<ResponseRouterData<CnpOnlineResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<CnpOnlineResponse, Self>) -> Result<Self, Self::Error> {
        if let Some(credit_response) = item.response.credit_response {
            let status = get_refund_status(credit_response.response)?;

            let refunds_response = RefundsResponseData {
                connector_refund_id: credit_response.cnp_txn_id.clone(),
                refund_status: status,
                status_code: item.http_code,
                acquirer_reference_number: None,
            };

            Ok(Self {
                resource_common_data: RefundFlowData {
                    status,
                    ..item.router_data.resource_common_data
                },
                response: Ok(refunds_response),
                ..item.router_data
            })
        } else {
            let error_response = ErrorResponse {
                code: item.response.response_code,
                message: item.response.message.clone(),
                reason: Some(item.response.message.clone()),
                status_code: item.http_code,
                attempt_status: Some(FlowStatus::Payment(common_enums::AttemptStatus::Failure)),
                connector_transaction_id: None,
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
            };

            Ok(Self {
                resource_common_data: RefundFlowData {
                    status: common_enums::RefundStatus::Failure,
                    ..item.router_data.resource_common_data
                },
                response: Err(error_response),
                ..item.router_data
            })
        }
    }
}

// Step 109-113: TryFrom for RSync response
impl TryFrom<ResponseRouterData<VantivSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<VantivSyncResponse, Self>) -> Result<Self, Self::Error> {
        let status = match item.response.payment_status {
            PaymentStatus::ProcessedSuccessfully => common_enums::RefundStatus::Success,
            PaymentStatus::TransactionDeclined => common_enums::RefundStatus::Failure,
            PaymentStatus::NotYetProcessed
            | PaymentStatus::StatusUnavailable
            | PaymentStatus::PaymentStatusNotFound => common_enums::RefundStatus::Pending,
        };

        let refunds_response = RefundsResponseData {
            connector_refund_id: item
                .response
                .payment_id
                .map(|id| id.to_string())
                .unwrap_or_else(|| "unknown".to_string()),
            refund_status: status,
            status_code: item.http_code,
            acquirer_reference_number: None,
        };

        Ok(Self {
            resource_common_data: RefundFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            response: Ok(refunds_response),
            ..item.router_data
        })
    }
}

// Step 114-123: TryFrom for Capture request and response
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        WorldpayvantivRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for CnpOnlineRequest<T>
{
    type Error = Report<IntegrationError>;
    fn try_from(
        item: WorldpayvantivRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = WorldpayvantivAuthType::try_from(&item.router_data.connector_config)?;
        let merchant_id = auth.merchant_id_for_currency(Some(item.router_data.request.currency))?;

        let authentication = Authentication {
            user: auth.user,
            password: auth.password,
        };

        let cnp_txn_id = item
            .router_data
            .request
            .get_connector_transaction_id()
            .change_context(IntegrationError::MissingConnectorTransactionID {
                context: IntegrationErrorContext {
                    additional_context: Some("Capture requires the cnpTxnId returned by Worldpay Vantiv for the original authorization.".to_owned()),
                    ..Default::default()
                },
            })?;
        let merchant_txn_id = item
            .router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();

        // Extract report_group from connector_feature_data
        let report_group = extract_report_group(&item.router_data.connector_config)
            .unwrap_or_else(|| "rtpGrp".to_string());

        let capture = CaptureRequest {
            id: format!("{}_{}", OperationId::Capture, merchant_txn_id),
            report_group,
            cnp_txn_id,
            amount: item.router_data.request.capture_money.amount(),
            enhanced_data: None,
        };

        Ok(Self {
            version: worldpayvantiv_constants::WORLDPAYVANTIV_VERSION.to_string(),
            xmlns: worldpayvantiv_constants::XMLNS.to_string(),
            merchant_id,
            authentication,
            authorization: None,
            sale: None,
            capture: Some(capture),
            auth_reversal: None,
            void: None,
            credit: None,
        })
    }
}

impl TryFrom<ResponseRouterData<CnpOnlineResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<CnpOnlineResponse, Self>) -> Result<Self, Self::Error> {
        if let Some(capture_response) = item.response.capture_response {
            let status = get_attempt_status(
                WorldpayvantivPaymentFlow::Capture,
                capture_response.response,
            )?;

            if is_payment_failure(status) {
                let error_response = ErrorResponse {
                    code: capture_response.response.to_string(),
                    message: capture_response.message.clone(),
                    reason: Some(capture_response.message.clone()),
                    status_code: item.http_code,
                    attempt_status: Some(FlowStatus::Payment(status)),
                    connector_transaction_id: Some(capture_response.cnp_txn_id.clone()),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                    typed_connector_response: None,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                };

                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status,
                        ..item.router_data.resource_common_data
                    },
                    response: Err(error_response),
                    ..item.router_data
                })
            } else {
                let payments_response = PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(
                        capture_response.cnp_txn_id.clone(),
                    ),
                    redirection_data: None,
                    mandate_reference: None,
                    connector_metadata: None,
                    network_txn_id: None,
                    network_txn_link_id: None,
                    connector_response_reference_id: Some(capture_response.id.clone()),
                    incremental_authorization_allowed: None,
                    status_code: item.http_code,
                    splits: None,
                    payment_account_reference: None,
                };

                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status,
                        ..item.router_data.resource_common_data
                    },
                    response: Ok(payments_response),
                    ..item.router_data
                })
            }
        } else {
            let error_response = ErrorResponse {
                code: item.response.response_code,
                message: item.response.message.clone(),
                reason: Some(item.response.message.clone()),
                status_code: item.http_code,
                attempt_status: Some(FlowStatus::Payment(
                    common_enums::AttemptStatus::CaptureFailed,
                )),
                connector_transaction_id: None,
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
            };

            Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: common_enums::AttemptStatus::CaptureFailed,
                    ..item.router_data.resource_common_data
                },
                response: Err(error_response),
                ..item.router_data
            })
        }
    }
}

// Step 124-133: TryFrom for Void request and response
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        WorldpayvantivRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for CnpOnlineRequest<T>
{
    type Error = Report<IntegrationError>;
    fn try_from(
        item: WorldpayvantivRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = WorldpayvantivAuthType::try_from(&item.router_data.connector_config)?;
        let merchant_id = auth.merchant_id_for_currency(item.router_data.request.currency)?;

        let authentication = Authentication {
            user: auth.user,
            password: auth.password,
        };

        let cnp_txn_id = item.router_data.request.connector_transaction_id.clone();
        let merchant_txn_id = item
            .router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();

        // Extract report_group from connector_feature_data
        let report_group = extract_report_group(&item.router_data.connector_config)
            .unwrap_or_else(|| "rtpGrp".to_string());

        let void = VoidRequest {
            id: format!("{}_{}", OperationId::Void, merchant_txn_id),
            report_group,
            cnp_txn_id,
        };

        Ok(Self {
            version: worldpayvantiv_constants::WORLDPAYVANTIV_VERSION.to_string(),
            xmlns: worldpayvantiv_constants::XMLNS.to_string(),
            merchant_id,
            authentication,
            authorization: None,
            sale: None,
            capture: None,
            auth_reversal: None,
            void: Some(void),
            credit: None,
        })
    }
}

impl TryFrom<ResponseRouterData<CnpOnlineResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<CnpOnlineResponse, Self>) -> Result<Self, Self::Error> {
        // Check for AuthReversal response first (pre-capture void)
        if let Some(auth_reversal_response) = item.response.auth_reversal_response {
            let status = get_attempt_status(
                WorldpayvantivPaymentFlow::Void,
                auth_reversal_response.response,
            )?;

            if is_payment_failure(status) {
                let error_response = ErrorResponse {
                    code: auth_reversal_response.response.to_string(),
                    message: auth_reversal_response.message.clone(),
                    reason: Some(auth_reversal_response.message.clone()),
                    status_code: item.http_code,
                    attempt_status: Some(FlowStatus::Payment(status)),
                    connector_transaction_id: Some(auth_reversal_response.cnp_txn_id.clone()),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                    typed_connector_response: None,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                };

                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status,
                        ..item.router_data.resource_common_data
                    },
                    response: Err(error_response),
                    ..item.router_data
                })
            } else {
                let payments_response = PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(
                        auth_reversal_response.cnp_txn_id.clone(),
                    ),
                    redirection_data: None,
                    mandate_reference: None,
                    connector_metadata: None,
                    network_txn_id: None,
                    network_txn_link_id: None,
                    connector_response_reference_id: Some(auth_reversal_response.id.clone()),
                    incremental_authorization_allowed: None,
                    status_code: item.http_code,
                    splits: None,
                    payment_account_reference: None,
                };

                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status,
                        ..item.router_data.resource_common_data
                    },
                    response: Ok(payments_response),
                    ..item.router_data
                })
            }
        } else if let Some(void_response) = item.response.void_response {
            // Fallback to void_response for compatibility
            let status =
                get_attempt_status(WorldpayvantivPaymentFlow::Void, void_response.response)?;

            if is_payment_failure(status) {
                let error_response = ErrorResponse {
                    code: void_response.response.to_string(),
                    message: void_response.message.clone(),
                    reason: Some(void_response.message.clone()),
                    status_code: item.http_code,
                    attempt_status: Some(FlowStatus::Payment(status)),
                    connector_transaction_id: Some(void_response.cnp_txn_id.clone()),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                    typed_connector_response: None,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                };

                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status,
                        ..item.router_data.resource_common_data
                    },
                    response: Err(error_response),
                    ..item.router_data
                })
            } else {
                let payments_response = PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(
                        void_response.cnp_txn_id.clone(),
                    ),
                    redirection_data: None,
                    mandate_reference: None,
                    connector_metadata: None,
                    network_txn_id: None,
                    network_txn_link_id: None,
                    connector_response_reference_id: Some(void_response.id.clone()),
                    incremental_authorization_allowed: None,
                    status_code: item.http_code,
                    splits: None,
                    payment_account_reference: None,
                };

                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status,
                        ..item.router_data.resource_common_data
                    },
                    response: Ok(payments_response),
                    ..item.router_data
                })
            }
        } else {
            let error_response = ErrorResponse {
                code: item.response.response_code,
                message: item.response.message.clone(),
                reason: Some(item.response.message.clone()),
                status_code: item.http_code,
                attempt_status: Some(FlowStatus::Payment(common_enums::AttemptStatus::VoidFailed)),
                connector_transaction_id: None,
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
            };

            Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: common_enums::AttemptStatus::VoidFailed,
                    ..item.router_data.resource_common_data
                },
                response: Err(error_response),
                ..item.router_data
            })
        }
    }
}

// TryFrom for VoidPC (VoidPostCapture) response
impl TryFrom<ResponseRouterData<CnpOnlineResponse, Self>>
    for RouterDataV2<VoidPC, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<CnpOnlineResponse, Self>) -> Result<Self, Self::Error> {
        if let Some(void_response) = item.response.void_response {
            let post_capture_void_status = get_post_capture_void_status(void_response.response);

            if post_capture_void_status.is_post_capture_void_failure() {
                let error_response = ErrorResponse {
                    code: void_response.response.to_string(),
                    message: void_response.message.clone(),
                    reason: Some(void_response.message.clone()),
                    status_code: item.http_code,
                    attempt_status: None,
                    connector_transaction_id: Some(void_response.cnp_txn_id.clone()),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                    typed_connector_response: None,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                };

                Ok(Self {
                    response: Err(error_response),
                    ..item.router_data
                })
            } else {
                let payments_response = PaymentsResponseData::PostCaptureVoidResponse {
                    post_capture_void_status,
                    connector_reference_id: Some(void_response.cnp_txn_id.clone()),
                    status_code: item.http_code,
                };

                Ok(Self {
                    response: Ok(payments_response),
                    ..item.router_data
                })
            }
        } else {
            let error_response = ErrorResponse {
                code: item.response.response_code,
                message: item.response.message.clone(),
                reason: Some(item.response.message.clone()),
                status_code: item.http_code,
                attempt_status: Some(FlowStatus::Payment(common_enums::AttemptStatus::VoidFailed)),
                connector_transaction_id: None,
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
            };

            Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: common_enums::AttemptStatus::VoidFailed,
                    ..item.router_data.resource_common_data
                },
                response: Err(error_response),
                ..item.router_data
            })
        }
    }
}

// TryFrom for IncrementalAuthorization request
// Worldpay Vantiv incremental auth sends a new authorization referencing the
// original transaction via originalNetworkTransactionId with the updated total amount.
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        WorldpayvantivRouterData<
            RouterDataV2<
                IncrementalAuthorization,
                PaymentFlowData,
                PaymentsIncrementalAuthorizationData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for WorldpayvantivPaymentsRequest<T>
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: WorldpayvantivRouterData<
            RouterDataV2<
                IncrementalAuthorization,
                PaymentFlowData,
                PaymentsIncrementalAuthorizationData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = WorldpayvantivAuthType::try_from(&item.router_data.connector_config)?;
        let merchant_id = auth.merchant_id_for_currency(Some(item.router_data.request.currency))?;

        let authentication = Authentication {
            user: auth.user,
            password: auth.password,
        };

        let connector_transaction_id = item
            .router_data
            .request
            .connector_transaction_id
            .get_connector_transaction_id()
            .change_context(IntegrationError::MissingConnectorTransactionID {
                context: IntegrationErrorContext {
                    additional_context: Some("Incremental authorization requires the cnpTxnId returned by Worldpay Vantiv for the original authorization.".to_owned()),
                    ..Default::default()
                },
            })?;

        let merchant_txn_id = item
            .router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();

        let report_group = extract_report_group(&item.router_data.connector_config)
            .unwrap_or_else(|| "rtpGrp".to_string());

        // Vantiv CNP schema requires id attribute to be <= 36 characters.
        // Prefer a short prefix + merchant_txn_id (truncate as needed).
        let id_prefix = "IA_";
        let max_txn_id_len = 36usize.saturating_sub(id_prefix.len());
        let truncated_txn = if merchant_txn_id.len() > max_txn_id_len {
            merchant_txn_id[..max_txn_id_len].to_string()
        } else {
            merchant_txn_id.clone()
        };
        // Worldpay Vantiv Incremental Auth schema (12.30+):
        // Requires only cnpTxnId, amount, and authIndicator=Incremental.
        // The `amount` is the INCREMENT (delta) to add, not the new total.
        let incremental_auth = Authorization {
            id: format!("{}{}", id_prefix, truncated_txn),
            report_group,
            customer_id: None,
            cnp_txn_id: Some(connector_transaction_id),
            order_id: None,
            amount: item.router_data.request.money.amount(),
            order_source: None,
            bill_to_address: None,
            ship_to_address: None,
            payment_info: None,
            enhanced_data: None,
            processing_instructions: None,
            cardholder_authentication: None,
            auth_indicator: Some(VantivAuthIndicator::Incremental),
        };

        let cnp_request = CnpOnlineRequest {
            version: worldpayvantiv_constants::WORLDPAYVANTIV_VERSION.to_string(),
            xmlns: worldpayvantiv_constants::XMLNS.to_string(),
            merchant_id,
            authentication,
            authorization: Some(incremental_auth),
            sale: None,
            capture: None,
            auth_reversal: None,
            void: None,
            credit: None,
        };

        Ok(Self { cnp_request })
    }
}

// TryFrom for IncrementalAuthorization response
// Re-uses the CnpOnlineResponse which contains an authorization_response field.
// Unlike regular Authorize, this returns PaymentsResponseData::IncrementalAuthorizationResponse.
impl TryFrom<ResponseRouterData<CnpOnlineResponse, Self>>
    for RouterDataV2<
        IncrementalAuthorization,
        PaymentFlowData,
        PaymentsIncrementalAuthorizationData,
        PaymentsResponseData,
    >
{
    type Error = Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<CnpOnlineResponse, Self>) -> Result<Self, Self::Error> {
        if let Some(auth_response) = item.response.authorization_response {
            let attempt_status =
                get_attempt_status(WorldpayvantivPaymentFlow::Auth, auth_response.response)?;

            if is_payment_failure(attempt_status) {
                let error_response = ErrorResponse {
                    code: auth_response.response.to_string(),
                    message: auth_response.message.clone(),
                    reason: Some(auth_response.message.clone()),
                    status_code: item.http_code,
                    attempt_status: Some(FlowStatus::Payment(attempt_status)),
                    connector_transaction_id: Some(auth_response.cnp_txn_id.clone()),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                    typed_connector_response: None,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                };

                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status: attempt_status,
                        ..item.router_data.resource_common_data
                    },
                    response: Err(error_response),
                    ..item.router_data
                })
            } else {
                let payments_response = PaymentsResponseData::IncrementalAuthorizationResponse {
                    connector_metadata: None,
                    status: common_enums::AuthorizationStatus::Success,
                    connector_authorization_id: Some(auth_response.cnp_txn_id.clone()),
                    status_code: item.http_code,
                };

                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status: attempt_status,
                        ..item.router_data.resource_common_data
                    },
                    response: Ok(payments_response),
                    ..item.router_data
                })
            }
        } else {
            let error_response = ErrorResponse {
                code: item.response.response_code,
                message: item.response.message.clone(),
                reason: Some(item.response.message.clone()),
                status_code: item.http_code,
                attempt_status: Some(FlowStatus::Payment(
                    common_enums::AttemptStatus::AuthorizationFailed,
                )),
                connector_transaction_id: None,
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
            };

            Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: common_enums::AttemptStatus::AuthorizationFailed,
                    ..item.router_data.resource_common_data
                },
                response: Err(error_response),
                ..item.router_data
            })
        }
    }
}

// Status mapping functions
fn get_attempt_status(
    flow: WorldpayvantivPaymentFlow,
    response: WorldpayvantivResponseCode,
) -> Result<common_enums::AttemptStatus, Report<ConnectorError>> {
    use common_enums::AttemptStatus;
    Ok(if response.is_approved() {
        match flow {
            WorldpayvantivPaymentFlow::Sale | WorldpayvantivPaymentFlow::Capture => {
                AttemptStatus::Charged
            }
            WorldpayvantivPaymentFlow::Auth => AttemptStatus::Authorized,
            WorldpayvantivPaymentFlow::Void => AttemptStatus::Voided,
            WorldpayvantivPaymentFlow::VoidPC => AttemptStatus::VoidedPostCapture,
        }
    } else {
        match flow {
            WorldpayvantivPaymentFlow::Sale => AttemptStatus::Failure,
            WorldpayvantivPaymentFlow::Auth => AttemptStatus::AuthorizationFailed,
            WorldpayvantivPaymentFlow::Capture => AttemptStatus::CaptureFailed,
            WorldpayvantivPaymentFlow::Void | WorldpayvantivPaymentFlow::VoidPC => {
                AttemptStatus::VoidFailed
            }
        }
    })
}

fn is_payment_failure(status: common_enums::AttemptStatus) -> bool {
    matches!(
        status,
        common_enums::AttemptStatus::Failure
            | common_enums::AttemptStatus::AuthorizationFailed
            | common_enums::AttemptStatus::CaptureFailed
            | common_enums::AttemptStatus::VoidFailed
    )
}

fn get_post_capture_void_status(
    response: WorldpayvantivResponseCode,
) -> common_enums::PostCaptureVoidStatus {
    if response.is_approved() {
        common_enums::PostCaptureVoidStatus::Succeeded
    } else {
        common_enums::PostCaptureVoidStatus::Failed
    }
}

fn get_refund_status(
    response: WorldpayvantivResponseCode,
) -> Result<common_enums::RefundStatus, Report<ConnectorError>> {
    match response {
        WorldpayvantivResponseCode::Approved
        | WorldpayvantivResponseCode::PartiallyApproved
        | WorldpayvantivResponseCode::OfflineApproval
        | WorldpayvantivResponseCode::OfflineApprovalUnableToGoOnline => {
            Ok(common_enums::RefundStatus::Success)
        }
        WorldpayvantivResponseCode::TransactionReceived => Ok(common_enums::RefundStatus::Pending),
        WorldpayvantivResponseCode::ProcessingNetworkUnavailable
        | WorldpayvantivResponseCode::IssuerUnavailable
        | WorldpayvantivResponseCode::ReSubmitTransaction
        | WorldpayvantivResponseCode::MerchantNotConfiguredForProcessingAtThisSite
        | WorldpayvantivResponseCode::TryAgainLater
        | WorldpayvantivResponseCode::InsufficientFunds
        | WorldpayvantivResponseCode::AuthorizationAmountHasAlreadyBeenDepleted
        | WorldpayvantivResponseCode::InsufficientFundsRetryAfter1Hour
        | WorldpayvantivResponseCode::InsufficientFundsRetryAfter24Hour
        | WorldpayvantivResponseCode::InsufficientFundsRetryAfter2Days
        | WorldpayvantivResponseCode::InsufficientFundsRetryAfter4Days
        | WorldpayvantivResponseCode::InsufficientFundsRetryAfter6Days
        | WorldpayvantivResponseCode::InsufficientFundsRetryAfter8Days
        | WorldpayvantivResponseCode::InsufficientFundsRetryAfter10Days
        | WorldpayvantivResponseCode::CallIssuer
        | WorldpayvantivResponseCode::CallAmex
        | WorldpayvantivResponseCode::CallDinersClub
        | WorldpayvantivResponseCode::CallDiscover
        | WorldpayvantivResponseCode::CallJbs
        | WorldpayvantivResponseCode::CallVisaMastercard
        | WorldpayvantivResponseCode::ExceedsApprovalAmountLimit
        | WorldpayvantivResponseCode::CallIndicatedNumber
        | WorldpayvantivResponseCode::ConsumerMultiUseVirtualCardNumberSoftDecline
        | WorldpayvantivResponseCode::ConsumerNonReloadablePrepaidCardSoftDecline
        | WorldpayvantivResponseCode::ConsumerSingleUseVirtualCardNumberSoftDecline
        | WorldpayvantivResponseCode::MerchantDoesntQualifyForProductCode
        | WorldpayvantivResponseCode::Lifecycle
        | WorldpayvantivResponseCode::Policy
        | WorldpayvantivResponseCode::InvalidOrExpiredCardContactCardholderToUpdate
        | WorldpayvantivResponseCode::InvalidTransactionOrCardRestrictionVerifyInformationAndResubmit
        | WorldpayvantivResponseCode::AtLeastOneOfOrigIdOrOrigCnpTxnIdIsRequired
        | WorldpayvantivResponseCode::OrigCnpTxnIdIsRequiredWhenShowStatusOnlyIsUsed
        | WorldpayvantivResponseCode::TransactionMustReferenceTheEstimatedAuth
        | WorldpayvantivResponseCode::IncrementedAuthExceedsMaxTransactionAmount
        | WorldpayvantivResponseCode::SubmittedMccNotAllowed
        | WorldpayvantivResponseCode::MerchantNotCertifiedEnabledForIias
        | WorldpayvantivResponseCode::IssuerGeneratedError
        | WorldpayvantivResponseCode::InvalidAmountHardDecline
        | WorldpayvantivResponseCode::ReversalUnsuccessful
        | WorldpayvantivResponseCode::MissingData
        | WorldpayvantivResponseCode::InvalidDeactivate
        | WorldpayvantivResponseCode::OverMaxBalance
        | WorldpayvantivResponseCode::InvalidActivate
        | WorldpayvantivResponseCode::NoTransactionFoundForReversal
        | WorldpayvantivResponseCode::IllegalTransaction
        | WorldpayvantivResponseCode::DuplicateTransaction
        | WorldpayvantivResponseCode::SystemError
        | WorldpayvantivResponseCode::MerchantDepleted
        | WorldpayvantivResponseCode::InvalidReversalTypeForCreditCardTransaction
        | WorldpayvantivResponseCode::SystemErrorMessageFormatError
        | WorldpayvantivResponseCode::SystemErrorCannotProcess
        | WorldpayvantivResponseCode::RefundRejectedDueToPendingDepositStatus
        | WorldpayvantivResponseCode::RefundRejectedDueToDeclinedDepositStatus
        | WorldpayvantivResponseCode::RefundRejectedByTheProcessingNetwork
        | WorldpayvantivResponseCode::CaptureCreditAndAuthReversalTagsCannotBeUsedForGiftCardTransactions
        | WorldpayvantivResponseCode::InvalidAccountNumber
        | WorldpayvantivResponseCode::AuthorizationHasExpiredNoNeedToReverse
        | WorldpayvantivResponseCode::InvalidTrackData
        | WorldpayvantivResponseCode::DepositIsAlreadyReferencedByAChargeback
        | WorldpayvantivResponseCode::AutomaticRefundAlreadyIssued
        | WorldpayvantivResponseCode::InvalidMerchant
        | WorldpayvantivResponseCode::InvalidTransaction
        | WorldpayvantivResponseCode::TransactionNotAllowedAtTerminal
        | WorldpayvantivResponseCode::ThisMethodOfPaymentDoesNotSupportAuthorizationReversals
        | WorldpayvantivResponseCode::ReversalAmountDoesNotMatchAuthorizationAmount
        | WorldpayvantivResponseCode::InvalidAmountSoftDecline
        | WorldpayvantivResponseCode::InvalidReportGroup
        | WorldpayvantivResponseCode::DoNotHonor
        | WorldpayvantivResponseCode::GenericDecline
        | WorldpayvantivResponseCode::DeclineRequestPositiveId
        | WorldpayvantivResponseCode::ThreeDSecureTransactionNotSupportedByMerchant
        | WorldpayvantivResponseCode::RestrictedByVantivDueToSecurityCodeMismatch
        | WorldpayvantivResponseCode::NoTransactionFoundWithSpecifiedTransactionId
        | WorldpayvantivResponseCode::AuthorizationNoLongerAvailable
        | WorldpayvantivResponseCode::TransactionNotVoidedAlreadySettled
        | WorldpayvantivResponseCode::AutoVoidOnRefund
        | WorldpayvantivResponseCode::InvalidAccountNumberOriginalOrNocUpdatedECheckAccountRequired
        | WorldpayvantivResponseCode::TotalCreditAmountExceedsCaptureAmount
        | WorldpayvantivResponseCode::ExceedTheThresholdForSendingRedeposits
        | WorldpayvantivResponseCode::DepositHasNotBeenReturnedForInsufficientNonSufficientFunds
        | WorldpayvantivResponseCode::RedepositAgainstInvalidTransactionType
        | WorldpayvantivResponseCode::InternalSystemErrorCallVantiv
        | WorldpayvantivResponseCode::OriginalTransactionHasBeenProcessedFutureRedepositsCanceled
        | WorldpayvantivResponseCode::SoftDeclineAutoRecyclingInProgress
        | WorldpayvantivResponseCode::HardDeclineAutoRecyclingComplete
        | WorldpayvantivResponseCode::TransactionDeclinedByTheProcessingNetwork
        | WorldpayvantivResponseCode::SecondaryAmountCannotExceedTheSaleAmount
        | WorldpayvantivResponseCode::ThisMethodOfPaymentDoesNotSupportSecondaryAmount
        | WorldpayvantivResponseCode::SecondaryAmountCannotBeLessThanZero
        | WorldpayvantivResponseCode::PartialTransactionIsNotSupportedWhenIncludingASecondaryAmount
        | WorldpayvantivResponseCode::SecondaryAmountRequiredOnPartialRefundWhenUsedOnDeposit
        | WorldpayvantivResponseCode::SecondaryAmountNotAllowedOnRefundIfNotIncludedOnDeposit
        | WorldpayvantivResponseCode::ProcessingNetworkError
        | WorldpayvantivResponseCode::InvalidEMail
        | WorldpayvantivResponseCode::InvalidCombinationOfAccountFundingTransactionTypeAndMcc
        | WorldpayvantivResponseCode::InvalidAccountFundingTransactionTypeForThisMethodOfPayment
        | WorldpayvantivResponseCode::MissingOneOrMoreReceiverFieldsForAccountFundingTransaction
        | WorldpayvantivResponseCode::SoftDeclineDecryptionFailed
        | WorldpayvantivResponseCode::HardDeclineInputDataIsInvalid
        | WorldpayvantivResponseCode::HardDeclineDecryptionFailed
        | WorldpayvantivResponseCode::SuspectedFraud
        | WorldpayvantivResponseCode::SystemErrorContactWorldpayRepresentative
        | WorldpayvantivResponseCode::InvalidTerminalId
        | WorldpayvantivResponseCode::DeclineReTryTransaction
        | WorldpayvantivResponseCode::RemoteFunctionUnknown
        | WorldpayvantivResponseCode::InvalidData
        | WorldpayvantivResponseCode::InvalidDataDataElementsMissing
        | WorldpayvantivResponseCode::InvalidDataDataFormatError
        | WorldpayvantivResponseCode::VerifyBillingAddress
        | WorldpayvantivResponseCode::InactiveAccount
        | WorldpayvantivResponseCode::InvalidAuth
        | WorldpayvantivResponseCode::CheckoutIdWasInvalid
        | WorldpayvantivResponseCode::CheckoutIdWasNotFound
        | WorldpayvantivResponseCode::TransactionTypeIsNotSupportedWithThisMethodOfPayment
        | WorldpayvantivResponseCode::UnreferencedOrphanRefundsAreNotAllowed
        | WorldpayvantivResponseCode::ThisFundingInstructionResultsInANegativeAccountBalance
        | WorldpayvantivResponseCode::ProcessingError
        | WorldpayvantivResponseCode::ThisFundingInstructionTypeIsInvalidForCanadianMerchants
        | WorldpayvantivResponseCode::ThisFundingInstructionTypeIsInvalid
        | WorldpayvantivResponseCode::AbsoluteDecline
        | WorldpayvantivResponseCode::TheMerchantProfileDoesNotAllowTheRequestedOperation
        | WorldpayvantivResponseCode::AmountGreaterThanLimitSpecifiedInTheMerchantProfile
        | WorldpayvantivResponseCode::AccountNotEligibleForRtp
        | WorldpayvantivResponseCode::NoValidParentDepositOrParentRefundFound
        | WorldpayvantivResponseCode::FundingInstructionHeldPleaseContactYourRelationshipManager
        | WorldpayvantivResponseCode::InvalidCreditAccount => Ok(common_enums::RefundStatus::Failure),
        _ => Err(Report::new(ConnectorError::UnexpectedResponseError {
            context: ResponseTransformationErrorContext {
                http_status_code: None,
                additional_context: Some("Invalid response code for refund".to_string()),
            },
        })),
    }
}
