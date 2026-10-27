use std::collections::HashMap;

use base64::Engine;
use common_enums::{AttemptStatus, CaptureMethod};
use common_utils::consts::NO_ERROR_MESSAGE;
use common_utils::{fp_utils::when, pii::SecretSerdeValue, types::MinorUnit};
use domain_types::{
    connector_flow::{
        Authorize, Capture, ClientAuthenticationToken, Refund, RepeatPayment,
        ServerAuthenticationToken, SetupMandate, Void, VoidPC,
    },
    connector_types::{
        ClientAuthenticationTokenData, ClientAuthenticationTokenRequestData,
        ConnectorSpecificClientAuthenticationResponse,
        JpmorganClientAuthenticationResponse as JpmorganClientAuthenticationResponseDomain,
        L2L3Data, MandateReferenceId, PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData,
        PaymentsCancelPostCaptureData, PaymentsCaptureData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData, RepeatPaymentData,
        ResponseId, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData,
        SetupMandateRequestData,
    },
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_address::OrderDetailsWithAmount,
    payment_method_data::{
        ApplePayDecryptedData, ApplePayPaymentData, BankDebitData, GooglePayDecryptedData,
        PaymentMethodData, PaymentMethodDataTypes, RawCardNumber, WalletData,
    },
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
};
use error_stack::ResultExt;
use hyperswitch_masking::{Maskable, PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use crate::connectors::jpmorgan::{requests, responses, JpmorganAmountConvertor};
use crate::{connectors::jpmorgan::JpmorganRouterData, types::ResponseRouterData, utils};
use domain_types::errors::{ConnectorError, IntegrationError, IntegrationErrorContext};
use domain_types::utils::is_payment_failure;

type Error = error_stack::Report<IntegrationError>;
type ResponseError = error_stack::Report<ConnectorError>;

const JPMORGAN_GETTING_STARTED_DOC: &str =
    "https://developer.payments.jpmorgan.com/docs/commerce-solutions/online-payments/guides/getting-started";
const JPMORGAN_THREE_DS_NOT_IMPLEMENTED: &str = "3DS payments";

const JPMORGAN_PURCHASE_ORDER_NUMBER_MAX_LENGTH: usize = 17;

const JPMORGAN_MAX_LINE_ITEMS: usize = 998;
const JPMORGAN_MAX_VAT_PERCENT: i64 = 9999;

const MAX_ORDER_NUMBER_LENGTH: usize = 40;

// JPMorgan's valueAddedTaxPercent implies two decimal places, so 2.5% is sent as "250".
const JPMORGAN_VAT_PERCENT_MULTIPLIER: f64 = 100.0;
// Four digits with two implied decimals allow at most 99.99%, encoded as "9999".
const JPMORGAN_MAX_VAT_PERCENTAGE: f64 = 99.99;

fn build_line_item(detail: &OrderDetailsWithAmount) -> requests::JpmorganLineItem {
    let line_item_description_text = detail
        .description
        .clone()
        .filter(|description| !description.trim().is_empty())
        .or_else(|| Some(detail.product_name.clone()).filter(|name| !name.trim().is_empty()));

    let merchant_product_identifier = detail.product_id.clone().or_else(|| detail.sku.clone());

    let quantity = i64::from(detail.quantity);
    let transaction_discount_amount = detail
        .unit_discount_amount
        .and_then(|unit_discount| unit_discount.get_amount_as_i64().checked_mul(quantity))
        .filter(|discount| *discount > 0)
        .map(MinorUnit::new);

    let line_item_taxes = detail
        .total_tax_amount
        .filter(|tax| tax.get_amount_as_i64() > 0)
        .map(|tax| requests::JpmorganLineItemTax {
            line_item_tax_amount: Some(tax),
            tax_percent: detail
                .tax_rate
                .filter(|rate| *rate >= 0.0 && rate.is_finite())
                .map(|rate| rate.to_string()),
            tax_type_code: detail.product_tax_code.clone(),
        })
        .map_or_else(Vec::new, |tax| vec![tax]);

    requests::JpmorganLineItem {
        line_item_description_text,
        merchant_product_identifier,
        item_comodity_code: detail.commodity_code.clone(),
        line_item_unit_quantity: (detail.quantity > 0).then(|| detail.quantity.to_string()),
        line_item_unit_of_measure_code: detail.unit_of_measure.clone(),
        unit_price_amount: Some(detail.amount),
        tax_inclusive_line_item_total_amount: detail.total_amount,
        transaction_discount_amount,
        line_item_tax_indicator: Some(!line_item_taxes.is_empty()),
        line_item_discount_indicator: Some(transaction_discount_amount.is_some()),
        line_item_taxes,
    }
}

fn build_level3(
    l2_l3_data: &L2L3Data,
    fallback_shipping_cost: Option<MinorUnit>,
) -> Option<requests::JpmorganLevel3> {
    let order_details = l2_l3_data.get_order_details()?;
    if order_details.is_empty() {
        return None;
    }
    if order_details.len() > JPMORGAN_MAX_LINE_ITEMS {
        tracing::warn!(
            line_items = order_details.len(),
            max_line_items = JPMORGAN_MAX_LINE_ITEMS,
            "Order carries more line items than JPMorgan accepts: Level 3 is omitted while \
             Level 2 data is still sent"
        );
        return None;
    }

    let line_items = order_details.iter().map(build_line_item).collect();

    let total_shipping_amount = l2_l3_data.get_shipping_cost().or(fallback_shipping_cost);
    let duty_amount = l2_l3_data.get_duty_amount();
    let value_added_tax_amount = l2_l3_data.get_value_added_tax_amount();
    let total_transaction_discount_amount = l2_l3_data.get_discount_amount();
    let shipping_value_added_tax_amount = l2_l3_data.get_shipping_amount_tax();
    let value_added_tax_invoice_reference_number = l2_l3_data
        .get_vat_invoice_reference_number()
        .filter(|reference| !reference.trim().is_empty());

    let value_added_tax_percent = l2_l3_data
        .get_value_added_tax_percent()
        .filter(|percent| {
            percent.is_finite() && (0.0..=JPMORGAN_MAX_VAT_PERCENTAGE).contains(percent)
        })
        .map(|percent| {
            utils::scaled_percentage_to_string(percent, JPMORGAN_VAT_PERCENT_MULTIPLIER)
        });

    let shipping_value_added_tax_percent = shipping_value_added_tax_amount
        .zip(total_shipping_amount)
        .map(|(tax, shipping)| (tax.get_amount_as_i64(), shipping.get_amount_as_i64()))
        .filter(|(_, shipping)| *shipping > 0)
        .and_then(|(tax, shipping)| tax.checked_mul(10_000).map(|scaled| scaled / shipping))
        .filter(|scaled_percent| (0..=JPMORGAN_MAX_VAT_PERCENT).contains(scaled_percent))
        .map(|scaled_percent| scaled_percent.to_string());

    Some(requests::JpmorganLevel3 {
        total_shipping_amount,
        duty_amount,
        ship_to_address_postal_code: l2_l3_data.get_shipping_zip(),
        ship_to_address_country_code: l2_l3_data.get_shipping_country(),
        ship_from_address_postal_code: l2_l3_data.get_shipping_origin_zip(),
        total_transaction_discount_amount,
        value_added_tax_amount,
        value_added_tax_percent,
        shipping_value_added_tax_amount,
        shipping_value_added_tax_percent,
        value_added_tax_invoice_reference_number,
        party_tax_government_issued_identifier: l2_l3_data.get_merchant_tax_registration_id(),
        line_items,
    })
}

fn build_retail_addenda(
    l2_l3_data: Option<&L2L3Data>,
    fallback_order_tax_amount: Option<MinorUnit>,
    fallback_shipping_cost: Option<MinorUnit>,
) -> Option<requests::JpmorganRetailAddenda> {
    let l2_l3_data = l2_l3_data?;

    let purchase_order_number = l2_l3_data
        .get_merchant_order_reference_id()
        .filter(|purchase_order_number| !purchase_order_number.is_empty())
        .and_then(|purchase_order_number| {
            if purchase_order_number.len() > JPMORGAN_PURCHASE_ORDER_NUMBER_MAX_LENGTH {
                tracing::warn!(
                    length = purchase_order_number.len(),
                    max_length = JPMORGAN_PURCHASE_ORDER_NUMBER_MAX_LENGTH,
                    "merchant_order_reference_id is longer than JPMorgan's purchaseOrderNumber \
                     limit: the field is omitted while the rest of retailAddenda is still sent"
                );
                None
            } else {
                Some(purchase_order_number)
            }
        });

    let order_date = l2_l3_data.get_order_date().map(|date| {
        format!(
            "{:04}-{:02}-{:02}",
            date.year(),
            u8::from(date.month()),
            date.day()
        )
    });

    let tax_amount = l2_l3_data
        .get_order_tax_amount()
        .or(fallback_order_tax_amount)
        .filter(|tax_amount| tax_amount.get_amount_as_i64() > 0);

    let tax_status = l2_l3_data.get_tax_status();
    let is_taxable =
        tax_status.map(|tax_status| matches!(tax_status, common_enums::TaxStatus::Taxable));
    let taxable_indicator = tax_status.map(|tax_status| match tax_status {
        common_enums::TaxStatus::Taxable => requests::JpmorganTaxableIndicator::Taxable,
        common_enums::TaxStatus::Exempt => requests::JpmorganTaxableIndicator::Exempt,
    });

    let summary_item_commodity_code = l2_l3_data
        .get_order_commodity_code()
        .filter(|code| !code.trim().is_empty());

    let level3 = build_level3(l2_l3_data, fallback_shipping_cost);

    let addenda = requests::JpmorganRetailAddenda {
        purchase_order_number,
        order_date,
        tax_amount,
        is_taxable,
        taxable_indicator,
        customer_vat_registration_number: l2_l3_data.get_customer_tax_registration_id(),
        merchant_vat_registration_number: l2_l3_data.get_merchant_vat_registration_id(),
        pst_tax_registration_number: l2_l3_data.get_merchant_tax_registration_id(),
        summary_item_commodity_code,
        level3,
    };

    // Destructured so that a field added later forces this emptiness check to
    // be revisited by the compiler rather than silently excluded from it.
    let requests::JpmorganRetailAddenda {
        purchase_order_number,
        order_date,
        tax_amount,
        is_taxable,
        taxable_indicator,
        customer_vat_registration_number,
        merchant_vat_registration_number,
        pst_tax_registration_number,
        summary_item_commodity_code,
        level3,
    } = &addenda;
    let all_none = purchase_order_number.is_none()
        && order_date.is_none()
        && tax_amount.is_none()
        && is_taxable.is_none()
        && taxable_indicator.is_none()
        && customer_vat_registration_number.is_none()
        && merchant_vat_registration_number.is_none()
        && pst_tax_registration_number.is_none()
        && summary_item_commodity_code.is_none()
        && level3.is_none();

    (!all_none).then_some(addenda)
}

/// `shipTo.shippingAddress.postalCode` — required by American Express for
/// Level 2 and optional on the other networks, so it is sent whenever the
/// merchant supplied a shipping address.
fn build_ship_to(
    l2_l3_data: Option<&L2L3Data>,
    address: &domain_types::payment_address::PaymentAddress,
) -> Option<requests::JpmorganShipTo> {
    l2_l3_data
        .and_then(|l2_l3_data| l2_l3_data.get_shipping_zip())
        .or_else(|| {
            address
                .get_shipping()
                .and_then(|shipping| shipping.address.as_ref())
                .and_then(|details| details.zip.clone())
        })
        .map(|postal_code| requests::JpmorganShipTo {
            shipping_address: requests::JpmorganShippingAddress { postal_code },
        })
}

/// Level 2 data and the independently supplied shipping destination for an
/// Authorize request.
fn authorize_level2_data<T: PaymentMethodDataTypes>(
    router_data: &RouterDataV2<
        Authorize,
        PaymentFlowData,
        PaymentsAuthorizeData<T>,
        PaymentsResponseData,
    >,
) -> (
    Option<requests::JpmorganRetailAddenda>,
    Option<requests::JpmorganShipTo>,
) {
    let l2_l3_data = router_data.resource_common_data.l2_l3_data.as_deref();
    let retail_addenda = build_retail_addenda(
        l2_l3_data,
        router_data
            .request
            .order_tax_amount
            .as_ref()
            .map(|order_tax_amount| order_tax_amount.amount),
        router_data
            .request
            .shipping_cost
            .as_ref()
            .map(|shipping_cost| shipping_cost.amount),
    );
    let ship_to = build_ship_to(l2_l3_data, &router_data.resource_common_data.address);
    (retail_addenda, ship_to)
}

impl requests::JpmorganSyncResource {
    pub(super) fn from_request(request: &PaymentsSyncData) -> Result<Self, Error> {
        let invalid = || IntegrationError::InvalidDataFormat {
            field_name: "connector_feature_data.resource",
            context: IntegrationErrorContext {
                suggested_action: Some(
                    "Return the resource type with its original transaction ID.".to_owned(),
                ),
                ..Default::default()
            },
        };
        let resource: Option<requests::JpmorganResourceContext> =
            if let Some(data) = &request.connector_feature_data {
                let metadata = data.peek().as_object().ok_or_else(invalid)?;
                match metadata.get("resource") {
                    Some(resource) => {
                        Some(serde_json::from_value(resource.clone()).change_context(invalid())?)
                    }
                    None => None,
                }
            } else {
                None
            };
        let id = request
            .connector_transaction_id
            .get_connector_transaction_id()
            .change_context(IntegrationError::MissingConnectorTransactionID {
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Supply the original connector_transaction_id for this Get request."
                            .to_owned(),
                    ),
                    ..Default::default()
                },
            })?;
        match resource {
            Some(_) => Ok(Self::Verification(id)),
            None => Ok(Self::Payment(id)),
        }
    }

    pub(super) fn url(&self, base: &str) -> String {
        match self {
            Self::Payment(id) => format!("{base}/payments/{}", urlencoding::encode(id)),
            Self::Verification(id) => format!("{base}/verifications/{}", urlencoding::encode(id)),
        }
    }

    pub(super) fn response_context(http_code: u16) -> ConnectorError {
        ConnectorError::ResponseHandlingFailed {
            context: domain_types::errors::ResponseTransformationErrorContext {
                http_status_code: Some(http_code),
                additional_context: Some(
                    "The response does not match the requested JPMorgan resource.".to_owned(),
                ),
            },
        }
    }

    pub(super) fn response_error(http_code: u16) -> ResponseError {
        Self::response_context(http_code).into()
    }
}

impl responses::JpmorganVerificationResponse {
    fn status(&self, http_code: u16) -> Result<AttemptStatus, ResponseError> {
        match self.response_status {
            responses::JpmorganVerificationStatus::Success => {
                if self.payment_method_type.card.is_none() || self.payment_method_type.ach.is_some()
                {
                    return Err(requests::JpmorganSyncResource::response_error(http_code));
                }
                Ok(AttemptStatus::Charged)
            }
            responses::JpmorganVerificationStatus::Denied
            | responses::JpmorganVerificationStatus::Error => Ok(AttemptStatus::Failure),
            responses::JpmorganVerificationStatus::Unknown => {
                Err(requests::JpmorganSyncResource::response_error(http_code))
            }
        }
    }

    fn into_payment_response(
        self,
        http_code: u16,
    ) -> Result<Result<PaymentsResponseData, ErrorResponse>, ResponseError> {
        let status = self.status(http_code)?;
        if is_payment_failure(status) {
            let network_diagnostics = JpmorganNetworkDiagnostics::from_response(
                self.payment_method_type.network_response(),
                Some(&self.host_message),
            );

            return Ok(Err(ErrorResponse {
                code: self.response_code,
                message: self.response_message.clone(),
                reason: Some(self.response_message),
                status_code: http_code,
                attempt_status: Some(FlowStatus::Payment(status)),
                connector_transaction_id: Some(self.transaction_id),
                network_decline_code: network_diagnostics.network_decline_code,
                network_error_message: network_diagnostics.network_error_message,
                network_advice_code: network_diagnostics.network_advice_code,
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
            }));
        }
        let metadata = requests::JpmorganVerificationMetadata {
            resource: requests::JpmorganResourceContext {
                resource_type: requests::JpmorganResourceType::Verification,
            },
        };
        let metadata = serde_json::to_value(metadata)
            .change_context(requests::JpmorganSyncResource::response_context(http_code))?;
        let network = self
            .payment_method_type
            .card
            .and_then(|card| card.network_response);
        Ok(Ok(PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(self.transaction_id),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: Some(metadata),
            network_txn_id: network
                .as_ref()
                .and_then(|network| network.network_transaction_id.clone()),
            network_txn_link_id: network.and_then(|network| network.transaction_link_id),
            connector_response_reference_id: Some(self.request_id),
            incremental_authorization_allowed: None,
            status_code: http_code,
            splits: None,
            payment_account_reference: None,
        }))
    }
}

fn sanitize_order_number(value: &str) -> Option<String> {
    let order_number: String = value
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '.' | '-'))
        .take(MAX_ORDER_NUMBER_LENGTH)
        .collect();

    (!order_number.trim().is_empty()).then_some(order_number)
}

fn merchant_order_number(merchant_order_id: Option<&str>, reference: &str) -> Option<String> {
    merchant_order_id
        .and_then(sanitize_order_number)
        .or_else(|| sanitize_order_number(reference))
}

impl TryFrom<Option<common_enums::BankType>> for requests::JpmorganAchAccountType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(bank_type: Option<common_enums::BankType>) -> Result<Self, Self::Error> {
        match bank_type {
            Some(common_enums::BankType::Savings) => Ok(Self::Savings),
            Some(common_enums::BankType::Checking) | None => Ok(Self::Checking),
            Some(bank) => Err(error_stack::report!(IntegrationError::NotSupported {
                message: format!("Bank type {bank:?} is not supported by jpmorgan"),
                connector: "jpmorgan",
                context: IntegrationErrorContext {
                    suggested_action: Some("Provide a valid bank account type".to_owned()),
                    additional_context: None,
                    doc_url: None,
                },
            })),
        }
    }
}

/// Build an `IntegrationErrorContext` for a missing JPMorgan connector config field.
fn jpmorgan_missing_field_context(field_name: &str) -> IntegrationErrorContext {
    IntegrationErrorContext {
        suggested_action: Some(format!(
            "Set the '{}' field in the JPMorgan connector configuration. This is required \
             by JPMorgan's Online Payments API for every payment request.",
            field_name
        )),
        doc_url: Some(JPMORGAN_GETTING_STARTED_DOC.to_owned()),
        additional_context: Some(format!(
            "JPMorgan requires '{}' as a mandatory field in the merchant software or \
             connector configuration.",
            field_name
        )),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct JpmorganAuthType {
    pub client_id: Secret<String>,
    pub thumbprint: Secret<String>,
    pub private_key: Secret<String>,
    pub resource_id: Secret<String>,
    pub merchant_id: HashMap<common_enums::Currency, Secret<String>>,
    pub company_name: Option<Secret<String>>,
    pub product_name: Option<Secret<String>>,
    pub merchant_purchase_description: Option<Secret<String>>,
    pub statement_descriptor: Option<Secret<String>>,
}

impl TryFrom<&ConnectorSpecificConfig> for JpmorganAuthType {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Jpmorgan {
                client_id,
                thumbprint,
                private_key,
                resource_id,
                merchant_id,
                company_name,
                product_name,
                merchant_purchase_description,
                statement_descriptor,
                ..
            } => Ok(Self {
                client_id: client_id.clone(),
                thumbprint: thumbprint.clone(),
                private_key: private_key.clone(),
                resource_id: resource_id.clone(),
                merchant_id: merchant_id.clone(),
                company_name: company_name.clone(),
                product_name: product_name.clone(),
                merchant_purchase_description: merchant_purchase_description.clone(),
                statement_descriptor: statement_descriptor.clone(),
            }),
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            }
            .into()),
        }
    }
}

const JPMORGAN_OAUTH_DOC: &str =
    "https://developer.payments.jpmorgan.com/api/commerce/online-payments/oauth-authentication";
const CLIENT_ASSERTION_TYPE: &str = "urn:ietf:params:oauth:client-assertion-type:jwt-bearer";
const CLIENT_ASSERTION_TTL_SECONDS: i64 = 300;

fn invalid_oauth_config(field: &'static str) -> IntegrationError {
    IntegrationError::InvalidConnectorConfig {
        config: field,
        context: IntegrationErrorContext {
            doc_url: Some(JPMORGAN_OAUTH_DOC.to_string()),
            ..Default::default()
        },
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct JpmorganClientAssertionClaims {
    jti: String,
    iat: i64,
    exp: i64,
    aud: String,
    iss: Secret<String>,
    sub: Secret<String>,
}

impl JpmorganAuthType {
    pub(super) fn merchant_id_for_currency(
        &self,
        currency: common_enums::Currency,
    ) -> Result<Secret<String>, Error> {
        self.merchant_id
            .get(&currency)
            .filter(|merchant_id| !merchant_id.peek().trim().is_empty())
            .cloned()
            .ok_or_else(|| IntegrationError::InvalidConnectorConfig {
                config: "merchant_id",
                context: IntegrationErrorContext {
                    suggested_action: Some(format!(
                        "Set a non-empty JPMorgan merchant_id.{currency} in the connector configuration.",
                    )),
                    ..Default::default()
                },
            }.into())
    }

    fn validate_credentials(&self) -> Result<(), Error> {
        for (value, name) in [
            (self.client_id.peek().as_str(), "client_id"),
            (self.thumbprint.peek().as_str(), "thumbprint"),
            (self.private_key.peek().as_str(), "private_key"),
            (self.resource_id.peek().as_str(), "resource_id"),
        ] {
            if value.trim().is_empty() {
                return Err(invalid_oauth_config(name).into());
            }
        }
        Ok(())
    }

    pub(super) fn token_headers(&self) -> Result<Vec<(String, Maskable<String>)>, Error> {
        self.validate_credentials()?;
        Ok(vec![(
            super::headers::CONTENT_TYPE.to_string(),
            "application/x-www-form-urlencoded".to_string().into(),
        )])
    }

    pub(super) fn token_url(secondary_base_url: Option<&str>) -> Result<String, Error> {
        let base_url =
            secondary_base_url.ok_or(IntegrationError::FailedToObtainIntegrationUrl {
                context: IntegrationErrorContext {
                    doc_url: Some(JPMORGAN_OAUTH_DOC.to_string()),
                    suggested_action: Some(
                        "Set JPMorgan secondary_base_url to the HTTPS OAuth host.".to_string(),
                    ),
                    ..Default::default()
                },
            })?;
        let token_url = url::Url::parse(base_url)
            .change_context(invalid_oauth_config("secondary_base_url"))?
            .join("/adfs/oauth2/token")
            .change_context(invalid_oauth_config("secondary_base_url"))?;

        Ok(token_url.to_string())
    }

    pub(super) fn token_request(
        &self,
        secondary_base_url: Option<&str>,
    ) -> Result<requests::JpmorganTokenRequest, Error> {
        self.validate_credentials()?;
        let token_url = Self::token_url(secondary_base_url)?;
        let thumbprint = &self.thumbprint;
        let private_key = &self.private_key;
        let resource_id = &self.resource_id;
        let kid = thumbprint
            .peek()
            .trim()
            .replace(':', "")
            .to_ascii_uppercase();
        if kid.is_empty() || !kid.bytes().all(|c| c.is_ascii_hexdigit()) {
            return Err(invalid_oauth_config("thumbprint").into());
        }
        let pem = super::BASE64_ENGINE
            .decode(private_key.peek())
            .change_context(invalid_oauth_config("private_key"))?;
        let signing_key = jsonwebtoken::EncodingKey::from_rsa_pem(&pem)
            .change_context(invalid_oauth_config("private_key"))?;
        let mut header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256);
        header.typ = Some("JWT".to_string());
        header.kid = Some(kid);
        let now = common_utils::date_time::now_unix_timestamp();
        let claims = JpmorganClientAssertionClaims {
            jti: common_utils::fp_utils::generate_uuid_v4(),
            iat: now,
            exp: now + CLIENT_ASSERTION_TTL_SECONDS,
            aud: token_url,
            iss: self.client_id.clone(),
            sub: self.client_id.clone(),
        };
        let assertion = jsonwebtoken::encode(&header, &claims, &signing_key)
            .change_context(invalid_oauth_config("private_key"))?;
        Ok(requests::JpmorganTokenRequest {
            grant_type: "client_credentials".to_string(),
            client_id: self.client_id.clone(),
            client_assertion_type: CLIENT_ASSERTION_TYPE.to_string(),
            client_assertion: Secret::new(assertion),
            resource: resource_id.clone(),
        })
    }
}

/// JPMorgan connector metadata containing merchant software information
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct JpmorganConnectorMetadataObject {
    pub company_name: Secret<String>,
    pub product_name: Secret<String>,
    pub merchant_purchase_description: Secret<String>,
    pub statement_descriptor: Secret<String>,
}

impl TryFrom<&Option<SecretSerdeValue>> for JpmorganConnectorMetadataObject {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(meta_data: &Option<SecretSerdeValue>) -> Result<Self, Self::Error> {
        let metadata: Self = utils::to_connector_meta_from_secret::<Self>(meta_data.clone())
            .change_context(IntegrationError::InvalidConnectorConfig {
                config: "merchant_connector_account.metadata",
                context: Default::default(),
            })?;
        Ok(metadata)
    }
}

// OAuth 2.0 transformers
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        JpmorganRouterData<
            RouterDataV2<
                ServerAuthenticationToken,
                MerchantAuthenticationFlowData,
                ServerAuthenticationTokenRequestData,
                ServerAuthenticationTokenResponseData,
            >,
            T,
        >,
    > for requests::JpmorganTokenRequest
{
    type Error = Error;
    fn try_from(
        item: JpmorganRouterData<
            RouterDataV2<
                ServerAuthenticationToken,
                MerchantAuthenticationFlowData,
                ServerAuthenticationTokenRequestData,
                ServerAuthenticationTokenResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        JpmorganAuthType::try_from(&item.router_data.connector_config)?.token_request(
            item.router_data
                .resource_common_data
                .connectors
                .jpmorgan
                .secondary_base_url
                .as_deref(),
        )
    }
}

impl<F> TryFrom<ResponseRouterData<responses::JpmorganAuthUpdateResponse, Self>>
    for RouterDataV2<
        F,
        MerchantAuthenticationFlowData,
        ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData,
    >
{
    type Error = ResponseError;
    fn try_from(
        item: ResponseRouterData<responses::JpmorganAuthUpdateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(ServerAuthenticationTokenResponseData {
                access_token: item.response.access_token,
                token_type: Some(item.response.token_type.clone()),
                expires_in: Some(item.response.expires_in),
            }),
            ..item.router_data
        })
    }
}

fn map_capture_method(
    capture_method: Option<CaptureMethod>,
) -> Result<requests::CapMethod, error_stack::Report<IntegrationError>> {
    match capture_method {
        Some(CaptureMethod::Automatic) | None => Ok(requests::CapMethod::Now),
        Some(CaptureMethod::Manual) => Ok(requests::CapMethod::Manual),
        Some(CaptureMethod::Scheduled)
        | Some(CaptureMethod::ManualMultiple)
        | Some(CaptureMethod::SequentialAutomatic) => {
            Err(error_stack::report!(IntegrationError::NotSupported {
                message: "Capture Method".to_string(),
                connector: "Jpmorgan",
                context: Default::default(),
            }))
        }
    }
}

/// Extract first name and last name from account holder name or billing info
fn extract_account_holder_names<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    router_data: &RouterDataV2<
        Authorize,
        PaymentFlowData,
        PaymentsAuthorizeData<T>,
        PaymentsResponseData,
    >,
    _bank_account_holder_name: &Option<Secret<String>>,
) -> Result<(Secret<String>, Secret<String>), error_stack::Report<IntegrationError>> {
    // Use billing address first_name and last_name directly (like Forte connector)
    let first_name = router_data
        .resource_common_data
        .get_billing_first_name()
        .ok()
        .unwrap_or_else(|| Secret::new("".to_string()));

    let last_name = router_data
        .resource_common_data
        .get_optional_billing_last_name()
        .unwrap_or_else(|| first_name.clone());

    Ok((first_name, last_name))
}

fn map_payment_channel_to_jpmorgan_merchant_sales_channel_name(
    payment_channel: Option<&common_enums::PaymentChannel>,
) -> Option<requests::JpmorganMerchantSalesChannelName> {
    match payment_channel {
        Some(common_enums::PaymentChannel::MailOrder) => {
            Some(requests::JpmorganMerchantSalesChannelName::MailOrder)
        }
        Some(common_enums::PaymentChannel::TelephoneOrder) => {
            Some(requests::JpmorganMerchantSalesChannelName::TelephoneOrder)
        }
        Some(_) | None => None,
    }
}

fn get_statement_descriptor(
    descriptor: Option<&domain_types::connector_types::BillingDescriptor>,
) -> Option<String> {
    descriptor.and_then(|descriptor| descriptor.statement_descriptor.clone())
}

fn map_partial_authorization_support(
    enable_partial_authorization: Option<bool>,
) -> requests::JpmorganPartialAuthorizationSupport {
    if enable_partial_authorization == Some(true) {
        requests::JpmorganPartialAuthorizationSupport::Supported
    } else {
        requests::JpmorganPartialAuthorizationSupport::NotSupported
    }
}

impl
    TryFrom<(
        &domain_types::router_request_types::AuthenticationData,
        &common_enums::CardNetwork,
    )> for requests::JpmorganAuthentication
{
    type Error = Error;

    fn try_from(
        (data, card_network): (
            &domain_types::router_request_types::AuthenticationData,
            &common_enums::CardNetwork,
        ),
    ) -> Result<Self, Self::Error> {
        let transaction_id = match card_network {
            common_enums::CardNetwork::Visa | common_enums::CardNetwork::Mastercard => {
                data.ds_trans_id.clone()
            }
            common_enums::CardNetwork::AmericanExpress | common_enums::CardNetwork::Discover => {
                None
            }
            _ => {
                return Err(IntegrationError::NotSupported {
                    message: "3DS for this card network".to_owned(),
                    connector: "jpmorgan",
                    context: Default::default(),
                }
                .into())
            }
        };
        let protocol = data.message_version.as_ref().map(ToString::to_string);
        let protocol = if *card_network == common_enums::CardNetwork::Mastercard {
            Some(protocol.ok_or_else(utils::missing_field_err(
                "authentication_data.message_version",
            ))?)
        } else {
            protocol
        };
        Ok(Self {
            token_authentication_value: None,
            electronic_commerce_indicator: Some(
                data.eci
                    .clone()
                    .ok_or_else(utils::missing_field_err("authentication_data.eci"))?,
            ),
            three_ds: Some(requests::JpmorganThreeDs {
                authentication_value: data
                    .cavv
                    .clone()
                    .ok_or_else(utils::missing_field_err("authentication_data.cavv"))?,
                authentication_transaction_id: transaction_id,
                three_ds_program_protocol: protocol,
            }),
        })
    }
}

impl<T: PaymentMethodDataTypes> requests::JpmorganCard<T> {
    fn wallet_field_context(field_name: &'static str) -> IntegrationErrorContext {
        IntegrationErrorContext {
            suggested_action: Some(format!(
                "Supply a valid {field_name} from the decrypted wallet payload."
            )),
            ..Default::default()
        }
    }

    fn invalid_wallet_field(field_name: &'static str) -> IntegrationError {
        IntegrationError::InvalidDataFormat {
            field_name,
            context: Self::wallet_field_context(field_name),
        }
    }

    fn wallet_authentication(
        cryptogram: &Secret<String>,
        eci: &Option<String>,
    ) -> Result<requests::JpmorganAuthentication, Error> {
        if cryptogram.peek().trim().is_empty() {
            return Err(Self::invalid_wallet_field("token_cryptogram").into());
        }
        Ok(requests::JpmorganAuthentication {
            token_authentication_value: Some(cryptogram.clone()),
            electronic_commerce_indicator: eci.clone(),
            three_ds: None,
        })
    }

    fn network_token_authentication(
        token: &domain_types::payment_method_data::NetworkTokenData,
        cryptogram: &Secret<String>,
    ) -> Result<requests::JpmorganAuthentication, Error> {
        let eci = match &token.eci {
            Some(eci) if eci.trim().is_empty() => {
                return Err(Self::invalid_wallet_field("network_token.eci").into())
            }
            Some(eci) => eci.clone(),
            None => match token.card_network.as_ref() {
                Some(common_enums::CardNetwork::Visa) => "7".to_owned(),
                Some(common_enums::CardNetwork::Mastercard) => "5".to_owned(),
                _ => {
                    return Err(IntegrationError::MissingRequiredField {
                        field_name: "network_token.eci",
                        context: Self::wallet_field_context("network_token.eci"),
                    }
                    .into())
                }
            },
        };
        Self::wallet_authentication(cryptogram, &Some(eci))
    }

    fn card_authentication(
        authentication_data: Option<&domain_types::router_request_types::AuthenticationData>,
        auth_type: common_enums::AuthenticationType,
        card_network: Option<&common_enums::CardNetwork>,
    ) -> Result<Option<requests::JpmorganAuthentication>, Error> {
        let Some(data) = authentication_data else {
            return if auth_type == common_enums::AuthenticationType::ThreeDs {
                Err(IntegrationError::NotImplemented(
                    JPMORGAN_THREE_DS_NOT_IMPLEMENTED.to_owned(),
                    Default::default(),
                )
                .into())
            } else {
                Ok(None)
            };
        };
        let network = card_network.ok_or_else(utils::missing_field_err("card.card_network"))?;
        requests::JpmorganAuthentication::try_from((data, network)).map(Some)
    }

    fn validate_apple_pay_storage(payment_method: &PaymentMethodData<T>) -> Result<(), Error> {
        if matches!(payment_method, PaymentMethodData::Wallet(WalletData::ApplePay(wallet)) if matches!(&wallet.payment_data, ApplePayPaymentData::Decrypted(data) if data.merchant_token_identifier.is_some()))
        {
            return Err(IntegrationError::NotSupported {
                message: "Apple Pay merchant-token storage without a documented payment account-number type".to_owned(),
                connector: "jpmorgan", context: Self::wallet_field_context("apple_pay.merchant_token_identifier"),
            }.into());
        }
        Ok(())
    }

    fn from_apple_pay(
        data: &ApplePayDecryptedData,
        payment_channel: Option<&common_enums::PaymentChannel>,
    ) -> Result<Self, Error> {
        Ok(Self {
            account_number: RawCardNumber(T::inner_from_card_number(
                data.application_primary_account_number.clone(),
            )),
            expiry: requests::Expiry {
                month: Secret::new(
                    data.get_expiry_month()
                        .peek()
                        .parse::<i32>()
                        .change_context(Self::invalid_wallet_field("wallet.expiry.month"))?,
                ),
                year: Secret::new(
                    data.get_four_digit_expiry_year()
                        .peek()
                        .parse::<i32>()
                        .change_context(Self::invalid_wallet_field("wallet.expiry.year"))?,
                ),
            },
            account_number_type: Some(requests::JpmorganAccountNumberType::DeviceToken),
            wallet_provider: Some(requests::JpmorganWalletProvider::ApplePay),
            authentication: Some(Self::wallet_authentication(
                &data.payment_data.online_payment_cryptogram,
                &data.payment_data.eci_indicator,
            )?),
            merchant_sales_channel_name:
                map_payment_channel_to_jpmorgan_merchant_sales_channel_name(payment_channel),
        })
    }

    fn from_google_pay(
        data: &GooglePayDecryptedData,
        payment_channel: Option<&common_enums::PaymentChannel>,
    ) -> Result<Self, Error> {
        // A present cryptogram classifies the payload as a 3DS cryptogram
        // (DEVICE_TOKEN); otherwise the payload is PAN-only.
        let (account_number_type, authentication) = match data.cryptogram.as_ref() {
            Some(cryptogram) => (
                requests::JpmorganAccountNumberType::DeviceToken,
                Some(Self::wallet_authentication(
                    cryptogram,
                    &data.eci_indicator,
                )?),
            ),
            None => {
                if data.eci_indicator.is_some() {
                    return Err(Self::invalid_wallet_field("google_pay.cryptogram").into());
                }
                (requests::JpmorganAccountNumberType::Pan, None)
            }
        };
        let exp_month = data
            .get_expiry_month()
            .change_context(Self::invalid_wallet_field("wallet.expiry.month"))?;
        let exp_year = data
            .get_four_digit_expiry_year()
            .change_context(Self::invalid_wallet_field("wallet.expiry.year"))?;

        Ok(Self {
            account_number: RawCardNumber(T::inner_from_card_number(
                data.application_primary_account_number.clone(),
            )),
            expiry: requests::Expiry {
                month: Secret::new(
                    exp_month
                        .peek()
                        .parse::<i32>()
                        .change_context(Self::invalid_wallet_field("wallet.expiry.month"))?,
                ),
                year: Secret::new(
                    exp_year
                        .peek()
                        .parse::<i32>()
                        .change_context(Self::invalid_wallet_field("wallet.expiry.year"))?,
                ),
            },
            account_number_type: Some(account_number_type),
            wallet_provider: Some(requests::JpmorganWalletProvider::GooglePay),
            authentication,
            merchant_sales_channel_name:
                map_payment_channel_to_jpmorgan_merchant_sales_channel_name(payment_channel),
        })
    }
}

impl<T: PaymentMethodDataTypes> requests::JpmorganPaymentsRequest<T> {
    fn supports_initial_storage(payment_method: &PaymentMethodData<T>) -> bool {
        match payment_method {
            PaymentMethodData::Card(_) | PaymentMethodData::NetworkToken(_) => true,
            PaymentMethodData::Wallet(WalletData::ApplePay(wallet)) => {
                matches!(wallet.payment_data, ApplePayPaymentData::Decrypted(_))
            }
            PaymentMethodData::Wallet(WalletData::GooglePay(wallet)) => matches!(
                wallet.tokenization_data,
                domain_types::payment_method_data::GpayTokenizationData::Decrypted(_)
            ),
            _ => false,
        }
    }

    fn prepare_cit(
        request: &PaymentsAuthorizeData<T>,
    ) -> Result<
        (
            requests::JpmorganAccountOnFile,
            Option<requests::JpmorganRecurring>,
        ),
        Error,
    > {
        if request.setup_future_usage != Some(common_enums::FutureUsage::OffSession) {
            return Ok((requests::JpmorganAccountOnFile::NotStored, None));
        }
        requests::JpmorganCard::validate_apple_pay_storage(&request.payment_method_data)?;
        let recurring = match request.mit_category {
            Some(
                common_enums::MitCategory::Recurring | common_enums::MitCategory::Subscription,
            ) => Some(requests::JpmorganRecurring {
                recurring_sequence: requests::JpmorganRecurringSequence::First,
                agreement_id: None,
                is_variable_amount: Some(true),
            }),
            Some(
                common_enums::MitCategory::Installment | common_enums::MitCategory::Resubmission,
            ) => {
                return Err(IntegrationError::NotSupported {
                    message: "Installment or resubmission agreements".to_owned(),
                    connector: "jpmorgan",
                    context: Default::default(),
                }
                .into())
            }
            _ => None,
        };
        Ok((requests::JpmorganAccountOnFile::ToBeStored, recurring))
    }

    fn from_decrypted_wallet(
        router_data: &RouterDataV2<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<T>,
            PaymentsResponseData,
        >,
        wallet_card: requests::JpmorganCard<T>,
    ) -> Result<Self, Error> {
        if router_data.request.authentication_data.is_some() {
            return Err(IntegrationError::NotSupported {
                message: "Separate 3DS authentication for decrypted wallet payments".to_owned(),
                connector: "jpmorgan",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Supply wallet authentication proof without a separate 3DS flow."
                            .to_owned(),
                    ),
                    ..Default::default()
                },
            }
            .into());
        }
        let auth = JpmorganAuthType::try_from(&router_data.connector_config)?;
        let (account_on_file, recurring) = Self::prepare_cit(&router_data.request)?;
        let (retail_addenda, ship_to) = authorize_level2_data(router_data);
        let request = Self {
            capture_method: map_capture_method(router_data.request.capture_method)?,
            amount: JpmorganAmountConvertor::convert(
                router_data.request.amount.amount,
                router_data.request.currency,
            )?,
            currency: router_data.request.currency,
            merchant: requests::JpmorganMerchant::try_from(&auth)?,
            payment_method_type: requests::JpmorganPaymentMethodType {
                card: Some(wallet_card),
                ach: None,
                googlepay: None,
                token: None,
            },
            account_holder: None,
            statement_descriptor: get_statement_descriptor(
                router_data.request.billing_descriptor.as_ref(),
            ),
            merchant_order_number: merchant_order_number(
                router_data.request.merchant_order_id.as_deref(),
                &router_data
                    .resource_common_data
                    .connector_request_reference_id,
            ),
            initiator_type: Some(requests::JpmorganInitiatorType::Cardholder),
            account_on_file: Some(account_on_file),
            is_amount_final: Some(true),
            retail_addenda,
            ship_to,
            recurring,
            partial_authorization_support: Some(map_partial_authorization_support(
                router_data.request.enable_partial_authorization,
            )),
        };
        Ok(request)
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        JpmorganRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for requests::JpmorganPaymentsRequest<T>
{
    type Error = Error;
    fn try_from(
        item: JpmorganRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        if router_data.request.off_session == Some(true) {
            return Err(IntegrationError::NotSupported {
                message: "Merchant-initiated payments through Authorize".to_owned(),
                connector: "jpmorgan",
                context: Default::default(),
            }
            .into());
        }
        if router_data.request.setup_future_usage == Some(common_enums::FutureUsage::OffSession)
            && !Self::supports_initial_storage(&router_data.request.payment_method_data)
        {
            return Err(IntegrationError::NotSupported {
                message: "Initial credential storage for this payment method".to_owned(),
                connector: "jpmorgan", context: IntegrationErrorContext {
                    suggested_action: Some("Supply a card, network token, or merchant-decrypted Apple Pay or Google Pay credential.".to_owned()),
                    ..Default::default()
                },
            }.into());
        }
        match &router_data.request.payment_method_data {
            PaymentMethodData::Card(card_data) => {
                let authentication = requests::JpmorganCard::<T>::card_authentication(
                    router_data.request.authentication_data.as_ref(),
                    router_data.resource_common_data.auth_type,
                    card_data.card_network.as_ref(),
                )?;
                let capture_method = map_capture_method(router_data.request.capture_method)?;

                let auth = JpmorganAuthType::try_from(&router_data.connector_config)?;

                let merchant = requests::JpmorganMerchant::try_from(&auth)?;

                let exp_month_str = card_data.card_exp_month.peek().to_string();
                let exp_year_str = card_data.get_expiry_year_4_digit().peek().to_string();

                // Vault token placeholders (e.g. "{{$card_exp_month}}") cannot be parsed as i32.
                // JPMorgan requires numeric expiry values, so proxy flows are not supported.
                when(
                    exp_month_str.contains("{{") || exp_year_str.contains("{{"),
                    || {
                        Err(error_stack::report!(IntegrationError::NotSupported {
                            message: "JPMorgan requires numeric expiry values; vault token placeholders are not supported for proxy flows".to_string(),
                            connector: "Jpmorgan",
                            context: Default::default(),
                        }))
                    },
                )?;

                let expiry = requests::Expiry {
                    month: Secret::new(exp_month_str.parse::<i32>().change_context(
                        IntegrationError::RequestEncodingFailed {
                            context: Default::default(),
                        },
                    )?),
                    year: Secret::new(exp_year_str.parse::<i32>().change_context(
                        IntegrationError::RequestEncodingFailed {
                            context: Default::default(),
                        },
                    )?),
                };

                let card = requests::JpmorganCard {
                    account_number: card_data.card_number.clone(),
                    expiry,
                    account_number_type: Some(requests::JpmorganAccountNumberType::Pan),
                    wallet_provider: None,
                    authentication,
                    merchant_sales_channel_name:
                        map_payment_channel_to_jpmorgan_merchant_sales_channel_name(
                            router_data.request.payment_channel.as_ref(),
                        ),
                };

                let (account_on_file, recurring) = Self::prepare_cit(&router_data.request)?;
                let payment_method_type = requests::JpmorganPaymentMethodType {
                    card: Some(card),
                    ach: None,
                    googlepay: None,
                    token: None,
                };

                let amount = JpmorganAmountConvertor::convert(
                    router_data.request.amount.amount,
                    router_data.request.currency,
                )?;

                let (retail_addenda, ship_to) = authorize_level2_data(router_data);
                let request = Self {
                    capture_method,
                    currency: router_data.request.currency,
                    amount,
                    merchant,
                    payment_method_type,
                    account_holder: None,
                    statement_descriptor: get_statement_descriptor(
                        router_data.request.billing_descriptor.as_ref(),
                    ),
                    merchant_order_number: merchant_order_number(
                        router_data.request.merchant_order_id.as_deref(),
                        &router_data
                            .resource_common_data
                            .connector_request_reference_id,
                    ),
                    initiator_type: Some(requests::JpmorganInitiatorType::Cardholder),
                    account_on_file: Some(account_on_file),
                    is_amount_final: Some(true),
                    recurring,
                    partial_authorization_support: Some(map_partial_authorization_support(
                        router_data.request.enable_partial_authorization,
                    )),
                    retail_addenda,
                    ship_to,
                };
                Ok(request)
            }
            PaymentMethodData::NetworkToken(token_data) => {
                if router_data.resource_common_data.auth_type
                    == common_enums::AuthenticationType::ThreeDs
                    || router_data.request.authentication_data.is_some()
                {
                    return Err(IntegrationError::NotImplemented(
                        JPMORGAN_THREE_DS_NOT_IMPLEMENTED.to_string(),
                        Default::default(),
                    )
                    .into());
                }
                let capture_method = map_capture_method(router_data.request.capture_method)?;

                let auth = JpmorganAuthType::try_from(&router_data.connector_config)?;

                let merchant = requests::JpmorganMerchant::try_from(&auth)?;

                let cryptogram = token_data.token_cryptogram.as_ref().ok_or_else(|| {
                    IntegrationError::MissingRequiredField {
                        field_name: "network_token.token_cryptogram",
                        context: requests::JpmorganCard::<T>::wallet_field_context(
                            "network_token.token_cryptogram",
                        ),
                    }
                })?;
                let authentication =
                    Some(requests::JpmorganCard::<T>::network_token_authentication(
                        token_data, cryptogram,
                    )?);

                let card = requests::JpmorganCard {
                    account_number: RawCardNumber(T::inner_from_card_number(
                        token_data
                            .token_number
                            .get_card_no()
                            .parse::<cards::CardNumber>()
                            .map_err(|_| {
                                requests::JpmorganCard::<T>::invalid_wallet_field(
                                    "network_token.token_number",
                                )
                            })?,
                    )),
                    expiry: requests::Expiry {
                        month: Secret::new(
                            token_data
                                .get_network_token_expiry_month()
                                .peek()
                                .parse::<i32>()
                                .change_context(
                                    requests::JpmorganCard::<T>::invalid_wallet_field(
                                        "wallet.expiry.month",
                                    ),
                                )?,
                        ),
                        year: Secret::new(
                            token_data
                                .get_expiry_year_4_digit()
                                .peek()
                                .parse::<i32>()
                                .change_context(
                                    requests::JpmorganCard::<T>::invalid_wallet_field(
                                        "wallet.expiry.year",
                                    ),
                                )?,
                        ),
                    },
                    account_number_type: Some(requests::JpmorganAccountNumberType::NetworkToken),
                    wallet_provider: None,
                    authentication,
                    merchant_sales_channel_name:
                        map_payment_channel_to_jpmorgan_merchant_sales_channel_name(
                            router_data.request.payment_channel.as_ref(),
                        ),
                };

                let (account_on_file, recurring) = Self::prepare_cit(&router_data.request)?;
                let payment_method_type = requests::JpmorganPaymentMethodType {
                    card: Some(card),
                    ach: None,
                    googlepay: None,
                    token: None,
                };

                let amount = JpmorganAmountConvertor::convert(
                    router_data.request.amount.amount,
                    router_data.request.currency,
                )?;

                let (retail_addenda, ship_to) = authorize_level2_data(router_data);
                let request = Self {
                    capture_method,
                    currency: router_data.request.currency,
                    amount,
                    merchant,
                    payment_method_type,
                    account_holder: None,
                    statement_descriptor: get_statement_descriptor(
                        router_data.request.billing_descriptor.as_ref(),
                    ),
                    merchant_order_number: merchant_order_number(
                        router_data.request.merchant_order_id.as_deref(),
                        &router_data
                            .resource_common_data
                            .connector_request_reference_id,
                    ),
                    initiator_type: Some(requests::JpmorganInitiatorType::Cardholder),
                    account_on_file: Some(account_on_file),
                    is_amount_final: Some(true),
                    recurring,
                    partial_authorization_support: Some(map_partial_authorization_support(
                        router_data.request.enable_partial_authorization,
                    )),
                    retail_addenda,
                    ship_to,
                };
                Ok(request)
            }
            PaymentMethodData::BankDebit(BankDebitData::AchBankDebit {
                account_number,
                routing_number,
                bank_account_holder_name,
                bank_type,
                ..
            }) => {
                let capture_method = map_capture_method(router_data.request.capture_method)?;

                let auth = JpmorganAuthType::try_from(&router_data.connector_config)?;

                let merchant = requests::JpmorganMerchant::try_from(&auth)?;

                // Extract first name and last name from account holder name or billing info
                let (first_name, last_name) =
                    extract_account_holder_names(router_data, bank_account_holder_name)?;

                let account_holder = requests::JpmorganAccountHolder {
                    first_name,
                    last_name,
                };

                // Determine account type based on bank_type field, default to Checking
                let account_type = requests::JpmorganAchAccountType::try_from(*bank_type)?;

                let ach = requests::JpmorganAch {
                    account_number: account_number.clone(),
                    financial_institution_routing_number: routing_number.clone(),
                    account_type,
                };

                let payment_method_type = requests::JpmorganPaymentMethodType {
                    card: None,
                    ach: Some(ach),
                    googlepay: None,
                    token: None,
                };

                let amount = JpmorganAmountConvertor::convert(
                    router_data.request.amount.amount,
                    router_data.request.currency,
                )?;

                let statement_descriptor =
                    get_statement_descriptor(router_data.request.billing_descriptor.as_ref());

                Ok(Self {
                    capture_method,
                    currency: router_data.request.currency,
                    amount,
                    merchant,
                    payment_method_type,
                    account_holder: Some(account_holder),
                    statement_descriptor,
                    merchant_order_number: None,
                    initiator_type: None,
                    account_on_file: None,
                    is_amount_final: None,
                    retail_addenda: None,
                    ship_to: None,
                    recurring: None,
                    partial_authorization_support: Some(map_partial_authorization_support(
                        router_data.request.enable_partial_authorization,
                    )),
                })
            }
            PaymentMethodData::PaymentMethodToken(token_data) => {
                let token = token_data.token.clone();

                let capture_method = map_capture_method(router_data.request.capture_method)?;

                let auth = JpmorganAuthType::try_from(&router_data.connector_config)?;

                let merchant = requests::JpmorganMerchant::try_from(&auth)?;

                // For CardToken, the token is passed in the payment_method_type
                // instead of raw card details
                let payment_method_type = requests::JpmorganPaymentMethodType {
                    card: None,
                    ach: None,
                    googlepay: None,
                    token: Some(token),
                };

                let amount = JpmorganAmountConvertor::convert(
                    router_data.request.amount.amount,
                    router_data.request.currency,
                )?;

                let account_holder = requests::JpmorganAccountHolder {
                    first_name: Secret::new("NA".to_string()),
                    last_name: Secret::new("NA".to_string()),
                };
                let statement_descriptor =
                    get_statement_descriptor(router_data.request.billing_descriptor.as_ref());

                let (retail_addenda, ship_to) = authorize_level2_data(router_data);
                Ok(Self {
                    capture_method,
                    currency: router_data.request.currency,
                    amount,
                    merchant,
                    payment_method_type,
                    account_holder: Some(account_holder),
                    statement_descriptor,
                    merchant_order_number: None,
                    initiator_type: None,
                    account_on_file: None,
                    is_amount_final: None,
                    retail_addenda,
                    ship_to,
                    recurring: None,
                    partial_authorization_support: Some(map_partial_authorization_support(
                        router_data.request.enable_partial_authorization,
                    )),
                })
            }
            PaymentMethodData::BankDebit(_) => {
                Err(error_stack::report!(IntegrationError::NotSupported {
                    message: "Only ACH Bank Debit is supported".to_string(),
                    connector: "Jpmorgan",
                    context: Default::default(),
                }))
            }
            PaymentMethodData::Wallet(wallet_data) => match wallet_data {
                WalletData::ApplePay(apple_pay_data) => match &apple_pay_data.payment_data {
                    ApplePayPaymentData::Decrypted(data) => Self::from_decrypted_wallet(
                        router_data,
                        requests::JpmorganCard::from_apple_pay(
                            data,
                            router_data.request.payment_channel.as_ref(),
                        )?,
                    ),
                    ApplePayPaymentData::Encrypted(_) => Err(IntegrationError::NotImplemented(
                        "Encrypted Apple Pay payments".to_owned(),
                        IntegrationErrorContext {
                            suggested_action: Some(
                                "Supply merchant-decrypted Apple Pay data.".to_owned(),
                            ),
                            ..Default::default()
                        },
                    )
                    .into()),
                },
                WalletData::GooglePay(google_pay_data) => {
                    match &google_pay_data.tokenization_data {
                        domain_types::payment_method_data::GpayTokenizationData::Encrypted(
                            encrypted_data,
                        ) => {
                            let capture_method =
                                map_capture_method(router_data.request.capture_method)?;

                            let auth = JpmorganAuthType::try_from(&router_data.connector_config)?;

                            let merchant = requests::JpmorganMerchant::try_from(&auth)?;

                            let amount = JpmorganAmountConvertor::convert(
                                router_data.request.amount.amount,
                                router_data.request.currency,
                            )?;

                            // Parse the Google Pay token string into its component fields.
                            // The token is a JSON string containing protocolVersion, signature,
                            // optionally intermediateSigningKey, and signedMessage.
                            let gpay_token: requests::GooglePayToken = serde_json::from_str(
                                &encrypted_data.token,
                            )
                            .change_context(IntegrationError::RequestEncodingFailed {
                                context: IntegrationErrorContext {
                                    suggested_action: Some(
                                        "Supply a valid Google Pay payment token from the wallet."
                                            .to_owned(),
                                    ),
                                    additional_context: Some(
                                        "The encrypted Google Pay token must be a JSON object with protocolVersion, signature, and signedMessage."
                                            .to_owned(),
                                    ),
                                    ..Default::default()
                                },
                            })?;

                            // Parse signedMessage to extract ephemeralPublicKey.
                            // signedMessage is itself a JSON string.
                            let signed_message: requests::GooglePaySignedMessage =
                                serde_json::from_str(gpay_token.signed_message.peek())
                                    .change_context(IntegrationError::RequestEncodingFailed {
                                        context: IntegrationErrorContext {
                                            suggested_action: Some(
                                                "Supply a signedMessage JSON payload inside the Google Pay token."
                                                    .to_owned(),
                                            ),
                                            additional_context: Some(
                                                "The signedMessage value must be a JSON object containing ephemeralPublicKey."
                                                    .to_owned(),
                                            ),
                                            ..Default::default()
                                        },
                                    })?;

                            // For ECv2, signature comes from intermediateSigningKey.signatures[0].
                            // For ECv1, signature comes from the top-level signature field.
                            let signature = if let Some(isk) = &gpay_token.intermediate_signing_key
                            {
                                isk.signatures.first().cloned().ok_or(
                                    IntegrationError::MissingRequiredField {
                                        field_name: "intermediateSigningKey.signatures[0]",
                                        context: Default::default(),
                                    },
                                )?
                            } else {
                                gpay_token.signature.clone()
                            };

                            let googlepay = requests::JpmorganGooglePay {
                                // latLong is required by JPMorgan; use "0,0" when not available
                                lat_long: "0,0".to_string(),
                                encrypted_payment_bundle:
                                    requests::JpmorganEncryptedPaymentBundle {
                                        // encryptedPayload is the raw signedMessage JSON string
                                        encrypted_payload: gpay_token.signed_message.clone(),
                                        encrypted_payment_header:
                                            requests::JpmorganEncryptedPaymentHeader {
                                                ephemeral_public_key: signed_message
                                                    .ephemeral_public_key,
                                            },
                                        signature,
                                        protocol_version: gpay_token.protocol_version,
                                    },
                            };

                            let payment_method_type = requests::JpmorganPaymentMethodType {
                                card: None,
                                ach: None,
                                googlepay: Some(googlepay),
                                token: None,
                            };

                            let (retail_addenda, ship_to) = authorize_level2_data(router_data);
                            Ok(Self {
                                capture_method,
                                currency: router_data.request.currency,
                                amount,
                                merchant,
                                payment_method_type,
                                // account_holder and statement_descriptor are not required
                                // for Google Pay encrypted flow
                                account_holder: None,
                                statement_descriptor: get_statement_descriptor(
                                    router_data.request.billing_descriptor.as_ref(),
                                ),
                                merchant_order_number: None,
                                initiator_type: None,
                                account_on_file: None,
                                is_amount_final: None,
                                retail_addenda,
                                ship_to,
                                recurring: None,
                                partial_authorization_support: Some(
                                    map_partial_authorization_support(
                                        router_data.request.enable_partial_authorization,
                                    ),
                                ),
                            })
                        }
                        domain_types::payment_method_data::GpayTokenizationData::Decrypted(
                            data,
                        ) => Self::from_decrypted_wallet(
                            router_data,
                            requests::JpmorganCard::from_google_pay(
                                data,
                                router_data.request.payment_channel.as_ref(),
                            )?,
                        ),
                    }
                }
                _ => Err(IntegrationError::NotImplemented(
                    "Wallet not supported".to_string(),
                    Default::default(),
                )
                .into()),
            },
            _ => Err(error_stack::report!(IntegrationError::NotSupported {
                message: "Payment method not supported".to_string(),
                connector: "Jpmorgan",
                context: Default::default(),
            })),
        }
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        JpmorganRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for requests::JpmorganCaptureRequest
{
    type Error = Error;
    fn try_from(
        item: JpmorganRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let capture_method = requests::CapMethod::Now;
        let amount_to_capture = item.router_data.request.amount_to_capture.amount;

        let amount =
            JpmorganAmountConvertor::convert(amount_to_capture, item.router_data.request.currency)?;

        // When AuthenticationType is `Manual`, Documentation suggests us to pass `isAmountFinal` field being `true`
        // isAmountFinal is by default `true`. Since Manual Multiple support is not added here, the field is not used.
        Ok(Self {
            capture_method,
            amount,
            currency: item.router_data.request.currency,
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        JpmorganRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for requests::JpmorganVoidRequest
{
    type Error = Error;
    fn try_from(
        _item: JpmorganRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self { is_void: true })
    }
}

/// VoidPC (post-capture void/reversal) request transformer.
///
/// JPMorgan uses the same `PATCH /payments/{id}` endpoint with `{"isVoid": true}`
/// for both pre-capture void and post-capture reversal. The transaction ID is used
/// to build the URL in the connector implementation.
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        JpmorganRouterData<
            RouterDataV2<
                VoidPC,
                PaymentFlowData,
                PaymentsCancelPostCaptureData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for requests::JpmorganVoidPcRequest
{
    type Error = Error;
    fn try_from(
        _item: JpmorganRouterData<
            RouterDataV2<
                VoidPC,
                PaymentFlowData,
                PaymentsCancelPostCaptureData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self { is_void: true })
    }
}

impl<F> TryFrom<ResponseRouterData<responses::JpmorganPaymentsResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>
{
    type Error = ResponseError;
    fn try_from(
        item: ResponseRouterData<responses::JpmorganPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Map JPMorgan's transaction state directly to `PostCaptureVoidStatus` —
        // the Reverse flow has its own status enum, so we don't go through
        // `AttemptStatus`. `Closed` / `Authorized` mean the PATCH was accepted
        // but the transaction did not move to `Voided`, so the reversal did
        // not apply. `Declined` / `Error` are matched explicitly to keep the
        // match exhaustive.
        let post_capture_void_status = match item.response.transaction_state {
            responses::JpmorganTransactionState::Voided => {
                common_enums::PostCaptureVoidStatus::Succeeded
            }
            responses::JpmorganTransactionState::Pending => {
                common_enums::PostCaptureVoidStatus::Pending
            }
            responses::JpmorganTransactionState::Closed
            | responses::JpmorganTransactionState::Authorized
            | responses::JpmorganTransactionState::Declined
            | responses::JpmorganTransactionState::Error => {
                common_enums::PostCaptureVoidStatus::Failed
            }
        };

        let response = if post_capture_void_status.is_post_capture_void_failure() {
            let network_diagnostics = item.response.network_diagnostics();
            Err(ErrorResponse {
                attempt_status: None,
                code: item.response.response_code.clone(),
                message: item
                    .response
                    .response_message
                    .clone()
                    .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                reason: item.response.response_message.clone(),
                status_code: item.http_code,
                connector_transaction_id: Some(item.response.transaction_id.clone()),
                network_decline_code: network_diagnostics.network_decline_code,
                network_advice_code: network_diagnostics.network_advice_code,
                network_error_message: network_diagnostics.network_error_message,
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
            })
        } else {
            Ok(PaymentsResponseData::PostCaptureVoidResponse {
                post_capture_void_status,
                connector_reference_id: Some(item.response.transaction_id.clone()),
                status_code: item.http_code,
            })
        };

        Ok(Self {
            response,
            ..item.router_data
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        JpmorganRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for requests::JpmorganRefundRequest
{
    type Error = Error;
    fn try_from(
        item: JpmorganRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = JpmorganAuthType::try_from(&item.router_data.connector_config)?;

        let merchant = requests::JpmorganMerchantRefund {
            merchant_software: requests::JpmorganMerchantSoftware {
                company_name: auth
                    .company_name
                    .ok_or(IntegrationError::MissingRequiredField {
                        field_name: "company_name",
                        context: Default::default(),
                    })?,
                product_name: auth
                    .product_name
                    .ok_or(IntegrationError::MissingRequiredField {
                        field_name: "product_name",
                        context: Default::default(),
                    })?,
            },
        };

        let amount = JpmorganAmountConvertor::convert(
            item.router_data.request.refund_amount.amount,
            item.router_data.request.currency,
        )?;

        let transaction_id = item.router_data.request.connector_transaction_id;

        Ok(Self {
            merchant,
            amount,
            currency: item.router_data.request.currency,
            payment_method_type: requests::JpmorganRefundPaymentMethodType {
                transaction_reference: requests::JpmorganTransactionReference {
                    transaction_reference_id: transaction_id,
                },
            },
        })
    }
}

fn map_transaction_state_to_attempt_status(
    transaction_state: &responses::JpmorganTransactionState,
    capture_method: &Option<requests::CapMethod>,
) -> AttemptStatus {
    match transaction_state {
        responses::JpmorganTransactionState::Closed => match capture_method {
            Some(requests::CapMethod::Now) => AttemptStatus::Charged,
            _ => AttemptStatus::Authorized,
        },
        responses::JpmorganTransactionState::Authorized => AttemptStatus::Authorized,
        responses::JpmorganTransactionState::Declined
        | responses::JpmorganTransactionState::Error => AttemptStatus::Failure,
        responses::JpmorganTransactionState::Pending => AttemptStatus::Pending,
        responses::JpmorganTransactionState::Voided => AttemptStatus::Voided,
    }
}

impl TryFrom<&responses::JpmorganPaymentsResponse> for PaymentsResponseData {
    type Error = ResponseError;
    fn try_from(item: &responses::JpmorganPaymentsResponse) -> Result<Self, Self::Error> {
        // Extract networkTransactionId from card.networkResponse for MIT flows
        let network_txn_id = item
            .payment_method_type
            .as_ref()
            .and_then(|pmt| pmt.card.as_ref())
            .and_then(|card| card.network_response.as_ref())
            .and_then(|nr| nr.network_transaction_id.clone());

        let network_txn_link_id = item
            .payment_method_type
            .as_ref()
            .and_then(|method| method.card.as_ref())
            .and_then(|card| card.network_response.as_ref())
            .and_then(|network| network.transaction_link_id.clone());

        Ok(Self::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(item.transaction_id.clone()),
            redirection_data: None,
            // Safetech reuse needs a real token and its credential metadata, not a payment transaction ID.
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id,
            network_txn_link_id,
            connector_response_reference_id: Some(item.request_id.clone()),
            incremental_authorization_allowed: None,
            status_code: item.response_code.parse::<u16>().unwrap_or(0),
            splits: None,
            payment_account_reference: None,
        })
    }
}

impl TryFrom<&responses::JpmorganPaymentsResponse> for AttemptStatus {
    type Error = ResponseError;
    fn try_from(item: &responses::JpmorganPaymentsResponse) -> Result<Self, Self::Error> {
        if matches!(
            item.response_status,
            responses::JpmorganTransactionStatus::Denied
                | responses::JpmorganTransactionStatus::Error
        ) {
            return Ok(Self::Failure);
        }
        Ok(map_transaction_state_to_attempt_status(
            &item.transaction_state,
            &item.capture_method,
        ))
    }
}

#[derive(Debug, Default)]
struct JpmorganNetworkDiagnostics {
    network_decline_code: Option<String>,
    network_advice_code: Option<String>,
    network_error_message: Option<String>,
}

impl JpmorganNetworkDiagnostics {
    fn from_response(
        network: Option<&responses::NetworkResponse>,
        host_message: Option<&str>,
    ) -> Self {
        Self {
            network_decline_code: network.and_then(|network| network.network_response_code.clone()),
            network_advice_code: network
                .and_then(|network| network.additional_data.as_ref())
                .and_then(|additional| additional.merchant_advice_code.clone()),
            network_error_message: host_message.map(str::to_owned),
        }
    }
}

impl responses::JpmorganPaymentsResponse {
    fn network_diagnostics(&self) -> JpmorganNetworkDiagnostics {
        JpmorganNetworkDiagnostics::from_response(
            self.payment_method_type
                .as_ref()
                .and_then(responses::PaymentMethodType::network_response),
            self.host_message.as_deref(),
        )
    }
}

fn get_partial_authorization_amounts(
    response: &responses::JpmorganPaymentsResponse,
    capture_method: Option<CaptureMethod>,
    enable_partial_authorization: Option<bool>,
    currency: common_enums::Currency,
) -> (
    Option<common_utils::types::Money>,
    Option<common_utils::types::Money>,
    Option<common_utils::types::Money>,
) {
    if response.partial_authorization != Some(true) {
        return (None, None, None);
    }

    let (amount_captured, amount_capturable) = match capture_method {
        Some(CaptureMethod::Manual) | Some(CaptureMethod::ManualMultiple) => {
            (None, response.amount)
        }
        _ => (response.amount, Some(MinorUnit::zero())),
    };

    let amount_authorized = enable_partial_authorization
        .filter(|flag| *flag)
        .and(response.amount);
    let to_money = |amount| common_utils::types::Money { amount, currency };

    (
        amount_captured.map(to_money),
        amount_capturable.map(to_money),
        amount_authorized.map(to_money),
    )
}

/// Build the `response` field for a JPMorgan payments flow: `Err(ErrorResponse)`
/// when the transaction was declined/errored, otherwise `Ok(PaymentsResponseData)`.
fn build_payments_response_result(
    response: &responses::JpmorganPaymentsResponse,
    http_code: u16,
    status: AttemptStatus,
) -> Result<Result<PaymentsResponseData, ErrorResponse>, ResponseError> {
    if is_payment_failure(status) {
        let diagnostics = response.network_diagnostics();
        Ok(Err(ErrorResponse {
            attempt_status: Some(FlowStatus::Payment(status)),
            code: response.response_code.clone(),
            message: response
                .response_message
                .clone()
                .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
            reason: response.response_message.clone(),
            status_code: http_code,
            connector_transaction_id: Some(response.transaction_id.clone()),
            network_decline_code: diagnostics.network_decline_code,
            network_advice_code: diagnostics.network_advice_code,
            network_error_message: diagnostics.network_error_message,
            typed_connector_response: None,
            raw_connector_response: None,
            raw_connector_request: None,
            typed_connector_request: None,
        }))
    } else {
        Ok(Ok(PaymentsResponseData::try_from(response)?))
    }
}

impl TryFrom<&responses::JpmorganRefundResponse> for RefundsResponseData {
    type Error = ResponseError;
    fn try_from(item: &responses::JpmorganRefundResponse) -> Result<Self, Self::Error> {
        let refund_status = responses::RefundStatus::from((
            item.response_status.clone(),
            item.transaction_state.clone(),
        ))
        .into();

        Ok(Self {
            connector_refund_id: item.transaction_id.clone(),
            refund_status,
            status_code: item.response_code.parse::<u16>().unwrap_or(0),
            acquirer_reference_number: None,
        })
    }
}

// Bridge pattern implementations for RouterDataV2

impl<T: PaymentMethodDataTypes, F>
    TryFrom<ResponseRouterData<responses::JpmorganPaymentsResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = ResponseError;
    fn try_from(
        item: ResponseRouterData<responses::JpmorganPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = AttemptStatus::try_from(&item.response)?;
        let response = build_payments_response_result(&item.response, item.http_code, status)?;
        let (amount_captured, amount_capturable, amount_authorized) =
            get_partial_authorization_amounts(
                &item.response,
                item.router_data.request.capture_method,
                item.router_data.request.enable_partial_authorization,
                item.router_data.request.currency,
            );

        Ok(Self {
            response,
            resource_common_data: PaymentFlowData {
                status,
                amount_captured,
                amount_capturable,
                amount_authorized,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

impl<F> TryFrom<ResponseRouterData<responses::JpmorganPSyncResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = ResponseError;
    fn try_from(
        item: ResponseRouterData<responses::JpmorganPSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let resource = requests::JpmorganSyncResource::from_request(&item.router_data.request)
            .change_context(requests::JpmorganSyncResource::response_context(
                item.http_code,
            ))?;
        let (amount_captured, amount_capturable, amount_authorized) = match &item.response {
            responses::JpmorganPSyncResponse::Payment(payment) => {
                get_partial_authorization_amounts(
                    payment,
                    item.router_data.request.capture_method,
                    payment.partial_authorization,
                    item.router_data.request.currency,
                )
            }
            responses::JpmorganPSyncResponse::Verification(_) => (None, None, None),
        };
        let (status, response) = match (resource, item.response) {
            (
                requests::JpmorganSyncResource::Payment(_),
                responses::JpmorganPSyncResponse::Payment(payment),
            ) => {
                let payment = *payment;
                let status = AttemptStatus::try_from(&payment)?;
                let response = build_payments_response_result(&payment, item.http_code, status)?;
                (status, response)
            }
            (
                requests::JpmorganSyncResource::Verification(id),
                responses::JpmorganPSyncResponse::Verification(verification),
            ) => {
                let verification = *verification;
                if verification.transaction_id != id {
                    return Err(requests::JpmorganSyncResource::response_error(
                        item.http_code,
                    ));
                }
                let status = verification.status(item.http_code)?;
                (status, verification.into_payment_response(item.http_code)?)
            }
            _ => {
                return Err(requests::JpmorganSyncResource::response_error(
                    item.http_code,
                ));
            }
        };

        Ok(Self {
            response,
            resource_common_data: PaymentFlowData {
                status,
                amount_captured,
                amount_capturable,
                amount_authorized,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

impl<F> TryFrom<ResponseRouterData<responses::JpmorganPaymentsResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = ResponseError;
    fn try_from(
        item: ResponseRouterData<responses::JpmorganPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = match AttemptStatus::try_from(&item.response)? {
            AttemptStatus::Failure => AttemptStatus::CaptureFailed,
            status => status,
        };
        let response = build_payments_response_result(&item.response, item.http_code, status)?;

        Ok(Self {
            response,
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

impl<F> TryFrom<ResponseRouterData<responses::JpmorganPaymentsResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = ResponseError;
    fn try_from(
        item: ResponseRouterData<responses::JpmorganPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = match AttemptStatus::try_from(&item.response)? {
            AttemptStatus::Failure => AttemptStatus::VoidFailed,
            status => status,
        };
        let response = build_payments_response_result(&item.response, item.http_code, status)?;

        Ok(Self {
            response,
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

impl<F> TryFrom<ResponseRouterData<responses::JpmorganRefundResponse, Self>>
    for RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = ResponseError;
    fn try_from(
        item: ResponseRouterData<responses::JpmorganRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = responses::RefundStatus::from((
            item.response.response_status.clone(),
            item.response.transaction_state.clone(),
        ))
        .into();
        let response_data = RefundsResponseData::try_from(&item.response)?;

        Ok(Self {
            response: Ok(response_data),
            resource_common_data: RefundFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

impl<F> TryFrom<ResponseRouterData<responses::JpmorganRefundResponse, Self>>
    for RouterDataV2<F, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = ResponseError;
    fn try_from(
        item: ResponseRouterData<responses::JpmorganRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = responses::RefundStatus::from((
            item.response.response_status.clone(),
            item.response.transaction_state.clone(),
        ))
        .into();
        let response_data = RefundsResponseData::try_from(&item.response)?;

        Ok(Self {
            response: Ok(response_data),
            resource_common_data: RefundFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ---- ClientAuthenticationToken flow types ----

/// Obtains an OAuth2 access token from JPMorgan for client-side SDK initialization.
/// The access_token serves as the client authentication token.
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        JpmorganRouterData<
            RouterDataV2<
                ClientAuthenticationToken,
                MerchantAuthenticationFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for requests::JpmorganClientAuthRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: JpmorganRouterData<
            RouterDataV2<
                ClientAuthenticationToken,
                MerchantAuthenticationFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        JpmorganAuthType::try_from(&item.router_data.connector_config)?.token_request(
            item.router_data
                .resource_common_data
                .connectors
                .jpmorgan
                .secondary_base_url
                .as_deref(),
        )
    }
}

impl TryFrom<ResponseRouterData<responses::JpmorganClientAuthResponse, Self>>
    for RouterDataV2<
        ClientAuthenticationToken,
        MerchantAuthenticationFlowData,
        ClientAuthenticationTokenRequestData,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<responses::JpmorganClientAuthResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;

        let session_data = ClientAuthenticationTokenData::ConnectorSpecific(Box::new(
            ConnectorSpecificClientAuthenticationResponse::Jpmorgan(
                JpmorganClientAuthenticationResponseDomain {
                    transaction_id: response.access_token.peek().to_string(),
                    request_id: response.token_type,
                },
            ),
        ));

        Ok(Self {
            response: Ok(PaymentsResponseData::ClientAuthenticationTokenResponse {
                session_data,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

impl TryFrom<&JpmorganAuthType> for requests::JpmorganMerchant {
    type Error = Error;
    fn try_from(auth: &JpmorganAuthType) -> Result<Self, Self::Error> {
        Ok(Self {
            merchant_software: requests::JpmorganMerchantSoftware {
                company_name: auth.company_name.clone().ok_or(
                    IntegrationError::MissingRequiredField {
                        field_name: "company_name",
                        context: jpmorgan_missing_field_context("company_name"),
                    },
                )?,
                product_name: auth.product_name.clone().ok_or(
                    IntegrationError::MissingRequiredField {
                        field_name: "product_name",
                        context: jpmorgan_missing_field_context("product_name"),
                    },
                )?,
            },
            soft_merchant: auth.merchant_purchase_description.clone().map(|d| {
                requests::JpmorganSoftMerchant {
                    merchant_purchase_description: d,
                }
            }),
        })
    }
}

// Build JPMorgan Expiry from card data (shared by SetupMandate / RepeatPayment).
fn build_jpmorgan_expiry(month: &str, year: &str) -> Result<requests::Expiry, Error> {
    let month = month
        .parse::<i32>()
        .change_context(IntegrationError::RequestEncodingFailed {
            context: Default::default(),
        })?;
    let year = year
        .parse::<i32>()
        .change_context(IntegrationError::RequestEncodingFailed {
            context: Default::default(),
        })?;
    Ok(requests::Expiry {
        month: Secret::new(month),
        year: Secret::new(year),
    })
}

// SetupMandate verifies credentials without a funds hold.
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        JpmorganRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for requests::JpmorganSetupMandateRequest<T>
{
    type Error = Error;
    fn try_from(
        item: JpmorganRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        let request = &router_data.request;
        let context = IntegrationErrorContext {
            suggested_action: Some("Use zero-value setup with a card or decrypted wallet. Use Authorize for a payment.".to_owned()),
            ..Default::default()
        };
        let amount = request
            .amount
            .as_ref()
            .map(|money| money.amount)
            .ok_or_else(|| IntegrationError::MissingRequiredField {
                field_name: "amount",
                context: context.clone(),
            })?;
        if amount != MinorUnit::zero() {
            return Err(IntegrationError::NotSupported {
                message: "Nonzero setup amount".to_owned(),
                connector: "jpmorgan",
                context,
            }
            .into());
        }
        requests::JpmorganCard::validate_apple_pay_storage(&request.payment_method_data)?;
        let recurring_sequence = match request.mit_category {
            Some(
                common_enums::MitCategory::Recurring | common_enums::MitCategory::Subscription,
            ) => Some(requests::JpmorganRecurringSequence::First),
            Some(
                common_enums::MitCategory::Installment | common_enums::MitCategory::Resubmission,
            ) => {
                return Err(IntegrationError::NotSupported {
                    message: "Installment or resubmission agreements".to_owned(),
                    connector: "jpmorgan",
                    context: Default::default(),
                }
                .into())
            }
            _ => None,
        };
        let card = match &request.payment_method_data {
            PaymentMethodData::Card(card) => requests::JpmorganSetupMandateCard {
                card: requests::JpmorganCard {
                    account_number: card.card_number.clone(),
                    expiry: build_jpmorgan_expiry(
                        card.card_exp_month.peek(),
                        card.get_expiry_year_4_digit().peek(),
                    )?,
                    account_number_type: Some(requests::JpmorganAccountNumberType::Pan),
                    wallet_provider: None,
                    authentication: requests::JpmorganCard::<T>::card_authentication(
                        request.authentication_data.as_ref(),
                        router_data.resource_common_data.auth_type,
                        card.card_network.as_ref(),
                    )?,
                    merchant_sales_channel_name:
                        map_payment_channel_to_jpmorgan_merchant_sales_channel_name(
                            request.payment_channel.as_ref(),
                        ),
                },
                cvv: Some(card.get_card_cvc_required()?),
            },
            PaymentMethodData::CardWithNoCvc(card) => requests::JpmorganSetupMandateCard {
                card: requests::JpmorganCard {
                    account_number: RawCardNumber(T::inner_from_card_number(
                        card.card_number.clone(),
                    )),
                    expiry: build_jpmorgan_expiry(
                        card.card_exp_month.peek(),
                        card.get_expiry_year_4_digit().peek(),
                    )?,
                    account_number_type: Some(requests::JpmorganAccountNumberType::Pan),
                    wallet_provider: None,
                    authentication: requests::JpmorganCard::<T>::card_authentication(
                        request.authentication_data.as_ref(),
                        router_data.resource_common_data.auth_type,
                        card.card_network.as_ref(),
                    )?,
                    merchant_sales_channel_name:
                        map_payment_channel_to_jpmorgan_merchant_sales_channel_name(
                            request.payment_channel.as_ref(),
                        ),
                },
                cvv: None,
            },
            PaymentMethodData::Wallet(WalletData::ApplePay(_))
            | PaymentMethodData::Wallet(WalletData::GooglePay(_))
                if request.authentication_data.is_some() =>
            {
                return Err(IntegrationError::NotSupported {
                    message: "Separate card 3DS proof for wallet verification".to_owned(),
                    connector: "jpmorgan",
                    context: Default::default(),
                }
                .into());
            }
            PaymentMethodData::Wallet(WalletData::ApplePay(wallet)) => match &wallet.payment_data {
                ApplePayPaymentData::Decrypted(data) => requests::JpmorganSetupMandateCard {
                    card: requests::JpmorganCard::from_apple_pay(
                        data,
                        request.payment_channel.as_ref(),
                    )?,
                    cvv: None,
                },
                ApplePayPaymentData::Encrypted(_) => {
                    return Err(IntegrationError::NotSupported {
                        message: "Encrypted Apple Pay verification".to_owned(),
                        connector: "jpmorgan",
                        context,
                    }
                    .into())
                }
            },
            PaymentMethodData::Wallet(WalletData::GooglePay(wallet)) => {
                match &wallet.tokenization_data {
                    domain_types::payment_method_data::GpayTokenizationData::Decrypted(data) => {
                        requests::JpmorganSetupMandateCard {
                            card: requests::JpmorganCard::from_google_pay(
                                data,
                                request.payment_channel.as_ref(),
                            )?,
                            cvv: None,
                        }
                    }
                    domain_types::payment_method_data::GpayTokenizationData::Encrypted(_) => {
                        return Err(IntegrationError::NotSupported {
                            message: "Encrypted Google Pay verification".to_owned(),
                            connector: "jpmorgan",
                            context,
                        }
                        .into())
                    }
                }
            }
            _ => {
                return Err(IntegrationError::NotSupported {
                    message: "Verification for this payment method".to_owned(),
                    connector: "jpmorgan",
                    context,
                }
                .into())
            }
        };
        let auth = JpmorganAuthType::try_from(&router_data.connector_config)?;
        Ok(Self {
            currency: request.currency,
            merchant: requests::JpmorganMerchant::try_from(&auth)?,
            payment_method_type: requests::JpmorganSetupMandatePaymentMethodType { card },
            recurring_sequence,
            merchant_order_number: merchant_order_number(
                request.merchant_order_id.as_deref(),
                &router_data
                    .resource_common_data
                    .connector_request_reference_id,
            ),
            initiator_type: requests::JpmorganInitiatorType::Cardholder,
            account_on_file: requests::JpmorganAccountOnFile::ToBeStored,
            statement_descriptor: get_statement_descriptor(
                router_data.request.billing_descriptor.as_ref(),
            ),
        })
    }
}

// SetupMandate response transformer
impl<T: PaymentMethodDataTypes>
    TryFrom<ResponseRouterData<responses::JpmorganSetupMandateResponse, Self>>
    for RouterDataV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    >
{
    type Error = ResponseError;
    fn try_from(
        item: ResponseRouterData<responses::JpmorganSetupMandateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let verification = item.response;
        if verification.currency != item.router_data.request.currency {
            return Err(requests::JpmorganSyncResource::response_error(
                item.http_code,
            ));
        }
        let status = verification.status(item.http_code)?;
        let response = verification.into_payment_response(item.http_code)?;

        Ok(Self {
            response,
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

impl<T: PaymentMethodDataTypes> requests::JpmorganRepeatPaymentRequest<T> {
    fn unsupported(message: &str) -> Error {
        IntegrationError::NotSupported {
            message: message.to_owned(),
            connector: "jpmorgan",
            context: Default::default(),
        }
        .into()
    }

    fn network_references(
        reference: &MandateReferenceId,
    ) -> Result<(String, Option<String>), Error> {
        let (ntid, tlid) = match reference {
            MandateReferenceId::NetworkMandateId(network) =>
                (network.network_transaction_id.clone(), network.transaction_link_id.clone()),
            MandateReferenceId::NetworkTokenWithNTI(network) =>
                (network.network_transaction_id.clone(), network.transaction_link_id.clone()),
            MandateReferenceId::ConnectorMandateId(_) => return Err(Self::unsupported(
                "Processor tokens without complete Safetech credentials are not supported by this direct network-reference flow",
            )),
        };
        Ok((ntid, tlid))
    }

    fn mit_recurring(
        request: &RepeatPaymentData<T>,
    ) -> Result<Option<requests::JpmorganRecurring>, Error> {
        match request.mit_category {
            Some(common_enums::MitCategory::Unscheduled) => Ok(None),
            Some(
                common_enums::MitCategory::Recurring | common_enums::MitCategory::Subscription,
            ) => Ok(Some(requests::JpmorganRecurring {
                recurring_sequence: requests::JpmorganRecurringSequence::Subsequent,
                agreement_id: None,
                is_variable_amount: Some(true),
            })),
            None => Err(IntegrationError::MissingRequiredField {
                field_name: "mit_category",
                context: Default::default(),
            }
            .into()),
            Some(
                common_enums::MitCategory::Installment | common_enums::MitCategory::Resubmission,
            ) => Err(Self::unsupported(
                "Installment and resubmission merchant-initiated payments",
            )),
        }
    }

    fn mit_card(
        request: &RepeatPaymentData<T>,
        ntid: String,
        tlid: Option<String>,
    ) -> Result<requests::JpmorganMitCardByNti<T>, Error> {
        let (number, month, year, provider, supplied_network) =
            match &request.payment_method_data {
                PaymentMethodData::CardDetailsForNetworkTransactionId(card) => (
                    card.card_number.clone(),
                    i32::from(*card.get_expiry_month_as_i8()?.peek()),
                    card.get_expiry_year_4_digit()
                        .peek()
                        .parse::<i32>()
                        .change_context(IntegrationError::RequestEncodingFailed {
                            context: Default::default(),
                        })?,
                    None,
                    card.card_network.clone(),
                ),
                PaymentMethodData::NetworkToken(token) => (
                    RawCardNumber(T::inner_from_card_number(
                        token
                            .token_number
                            .get_card_no()
                            .parse::<cards::CardNumber>()
                            .map_err(|_| IntegrationError::InvalidDataFormat {
                                field_name: "network_token.token_number",
                                context: Default::default(),
                            })?,
                    )),
                    token
                        .get_network_token_expiry_month()
                        .peek()
                        .parse::<i32>()
                        .change_context(IntegrationError::RequestEncodingFailed {
                            context: Default::default(),
                        })?,
                    token
                        .get_expiry_year_4_digit()
                        .peek()
                        .parse::<i32>()
                        .change_context(IntegrationError::RequestEncodingFailed {
                            context: Default::default(),
                        })?,
                    None,
                    token.card_network.clone(),
                ),
                PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(token) => {
                    let provider = match token.token_source {
                        Some(domain_types::payment_method_data::TokenSource::ApplePay) => {
                            requests::JpmorganWalletProvider::ApplePay
                        }
                        Some(domain_types::payment_method_data::TokenSource::GooglePay) => {
                            requests::JpmorganWalletProvider::GooglePay
                        }
                        None => {
                            return Err(IntegrationError::MissingRequiredField {
                                field_name: "decrypted_wallet_token.token_source",
                                context: Default::default(),
                            }
                            .into())
                        }
                    };
                    (
                        RawCardNumber(T::inner_from_card_number(
                            token
                                .decrypted_token
                                .get_card_no()
                                .parse::<cards::CardNumber>()
                                .map_err(|_| IntegrationError::InvalidDataFormat {
                                    field_name: "decrypted_wallet_token.decrypted_token",
                                    context: Default::default(),
                                })?,
                        )),
                        i32::from(*token.get_expiry_month_as_i8()?.peek()),
                        token
                            .get_expiry_year_4_digit()
                            .peek()
                            .parse::<i32>()
                            .change_context(IntegrationError::RequestEncodingFailed {
                                context: Default::default(),
                            })?,
                        Some(provider),
                        token.card_network.clone(),
                    )
                }
                PaymentMethodData::Card(_)
                | PaymentMethodData::CardWithNoCvc(_)
                | PaymentMethodData::CardRedirect(_)
                | PaymentMethodData::Wallet(_)
                | PaymentMethodData::PayLater(_)
                | PaymentMethodData::BankRedirect(_)
                | PaymentMethodData::BankDebit(_)
                | PaymentMethodData::BankTransfer(_)
                | PaymentMethodData::Crypto(_)
                | PaymentMethodData::MandatePayment
                | PaymentMethodData::Reward
                | PaymentMethodData::RealTimePayment(_)
                | PaymentMethodData::Upi(_)
                | PaymentMethodData::Voucher(_)
                | PaymentMethodData::GiftCard(_)
                | PaymentMethodData::PaymentMethodToken(_)
                | PaymentMethodData::OpenBanking(_)
                | PaymentMethodData::MobilePayment(_) => return Err(Self::unsupported(
                    "MIT requires card details for a network transaction ID, a network token, or decrypted wallet token details",
                )),
            };
        let network = supplied_network.ok_or(IntegrationError::MissingRequiredField {
            field_name: "payment_method_data.card_network",
            context: Default::default(),
        })?;
        if network == common_enums::CardNetwork::Mastercard && tlid.is_none() {
            return Err(IntegrationError::MissingRequiredField {
                field_name: "mandate_reference.transaction_link_id",
                context: Default::default(),
            }
            .into());
        }
        Ok(requests::JpmorganMitCardByNti {
            account_number: number,
            expiry: requests::Expiry {
                month: Secret::new(month),
                year: Secret::new(year),
            },
            wallet_provider: provider,
            original_network_transaction_id: ntid,
            original_transaction_link_id: tlid,
            merchant_sales_channel_name:
                map_payment_channel_to_jpmorgan_merchant_sales_channel_name(
                    request.payment_channel.as_ref(),
                ),
        })
    }
}

// RepeatPayment (subsequent MIT) request transformer
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        JpmorganRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for requests::JpmorganRepeatPaymentRequest<T>
{
    type Error = Error;
    fn try_from(
        item: JpmorganRouterData<
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

        let auth = JpmorganAuthType::try_from(&router_data.connector_config)?;
        let merchant = requests::JpmorganMerchant::try_from(&auth)?;

        let request = &router_data.request;
        let (ntid, tlid) = Self::network_references(&request.mandate_reference)?;
        let capture_method = map_capture_method(router_data.request.capture_method)?;
        let amount = item
            .connector
            .amount_converter
            .convert(
                router_data.request.amount.amount,
                router_data.request.currency,
            )
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;

        let payment_method_type = requests::JpmorganRepeatPaymentMethodType {
            card: Self::mit_card(request, ntid, tlid)?,
        };
        let recurring = Self::mit_recurring(request)?;

        Ok(Self {
            capture_method,
            amount,
            currency: router_data.request.currency,
            merchant,
            payment_method_type,
            recurring,
            merchant_order_number: merchant_order_number(
                request.merchant_order_id.as_deref(),
                &router_data
                    .resource_common_data
                    .connector_request_reference_id,
            ),
            initiator_type: requests::JpmorganInitiatorType::Merchant,
            account_on_file: requests::JpmorganAccountOnFile::Stored,
            is_amount_final: true,
            statement_descriptor: get_statement_descriptor(
                router_data.request.billing_descriptor.as_ref(),
            ),
            partial_authorization_support: Some(map_partial_authorization_support(
                router_data.request.enable_partial_authorization,
            )),
        })
    }
}

// RepeatPayment response transformer
impl<T: PaymentMethodDataTypes>
    TryFrom<ResponseRouterData<responses::JpmorganPaymentsResponse, Self>>
    for RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = ResponseError;
    fn try_from(
        item: ResponseRouterData<responses::JpmorganPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = AttemptStatus::try_from(&item.response)?;
        let response = build_payments_response_result(&item.response, item.http_code, status)?;
        let (amount_captured, amount_capturable, amount_authorized) =
            get_partial_authorization_amounts(
                &item.response,
                item.router_data.request.capture_method,
                item.router_data.request.enable_partial_authorization,
                item.router_data.request.currency,
            );

        Ok(Self {
            response,
            resource_common_data: PaymentFlowData {
                status,
                amount_captured,
                amount_capturable,
                amount_authorized,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}
