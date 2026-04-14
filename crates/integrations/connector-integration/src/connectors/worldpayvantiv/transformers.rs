use std::borrow::Cow;

use common_enums::{self, CountryAlpha2, Currency};
use common_utils::{id_type::CustomerId, types::MinorUnit, StringMajorUnit};
use domain_types::{
    connector_flow::{Authorize, Capture, PSync, RSync, Refund, RepeatPayment, SetupMandate, Void, VoidPC},
    connector_types::{
        MandateReference, MandateReferenceId, PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData,
        PaymentsCancelPostCaptureData, PaymentsCaptureData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData, RepeatPaymentData,
        ResponseId, SetupMandateRequestData,
    },
    errors::{ConnectorError, IntegrationError},
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes, RawCardNumber, WalletData},
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    ResponseTransformationErrorContext,
};
use error_stack::{Report, ResultExt};
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use crate::{connectors::worldpayvantiv::WorldpayvantivRouterData, types::ResponseRouterData};

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

// Serialize implementation
impl<T: PaymentMethodDataTypes + Serialize> Serialize for WorldpayvantivPaymentsRequest<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let full_xml =
            crate::utils::serialize_to_xml_string_with_root("cnpOnlineRequest", &self.cnp_request)
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
    > for WorldpayvantivPaymentsRequest<T>
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
        let auth = WorldpayvantivAuthType::try_from(&item.router_data.connector_config)?;

        let authentication = Authentication {
            user: auth.user,
            password: auth.password,
        };

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
                    context: Default::default(),
                }
                .into());
            }
        };

        let merchant_txn_id = get_valid_transaction_id(
            item.router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            "transaction_id",
        )?;
        let amount = item.router_data.request.minor_amount;

        // Extract report group from metadata or use default
        let report_group = extract_report_group(&item.router_data.connector_config)
            .unwrap_or_else(|| "rtpGrp".to_string());

        let bill_to_address = get_billing_address(&item.router_data.resource_common_data);
        let ship_to_address = get_shipping_address(&item.router_data.resource_common_data);

        let (authorization, sale) =
            if item.router_data.request.is_auto_capture() && amount != MinorUnit::zero() {
                let sale = Sale {
                    id: format!("{}_{}", OperationId::Sale, merchant_txn_id),
                    report_group: report_group.clone(),
                    customer_id: extract_customer_id(
                        &item.router_data.resource_common_data.customer_id,
                    )
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
                    processing_type: None,
                    original_network_transaction_id: None,
                };
                (None, Some(sale))
            } else {
                let authorization = Authorization {
                    id: format!("{}_{}", OperationId::Auth, merchant_txn_id),
                    report_group: report_group.clone(),
                    customer_id: extract_customer_id(
                        &item.router_data.resource_common_data.customer_id,
                    )
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
                (Some(authorization), None)
            };

        let cnp_request = CnpOnlineRequest {
            version: worldpayvantiv_constants::WORLDPAYVANTIV_VERSION.to_string(),
            xmlns: worldpayvantiv_constants::XMLNS.to_string(),
            merchant_id: auth.merchant_id,
            authentication,
            authorization,
            sale,
            capture: None,
            auth_reversal: None,
            void: None,
            credit: None,
        };

        Ok(Self { cnp_request })
    }
}

pub(super) mod worldpayvantiv_constants {
    pub const WORLDPAYVANTIV_VERSION: &str = "12.23";
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
    pub merchant_id: Secret<String>,
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
                report_group,
                merchant_config_currency,
                ..
            } => Ok(Self {
                user: user.to_owned(),
                password: password.to_owned(),
                merchant_id: merchant_id.to_owned(),
                report_group: report_group.clone(),
                merchant_config_currency: merchant_config_currency.clone(),
            }),
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            }
            .into()),
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub processing_type: Option<VantivProcessingType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_network_transaction_id: Option<Secret<String>>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_transaction_id: Option<Secret<String>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenData {
    pub token: TokenizationData,
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

#[derive(Debug, Clone, Serialize)]
pub enum VantivProcessingType {
    #[serde(rename = "initialCOF")]
    InitialCOF,
    #[serde(rename = "merchantInitiatedCOF")]
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
                context: Default::default(),
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
                context: Default::default(),
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

// Response codes (comprehensive list)
#[derive(Debug, strum::Display, Serialize, Deserialize, PartialEq, Clone, Copy)]
pub enum WorldpayvantivResponseCode {
    #[serde(rename = "000")]
    Approved,
    #[serde(rename = "001")]
    TransactionReceived,
    #[serde(rename = "010")]
    PartiallyApproved,
    #[serde(rename = "110")]
    InsufficientFunds,
    #[serde(rename = "120")]
    CallIssuer,
    #[serde(rename = "121")]
    ExceedsApprovalAmountLimit,
    #[serde(rename = "123")]
    ExceedsActivityAmountLimit,
    #[serde(rename = "125")]
    InvalidEffectiveDate,
    #[serde(rename = "301")]
    InvalidAccountNumber,
    #[serde(rename = "302")]
    AccountNumberDoesNotMatchPaymentType,
    #[serde(rename = "303")]
    InvalidExpirationDate,
    #[serde(rename = "304")]
    InvalidCVV,
    #[serde(rename = "305")]
    InvalidCardValidationNum,
    #[serde(rename = "306")]
    ExpiredCard,
    #[serde(rename = "307")]
    InvalidPin,
    #[serde(rename = "308")]
    InvalidTransactionType,
    #[serde(rename = "310")]
    AccountNumberNotOnFile,
    #[serde(rename = "311")]
    AccountNumberLocked,
    #[serde(rename = "320")]
    InvalidLocation,
    #[serde(rename = "321")]
    InvalidMerchantId,
    #[serde(rename = "322")]
    InvalidLocation2,
    #[serde(rename = "323")]
    InvalidMerchantClassCode,
    #[serde(rename = "324")]
    InvalidExpirationDate2,
    #[serde(rename = "325")]
    InvalidData,
    #[serde(rename = "326")]
    InvalidPin2,
    #[serde(rename = "327")]
    ExceedsNumberofPINEntryTries,
    #[serde(rename = "328")]
    InvalidCryptoBox,
    #[serde(rename = "329")]
    InvalidRequestFormat,
    #[serde(rename = "330")]
    InvalidApplicationData,
    #[serde(rename = "340")]
    InvalidMerchantCategoryCode,
    #[serde(rename = "346")]
    TransactionCannotBeCompleted,
    #[serde(rename = "347")]
    TransactionTypeNotSupportedForCard,
    #[serde(rename = "349")]
    TransactionTypeNotAllowedAtTerminal,
    #[serde(rename = "350")]
    GenericDecline,
    #[serde(rename = "351")]
    DeclineByCard,
    #[serde(rename = "352")]
    DoNotHonor,
    #[serde(rename = "353")]
    InvalidMerchant,
    #[serde(rename = "354")]
    PickUpCard,
    #[serde(rename = "355")]
    CardOk,
    #[serde(rename = "356")]
    CallVoiceOperator,
    #[serde(rename = "357")]
    StopRecurring,
    #[serde(rename = "358")]
    NoChecking,
    #[serde(rename = "359")]
    NoCreditAccount,
    #[serde(rename = "360")]
    NoCreditAccountType,
    #[serde(rename = "361")]
    InvalidCreditPlan,
    #[serde(rename = "362")]
    InvalidTransactionCode,
    #[serde(rename = "363")]
    TransactionNotPermittedToCardholderAccount,
    #[serde(rename = "364")]
    TransactionNotPermittedToMerchant,
    #[serde(rename = "365")]
    PINTryExceeded,
    #[serde(rename = "366")]
    SecurityViolation,
    #[serde(rename = "367")]
    HardCapturePickUpCard,
    #[serde(rename = "368")]
    ResponseReceivedTooLate,
    #[serde(rename = "370")]
    SoftDecline,
    #[serde(rename = "400")]
    ContactCardIssuer,
    #[serde(rename = "401")]
    CallVoiceCenter,
    #[serde(rename = "402")]
    InvalidMerchantTerminal,
    #[serde(rename = "410")]
    InvalidAmount,
    #[serde(rename = "411")]
    ResubmitTransaction,
    #[serde(rename = "412")]
    InvalidTransaction,
    #[serde(rename = "413")]
    MerchantNotFound,
    #[serde(rename = "501")]
    PickUpCard2,
    #[serde(rename = "502")]
    ExpiredCard2,
    #[serde(rename = "503")]
    SuspectedFraud,
    #[serde(rename = "504")]
    ContactCardIssuer2,
    #[serde(rename = "505")]
    DoNotHonor2,
    #[serde(rename = "506")]
    InvalidMerchant2,
    #[serde(rename = "507")]
    InsufficientFunds2,
    #[serde(rename = "508")]
    AccountNumberNotOnFile2,
    #[serde(rename = "509")]
    InvalidAmount2,
    #[serde(rename = "510")]
    InvalidCardNumber,
    #[serde(rename = "511")]
    InvalidExpirationDate3,
    #[serde(rename = "512")]
    InvalidCVV2,
    #[serde(rename = "513")]
    InvalidCardValidationNum2,
    #[serde(rename = "514")]
    InvalidPin3,
    #[serde(rename = "515")]
    CardRestricted,
    #[serde(rename = "516")]
    OverCreditLimit,
    #[serde(rename = "517")]
    AccountClosed,
    #[serde(rename = "518")]
    AccountFrozen,
    #[serde(rename = "519")]
    InvalidTransactionType2,
    #[serde(rename = "520")]
    InvalidMerchantId2,
    #[serde(rename = "521")]
    ProcessorNotAvailable,
    #[serde(rename = "522")]
    NetworkTimeOut,
    #[serde(rename = "523")]
    SystemError,
    #[serde(rename = "524")]
    DuplicateTransaction,
    #[serde(rename = "601")]
    OfflineApproval,
    #[serde(rename = "602")]
    VoiceAuthRequired,
    #[serde(rename = "603")]
    AuthenticationRequired,
    #[serde(rename = "604")]
    SecurityCodeRequired,
    #[serde(rename = "605")]
    SecurityCodeNotMatch,
    #[serde(rename = "606")]
    ZipCodeNotMatch,
    #[serde(rename = "607")]
    AddressNotMatch,
    #[serde(rename = "608")]
    AVSFailure,
    #[serde(rename = "609")]
    CVVFailure,
    #[serde(rename = "610")]
    ServiceNotAllowed,
    #[serde(rename = "820")]
    CreditNotSupported,
    #[serde(rename = "821")]
    InvalidCreditAmount,
    #[serde(rename = "822")]
    CreditAmountExceedsDebitAmount,
    #[serde(rename = "823")]
    RefundNotSupported,
    #[serde(rename = "824")]
    InvalidRefundAmount,
    #[serde(rename = "825")]
    RefundAmountExceedsOriginalAmount,
    #[serde(rename = "826")]
    VoidNotSupported,
    #[serde(rename = "827")]
    VoidNotAllowed,
    #[serde(rename = "828")]
    CaptureNotSupported,
    #[serde(rename = "829")]
    CaptureNotAllowed,
    #[serde(rename = "830")]
    InvalidCaptureAmount,
    #[serde(rename = "831")]
    CaptureAmountExceedsAuthAmount,
    #[serde(rename = "832")]
    TransactionAlreadySettled,
    #[serde(rename = "833")]
    TransactionAlreadyVoided,
    #[serde(rename = "834")]
    TransactionAlreadyCaptured,
    #[serde(rename = "835")]
    TransactionNotFound,
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

// Step 90-93: TryFrom for Authorize response
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<CnpOnlineResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<CnpOnlineResponse, Self>) -> Result<Self, Self::Error> {
        match (
            item.response.sale_response.as_ref(),
            item.response.authorization_response.as_ref(),
        ) {
            (Some(sale_response), None) => {
                let status =
                    get_attempt_status(WorldpayvantivPaymentFlow::Sale, sale_response.response)?;

                if is_payment_failure(status) {
                    let error_response = ErrorResponse {
                        code: sale_response.response.to_string(),
                        message: sale_response.message.clone(),
                        reason: Some(sale_response.message.clone()),
                        status_code: item.http_code,
                        attempt_status: Some(status),
                        connector_transaction_id: Some(sale_response.cnp_txn_id.clone()),
                        network_decline_code: None,
                        network_advice_code: None,
                        network_error_message: None,
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
                            sale_response.cnp_txn_id.clone(),
                        ),
                        redirection_data: None,
                        mandate_reference: None,
                        connector_metadata: None,
                        network_txn_id: sale_response
                            .network_transaction_id
                            .clone()
                            .map(|id| id.expose()),
                        connector_response_reference_id: Some(sale_response.order_id.clone()),
                        incremental_authorization_allowed: None,
                        status_code: item.http_code,
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
            (None, Some(auth_response)) => {
                let status =
                    get_attempt_status(WorldpayvantivPaymentFlow::Auth, auth_response.response)?;

                if is_payment_failure(status) {
                    let error_response = ErrorResponse {
                        code: auth_response.response.to_string(),
                        message: auth_response.message.clone(),
                        reason: Some(auth_response.message.clone()),
                        status_code: item.http_code,
                        attempt_status: Some(status),
                        connector_transaction_id: Some(auth_response.cnp_txn_id.clone()),
                        network_decline_code: None,
                        network_advice_code: None,
                        network_error_message: None,
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
                            auth_response.cnp_txn_id.clone(),
                        ),
                        redirection_data: None,
                        mandate_reference: None,
                        connector_metadata: None,
                        network_txn_id: auth_response
                            .network_transaction_id
                            .clone()
                            .map(|id| id.expose()),
                        connector_response_reference_id: Some(auth_response.order_id.clone()),
                        incremental_authorization_allowed: None,
                        status_code: item.http_code,
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
            (None, None) => {
                let error_response = ErrorResponse {
                    code: item.response.response_code,
                    message: item.response.message.clone(),
                    reason: Some(item.response.message.clone()),
                    status_code: item.http_code,
                    attempt_status: Some(common_enums::AttemptStatus::Failure),
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                };

                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status: common_enums::AttemptStatus::Failure,
                        ..item.router_data.resource_common_data
                    },
                    response: Err(error_response),
                    ..item.router_data
                })
            }
            (_, _) => Err(Report::from(
                crate::utils::unexpected_response_fail(item.http_code, "worldpayvantiv: unexpected response for this operation; retry with idempotency keys and check connector status."),
            )
            .attach_printable(
                "Only one of 'sale_response' or 'authorization_response' is expected",
            )),
        }
    }
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
                            context: Default::default(),
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
                                    context: Default::default(),
                                })?;
                            let expiry_year = google_pay_decrypted_data
                                .get_four_digit_expiry_year()
                                .change_context(IntegrationError::InvalidDataFormat {
                                    field_name: "google_pay_decrypted_data.card_exp_year",
                                    context: Default::default(),
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
                                context: Default::default(),
                            }
                            .into())
                        }
                    }
                }
                _ => Err(IntegrationError::NotSupported {
                    message: "Wallet type".to_string(),
                    connector: "worldpayvantiv",
                    context: Default::default(),
                }
                .into()),
            }
        }
        _ => Err(IntegrationError::NotSupported {
            message: "Payment method".to_string(),
            connector: "worldpayvantiv",
            context: Default::default(),
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
            context: Default::default(),
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
            context: Default::default(),
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
            context: Default::default(),
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
            // Fallback to simple status mapping if no merchant_txn_id available
            match item.response.payment_status {
                PaymentStatus::ProcessedSuccessfully => common_enums::AttemptStatus::Charged,
                PaymentStatus::TransactionDeclined => common_enums::AttemptStatus::Failure,
                _ => item.router_data.resource_common_data.status,
            }
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
            connector_response_reference_id: item
                .response
                .payment_detail
                .as_ref()
                .and_then(|detail| detail.merchant_txn_id.clone()),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
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

        let authentication = Authentication {
            user: auth.user,
            password: auth.password,
        };

        let cnp_txn_id = item
            .router_data
            .request
            .get_connector_transaction_id()
            .change_context(IntegrationError::MissingConnectorTransactionID {
                context: Default::default(),
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
            amount: item.router_data.request.minor_amount_to_capture,
            enhanced_data: None,
        };

        let cnp_request = CnpOnlineRequest {
            version: worldpayvantiv_constants::WORLDPAYVANTIV_VERSION.to_string(),
            xmlns: worldpayvantiv_constants::XMLNS.to_string(),
            merchant_id: auth.merchant_id,
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
            merchant_id: auth.merchant_id,
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
            amount: item.router_data.request.minor_refund_amount,
        };

        let cnp_request = CnpOnlineRequest {
            version: worldpayvantiv_constants::WORLDPAYVANTIV_VERSION.to_string(),
            xmlns: worldpayvantiv_constants::XMLNS.to_string(),
            merchant_id: auth.merchant_id,
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
            merchant_id: auth.merchant_id,
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
            let status = match credit_response.response {
                WorldpayvantivResponseCode::Approved
                | WorldpayvantivResponseCode::TransactionReceived => {
                    common_enums::RefundStatus::Pending
                }
                _ => common_enums::RefundStatus::Failure,
            };

            let refunds_response = RefundsResponseData {
                connector_refund_id: credit_response.cnp_txn_id.clone(),
                refund_status: status,
                status_code: item.http_code,
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
                attempt_status: Some(common_enums::AttemptStatus::Failure),
                connector_transaction_id: None,
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
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
            _ => item.router_data.resource_common_data.status,
        };

        let refunds_response = RefundsResponseData {
            connector_refund_id: item
                .response
                .payment_id
                .map(|id| id.to_string())
                .unwrap_or_else(|| "unknown".to_string()),
            refund_status: status,
            status_code: item.http_code,
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

        let authentication = Authentication {
            user: auth.user,
            password: auth.password,
        };

        let cnp_txn_id = item
            .router_data
            .request
            .get_connector_transaction_id()
            .change_context(IntegrationError::MissingConnectorTransactionID {
                context: Default::default(),
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
            amount: item.router_data.request.minor_amount_to_capture,
            enhanced_data: None,
        };

        Ok(Self {
            version: worldpayvantiv_constants::WORLDPAYVANTIV_VERSION.to_string(),
            xmlns: worldpayvantiv_constants::XMLNS.to_string(),
            merchant_id: auth.merchant_id,
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
                    attempt_status: Some(status),
                    connector_transaction_id: Some(capture_response.cnp_txn_id.clone()),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
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
                    connector_response_reference_id: Some(capture_response.id.clone()),
                    incremental_authorization_allowed: None,
                    status_code: item.http_code,
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
                attempt_status: Some(common_enums::AttemptStatus::CaptureFailed),
                connector_transaction_id: None,
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
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
            merchant_id: auth.merchant_id,
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
                    attempt_status: Some(status),
                    connector_transaction_id: Some(auth_reversal_response.cnp_txn_id.clone()),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
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
                    connector_response_reference_id: Some(auth_reversal_response.id.clone()),
                    incremental_authorization_allowed: None,
                    status_code: item.http_code,
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
                    attempt_status: Some(status),
                    connector_transaction_id: Some(void_response.cnp_txn_id.clone()),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
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
                    connector_response_reference_id: Some(void_response.id.clone()),
                    incremental_authorization_allowed: None,
                    status_code: item.http_code,
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
                attempt_status: Some(common_enums::AttemptStatus::VoidFailed),
                connector_transaction_id: None,
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
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
            let status =
                get_attempt_status(WorldpayvantivPaymentFlow::VoidPC, void_response.response)?;

            if is_payment_failure(status) {
                let error_response = ErrorResponse {
                    code: void_response.response.to_string(),
                    message: void_response.message.clone(),
                    reason: Some(void_response.message.clone()),
                    status_code: item.http_code,
                    attempt_status: Some(status),
                    connector_transaction_id: Some(void_response.cnp_txn_id.clone()),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
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
                    connector_response_reference_id: Some(void_response.id.clone()),
                    incremental_authorization_allowed: None,
                    status_code: item.http_code,
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
                attempt_status: Some(common_enums::AttemptStatus::VoidFailed),
                connector_transaction_id: None,
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
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

// Status mapping functions
fn get_attempt_status(
    flow: WorldpayvantivPaymentFlow,
    response: WorldpayvantivResponseCode,
) -> Result<common_enums::AttemptStatus, Report<ConnectorError>> {
    match response {
        WorldpayvantivResponseCode::Approved
        | WorldpayvantivResponseCode::PartiallyApproved
        | WorldpayvantivResponseCode::OfflineApproval
        | WorldpayvantivResponseCode::TransactionReceived => match flow {
            WorldpayvantivPaymentFlow::Sale => Ok(common_enums::AttemptStatus::Pending),
            WorldpayvantivPaymentFlow::Auth => Ok(common_enums::AttemptStatus::Authorizing),
            WorldpayvantivPaymentFlow::Capture => Ok(common_enums::AttemptStatus::CaptureInitiated),
            WorldpayvantivPaymentFlow::Void => Ok(common_enums::AttemptStatus::VoidInitiated),
            WorldpayvantivPaymentFlow::VoidPC => {
                Ok(common_enums::AttemptStatus::VoidPostCaptureInitiated)
            }
        },
        // Decline codes - all other response codes not listed above
        WorldpayvantivResponseCode::InsufficientFunds
        | WorldpayvantivResponseCode::CallIssuer
        | WorldpayvantivResponseCode::ExceedsApprovalAmountLimit
        | WorldpayvantivResponseCode::ExceedsActivityAmountLimit
        | WorldpayvantivResponseCode::InvalidEffectiveDate
        | WorldpayvantivResponseCode::InvalidAccountNumber
        | WorldpayvantivResponseCode::AccountNumberDoesNotMatchPaymentType
        | WorldpayvantivResponseCode::InvalidExpirationDate
        | WorldpayvantivResponseCode::InvalidCVV
        | WorldpayvantivResponseCode::InvalidCardValidationNum
        | WorldpayvantivResponseCode::ExpiredCard
        | WorldpayvantivResponseCode::InvalidPin
        | WorldpayvantivResponseCode::InvalidTransactionType
        | WorldpayvantivResponseCode::AccountNumberNotOnFile
        | WorldpayvantivResponseCode::AccountNumberLocked
        | WorldpayvantivResponseCode::InvalidLocation
        | WorldpayvantivResponseCode::InvalidMerchantId
        | WorldpayvantivResponseCode::InvalidLocation2
        | WorldpayvantivResponseCode::InvalidMerchantClassCode
        | WorldpayvantivResponseCode::InvalidExpirationDate2
        | WorldpayvantivResponseCode::InvalidData
        | WorldpayvantivResponseCode::InvalidPin2
        | WorldpayvantivResponseCode::ExceedsNumberofPINEntryTries
        | WorldpayvantivResponseCode::InvalidCryptoBox
        | WorldpayvantivResponseCode::InvalidRequestFormat
        | WorldpayvantivResponseCode::InvalidApplicationData
        | WorldpayvantivResponseCode::InvalidMerchantCategoryCode
        | WorldpayvantivResponseCode::TransactionCannotBeCompleted
        | WorldpayvantivResponseCode::TransactionTypeNotSupportedForCard
        | WorldpayvantivResponseCode::TransactionTypeNotAllowedAtTerminal
        | WorldpayvantivResponseCode::GenericDecline
        | WorldpayvantivResponseCode::DeclineByCard
        | WorldpayvantivResponseCode::DoNotHonor
        | WorldpayvantivResponseCode::InvalidMerchant
        | WorldpayvantivResponseCode::PickUpCard
        | WorldpayvantivResponseCode::CardOk
        | WorldpayvantivResponseCode::CallVoiceOperator
        | WorldpayvantivResponseCode::StopRecurring
        | WorldpayvantivResponseCode::NoChecking
        | WorldpayvantivResponseCode::NoCreditAccount
        | WorldpayvantivResponseCode::NoCreditAccountType
        | WorldpayvantivResponseCode::InvalidCreditPlan
        | WorldpayvantivResponseCode::InvalidTransactionCode
        | WorldpayvantivResponseCode::TransactionNotPermittedToCardholderAccount
        | WorldpayvantivResponseCode::TransactionNotPermittedToMerchant
        | WorldpayvantivResponseCode::PINTryExceeded
        | WorldpayvantivResponseCode::SecurityViolation
        | WorldpayvantivResponseCode::HardCapturePickUpCard
        | WorldpayvantivResponseCode::ResponseReceivedTooLate
        | WorldpayvantivResponseCode::SoftDecline
        | WorldpayvantivResponseCode::ContactCardIssuer
        | WorldpayvantivResponseCode::CallVoiceCenter
        | WorldpayvantivResponseCode::InvalidMerchantTerminal
        | WorldpayvantivResponseCode::InvalidAmount
        | WorldpayvantivResponseCode::ResubmitTransaction
        | WorldpayvantivResponseCode::InvalidTransaction
        | WorldpayvantivResponseCode::MerchantNotFound
        | WorldpayvantivResponseCode::PickUpCard2
        | WorldpayvantivResponseCode::ExpiredCard2
        | WorldpayvantivResponseCode::SuspectedFraud
        | WorldpayvantivResponseCode::ContactCardIssuer2
        | WorldpayvantivResponseCode::DoNotHonor2
        | WorldpayvantivResponseCode::InvalidMerchant2
        | WorldpayvantivResponseCode::InsufficientFunds2
        | WorldpayvantivResponseCode::AccountNumberNotOnFile2
        | WorldpayvantivResponseCode::InvalidAmount2
        | WorldpayvantivResponseCode::InvalidCardNumber
        | WorldpayvantivResponseCode::InvalidExpirationDate3
        | WorldpayvantivResponseCode::InvalidCVV2
        | WorldpayvantivResponseCode::InvalidCardValidationNum2
        | WorldpayvantivResponseCode::InvalidPin3
        | WorldpayvantivResponseCode::CardRestricted
        | WorldpayvantivResponseCode::OverCreditLimit
        | WorldpayvantivResponseCode::AccountClosed
        | WorldpayvantivResponseCode::AccountFrozen
        | WorldpayvantivResponseCode::InvalidTransactionType2
        | WorldpayvantivResponseCode::InvalidMerchantId2
        | WorldpayvantivResponseCode::ProcessorNotAvailable
        | WorldpayvantivResponseCode::NetworkTimeOut
        | WorldpayvantivResponseCode::SystemError
        | WorldpayvantivResponseCode::DuplicateTransaction
        | WorldpayvantivResponseCode::VoiceAuthRequired
        | WorldpayvantivResponseCode::AuthenticationRequired
        | WorldpayvantivResponseCode::SecurityCodeRequired
        | WorldpayvantivResponseCode::SecurityCodeNotMatch
        | WorldpayvantivResponseCode::ZipCodeNotMatch
        | WorldpayvantivResponseCode::AddressNotMatch
        | WorldpayvantivResponseCode::AVSFailure
        | WorldpayvantivResponseCode::CVVFailure
        | WorldpayvantivResponseCode::ServiceNotAllowed
        | WorldpayvantivResponseCode::CreditNotSupported
        | WorldpayvantivResponseCode::InvalidCreditAmount
        | WorldpayvantivResponseCode::CreditAmountExceedsDebitAmount
        | WorldpayvantivResponseCode::RefundNotSupported
        | WorldpayvantivResponseCode::InvalidRefundAmount
        | WorldpayvantivResponseCode::RefundAmountExceedsOriginalAmount
        | WorldpayvantivResponseCode::VoidNotSupported
        | WorldpayvantivResponseCode::VoidNotAllowed
        | WorldpayvantivResponseCode::CaptureNotSupported
        | WorldpayvantivResponseCode::CaptureNotAllowed
        | WorldpayvantivResponseCode::InvalidCaptureAmount
        | WorldpayvantivResponseCode::CaptureAmountExceedsAuthAmount
        | WorldpayvantivResponseCode::TransactionAlreadySettled
        | WorldpayvantivResponseCode::TransactionAlreadyVoided
        | WorldpayvantivResponseCode::TransactionAlreadyCaptured
        | WorldpayvantivResponseCode::TransactionNotFound => match flow {
            WorldpayvantivPaymentFlow::Sale => Ok(common_enums::AttemptStatus::Failure),
            WorldpayvantivPaymentFlow::Auth => Ok(common_enums::AttemptStatus::AuthorizationFailed),
            WorldpayvantivPaymentFlow::Capture => Ok(common_enums::AttemptStatus::CaptureFailed),
            WorldpayvantivPaymentFlow::Void | WorldpayvantivPaymentFlow::VoidPC => {
                Ok(common_enums::AttemptStatus::VoidFailed)
            }
        },
    }
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

// Extract MMYY expDate string (Vantiv format) from SetupMandate payment method
// data. Returns None for non-card payment methods; RepeatPayment will then
// reject the request.
fn extract_exp_date_mmyy<T: PaymentMethodDataTypes>(
    pmd: &PaymentMethodData<T>,
) -> Option<String> {
    match pmd {
        PaymentMethodData::Card(card_data) => {
            let year_str = card_data.card_exp_year.peek();
            let formatted_year = if year_str.len() == 4 {
                &year_str[2..]
            } else {
                year_str.as_str()
            };
            Some(format!("{}{}", card_data.card_exp_month.peek(), formatted_year))
        }
        _ => None,
    }
}

// TryFrom for SetupMandate request — builds a zero-dollar Authorization
// that Vantiv automatically tokenizes, so the cnpToken returned in the
// tokenResponse element can be surfaced as connector_mandate_id for
// subsequent MIT / RepeatPayment flows.
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        WorldpayvantivRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
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
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = WorldpayvantivAuthType::try_from(&item.router_data.connector_config)?;

        let authentication = Authentication {
            user: auth.user,
            password: auth.password,
        };

        let payment_method_data = &item.router_data.request.payment_method_data;
        let order_source = OrderSource::from(payment_method_data.clone());

        let payment_info = match payment_method_data {
            PaymentMethodData::Card(card_data) => {
                let card_type = match card_data.card_network.clone() {
                    Some(network) => WorldpayvativCardType::try_from(network)?,
                    None => {
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
                    message: "Payment method for SetupMandate".to_string(),
                    connector: "worldpayvantiv",
                    context: Default::default(),
                }
                .into());
            }
        };

        // The SetupMandate `connector_request_reference_id` can be the
        // merchant_recurring_payment_id (UUID, 36 chars) which exceeds
        // Vantiv's 28-character cap on `<authorization id=...>`. Truncate
        // rather than failing — the id is only used for connector-side
        // tracing and the order_id keeps the full reference.
        let raw_merchant_txn_id = item
            .router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();
        let merchant_txn_id = if raw_merchant_txn_id.len()
            > worldpayvantiv_constants::MAX_PAYMENT_REFERENCE_ID_LENGTH
        {
            raw_merchant_txn_id
                .chars()
                .take(worldpayvantiv_constants::MAX_PAYMENT_REFERENCE_ID_LENGTH)
                .collect::<String>()
        } else {
            raw_merchant_txn_id
        };

        let report_group = extract_report_group(&item.router_data.connector_config)
            .unwrap_or_else(|| "rtpGrp".to_string());

        let bill_to_address = get_billing_address(&item.router_data.resource_common_data);
        let ship_to_address = get_shipping_address(&item.router_data.resource_common_data);

        // Zero-dollar Authorization: Vantiv returns tokenResponse automatically
        // when tokenization is enabled on the merchant account (standard for
        // mandate setup).
        let authorization = Authorization {
            id: format!("{}_{}", OperationId::Auth, merchant_txn_id),
            report_group,
            customer_id: extract_customer_id(&item.router_data.resource_common_data.customer_id)
                .map(Secret::new),
            order_id: merchant_txn_id,
            amount: MinorUnit::zero(),
            order_source,
            bill_to_address,
            ship_to_address,
            payment_info,
            enhanced_data: None,
            processing_instructions: None,
            cardholder_authentication: None,
        };

        let cnp_request = CnpOnlineRequest {
            version: worldpayvantiv_constants::WORLDPAYVANTIV_VERSION.to_string(),
            xmlns: worldpayvantiv_constants::XMLNS.to_string(),
            merchant_id: auth.merchant_id,
            authentication,
            authorization: Some(authorization),
            sale: None,
            capture: None,
            auth_reversal: None,
            void: None,
            credit: None,
        };

        Ok(Self { cnp_request })
    }
}

// TryFrom for SetupMandate response — extracts cnpToken from the
// authorization_response.tokenResponse element and surfaces it as
// connector_mandate_id on the MandateReference so downstream
// RepeatPayment flows can charge the saved card.
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<CnpOnlineResponse, Self>>
    for RouterDataV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    >
{
    type Error = Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<CnpOnlineResponse, Self>) -> Result<Self, Self::Error> {
        let auth_response = match item.response.authorization_response.as_ref() {
            Some(r) => r,
            None => {
                let error_response = ErrorResponse {
                    code: item.response.response_code,
                    message: item.response.message.clone(),
                    reason: Some(item.response.message.clone()),
                    status_code: item.http_code,
                    attempt_status: Some(common_enums::AttemptStatus::Failure),
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                };
                return Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status: common_enums::AttemptStatus::Failure,
                        ..item.router_data.resource_common_data
                    },
                    response: Err(error_response),
                    ..item.router_data
                });
            }
        };

        let status = get_attempt_status(WorldpayvantivPaymentFlow::Auth, auth_response.response)?;

        if is_payment_failure(status) {
            let error_response = ErrorResponse {
                code: auth_response.response.to_string(),
                message: auth_response.message.clone(),
                reason: Some(auth_response.message.clone()),
                status_code: item.http_code,
                attempt_status: Some(status),
                connector_transaction_id: Some(auth_response.cnp_txn_id.clone()),
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
            };
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    ..item.router_data.resource_common_data
                },
                response: Err(error_response),
                ..item.router_data
            });
        }

        // Extract cnpToken from tokenResponse; fall back to cnpTxnId
        // if the merchant account does not have tokenization enabled.
        // Vantiv requires an <expDate> alongside <cnpToken> on MIT sale requests,
        // so when the card details are available we pack them together as
        // "cnpToken|MMYY" into the connector_mandate_id. RepeatPayment splits
        // on `|` to recover both fields.
        let cnp_token = auth_response
            .token_response
            .as_ref()
            .map(|t| t.cnp_token.clone().expose())
            .unwrap_or_else(|| auth_response.cnp_txn_id.clone());

        let packed_exp_date = extract_exp_date_mmyy(&item.router_data.request.payment_method_data);
        let connector_mandate_id = match packed_exp_date.as_deref() {
            Some(exp) if !exp.is_empty() => format!("{cnp_token}|{exp}"),
            _ => cnp_token,
        };

        let network_txn_id = auth_response
            .network_transaction_id
            .clone()
            .map(|id| id.expose());

        // Surface NTI inside `connector_mandate_request_reference_id` so the
        // downstream RepeatPayment (MIT) flow can submit it as
        // <originalNetworkTransactionId> alongside the cnpToken. The same
        // value is also returned on `PaymentsResponseData.network_txn_id`
        // for protocol parity.
        let mandate_reference = Some(Box::new(MandateReference {
            connector_mandate_id: Some(connector_mandate_id),
            payment_method_id: None,
            connector_mandate_request_reference_id: network_txn_id.clone(),
        }));

        let payments_response = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(auth_response.cnp_txn_id.clone()),
            redirection_data: None,
            mandate_reference,
            connector_metadata: None,
            network_txn_id,
            connector_response_reference_id: Some(auth_response.order_id.clone()),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
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

// TryFrom for RepeatPayment (MIT) request — builds a cnpAPI <sale> carrying
// <token><cnpToken>..</cnpToken><expDate>MMYY</expDate></token>,
// <processingType>merchantInitiatedCOF</processingType>, and
// <originalNetworkTransactionId>..</originalNetworkTransactionId> so Vantiv
// treats it as a recurring/subsequent merchant-initiated charge against the
// saved card captured at SetupMandate.
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
    > for WorldpayvantivPaymentsRequest<T>
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
        let router_data = &item.router_data;
        let auth = WorldpayvantivAuthType::try_from(&router_data.connector_config)?;

        let authentication = Authentication {
            user: auth.user,
            password: auth.password,
        };

        // SetupMandate packed the cnpToken as "cnpToken|MMYY" and stashed NTI
        // under `connector_mandate_request_reference_id`. Recover both.
        let (cnp_token, exp_date, original_nti) = match &router_data.request.mandate_reference {
            MandateReferenceId::ConnectorMandateId(cm) => {
                let packed = cm.get_connector_mandate_id().ok_or_else(|| {
                    IntegrationError::MissingRequiredField {
                        field_name: "connector_mandate_id",
                        context: Default::default(),
                    }
                })?;
                let (token, exp) = match packed.split_once('|') {
                    Some((t, e)) => (t.to_string(), Some(e.to_string())),
                    None => (packed, None),
                };
                let nti = cm
                    .get_connector_mandate_request_reference_id()
                    .map(Secret::new);
                (token, exp, nti)
            }
            _ => {
                return Err(IntegrationError::NotSupported {
                    message: "Worldpayvantiv MIT requires ConnectorMandateId with cnpToken"
                        .to_string(),
                    connector: "worldpayvantiv",
                    context: Default::default(),
                }
                .into())
            }
        };

        let exp_date = exp_date.ok_or_else(|| IntegrationError::MissingRequiredField {
            field_name: "token.expDate",
            context: Default::default(),
        })?;

        let payment_info = PaymentInfo::Token(TokenData {
            token: TokenizationData {
                cnp_token: Secret::new(cnp_token),
                exp_date: Secret::new(exp_date),
            },
        });

        let raw_merchant_txn_id = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();
        let merchant_txn_id = if raw_merchant_txn_id.len()
            > worldpayvantiv_constants::MAX_PAYMENT_REFERENCE_ID_LENGTH
        {
            raw_merchant_txn_id
                .chars()
                .take(worldpayvantiv_constants::MAX_PAYMENT_REFERENCE_ID_LENGTH)
                .collect::<String>()
        } else {
            raw_merchant_txn_id
        };

        let report_group = extract_report_group(&router_data.connector_config)
            .unwrap_or_else(|| "rtpGrp".to_string());

        let bill_to_address = get_billing_address(&router_data.resource_common_data);
        let ship_to_address = get_shipping_address(&router_data.resource_common_data);

        let amount = router_data.request.minor_amount;

        let sale = Sale {
            id: format!("{}_{}", OperationId::Sale, merchant_txn_id),
            report_group,
            customer_id: extract_customer_id(&router_data.resource_common_data.customer_id)
                .map(Secret::new),
            order_id: merchant_txn_id,
            amount,
            order_source: OrderSource::Ecommerce,
            bill_to_address,
            ship_to_address,
            payment_info,
            enhanced_data: None,
            processing_instructions: None,
            cardholder_authentication: None,
            processing_type: Some(VantivProcessingType::MerchantInitiatedCOF),
            original_network_transaction_id: original_nti,
        };

        let cnp_request = CnpOnlineRequest {
            version: worldpayvantiv_constants::WORLDPAYVANTIV_VERSION.to_string(),
            xmlns: worldpayvantiv_constants::XMLNS.to_string(),
            merchant_id: auth.merchant_id,
            authentication,
            authorization: None,
            sale: Some(sale),
            capture: None,
            auth_reversal: None,
            void: None,
            credit: None,
        };

        Ok(Self { cnp_request })
    }
}

// TryFrom for RepeatPayment response — reuses <saleResponse> parsing.
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<CnpOnlineResponse, Self>>
    for RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<CnpOnlineResponse, Self>) -> Result<Self, Self::Error> {
        let sale_response = match item.response.sale_response.as_ref() {
            Some(r) => r,
            None => {
                let error_response = ErrorResponse {
                    code: item.response.response_code,
                    message: item.response.message.clone(),
                    reason: Some(item.response.message.clone()),
                    status_code: item.http_code,
                    attempt_status: Some(common_enums::AttemptStatus::Failure),
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                };
                return Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status: common_enums::AttemptStatus::Failure,
                        ..item.router_data.resource_common_data
                    },
                    response: Err(error_response),
                    ..item.router_data
                });
            }
        };

        let status =
            get_attempt_status(WorldpayvantivPaymentFlow::Sale, sale_response.response)?;

        if is_payment_failure(status) {
            let error_response = ErrorResponse {
                code: sale_response.response.to_string(),
                message: sale_response.message.clone(),
                reason: Some(sale_response.message.clone()),
                status_code: item.http_code,
                attempt_status: Some(status),
                connector_transaction_id: Some(sale_response.cnp_txn_id.clone()),
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
            };
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    ..item.router_data.resource_common_data
                },
                response: Err(error_response),
                ..item.router_data
            });
        }

        let network_txn_id = sale_response
            .network_transaction_id
            .clone()
            .map(|id| id.expose());

        let payments_response = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(sale_response.cnp_txn_id.clone()),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id,
            connector_response_reference_id: Some(sale_response.order_id.clone()),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
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
