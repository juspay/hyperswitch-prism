use std::fmt::Debug;

use common_enums::{
    AttemptStatus, CaptureMethod, CardNetwork, FutureUsage, MitCategory, PaymentChannel,
    RefundStatus,
};
use domain_types::{
    connector_flow::{
        Authorize, Capture, CreateConnectorCustomer, PSync, RSync, Refund, RepeatPayment,
        SetupMandate, Void,
    },
    connector_types::{
        ConnectorCustomerData, ConnectorCustomerResponse, MandateIds, MandateReference,
        MandateReferenceId, PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData,
        PaymentsCaptureData, PaymentsResponseData, PaymentsSyncData, RecurringMandatePaymentData,
        RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData, RepeatPaymentData,
        ResponseId, SetupMandateRequestData,
    },
    errors::{ConnectorError, IntegrationError},
    payment_method_data::{
        Card, CardDetailsForNetworkTransactionId, PaymentMethodData, PaymentMethodDataTypes,
    },
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
};
use error_stack::{Report, ResultExt};
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use super::{
    requests::{
        TsysXmlAddCustomerCardDetails, TsysXmlAddCustomerRequest, TsysXmlAddCustomerWalletDetails,
        TsysXmlAdditionalTaxDetails, TsysXmlAuthorizationIndicator, TsysXmlAuthorizeBody,
        TsysXmlAuthorizeRequest, TsysXmlBillingType, TsysXmlCaptureRequest,
        TsysXmlCardAuthenticationRequest, TsysXmlCardDataInputMode,
        TsysXmlCardDataOutputCapability, TsysXmlCardDataSource, TsysXmlCardOnFile,
        TsysXmlCardPresentDetail, TsysXmlCardholderAuthenticationEntity,
        TsysXmlCardholderAuthenticationMethod, TsysXmlCardholderPresentDetail,
        TsysXmlCommercialCardLevel, TsysXmlIsRecurring, TsysXmlMaxPinLength,
        TsysXmlMcCitStatusIndicator, TsysXmlMit, TsysXmlMitIndicator, TsysXmlPersonalDetails,
        TsysXmlProductDetails, TsysXmlProductDiscountDetails, TsysXmlProductDiscountIndicator,
        TsysXmlProductModifierDetails, TsysXmlProductTaxDetails, TsysXmlRegisteredUserIndicator,
        TsysXmlRepeatPaymentRequest, TsysXmlReturnRequest, TsysXmlTerminalAuthenticationCapability,
        TsysXmlTerminalCapability, TsysXmlTerminalCardCaptureCapability,
        TsysXmlTerminalOperatingEnvironment, TsysXmlTerminalOutputCapability,
        TsysXmlTransactionInquiryRequest, TsysXmlVoidRequest, TsysXmlWalletDetailsRef,
        TsysXmlYesNo,
    },
    responses::{
        TsysXmlAddCustomerResponse, TsysXmlAuthorizeResponse, TsysXmlCaptureResponse,
        TsysXmlCardAuthenticationResponse, TsysXmlRepeatPaymentResponse, TsysXmlReturnResponse,
        TsysXmlStatus, TsysXmlTransactionInquiryResponse, TsysXmlTransactionState,
        TsysXmlVoidResponse,
    },
    TsysXmlRouterData,
};
use crate::types::ResponseRouterData;

// =============================================================================
// Connector metadata schema (parsed from `PaymentsAuthorizeData.metadata`)
// =============================================================================

/// Top-level wrapper — the merchant supplies `connector_metadata.tsys_xml.{...}`.
#[derive(Debug, Default, Deserialize, Clone)]
struct TsysXmlMerchantMetadata {
    #[serde(default)]
    tsys_xml: Option<TsysXmlMerchantMetadataInner>,
}

#[derive(Debug, Default, Deserialize, Clone)]
struct TsysXmlMerchantMetadataInner {
    #[serde(default)]
    acceptor: Option<TsysXmlAcceptorMetadata>,
    #[serde(default)]
    terminal_data: Option<TsysXmlTerminalDataOverrides>,
    #[serde(default)]
    commercial_card: Option<TsysXmlCommercialCardMetadata>,
}

#[derive(Debug, Default, Deserialize, Clone)]
struct TsysXmlAcceptorMetadata {
    street_address: Option<String>,
    customer_service_phone: Option<String>,
    phone: Option<String>,
    url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
enum TsysXmlCommercialCardLevelMetadata {
    Level2,
    Level3,
}

#[derive(Debug, Default, Clone, Deserialize)]
struct TsysXmlCommercialCardMetadata {
    level: Option<TsysXmlCommercialCardLevelMetadata>,
    purchase_order: Option<String>,
    charge_descriptor: Option<String>,
    charge_descriptor_2: Option<String>,
    charge_descriptor_3: Option<String>,
    charge_descriptor_4: Option<String>,
    customer_ref_id: Option<String>,
    supplier_reference_number: Option<String>,
    customer_vat_number: Option<String>,
    order_date: Option<String>,
    summary_commodity_code: Option<String>,
    vat_invoice: Option<String>,
    ship_from_zip: Option<String>,
    ship_to_zip: Option<String>,
    destination_country_code: Option<String>,
    tax_type: Option<String>,
    tax_category: Option<String>,
    tax_rate: Option<String>,
}

/// Mandate-level metadata carried via `RecurringMandatePaymentData.mandate_metadata`.
///
/// Everything that cert needs but HS has no native field for. Only consulted
/// inside `compute_recurring_context()`.
#[derive(Debug, Default, Deserialize, Clone)]
struct TsysXmlMandateMetadata {
    /// Total installment payments. Required when `mit_category == Installment`.
    #[serde(default)]
    payment_count: Option<u32>,
    /// Which payment in the installment series.
    /// Required when `mit_category == Installment`.
    #[serde(default)]
    current_payment_count: Option<u32>,
    /// MC Recurring sub-discriminator: `"standing"` (default → C102 / M102) or
    /// `"subscription"` (→ C103 / M103). HS's `MitCategory::Recurring`
    /// collapses both; this lets cert tests pick the right MC intent code.
    #[serde(default)]
    mc_subtype: Option<String>,
    /// Disc/JCB/Diners/CUP Installment `<mitIndicator>` override: `"s"`
    /// (default) or `"t"`.
    #[serde(default)]
    installment_variant: Option<String>,
}

#[derive(Debug, Default, Deserialize, Clone)]
struct TsysXmlTerminalDataOverrides {
    terminal_capability: Option<TsysXmlTerminalCapability>,
    terminal_operating_environment: Option<TsysXmlTerminalOperatingEnvironment>,
    cardholder_authentication_method: Option<TsysXmlCardholderAuthenticationMethod>,
    terminal_authentication_capability: Option<TsysXmlTerminalAuthenticationCapability>,
    terminal_output_capability: Option<TsysXmlTerminalOutputCapability>,
    max_pin_length: Option<TsysXmlMaxPinLength>,
    terminal_card_capture_capability: Option<TsysXmlTerminalCardCaptureCapability>,
    cardholder_present_detail: Option<TsysXmlCardholderPresentDetail>,
    card_present_detail: Option<TsysXmlCardPresentDetail>,
    card_data_input_mode: Option<TsysXmlCardDataInputMode>,
    cardholder_authentication_entity: Option<TsysXmlCardholderAuthenticationEntity>,
    card_data_output_capability: Option<TsysXmlCardDataOutputCapability>,
}

/// Resolved Recurring/Installment context for a single Authorize / Setup call.
///
/// Built by `compute_recurring_context()` from `metadata.tsys_xml.recurring`.
/// Carries everything the downstream body-builders need — string-typed
/// metadata values are parsed into the strongly-typed enums here so the
/// transformer body sites stay free of `match` / `parse` plumbing.
#[derive(Debug, Default, Clone)]
struct RecurringContext {
    /// True when the merchant supplied `metadata.tsys_xml.recurring`.
    /// Drives terminalData preset switching and `<cvv2>` suppression.
    enabled: bool,
    /// `Some(Y)` when we should emit `<isRecurring>Y</isRecurring>`. Defaults
    /// to `Some(Y)` when `enabled` is true unless the merchant explicitly set
    /// `is_recurring=false`.
    is_recurring_flag: Option<TsysXmlIsRecurring>,
    /// Resolved `<billingType>`.
    billing_type: Option<TsysXmlBillingType>,
    payment_count: Option<u32>,
    current_payment_count: Option<u32>,
    /// MC CIT only (Step 4). Parsed from `recurring.mc_cit_status_indicator`.
    mc_cit_status_indicator: Option<TsysXmlMcCitStatusIndicator>,
    /// Public recurring samples emit `<mitStatusIndicator>` for MasterCard
    /// (`M102` / `M103` / `M104`) and Discover-family (`R` / `S` / `T`) MITs.
    mit_status_indicator: Option<TsysXmlMitIndicator>,
    /// Discover/JCB/Diners/CUP MIT only. Minor units — emit conversion happens
    /// at the body-build site using the connector's `StringMajorUnit` helper.
    original_recurring_amount_minor: Option<i64>,
}

#[derive(Debug, Default, Clone)]
struct CommercialCardContext {
    sales_tax: Option<common_utils::types::StringMajorUnit>,
    additional_tax_details: Vec<TsysXmlAdditionalTaxDetails>,
    shipping_charges: Option<common_utils::types::StringMajorUnit>,
    duty_charges: Option<common_utils::types::StringMajorUnit>,
    product_details: Vec<TsysXmlProductDetails>,
    commercial_card_level: Option<TsysXmlCommercialCardLevel>,
    purchase_order: Option<String>,
    charge_descriptor: Option<String>,
    charge_descriptor_2: Option<String>,
    charge_descriptor_3: Option<String>,
    charge_descriptor_4: Option<String>,
    customer_vat_number: Option<String>,
    customer_ref_id: Option<String>,
    supplier_reference_number: Option<String>,
    order_date: Option<String>,
    summary_commodity_code: Option<String>,
    vat_invoice: Option<String>,
    ship_from_zip: Option<String>,
    ship_to_zip: Option<String>,
    destination_country_code: Option<String>,
}

#[derive(Debug, Default, Clone)]
struct ThreeDsContext {
    secure_code: Option<Secret<String>>,
    ucaf_collection_indicator: Option<String>,
    directory_server_transaction_id: Option<String>,
    eci_indicator: Option<String>,
}

fn compute_three_ds_context<
    T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize,
>(
    router_data: &RouterDataV2<
        Authorize,
        PaymentFlowData,
        PaymentsAuthorizeData<T>,
        PaymentsResponseData,
    >,
    _card_network: Option<&CardNetwork>,
) -> ThreeDsContext {
    let authentication_data = router_data.request.authentication_data.as_ref();
    let secure_code = authentication_data.and_then(|data| data.cavv.clone());
    let ucaf_collection_indicator =
        authentication_data.and_then(|data| data.ucaf_collection_indicator.clone());
    let directory_server_transaction_id =
        authentication_data.and_then(|data| data.ds_trans_id.clone());
    let eci_indicator = authentication_data.and_then(|data| data.eci.clone());

    ThreeDsContext {
        secure_code,
        ucaf_collection_indicator,
        directory_server_transaction_id,
        eci_indicator,
    }
}

/// Build a `RecurringContext` from HS-native inputs.
///
/// Drives the transformer's recurring/installment branch entirely off
/// `mit_category` (+ `recurring_mandate_payment_data` for amount/counters and
/// brand-specific cert quirks). Returns an empty (`enabled=false`) context for
/// `None | Some(Unscheduled) | Some(Resubmission)` so non-recurring callers
/// short-circuit cleanly.
fn compute_recurring_context(
    mit_category: Option<MitCategory>,
    recurring_data: Option<&RecurringMandatePaymentData>,
    card_network: Option<&CardNetwork>,
) -> Result<RecurringContext, Report<IntegrationError>> {
    let (is_recurring_flag, billing_type) = match mit_category.as_ref() {
        Some(MitCategory::Recurring) => (Some(TsysXmlIsRecurring::Y), None),
        Some(MitCategory::Installment) => (
            Some(TsysXmlIsRecurring::Y),
            Some(TsysXmlBillingType::Installment),
        ),
        // Unscheduled / Resubmission / None → recurring presets do not apply.
        Some(MitCategory::Unscheduled) | Some(MitCategory::Resubmission) | None => {
            return Ok(RecurringContext::default())
        }
    };

    // `mandate_metadata` carries everything HS native fields can't express
    // (installment counters + MC standing-vs-subscription).
    let mm = match recurring_data.and_then(|d| d.mandate_metadata.as_ref()) {
        Some(raw) => serde_json::from_value::<TsysXmlMandateMetadata>(raw.peek().clone())
            .change_context(IntegrationError::InvalidDataFormat {
                field_name: "recurring_mandate_payment_data.mandate_metadata",
                context: Default::default(),
            })?,
        None => TsysXmlMandateMetadata::default(),
    };

    // Cert script: Installment Sale/Auth must carry both <paymentCount> and
    // <currentPaymentCount>. Fail closed if either is missing.
    if matches!(mit_category.as_ref(), Some(MitCategory::Installment))
        && (mm.payment_count.is_none() || mm.current_payment_count.is_none())
    {
        return Err(IntegrationError::MissingRequiredField {
            field_name:
                "recurring_mandate_payment_data.mandate_metadata.{payment_count,current_payment_count} required when mit_category=Installment",
            context: Default::default(),
        }
        .into());
    }

    let discover_family_mit_indicator = match (mit_category.as_ref(), card_network) {
        (
            Some(MitCategory::Recurring),
            Some(CardNetwork::Discover)
            | Some(CardNetwork::JCB)
            | Some(CardNetwork::DinersClub)
            | Some(CardNetwork::UnionPay),
        ) => Some(TsysXmlMitIndicator::R),
        (
            Some(MitCategory::Installment),
            Some(CardNetwork::Discover)
            | Some(CardNetwork::JCB)
            | Some(CardNetwork::DinersClub)
            | Some(CardNetwork::UnionPay),
        ) => Some(
            if mm
                .installment_variant
                .as_deref()
                .is_some_and(|s| s.eq_ignore_ascii_case("t"))
            {
                TsysXmlMitIndicator::T
            } else {
                TsysXmlMitIndicator::S
            },
        ),
        _ => None,
    };

    // MC C102/C103/C104 (CIT) and M102/M103/M104 (MIT) per cert intent codes.
    let (mc_cit_status_indicator, mc_mit_status_indicator) =
        match (mit_category.as_ref(), card_network) {
            (Some(MitCategory::Recurring), Some(CardNetwork::Mastercard)) => {
                let is_subscription = mm
                    .mc_subtype
                    .as_deref()
                    .is_some_and(|s| s.eq_ignore_ascii_case("subscription"));
                if is_subscription {
                    (
                        Some(TsysXmlMcCitStatusIndicator::C103),
                        Some(TsysXmlMitIndicator::M103),
                    )
                } else {
                    (
                        Some(TsysXmlMcCitStatusIndicator::C102),
                        Some(TsysXmlMitIndicator::M102),
                    )
                }
            }
            (Some(MitCategory::Installment), Some(CardNetwork::Mastercard)) => (
                Some(TsysXmlMcCitStatusIndicator::C104),
                Some(TsysXmlMitIndicator::M104),
            ),
            _ => (None, None),
        };

    // <originalRecurringAmount> comes from HS-native original_payment_authorized_amount.
    let original_recurring_amount_minor = recurring_data
        .and_then(|d| d.original_payment_authorized_amount.as_ref())
        .map(|m| m.get_amount_as_i64());

    Ok(RecurringContext {
        enabled: true,
        is_recurring_flag,
        billing_type,
        payment_count: mm.payment_count,
        current_payment_count: mm.current_payment_count,
        mc_cit_status_indicator,
        mit_status_indicator: mc_mit_status_indicator.or(discover_family_mit_indicator),
        original_recurring_amount_minor,
    })
}

/// Auth bundle for TsysXml (TransIT) — flattened into the XML request body.
///
/// TransIT does not use HTTP auth headers; instead each request carries the
/// `deviceID`, `transactionKey`, and `developerID` inline in the XML payload.
#[derive(Debug, Clone)]
pub struct TsysXmlAuthType {
    pub device_id: Secret<String>,
    pub transaction_key: Secret<String>,
    pub developer_id: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for TsysXmlAuthType {
    type Error = Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::TsysXml {
                device_id,
                transaction_key,
                developer_id,
                ..
            } => Ok(Self {
                device_id: device_id.to_owned(),
                transaction_key: transaction_key.to_owned(),
                developer_id: developer_id.to_owned(),
            }),
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            }
            .into()),
        }
    }
}

/// Minimal error envelope for TsysXml.
///
/// TransIT signals failure with `<status>FAIL</status>` and supplies a
/// `<responseCode>` / `<responseMessage>` pair. The exact element layout will be
/// hardened further per-flow; this scaffold provides only what
/// `build_error_response` needs.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct TsysXmlErrorResponse {
    #[serde(rename = "status", default, alias = "Status")]
    pub status: Option<String>,
    #[serde(rename = "responseCode", default, alias = "ResponseCode")]
    pub response_code: Option<String>,
    #[serde(rename = "responseMessage", default, alias = "ResponseMessage")]
    pub response_message: Option<String>,
}

// =============================================================================
// AUTHORIZE — request transformer
// =============================================================================

fn format_expiration_date(card: &Card<impl PaymentMethodDataTypes>) -> Secret<String> {
    // TransIT documents `MM/YY` (tech spec § Sale/Auth Field Reference). Normalize
    // 4-digit years down to 2 digits.
    let month = card.card_exp_month.peek().clone();
    let year_full = card.card_exp_year.peek().clone();
    let year_short = if year_full.len() == 4 {
        year_full[2..].to_string()
    } else {
        year_full
    };
    Secret::new(format!("{}/{}", month, year_short))
}

fn format_decimal(value: f64) -> String {
    let mut rendered = format!("{value:.4}");
    while rendered.contains('.') && rendered.ends_with('0') {
        rendered.pop();
    }
    if rendered.ends_with('.') {
        rendered.push('0');
    }
    rendered
}

fn truncate_chars(value: &str, max_len: usize) -> String {
    value.chars().take(max_len).collect()
}

fn sanitize_alphanumeric_space(value: &str, max_len: usize) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || ch.is_ascii_whitespace())
        .take(max_len)
        .collect()
}

fn sanitize_optional_alphanumeric_space(value: Option<String>, max_len: usize) -> Option<String> {
    value
        .map(|value| sanitize_alphanumeric_space(&value, max_len))
        .filter(|value| !value.is_empty())
}

fn normalize_tsys_order_date(value: Option<String>) -> Option<String> {
    value.map(|date| {
        let mut parts = date.split('-');
        match (parts.next(), parts.next(), parts.next(), parts.next()) {
            (Some(year), Some(month), Some(day), None)
                if year.len() == 4 && month.len() == 2 && day.len() == 2 =>
            {
                format!("{month}/{day}/{year}")
            }
            _ => date,
        }
    })
}

fn normalize_tsys_country_code(value: Option<String>) -> Option<String> {
    value.map(|code| match code.as_str() {
        "840" => "USA".to_string(),
        _ => code,
    })
}

fn format_country_alpha3(country: common_enums::CountryAlpha2) -> String {
    common_enums::CountryAlpha2::from_alpha2_to_alpha3(country).to_string()
}

fn compute_commercial_card_context<
    T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize,
>(
    router_data: &RouterDataV2<
        Authorize,
        PaymentFlowData,
        PaymentsAuthorizeData<T>,
        PaymentsResponseData,
    >,
    commercial_meta: Option<&TsysXmlCommercialCardMetadata>,
    card_network: Option<&CardNetwork>,
) -> Result<CommercialCardContext, Report<IntegrationError>> {
    let Some(commercial_meta) = commercial_meta else {
        return Ok(CommercialCardContext::default());
    };

    let commercial_card_level = match commercial_meta.level {
        Some(TsysXmlCommercialCardLevelMetadata::Level2) => TsysXmlCommercialCardLevel::Level2,
        Some(TsysXmlCommercialCardLevelMetadata::Level3) => TsysXmlCommercialCardLevel::Level3,
        None => {
            return Err(IntegrationError::MissingRequiredField {
                field_name: "metadata.tsys_xml.commercial_card.level",
                context: Default::default(),
            }
            .into())
        }
    };
    let is_level3 = matches!(commercial_card_level, TsysXmlCommercialCardLevel::Level3);
    let is_visa_or_mastercard = matches!(
        card_network,
        Some(CardNetwork::Visa) | Some(CardNetwork::Mastercard)
    );
    let is_mastercard = matches!(card_network, Some(CardNetwork::Mastercard));
    let is_amex = matches!(card_network, Some(CardNetwork::AmericanExpress));
    let zero_amount = super::TsysXmlAmountConvertor::convert(
        common_utils::types::MinorUnit::new(0),
        router_data.request.currency,
    )?;

    let l2_l3_data = router_data.resource_common_data.l2_l3_data.as_deref();
    let _billing_descriptor = router_data.request.billing_descriptor.as_ref();
    let shipping_address = router_data.resource_common_data.get_shipping_address().ok();
    let billing_address = router_data.resource_common_data.get_billing_address().ok();
    let connector_request_reference_id = router_data
        .resource_common_data
        .connector_request_reference_id
        .clone();

    let order_details = l2_l3_data
        .and_then(|data| data.get_order_details())
        .or_else(|| router_data.resource_common_data.order_details.clone())
        .unwrap_or_default();
    let order_tax_amount = l2_l3_data
        .and_then(|data| data.get_order_tax_amount())
        .or(router_data.request.order_tax_amount);
    let order_reference = l2_l3_data
        .and_then(|data| data.get_merchant_order_reference_id())
        .or_else(|| router_data.request.merchant_order_id.clone());

    let sales_tax = order_tax_amount
        .map(|amount| super::TsysXmlAmountConvertor::convert(amount, router_data.request.currency))
        .transpose()?;
    let shipping_charges = l2_l3_data
        .and_then(|data| data.get_shipping_cost())
        .or(router_data.request.shipping_cost)
        .map(|amount| super::TsysXmlAmountConvertor::convert(amount, router_data.request.currency))
        .transpose()?;
    let duty_charges = l2_l3_data
        .and_then(|data| data.get_duty_amount())
        .map(|amount| super::TsysXmlAmountConvertor::convert(amount, router_data.request.currency))
        .transpose()?;

    let derived_tax_rate = commercial_meta
        .tax_rate
        .clone()
        .or_else(|| {
            order_details
                .iter()
                .find_map(|detail| detail.tax_rate.map(format_decimal))
        })
        .or_else(|| {
            let transaction_amount = router_data.request.minor_amount.get_amount_as_i64();
            let sales_tax_amount = order_tax_amount.map(|amount| amount.get_amount_as_i64())?;
            if transaction_amount == 0 || sales_tax_amount == 0 {
                None
            } else {
                Some(format_decimal(
                    (sales_tax_amount as f64 / transaction_amount as f64) * 100.0,
                ))
            }
        })
        .or_else(|| is_level3.then_some("0".to_string()));

    let additional_tax_details = if is_level3 && is_visa_or_mastercard {
        let tax_amount =
            sales_tax
                .clone()
                .ok_or_else(|| IntegrationError::MissingRequiredField {
                    field_name:
                        "salesTax required for metadata.tsys_xml.commercial_card.level=level3",
                    context: Default::default(),
                })?;

        vec![TsysXmlAdditionalTaxDetails {
            tax_type: commercial_meta
                .tax_type
                .clone()
                .unwrap_or_else(|| "VAT".to_string()),
            tax_amount,
            tax_rate: Some(derived_tax_rate.clone().unwrap_or_else(|| "0".to_string())),
            tax_category: Some(
                commercial_meta
                    .tax_category
                    .clone()
                    .unwrap_or_else(|| "VAT".to_string()),
            ),
        }]
    } else {
        Vec::new()
    };

    let product_details = if is_level3 && is_visa_or_mastercard {
        if order_details.is_empty() {
            return Err(IntegrationError::MissingRequiredField {
                field_name:
                    "order_details required for metadata.tsys_xml.commercial_card.level=level3",
                context: Default::default(),
            }
            .into());
        }

        order_details
            .iter()
            .map(|detail| {
                let price = super::TsysXmlAmountConvertor::convert(
                    detail.amount,
                    router_data.request.currency,
                )?;
                let unit_discount_amount = detail
                    .unit_discount_amount
                    .map(|amount| {
                        super::TsysXmlAmountConvertor::convert(amount, router_data.request.currency)
                    })
                    .transpose()?
                    .unwrap_or_else(|| zero_amount.clone());
                let has_discount = detail
                    .unit_discount_amount
                    .map(|amount| amount.get_amount_as_i64() > 0)
                    .unwrap_or(false);
                let discount_percentage = detail.unit_discount_amount.and_then(|discount| {
                    let line_amount = detail.amount.get_amount_as_i64();
                    (line_amount > 0).then(|| {
                        format_decimal(
                            (discount.get_amount_as_i64() as f64 / line_amount as f64) * 100.0,
                        )
                    })
                });
                let product_tax_amount = detail
                    .total_tax_amount
                    .map(|amount| {
                        super::TsysXmlAmountConvertor::convert(amount, router_data.request.currency)
                    })
                    .transpose()?
                    .unwrap_or_else(|| zero_amount.clone());
                let product_commodity_code = detail
                    .commodity_code
                    .clone()
                    .or_else(|| commercial_meta.summary_commodity_code.clone())
                    .or_else(|| detail.upc.clone())
                    .or_else(|| detail.product_id.clone())
                    .or_else(|| detail.sku.clone())
                    .map(|code| sanitize_alphanumeric_space(&code, 12));

                if is_visa_or_mastercard && product_commodity_code.is_none() {
                    return Err(IntegrationError::MissingRequiredField {
                        field_name: "productCommodityCode required for Visa Level 3",
                        context: Default::default(),
                    }
                    .into());
                }

                Ok(TsysXmlProductDetails {
                    product_code: detail
                        .product_id
                        .clone()
                        .or_else(|| detail.sku.clone())
                        .or_else(|| detail.upc.clone())
                        .map(|code| sanitize_alphanumeric_space(&code, 20))
                        .filter(|code| !code.is_empty())
                        .unwrap_or_else(|| sanitize_alphanumeric_space(&detail.product_name, 20)),
                    product_name: truncate_chars(&detail.product_name, 50),
                    price,
                    quantity: u32::from(detail.quantity),
                    measurement_unit: detail
                        .unit_of_measure
                        .clone()
                        .or_else(|| Some("EA".to_string())),
                    product_discount_details: Some(TsysXmlProductDiscountDetails {
                        product_discount_name: "Line Item Discount".to_string(),
                        product_discount_amount: unit_discount_amount,
                        product_discount_percentage: discount_percentage,
                        product_discount_type: "DISCOUNT".to_string(),
                        priority: 1,
                        stackable: if has_discount {
                            TsysXmlYesNo::Yes
                        } else {
                            TsysXmlYesNo::No
                        },
                    }),
                    product_tax_details: Some(TsysXmlProductTaxDetails {
                        product_tax_name: detail
                            .product_tax_code
                            .clone()
                            .or_else(|| Some("TAX".to_string())),
                        product_tax_amount: Some(product_tax_amount),
                        product_tax_percentage: Some(
                            detail
                                .tax_rate
                                .map(format_decimal)
                                .or_else(|| derived_tax_rate.clone())
                                .unwrap_or_else(|| "0".to_string()),
                        ),
                        product_tax_type: detail
                            .product_tax_code
                            .clone()
                            .map(|tax_code| truncate_chars(&tax_code, 4))
                            .or_else(|| commercial_meta.tax_type.clone()),
                    }),
                    product_variation: detail
                        .sub_category
                        .clone()
                        .or_else(|| detail.category.clone()),
                    product_modifier_details: detail
                        .brand
                        .clone()
                        .or_else(|| detail.category.clone())
                        .map(|modifier_name| TsysXmlProductModifierDetails {
                            modifier_name: truncate_chars(&modifier_name, 50),
                            modifier_value: detail
                                .sub_category
                                .clone()
                                .or_else(|| detail.description.clone())
                                .map(|value| truncate_chars(&value, 25)),
                            modifier_price: None,
                        }),
                    product_notes: detail
                        .description
                        .clone()
                        .map(|description| truncate_chars(&description, 100)),
                    product_discount_indicator: Some(if has_discount {
                        TsysXmlProductDiscountIndicator::Y
                    } else {
                        TsysXmlProductDiscountIndicator::N
                    }),
                    product_commodity_code,
                })
            })
            .collect::<Result<Vec<_>, Report<IntegrationError>>>()?
    } else if is_level3 {
        // AMEX Level III rows do not explicitly require these fields in the
        // available certification matrix; keep enrichment best-effort only.
        order_details
            .iter()
            .map(|detail| {
                let price = super::TsysXmlAmountConvertor::convert(
                    detail.amount,
                    router_data.request.currency,
                )?;
                let unit_discount_amount = detail
                    .unit_discount_amount
                    .map(|amount| {
                        super::TsysXmlAmountConvertor::convert(amount, router_data.request.currency)
                    })
                    .transpose()?
                    .unwrap_or_else(|| zero_amount.clone());
                let has_discount = detail
                    .unit_discount_amount
                    .map(|amount| amount.get_amount_as_i64() > 0)
                    .unwrap_or(false);
                let discount_percentage = detail.unit_discount_amount.and_then(|discount| {
                    let line_amount = detail.amount.get_amount_as_i64();
                    (line_amount > 0).then(|| {
                        format_decimal(
                            (discount.get_amount_as_i64() as f64 / line_amount as f64) * 100.0,
                        )
                    })
                });
                let product_tax_amount = detail
                    .total_tax_amount
                    .map(|amount| {
                        super::TsysXmlAmountConvertor::convert(amount, router_data.request.currency)
                    })
                    .transpose()?
                    .unwrap_or_else(|| zero_amount.clone());
                let product_commodity_code = detail
                    .commodity_code
                    .clone()
                    .or_else(|| commercial_meta.summary_commodity_code.clone())
                    .or_else(|| detail.upc.clone())
                    .or_else(|| detail.product_id.clone())
                    .or_else(|| detail.sku.clone())
                    .map(|code| sanitize_alphanumeric_space(&code, 12));

                Ok(TsysXmlProductDetails {
                    product_code: detail
                        .product_id
                        .clone()
                        .or_else(|| detail.sku.clone())
                        .or_else(|| detail.upc.clone())
                        .map(|code| sanitize_alphanumeric_space(&code, 20))
                        .filter(|code| !code.is_empty())
                        .unwrap_or_else(|| sanitize_alphanumeric_space(&detail.product_name, 20)),
                    product_name: truncate_chars(&detail.product_name, 50),
                    price,
                    quantity: u32::from(detail.quantity),
                    measurement_unit: detail
                        .unit_of_measure
                        .clone()
                        .or_else(|| Some("EA".to_string())),
                    product_discount_details: Some(TsysXmlProductDiscountDetails {
                        product_discount_name: "Line Item Discount".to_string(),
                        product_discount_amount: unit_discount_amount,
                        product_discount_percentage: discount_percentage,
                        product_discount_type: "DISCOUNT".to_string(),
                        priority: 1,
                        stackable: if has_discount {
                            TsysXmlYesNo::Yes
                        } else {
                            TsysXmlYesNo::No
                        },
                    }),
                    product_tax_details: Some(TsysXmlProductTaxDetails {
                        product_tax_name: detail
                            .product_tax_code
                            .clone()
                            .or_else(|| Some("TAX".to_string())),
                        product_tax_amount: Some(product_tax_amount),
                        product_tax_percentage: Some(
                            detail
                                .tax_rate
                                .map(format_decimal)
                                .or_else(|| derived_tax_rate.clone())
                                .unwrap_or_else(|| "0".to_string()),
                        ),
                        product_tax_type: detail
                            .product_tax_code
                            .clone()
                            .map(|tax_code| truncate_chars(&tax_code, 4))
                            .or_else(|| commercial_meta.tax_type.clone()),
                    }),
                    product_variation: detail
                        .sub_category
                        .clone()
                        .or_else(|| detail.category.clone()),
                    product_modifier_details: detail
                        .brand
                        .clone()
                        .or_else(|| detail.category.clone())
                        .map(|modifier_name| TsysXmlProductModifierDetails {
                            modifier_name: truncate_chars(&modifier_name, 50),
                            modifier_value: detail
                                .sub_category
                                .clone()
                                .or_else(|| detail.description.clone())
                                .map(|value| truncate_chars(&value, 25)),
                            modifier_price: None,
                        }),
                    product_notes: detail
                        .description
                        .clone()
                        .map(|description| truncate_chars(&description, 100)),
                    product_discount_indicator: Some(if has_discount {
                        TsysXmlProductDiscountIndicator::Y
                    } else {
                        TsysXmlProductDiscountIndicator::N
                    }),
                    product_commodity_code,
                })
            })
            .collect::<Result<Vec<_>, Report<IntegrationError>>>()?
    } else {
        Vec::new()
    };

    let purchase_order = sanitize_optional_alphanumeric_space(
        commercial_meta
            .purchase_order
            .clone()
            .or_else(|| order_reference.clone())
            .or_else(|| Some(connector_request_reference_id.clone())),
        25,
    );
    let charge_descriptor = commercial_meta.charge_descriptor.clone();
    let supplier_reference_number = (!is_level3 || is_amex)
        .then(|| {
            sanitize_optional_alphanumeric_space(
                commercial_meta
                    .supplier_reference_number
                    .clone()
                    .or_else(|| order_reference.clone())
                    .or_else(|| Some(connector_request_reference_id.clone())),
                9,
            )
        })
        .flatten();
    let customer_vat_number =
        sanitize_optional_alphanumeric_space(commercial_meta.customer_vat_number.clone(), 13);
    let customer_ref_id = (!is_level3 || is_amex)
        .then(|| {
            sanitize_optional_alphanumeric_space(
                commercial_meta
                    .customer_ref_id
                    .clone()
                    .or_else(|| order_reference.clone())
                    .or_else(|| Some(connector_request_reference_id.clone())),
                17,
            )
        })
        .flatten();
    let order_date = normalize_tsys_order_date(commercial_meta.order_date.clone());
    let summary_commodity_code =
        sanitize_optional_alphanumeric_space(commercial_meta.summary_commodity_code.clone(), 4);
    let vat_invoice = sanitize_optional_alphanumeric_space(commercial_meta.vat_invoice.clone(), 15);
    let ship_from_zip = commercial_meta.ship_from_zip.clone();
    let ship_to_zip = commercial_meta.ship_to_zip.clone().or_else(|| {
        l2_l3_data
            .and_then(|data| data.get_shipping_zip())
            .map(|zip| zip.expose())
            .or_else(|| {
                shipping_address
                    .and_then(|address| address.zip.clone())
                    .map(|zip| zip.expose())
            })
            .or_else(|| {
                billing_address
                    .and_then(|address| address.zip.clone())
                    .map(|zip| zip.expose())
            })
    });
    let destination_country_code =
        normalize_tsys_country_code(commercial_meta.destination_country_code.clone().or_else(
            || {
                l2_l3_data
                    .and_then(|data| data.get_shipping_country())
                    .map(format_country_alpha3)
                    .or_else(|| {
                        shipping_address
                            .and_then(|address| address.country)
                            .map(format_country_alpha3)
                    })
                    .or_else(|| {
                        billing_address
                            .and_then(|address| address.country)
                            .map(format_country_alpha3)
                    })
            },
        ));

    if is_level3 && is_visa_or_mastercard {
        if sales_tax.is_none() {
            return Err(IntegrationError::MissingRequiredField {
                field_name:
                    "salesTax required for TSYS commercial-card Level III (Visa/Mastercard)",
                context: Default::default(),
            }
            .into());
        }
        if purchase_order.is_none() {
            return Err(IntegrationError::MissingRequiredField {
                field_name: "purchaseOrder required for Visa/Mastercard Level III",
                context: Default::default(),
            }
            .into());
        }
        if shipping_charges.is_none() {
            return Err(IntegrationError::MissingRequiredField {
                field_name: "shippingCharges required for Visa/Mastercard Level III",
                context: Default::default(),
            }
            .into());
        }
        if duty_charges.is_none() {
            return Err(IntegrationError::MissingRequiredField {
                field_name: "dutyCharges required for Visa/Mastercard Level III",
                context: Default::default(),
            }
            .into());
        }
        if is_mastercard
            && destination_country_code
                .as_ref()
                .is_none_or(|code| code.len() != 3)
        {
            return Err(IntegrationError::MissingRequiredField {
                field_name:
                    "destinationCountryCode required and must be 3-digit for Mastercard Level III",
                context: Default::default(),
            }
            .into());
        }
    }

    if matches!(commercial_card_level, TsysXmlCommercialCardLevel::Level2) {
        if is_visa_or_mastercard && purchase_order.is_none() {
            return Err(IntegrationError::MissingRequiredField {
                field_name: "purchaseOrder required for Visa/Mastercard Level II",
                context: Default::default(),
            }
            .into());
        }
        if sales_tax.is_none() {
            return Err(IntegrationError::MissingRequiredField {
                field_name: "salesTax required for TSYS commercial-card Level II",
                context: Default::default(),
            }
            .into());
        }
    }

    if matches!(commercial_card_level, TsysXmlCommercialCardLevel::Level2) && is_amex {
        for (field_name, is_missing) in [
            (
                "supplierReferenceNumber",
                supplier_reference_number.is_none(),
            ),
            ("customerRefID", customer_ref_id.is_none()),
            ("shipToZip", ship_to_zip.is_none()),
            ("chargeDescriptor", charge_descriptor.is_none()),
        ] {
            if is_missing {
                return Err(IntegrationError::MissingRequiredField {
                    field_name,
                    context: Default::default(),
                }
                .into());
            }
        }
    }

    if is_level3 && is_visa_or_mastercard {
        for (field_name, is_missing) in [
            ("purchaseOrder", purchase_order.is_none()),
            ("orderDate", order_date.is_none()),
            ("summaryCommodityCode", summary_commodity_code.is_none()),
            ("vatInvoice", vat_invoice.is_none()),
            ("shipFromZip", ship_from_zip.is_none()),
            ("shipToZip", ship_to_zip.is_none()),
            ("destinationCountryCode", destination_country_code.is_none()),
        ] {
            if is_missing {
                return Err(IntegrationError::MissingRequiredField {
                    field_name,
                    context: Default::default(),
                }
                .into());
            }
        }
    }

    if is_level3 && matches!(card_network, Some(CardNetwork::Visa)) && customer_vat_number.is_none()
    {
        return Err(IntegrationError::MissingRequiredField {
            field_name: "customerVATNumber required for Visa Level 3",
            context: Default::default(),
        }
        .into());
    }

    if is_level3 && is_visa_or_mastercard && additional_tax_details.is_empty() {
        return Err(IntegrationError::MissingRequiredField {
            field_name: "additionalTaxDetails required for Visa/Mastercard Level III",
            context: Default::default(),
        }
        .into());
    }

    Ok(CommercialCardContext {
        sales_tax,
        additional_tax_details,
        shipping_charges,
        duty_charges,
        product_details,
        commercial_card_level: Some(commercial_card_level),
        purchase_order: purchase_order.clone(),
        charge_descriptor,
        charge_descriptor_2: commercial_meta.charge_descriptor_2.clone(),
        charge_descriptor_3: commercial_meta.charge_descriptor_3.clone(),
        charge_descriptor_4: commercial_meta.charge_descriptor_4.clone(),
        customer_vat_number,
        customer_ref_id,
        supplier_reference_number,
        order_date,
        summary_commodity_code,
        vat_invoice,
        ship_from_zip,
        ship_to_zip,
        destination_country_code,
    })
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TsysXmlRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for TsysXmlAuthorizeRequest<T>
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: TsysXmlRouterData<
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
        let auth = TsysXmlAuthType::try_from(&router_data.connector_config)?;

        // Mandate-driven dispatch: when the upstream HS request supplies a
        // `connector_mandate_id` we recognize one of:
        //   - `cust:CCC:WWW`  → Path B (vault token MIT). Omit PAN/expiry/cvv2;
        //                       emit customerCode + walletDetails.
        //   - `ntid:XXX`      → Path A (network-token MIT). Keep PAN, emit
        //                       previousNetworkTransactionID + cardOnFile + mit.
        //   - everything else → fall through to CIT / one-shot logic (PAN-bearing).
        // We split on the FIRST ':' to find the prefix so that walletIDs / NTIDs
        // containing colons still round-trip correctly.
        let mandate_dispatch = decode_mandate_dispatch(router_data.request.mandate_id.as_ref());

        // CIT signal (no prior mandate but caller intends to store creds).
        let is_cit_setup = matches!(mandate_dispatch, MandateDispatch::None)
            && (router_data.request.setup_future_usage == Some(FutureUsage::OffSession)
                || router_data.request.off_session == Some(true));

        // Path B (vault) does NOT need card data — we emit customerCode + walletID
        // instead. Every other branch (Path A / CIT / one-shot) needs card-bearing
        // data. CIT and one-shot arrive as `PaymentMethodData::Card`; Path A MIT
        // replays from HS arrive as `PaymentMethodData::CardDetailsForNetworkTransactionId`
        // (no CVV — cert forbids `<cvv2>` on recurring/installment anyway).
        let card_opt = match &router_data.request.payment_method_data {
            PaymentMethodData::Card(card) => Some(card),
            _ => None,
        };
        let nti_card_opt: Option<&CardDetailsForNetworkTransactionId> =
            match &router_data.request.payment_method_data {
                PaymentMethodData::CardDetailsForNetworkTransactionId(nti) => Some(nti),
                _ => None,
            };
        if matches!(mandate_dispatch, MandateDispatch::Vault { .. }) {
            // Vault path doesn't read card_opt / nti_card_opt — keep them as-is and
            // the downstream branch handles the customerCode/walletID emission.
        } else if card_opt.is_none() && nti_card_opt.is_none() {
            return Err(IntegrationError::NotSupported {
                message: "Selected payment method".to_string(),
                connector: "tsys_xml",
                context: Default::default(),
            }
            .into());
        }
        let card = card_opt;

        let transaction_amount = super::TsysXmlAmountConvertor::convert(
            router_data.request.minor_amount,
            router_data.request.currency,
        )?;

        // Billing address fields used by AVS (addressLine1 + zip). Both REQUIRED
        // by the e-commerce certification script.
        let billing = router_data
            .resource_common_data
            .address
            .get_payment_billing()
            .and_then(|b| b.address.as_ref());
        let address_line1 = billing.and_then(|a| a.line1.clone()).ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "billing.address.line1",
                context: Default::default(),
            })
        })?;
        let zip = billing.and_then(|a| a.zip.clone()).ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "billing.address.zip",
                context: Default::default(),
            })
        })?;

        // Card network drives several MC/AMEX/Discover-only fields AND the
        // brand-specific MIT/CIT indicator derivation. On Path B (vault MIT)
        // no card object is available — we skip the network-driven optional
        // fields entirely. Path A MIT (NTID) carries network via the
        // CardDetailsForNetworkTransactionId variant.
        let card_network = card
            .and_then(|c| c.card_network.clone())
            .or_else(|| nti_card_opt.and_then(|n| n.card_network.clone()));

        // Parse connector metadata. Recurring is driven natively from
        // `mit_category`; the metadata layer only carries acceptor, terminal,
        // and explicit commercial-card opt-in details.
        let merchant_metadata_early = match router_data.request.metadata.as_ref() {
            Some(meta) => serde_json::from_value::<TsysXmlMerchantMetadata>(meta.clone().expose())
                .change_context(IntegrationError::InvalidDataFormat {
                    field_name: "connector_metadata.tsys_xml",
                    context: Default::default(),
                })?,
            None => TsysXmlMerchantMetadata::default(),
        };
        let merchant_inner_early = merchant_metadata_early.tsys_xml.unwrap_or_default();
        let commercial_meta = merchant_inner_early.commercial_card.clone();
        let acceptor_meta = merchant_inner_early.acceptor;
        let terminal_overrides = merchant_inner_early.terminal_data.unwrap_or_default();

        // Build recurring context from HS-native fields. Returns enabled=false
        // for non-recurring flows, so all downstream branches degrade cleanly.
        // `recurring_mandate_payment_data` lives on `PaymentFlowData` (the
        // resource_common_data side) — not on `PaymentsAuthorizeData`.
        let recurring_context = compute_recurring_context(
            router_data.request.mit_category.clone(),
            router_data
                .resource_common_data
                .recurring_mandate_payment_data
                .as_ref(),
            card_network.as_ref(),
        )?;
        let commercial_card_context = compute_commercial_card_context(
            router_data,
            commercial_meta.as_ref(),
            card_network.as_ref(),
        )?;
        let three_ds_context = compute_three_ds_context(router_data, card_network.as_ref());

        // Channel-driven cardDataSource selection — replaces the previous
        // hardcoded Internet default. In recurring/installment context the
        // cert script forbids INTERNET and requires MAIL or PHONE; we default
        // to MAIL when no explicit channel is supplied.
        let channel = router_data.request.payment_channel.clone();
        let card_data_source = match channel {
            Some(PaymentChannel::TelephoneOrder) => TsysXmlCardDataSource::Phone,
            Some(PaymentChannel::MailOrder) => TsysXmlCardDataSource::Mail,
            Some(PaymentChannel::Ecommerce) | None => {
                if recurring_context.enabled {
                    TsysXmlCardDataSource::Mail
                } else {
                    TsysXmlCardDataSource::Internet
                }
            }
        };

        // Capture method drives MC/AMEX authorizationIndicator.
        let is_manual_capture = matches!(
            router_data.request.capture_method,
            Some(CaptureMethod::Manual) | Some(CaptureMethod::ManualMultiple)
        );

        let authorization_indicator = match card_network {
            Some(CardNetwork::Mastercard) => {
                if recurring_context.enabled && is_manual_capture {
                    None
                } else {
                    Some(if is_manual_capture {
                        TsysXmlAuthorizationIndicator::Preauth
                    } else {
                        TsysXmlAuthorizationIndicator::Final
                    })
                }
            }
            Some(CardNetwork::AmericanExpress) => Some(if is_manual_capture {
                TsysXmlAuthorizationIndicator::Preauth
            } else {
                TsysXmlAuthorizationIndicator::Final
            }),
            _ => None,
        };

        // Acceptor fields — MC only, all four required together.
        let (
            acceptor_street_address,
            acceptor_customer_service_phone_number,
            acceptor_phone_number,
            acceptor_url_address,
        ) = if matches!(card_network, Some(CardNetwork::Mastercard))
            && !(recurring_context.enabled && is_manual_capture)
        {
            let a = acceptor_meta.ok_or_else(|| {
                error_stack::report!(IntegrationError::MissingRequiredField {
                    field_name: "metadata.tsys_xml.acceptor.* required for MasterCard",
                    context: Default::default(),
                })
            })?;
            match (a.street_address, a.customer_service_phone, a.phone, a.url) {
                (Some(s), Some(cs), Some(p), Some(u)) => (Some(s), Some(cs), Some(p), Some(u)),
                _ => {
                    return Err(IntegrationError::MissingRequiredField {
                        field_name: "metadata.tsys_xml.acceptor.* required for MasterCard",
                        context: Default::default(),
                    }
                    .into());
                }
            }
        } else if matches!(card_network, Some(CardNetwork::Mastercard))
            && recurring_context.enabled
            && is_manual_capture
        {
            // TSYS accepts the recurring MIT Sale samples with the MC acceptor
            // block, but Auth/PreAuth-style recurring MITs are still XSD-probing.
            // Keep the manual-capture recurring branch minimal until TSYS shows
            // the acceptor group is allowed on this root message shape.
            (None, None, None, None)
        } else {
            (None, None, None, None)
        };

        // The public recurring/installment keyed samples for Discover/JCB/Diners
        // do not emit these fields on the MIT path. Keep them for non-recurring
        // flows only until TSYS cert/XSD requires otherwise.
        let (registered_user_indicator, last_registered_change_date) = if recurring_context.enabled
        {
            (None, None)
        } else {
            match card_network {
                Some(CardNetwork::Discover)
                | Some(CardNetwork::JCB)
                | Some(CardNetwork::DinersClub)
                | Some(CardNetwork::UnionPay) => (
                    Some(TsysXmlRegisteredUserIndicator::No),
                    Some("00/00/0000".to_string()),
                ),
                _ => (None, None),
            }
        };

        // terminalData fields — flat in the XSD. Each field is resolved as:
        //   1. explicit merchant override (`metadata.tsys_xml.terminal_data.*`)
        //   2. recurring/installment preset (cert script § Authorization Requirements
        //      for Recurring/Installments) — only when `recurring_context.enabled`
        //   3. channel-driven preset (e-commerce / MOTO)
        //   4. baseline default
        let terminal_capability = terminal_overrides
            .terminal_capability
            .unwrap_or(TsysXmlTerminalCapability::KeyedEntryOnly);
        // Recurring/installment terminalOperatingEnvironment per cert:
        //   - MC: NO_TERMINAL
        //   - all other brands: OFF_MERCHANT_PREMISES_UNATTENDED
        let terminal_operating_environment = terminal_overrides
            .terminal_operating_environment
            .unwrap_or_else(|| {
                if recurring_context.enabled {
                    match card_network {
                        Some(CardNetwork::Mastercard) => {
                            TsysXmlTerminalOperatingEnvironment::NoTerminal
                        }
                        _ => TsysXmlTerminalOperatingEnvironment::OffMerchantPremisesUnattended,
                    }
                } else {
                    TsysXmlTerminalOperatingEnvironment::NoTerminal
                }
            });
        let cardholder_authentication_method = terminal_overrides
            .cardholder_authentication_method
            .unwrap_or(TsysXmlCardholderAuthenticationMethod::NotAuthenticated);
        let terminal_authentication_capability = terminal_overrides
            .terminal_authentication_capability
            .unwrap_or(TsysXmlTerminalAuthenticationCapability::NoCapability);
        // Recurring cert requires DISPLAY_ONLY; e-com path keeps the existing
        // `None` baseline.
        let terminal_output_capability = terminal_overrides
            .terminal_output_capability
            .unwrap_or_else(|| {
                if recurring_context.enabled {
                    TsysXmlTerminalOutputCapability::DisplayOnly
                } else {
                    TsysXmlTerminalOutputCapability::None
                }
            });
        let max_pin_length = terminal_overrides
            .max_pin_length
            .unwrap_or(TsysXmlMaxPinLength::NotSupported);
        let terminal_card_capture_capability = terminal_overrides
            .terminal_card_capture_capability
            .unwrap_or(TsysXmlTerminalCardCaptureCapability::NoCapability);
        // Recurring/installment cardholderPresentDetail:
        //   - installment → CARDHOLDER_NOT_PRESENT_INSTALLMENT_TRANSACTION
        //   - recurring   → CARDHOLDER_NOT_PRESENT_RECURRING_TRANSACTION
        // MC requires the RECURRING variant on both CIT and MIT of a recurring
        // series — which falls out naturally because the merchant flips
        // `billing_type=INSTALLMENT` only for installment rows.
        let cardholder_present_detail = terminal_overrides
            .cardholder_present_detail
            .unwrap_or_else(|| {
                if recurring_context.enabled {
                    if recurring_context.billing_type.is_some() {
                        TsysXmlCardholderPresentDetail::CardholderNotPresentInstallmentTransaction
                    } else {
                        TsysXmlCardholderPresentDetail::CardholderNotPresentRecurringTransaction
                    }
                } else {
                    match channel {
                        Some(PaymentChannel::TelephoneOrder) => {
                            TsysXmlCardholderPresentDetail::CardholderNotPresentPhoneTransaction
                        }
                        Some(PaymentChannel::MailOrder) => {
                            TsysXmlCardholderPresentDetail::CardholderNotPresentMailTransaction
                        }
                        _ => TsysXmlCardholderPresentDetail::CardholderNotPresentElectronicCommerce,
                    }
                }
            });
        let card_present_detail = terminal_overrides
            .card_present_detail
            .unwrap_or(TsysXmlCardPresentDetail::CardNotPresent);
        // Recurring/installment requires MIT_STORED_ON_FILE — overrides the
        // channel-driven default.
        let card_data_input_mode = terminal_overrides.card_data_input_mode.unwrap_or_else(|| {
            if recurring_context.enabled {
                TsysXmlCardDataInputMode::MerchantInitiatedTransactionCardCredentialStoredOnFile
            } else {
                match channel {
                    Some(PaymentChannel::Ecommerce) | None => {
                        TsysXmlCardDataInputMode::PanEntryElectronicCommerceIncludingRemoteChip
                    }
                    _ => TsysXmlCardDataInputMode::KeyEnteredInput,
                }
            }
        });
        let cardholder_authentication_entity = terminal_overrides
            .cardholder_authentication_entity
            .unwrap_or(TsysXmlCardholderAuthenticationEntity::NotAuthenticated);
        let card_data_output_capability = terminal_overrides
            .card_data_output_capability
            .unwrap_or(TsysXmlCardDataOutputCapability::None);

        // Path-specific card-source fields: Path A / CIT / one-shot carry PAN;
        // Path B carries customerCode + walletDetails instead.
        let (card_number, expiration_date, cvv2_opt, customer_code_opt, wallet_details_opt) =
            if let MandateDispatch::Vault {
                customer_code,
                wallet_id,
            } = &mandate_dispatch
            {
                (
                    None,
                    None,
                    None,
                    Some(Secret::new(customer_code.clone())),
                    Some(TsysXmlWalletDetailsRef {
                        wallet_id: Secret::new(wallet_id.clone()),
                    }),
                )
            } else if let Some(card) = card {
                // cert: `<cvv2>` must NOT be sent on recurring / installment.
                let cvv = if recurring_context.enabled || card.card_cvc.peek().is_empty() {
                    None
                } else {
                    Some(card.card_cvc.clone())
                };
                (
                    Some(Secret::new(card.card_number.peek().to_string())),
                    Some(format_expiration_date(card)),
                    cvv,
                    None,
                    None,
                )
            } else if let Some(nti) = nti_card_opt {
                // Path A MIT replay: CardDetailsForNetworkTransactionId has card_number
                // + expiry but NO CVV (cert forbids cvv2 on recurring/installment).
                // Normalize expiry to MM/YY identically to format_expiration_date.
                let month = nti.card_exp_month.peek().clone();
                let year_full = nti.card_exp_year.peek().clone();
                let year_short = if year_full.len() == 4 {
                    year_full[2..].to_string()
                } else {
                    year_full
                };
                (
                    Some(Secret::new(nti.card_number.peek().to_string())),
                    Some(Secret::new(format!("{}/{}", month, year_short))),
                    None,
                    None,
                    None,
                )
            } else {
                // Unreachable — guarded above; fail closed if reached.
                return Err(IntegrationError::NotSupported {
                    message: "Selected payment method".to_string(),
                    connector: "tsys_xml",
                    context: Default::default(),
                }
                .into());
            };

        // cardOnFile + MIT block + COFTI / previousNetworkTransactionID — driven
        // jointly by `mandate_dispatch` and `recurring_context`.
        //
        // Field routing in recurring/MIT mode:
        //   - NTID dispatch → emit `<cardOnFileTransactionIdentifier>` for the
        //     brands whose published recurring samples carry it (Visa +
        //     Discover/JCB/Diners/CUP). MasterCard recurring uses
        //     `mitStatusIndicator` without COFTI on the public sample page.
        //   - Vault dispatch → no NTID-style field.
        //
        // TransIT Sale accepts network-transaction-id replay through
        // `<cardOnFileTransactionIdentifier>` for both recurring and
        // unscheduled COF flows. `<previousNetworkTransactionID>` is not valid
        // at this point in the Sale element order.
        let (
            card_on_file,
            mit_block,
            previous_network_transaction_id,
            card_on_file_transaction_identifier,
        ) = match (
            &mandate_dispatch,
            recurring_context.enabled,
            card_network.as_ref(),
        ) {
            (MandateDispatch::Ntid { .. }, true, Some(CardNetwork::Mastercard))
            | (MandateDispatch::Ntid { .. }, true, Some(CardNetwork::AmericanExpress)) => {
                (Some(TsysXmlCardOnFile::Y), None, None, None)
            }
            (MandateDispatch::Ntid { ntid }, true, _) => {
                (Some(TsysXmlCardOnFile::Y), None, None, Some(ntid.clone()))
            }
            (MandateDispatch::Ntid { ntid }, false, _) => {
                (Some(TsysXmlCardOnFile::Y), None, None, Some(ntid.clone()))
            }
            (MandateDispatch::Vault { .. }, true, _) => {
                (Some(TsysXmlCardOnFile::Y), None, None, None)
            }
            (MandateDispatch::Vault { .. }, false, _) => (
                Some(TsysXmlCardOnFile::Y),
                Some(TsysXmlMit {
                    mit_indicator: TsysXmlMitIndicator::R,
                }),
                None,
                None,
            ),
            (MandateDispatch::None, _, _) if is_cit_setup => (
                // CIT (storing the credential for future MIT) — flag cardOnFile=Y,
                // no MIT indicator and no COFTI/previousNetworkTransactionID.
                Some(TsysXmlCardOnFile::Y),
                None,
                None,
                None,
            ),
            (MandateDispatch::None, _, _) => (None, None, None, None),
        };

        // `<originalRecurringAmount>` — Discover/JCB/Diners/CUP MIT requirement.
        // Convert merchant-supplied minor units through the connector's amount
        // converter for wire consistency with `<transactionAmount>`.
        let original_recurring_amount = match (
            recurring_context.original_recurring_amount_minor,
            card_network.as_ref(),
            &mandate_dispatch,
        ) {
            (
                Some(minor),
                Some(CardNetwork::Discover)
                | Some(CardNetwork::JCB)
                | Some(CardNetwork::DinersClub)
                | Some(CardNetwork::UnionPay),
                MandateDispatch::Ntid { .. } | MandateDispatch::Vault { .. },
            ) => {
                use common_utils::types::MinorUnit;
                let minor_unit = MinorUnit::new(minor);
                Some(super::TsysXmlAmountConvertor::convert(
                    minor_unit,
                    router_data.request.currency,
                )?)
            }
            _ => None,
        };

        let (cit_status_indicator, mit_status_indicator) = match &mandate_dispatch {
            MandateDispatch::Ntid { .. } | MandateDispatch::Vault { .. } => {
                (None, recurring_context.mit_status_indicator)
            }
            MandateDispatch::None if is_cit_setup => {
                (recurring_context.mc_cit_status_indicator, None)
            }
            MandateDispatch::None => (None, None),
        };

        let partial_auth_support = if recurring_context.enabled
            || !matches!(mandate_dispatch, MandateDispatch::None)
            || commercial_card_context.commercial_card_level.is_some()
        {
            None
        } else {
            Some("YES".to_string())
        };

        let body = TsysXmlAuthorizeBody {
            device_id: auth.device_id,
            transaction_key: auth.transaction_key,
            card_data_source,
            transaction_amount,
            sales_tax: commercial_card_context.sales_tax,
            additional_tax_details: commercial_card_context.additional_tax_details,
            shipping_charges: commercial_card_context.shipping_charges,
            duty_charges: commercial_card_context.duty_charges,
            card_number,
            expiration_date,
            // TransIT cert "Do Not Send" CVV scenario: emit no `<cvv2>` when empty
            // (cert script row 113 — AMEX with absent CVV is still approved).
            cvv2: cvv2_opt,
            secure_code: three_ds_context.secure_code,
            security_protocol: None,
            ucaf_collection_indicator: three_ds_context.ucaf_collection_indicator,
            digital_payment_cryptogram: None,
            program_protocol: None,
            directory_server_transaction_id: three_ds_context.directory_server_transaction_id,
            eci_indicator: three_ds_context.eci_indicator,
            customer_code: customer_code_opt,
            wallet_details: wallet_details_opt,
            card_on_file_transaction_identifier,
            previous_network_transaction_id,
            cit_status_indicator,
            mit_status_indicator,
            address_line1,
            zip,
            external_reference_id: router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            product_details: commercial_card_context.product_details,
            commercial_card_level: commercial_card_context.commercial_card_level,
            purchase_order: commercial_card_context.purchase_order,
            charge_descriptor: commercial_card_context.charge_descriptor,
            charge_descriptor_2: commercial_card_context.charge_descriptor_2,
            charge_descriptor_3: commercial_card_context.charge_descriptor_3,
            charge_descriptor_4: commercial_card_context.charge_descriptor_4,
            customer_vat_number: commercial_card_context.customer_vat_number,
            customer_ref_id: commercial_card_context.customer_ref_id,
            supplier_reference_number: commercial_card_context.supplier_reference_number,
            order_date: commercial_card_context.order_date,
            summary_commodity_code: commercial_card_context.summary_commodity_code,
            vat_invoice: commercial_card_context.vat_invoice,
            ship_from_zip: commercial_card_context.ship_from_zip,
            ship_to_zip: commercial_card_context.ship_to_zip,
            destination_country_code: commercial_card_context.destination_country_code,
            card_on_file,
            partial_auth_support,
            terminal_capability,
            terminal_operating_environment,
            cardholder_authentication_method,
            terminal_authentication_capability,
            terminal_output_capability,
            max_pin_length,
            terminal_card_capture_capability,
            cardholder_present_detail,
            card_present_detail,
            card_data_input_mode,
            cardholder_authentication_entity,
            card_data_output_capability,
            developer_id: auth.developer_id,
            is_recurring: recurring_context.is_recurring_flag,
            billing_type: recurring_context.billing_type,
            payment_count: recurring_context.payment_count,
            current_payment_count: recurring_context.current_payment_count,
            original_recurring_amount,
            registered_user_indicator,
            last_registered_change_date,
            authorization_indicator,
            acceptor_street_address,
            acceptor_customer_service_phone_number,
            acceptor_phone_number,
            acceptor_url_address,
            mit: mit_block,
            _marker: std::marker::PhantomData,
        };

        Ok(if is_manual_capture {
            Self::Auth(body)
        } else {
            Self::Sale(body)
        })
    }
}

// =============================================================================
// Mandate dispatch helper
// =============================================================================

/// Result of decoding an upstream `connector_mandate_id` ("cust:CCC:WWW" or
/// "ntid:XXX") into a Path A / Path B / fall-through directive.
#[derive(Debug, Clone)]
enum MandateDispatch {
    /// Path B — vault token MIT. Emit customerCode + walletDetails.
    Vault {
        customer_code: String,
        wallet_id: String,
    },
    /// Path A — network-token MIT. Emit cardOnFile + MIT + previousNetworkTransactionID.
    Ntid { ntid: String },
    /// No mandate id (or a mandate id we couldn't decode) — caller decides
    /// whether to treat the request as a CIT or a one-shot.
    None,
}

/// Decode `MandateIds.mandate_reference_id` into a `MandateDispatch`.
///
/// We look at the `ConnectorMandateId` variant first (this is where prior
/// CreateConnectorCustomer / SetupMandate responses encode the mandate id).
/// Falls back to `NetworkMandateId` so plain NTIDs surfaced by HS are still
/// treated as Path A.
fn decode_mandate_dispatch(mandate_id: Option<&MandateIds>) -> MandateDispatch {
    let Some(mandate_id) = mandate_id else {
        return MandateDispatch::None;
    };

    if let Some(MandateReferenceId::ConnectorMandateId(connector_mandate_ids)) =
        mandate_id.mandate_reference_id.as_ref()
    {
        if let Some(raw) = connector_mandate_ids.get_connector_mandate_id() {
            return decode_mandate_id_string(&raw);
        }
    }

    // NetworkMandateId — treat as a raw NTID (Path A) so HS-stored network
    // transaction ids still drive the MIT path.
    if let Some(MandateReferenceId::NetworkMandateId(ntid)) =
        mandate_id.mandate_reference_id.as_ref()
    {
        return MandateDispatch::Ntid { ntid: ntid.clone() };
    }

    MandateDispatch::None
}

/// Parse the prefix-encoded mandate id our CreateConnectorCustomer /
/// SetupMandate flows emit:
/// - `cust:<customerCode>:<walletID>` → Path B
/// - `ntid:<cardTransactionIdentifier>` → Path A
/// Anything else → `None` (fall through to CIT / one-shot decision).
fn decode_mandate_id_string(raw: &str) -> MandateDispatch {
    if let Some(rest) = raw.strip_prefix("cust:") {
        // splitn(2, ':') so wallet IDs containing additional colons survive.
        let mut parts = rest.splitn(2, ':');
        match (parts.next(), parts.next()) {
            (Some(customer_code), Some(wallet_id))
                if !customer_code.is_empty() && !wallet_id.is_empty() =>
            {
                return MandateDispatch::Vault {
                    customer_code: customer_code.to_string(),
                    wallet_id: wallet_id.to_string(),
                };
            }
            _ => {}
        }
    }
    if let Some(ntid) = raw.strip_prefix("ntid:") {
        if !ntid.is_empty() {
            return MandateDispatch::Ntid {
                ntid: ntid.to_string(),
            };
        }
    }
    MandateDispatch::None
}

// =============================================================================
// AUTHORIZE — response transformer
// =============================================================================

/// Successful response codes per tech spec § Status Mappings.
///
/// `A0000` = full approval, `A0002` = partial approval. Anything else combined
/// with `status=PASS` is treated as an unexpected success surface (fail closed)
/// to surface upstream.
fn map_authorize_status(response: &TsysXmlAuthorizeResponse) -> AttemptStatus {
    let body = response.body();
    match (
        body.status.as_ref(),
        body.response_code.as_deref(),
        response,
    ) {
        (Some(TsysXmlStatus::Pass), Some("A0000"), TsysXmlAuthorizeResponse::SaleResponse(_)) => {
            AttemptStatus::Charged
        }
        (Some(TsysXmlStatus::Pass), Some("A0000"), TsysXmlAuthorizeResponse::AuthResponse(_)) => {
            AttemptStatus::Authorized
        }
        (Some(TsysXmlStatus::Pass), Some("A0002"), _) => AttemptStatus::PartialCharged,
        (Some(TsysXmlStatus::Fail), _, _) => AttemptStatus::Failure,
        // Unknown / missing — fail closed.
        _ => AttemptStatus::Failure,
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<TsysXmlAuthorizeResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TsysXmlAuthorizeResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let response = &item.response;
        let body = response.body();

        let status = map_authorize_status(response);

        // Failure surface: surface code/message but keep transactionID if TransIT
        // gave us one (tech spec § Error Codes — decline envelopes still carry
        // <transactionID>).
        if matches!(status, AttemptStatus::Failure) {
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(ErrorResponse {
                    status_code: item.http_code,
                    code: body
                        .response_code
                        .clone()
                        .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string()),
                    message: body
                        .response_message
                        .clone()
                        .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string()),
                    reason: body.response_message.clone(),
                    attempt_status: Some(AttemptStatus::Failure),
                    connector_transaction_id: body.transaction_id.clone(),
                    network_decline_code: body.host_response_code.clone(),
                    network_advice_code: None,
                    network_error_message: body.response_message.clone(),
                }),
                ..router_data.clone()
            });
        }

        // Success path requires a transactionID — without one we cannot drive
        // subsequent Capture/Void/Refund flows, so reject as a deserialization
        // problem.
        let transaction_id = body.transaction_id.clone().ok_or_else(|| {
            crate::utils::response_deserialization_fail(
                item.http_code,
                "tsys_xml: success response missing <transactionID>; confirm API contract.",
            )
        })?;

        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(transaction_id.clone()),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: body.auth_code.clone(),
            connector_response_reference_id: Some(transaction_id),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data.clone()
            },
            response: Ok(payments_response_data),
            ..router_data.clone()
        })
    }
}

// =============================================================================
// PSYNC — request transformer
// =============================================================================
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TsysXmlRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    > for TsysXmlTransactionInquiryRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: TsysXmlRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = TsysXmlAuthType::try_from(&router_data.connector_config)?;

        let transaction_id = router_data.request.get_connector_transaction_id()?;

        Ok(Self {
            device_id: auth.device_id,
            transaction_key: auth.transaction_key,
            developer_id: auth.developer_id,
            transaction_id,
        })
    }
}

// =============================================================================
// PSYNC — response transformer
// =============================================================================

/// Map TransIT PSync (`<status>` + `<transactionState>`) to `AttemptStatus`
/// per tech spec § Status Mappings.
fn map_sync_status(response: &TsysXmlTransactionInquiryResponse) -> AttemptStatus {
    match (
        response.status.as_ref(),
        response.transaction_state.as_ref(),
    ) {
        (Some(TsysXmlStatus::Pass), Some(TsysXmlTransactionState::Authorized)) => {
            AttemptStatus::Authorized
        }
        (Some(TsysXmlStatus::Pass), Some(TsysXmlTransactionState::Captured)) => {
            AttemptStatus::Charged
        }
        (Some(TsysXmlStatus::Pass), Some(TsysXmlTransactionState::Settled)) => {
            AttemptStatus::Charged
        }
        (Some(TsysXmlStatus::Pass), Some(TsysXmlTransactionState::Voided)) => AttemptStatus::Voided,
        (Some(TsysXmlStatus::Pass), Some(TsysXmlTransactionState::Returned)) => {
            AttemptStatus::AutoRefunded
        }
        (Some(TsysXmlStatus::Fail), _) => AttemptStatus::Failure,
        // Unknown / missing transactionState — keep Pending and log a warning
        // rather than panicking. UCS callers will retry the sync.
        _ => {
            tracing::warn!(
                "tsys_xml: PSync response missing or unrecognized transactionState; defaulting to Pending"
            );
            AttemptStatus::Pending
        }
    }
}

impl TryFrom<ResponseRouterData<TsysXmlTransactionInquiryResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TsysXmlTransactionInquiryResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let response = &item.response;

        let status = map_sync_status(response);

        if matches!(status, AttemptStatus::Failure) {
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(ErrorResponse {
                    status_code: item.http_code,
                    code: response
                        .response_code
                        .clone()
                        .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string()),
                    message: response
                        .response_message
                        .clone()
                        .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string()),
                    reason: response.response_message.clone(),
                    attempt_status: Some(AttemptStatus::Failure),
                    connector_transaction_id: response.transaction_id.clone(),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: response.response_message.clone(),
                }),
                ..router_data.clone()
            });
        }

        // For success / pending: prefer the response's transactionID when
        // present; otherwise fall back to what we asked about so the caller
        // never loses the reference.
        let connector_txn_id = match response.transaction_id.clone() {
            Some(id) => id,
            None => router_data
                .request
                .get_connector_transaction_id()
                .map_err(|_| {
                    crate::utils::response_deserialization_fail(
                        item.http_code,
                        "tsys_xml: PSync response and request both missing transactionID.",
                    )
                })?,
        };

        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(connector_txn_id.clone()),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            connector_response_reference_id: Some(connector_txn_id),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data.clone()
            },
            response: Ok(payments_response_data),
            ..router_data.clone()
        })
    }
}

// =============================================================================
// CAPTURE — request transformer
// =============================================================================
fn compute_capture_sales_tax<
    T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize,
>(
    router_data: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
) -> Result<Option<common_utils::types::StringMajorUnit>, Report<IntegrationError>> {
    let merchant_metadata = match router_data.request.metadata.as_ref() {
        Some(meta) => serde_json::from_value::<TsysXmlMerchantMetadata>(meta.clone().expose())
            .change_context(IntegrationError::InvalidDataFormat {
                field_name: "connector_metadata.tsys_xml",
                context: Default::default(),
            })?,
        None => TsysXmlMerchantMetadata::default(),
    };
    let commercial_card_meta = merchant_metadata
        .tsys_xml
        .and_then(|inner| inner.commercial_card);
    if commercial_card_meta.is_none() {
        return Ok(None);
    }

    router_data
        .request
        .order_tax_amount
        .or_else(|| {
            router_data
                .resource_common_data
                .l2_l3_data
                .as_deref()
                .and_then(|data| data.get_order_tax_amount())
        })
        .map(|amount| super::TsysXmlAmountConvertor::convert(amount, router_data.request.currency))
        .transpose()
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TsysXmlRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for TsysXmlCaptureRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: TsysXmlRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = TsysXmlAuthType::try_from(&router_data.connector_config)?;

        // The auth's <transactionID> drives the capture — it is required.
        let transaction_id = router_data.request.get_connector_transaction_id()?;

        let transaction_amount = super::TsysXmlAmountConvertor::convert(
            router_data.request.minor_amount_to_capture,
            router_data.request.currency,
        )?;
        let sales_tax = compute_capture_sales_tax::<T>(router_data)?;

        // TODO(tsys_xml): wire seq_number / payment_count for multi-clearing
        // (split-shipment) via add-connector-flow. PR-1 ships single-capture only.
        Ok(Self {
            device_id: auth.device_id,
            transaction_key: auth.transaction_key,
            developer_id: auth.developer_id,
            transaction_id,
            transaction_amount,
            sales_tax,
            seq_number: None,
            payment_count: None,
        })
    }
}

// =============================================================================
// CAPTURE — response transformer
// =============================================================================

/// Map TransIT Capture (`<status>` + `<responseCode>`) to `AttemptStatus` per
/// tech spec § Status Mappings.
///
/// - `PASS` + `A0000` → `Charged`
/// - `PASS` + `A0002` → `PartialCharged`
/// - `FAIL` (any code) → `CaptureFailed`
/// - Anything else → `CaptureFailed` (fail closed)
fn map_capture_status(response: &TsysXmlCaptureResponse) -> AttemptStatus {
    match (response.status.as_ref(), response.response_code.as_deref()) {
        (Some(TsysXmlStatus::Pass), Some("A0000")) => AttemptStatus::Charged,
        (Some(TsysXmlStatus::Pass), Some("A0002")) => AttemptStatus::PartialCharged,
        (Some(TsysXmlStatus::Fail), _) => AttemptStatus::CaptureFailed,
        _ => AttemptStatus::CaptureFailed,
    }
}

impl TryFrom<ResponseRouterData<TsysXmlCaptureResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TsysXmlCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let response = &item.response;

        let status = map_capture_status(response);

        if matches!(status, AttemptStatus::CaptureFailed) {
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(ErrorResponse {
                    status_code: item.http_code,
                    code: response
                        .response_code
                        .clone()
                        .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string()),
                    message: response
                        .response_message
                        .clone()
                        .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string()),
                    reason: response.response_message.clone(),
                    attempt_status: Some(AttemptStatus::CaptureFailed),
                    connector_transaction_id: response.transaction_id.clone(),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: response.response_message.clone(),
                }),
                ..router_data.clone()
            });
        }

        // Success path: prefer response's transactionID; fall back to the auth
        // txn id we sent (TransIT's capture echoes the same id).
        let connector_txn_id = match response.transaction_id.clone() {
            Some(id) => id,
            None => router_data
                .request
                .get_connector_transaction_id()
                .map_err(|_| {
                    crate::utils::response_deserialization_fail(
                        item.http_code,
                        "tsys_xml: Capture response missing <transactionID> and request had none.",
                    )
                })?,
        };

        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(connector_txn_id.clone()),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            connector_response_reference_id: Some(connector_txn_id),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data.clone()
            },
            response: Ok(payments_response_data),
            ..router_data.clone()
        })
    }
}

// =============================================================================
// REFUND — request transformer
// =============================================================================
//
// TransIT Return supports three modes from the same `<Return>` element shape:
//
//   1. Referenced full    — `transactionID` only (no `transactionAmount`).
//   2. Referenced partial — `transactionID` + `transactionAmount`.
//   3. Unreferenced       — NO `transactionID`; raw card data + `transactionAmount`.
//
// Mode selection happens here based on `RefundsData`:
//   * non-empty `connector_transaction_id` → referenced (we always emit
//     `transactionAmount` in PR-1; "omit for full" is a TODO follow-up so the
//     gateway recognises the partial vs. full distinction without us guessing
//     the original amount).
//   * empty `connector_transaction_id` → unreferenced; raw card data is
//     required. `RefundsData` does not surface `payment_method_data` today, so
//     this path returns `MissingRequiredField` until upstream wires card data
//     through for refunds.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TsysXmlRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for TsysXmlReturnRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: TsysXmlRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = TsysXmlAuthType::try_from(&router_data.connector_config)?;

        let transaction_amount = super::TsysXmlAmountConvertor::convert(
            router_data.request.minor_refund_amount,
            router_data.request.currency,
        )?;

        let connector_transaction_id = router_data.request.connector_transaction_id.clone();

        if !connector_transaction_id.is_empty() {
            // Referenced mode (full or partial). PR-1 always emits
            // `transactionAmount` so the gateway sees the explicit value; a
            // follow-up TODO will compare `refund_amount` to the original
            // captured amount and omit `transactionAmount` for full refunds.
            Ok(Self {
                device_id: auth.device_id,
                transaction_key: auth.transaction_key,
                developer_id: auth.developer_id,
                transaction_id: Some(connector_transaction_id),
                card_data_source: None,
                card_number: None,
                expiration_date: None,
                cvv2: None,
                transaction_amount: Some(transaction_amount),
            })
        } else {
            // Unreferenced mode: full card data must be supplied. `RefundsData`
            // does not carry `payment_method_data` today, so PR-1 surfaces this
            // as a missing-field error rather than silently producing an
            // invalid request.
            Err(IntegrationError::MissingRequiredField {
                field_name: "payment_method_data for unreferenced refund",
                context: Default::default(),
            }
            .into())
        }
    }
}

// =============================================================================
// REFUND — response transformer
// =============================================================================

/// Map TransIT Return (`<status>` + `<responseCode>`) to `RefundStatus` per
/// tech spec § Status Mappings.
///
/// - `PASS` + `A0000` → `Success` — full referenced refund completed.
/// - `PASS` + `A0002` → `Success` — partial approval (refundedAmount in the
///   response reflects the actual amount processed).
/// - `PASS` + `A0014` → `Success` — Return requested against an unsettled
///   transaction; TSYS converts it to a pre-settlement Void. Effective refund
///   from the merchant's perspective. Verified live (`<ReturnResponse>` with
///   `responseMessage: "Return requested, Void successful"`).
/// - `FAIL` (any code) → `Failure`
/// - Anything else → `Failure` (fail closed)
fn map_refund_status(response: &TsysXmlReturnResponse) -> RefundStatus {
    match (response.status.as_ref(), response.response_code.as_deref()) {
        (Some(TsysXmlStatus::Pass), Some("A0000" | "A0002" | "A0014")) => RefundStatus::Success,
        (Some(TsysXmlStatus::Fail), _) => RefundStatus::Failure,
        _ => RefundStatus::Failure,
    }
}

impl TryFrom<ResponseRouterData<TsysXmlReturnResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TsysXmlReturnResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let response = &item.response;

        let refund_status = map_refund_status(response);

        if matches!(refund_status, RefundStatus::Failure) {
            return Ok(Self {
                resource_common_data: RefundFlowData {
                    status: refund_status,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(ErrorResponse {
                    status_code: item.http_code,
                    code: response
                        .response_code
                        .clone()
                        .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string()),
                    message: response
                        .response_message
                        .clone()
                        .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string()),
                    reason: response.response_message.clone(),
                    attempt_status: None,
                    connector_transaction_id: response.transaction_id.clone(),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: response.response_message.clone(),
                }),
                ..router_data.clone()
            });
        }

        // Success path: TransIT echoes the original capture's transactionID for
        // referenced returns; we treat that as the refund identifier for PR-1.
        // RSync will refine this once we know the on-wire id semantics.
        let connector_refund_id = response.transaction_id.clone().ok_or_else(|| {
            crate::utils::response_deserialization_fail(
                item.http_code,
                "tsys_xml: Return response missing <transactionID>; confirm API contract.",
            )
        })?;

        let refunds_response_data = RefundsResponseData {
            connector_refund_id,
            refund_status,
            status_code: item.http_code,
        };

        Ok(Self {
            resource_common_data: RefundFlowData {
                status: refund_status,
                ..router_data.resource_common_data.clone()
            },
            response: Ok(refunds_response_data),
            ..router_data.clone()
        })
    }
}

// =============================================================================
// RSYNC — request transformer (REUSES TsysXmlTransactionInquiryRequest)
// =============================================================================
//
// TransIT refunds are sync-final on `<ReturnResponse>`; there is no dedicated
// refund-status-poll endpoint. HS still dispatches RSync though, so we
// re-issue a `<TransactionInquiry>` against the original refund's
// `transactionID` (echoed back by TransIT as `connector_refund_id` in our
// Return response transformer). If upstream lacks a refund id we fall back to
// the original payment transactionID — both are valid keys for TransIT's
// inquiry endpoint.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TsysXmlRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    > for TsysXmlTransactionInquiryRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: TsysXmlRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = TsysXmlAuthType::try_from(&router_data.connector_config)?;

        // Prefer `connector_refund_id` (TransIT's echoed `<transactionID>` from
        // the original `<ReturnResponse>`); fall back to the original payment's
        // `connector_transaction_id` if the refund id wasn't recorded.
        let transaction_id = if !router_data.request.connector_refund_id.is_empty() {
            router_data.request.connector_refund_id.clone()
        } else if !router_data.request.connector_transaction_id.is_empty() {
            router_data.request.connector_transaction_id.clone()
        } else {
            return Err(IntegrationError::MissingRequiredField {
                field_name: "connector_refund_id or connector_transaction_id",
                context: Default::default(),
            }
            .into());
        };

        Ok(Self {
            device_id: auth.device_id,
            transaction_key: auth.transaction_key,
            developer_id: auth.developer_id,
            transaction_id,
        })
    }
}

// =============================================================================
// RSYNC — response transformer (REUSES TsysXmlTransactionInquiryResponse)
// =============================================================================

/// Map TransIT TransactionInquiry (`<status>` + `<transactionState>`) to
/// `RefundStatus` per tech spec § Status Mappings.
///
/// - `PASS` + `RETURNED` → `Success` (refund applied, awaiting batch settle)
/// - `PASS` + `SETTLED`  → `Success` (refund batch settled — terminal success)
/// - `PASS` + `VOIDED`   → `Failure` (the return itself was reversed; refund
///   didn't actually go through).
///   TODO(tsys_xml): VOIDED-on-RSync semantics depend on whether TransIT
///   distinguishes "return reversed before settle" vs "original auth voided";
///   confirm with TSYS whether `Failure` is the correct terminal mapping.
/// - `FAIL`              → `Failure`
/// - Unknown / missing   → `Pending` (do NOT fail; let HS poll again).
fn map_rsync_status(response: &TsysXmlTransactionInquiryResponse) -> RefundStatus {
    match (
        response.status.as_ref(),
        response.transaction_state.as_ref(),
    ) {
        (Some(TsysXmlStatus::Pass), Some(TsysXmlTransactionState::Returned)) => {
            RefundStatus::Success
        }
        (Some(TsysXmlStatus::Pass), Some(TsysXmlTransactionState::Settled)) => {
            RefundStatus::Success
        }
        // TODO(tsys_xml): confirm VOIDED semantics with TSYS — currently treated
        // as terminal Failure because a voided return means the refund didn't
        // settle to the cardholder.
        (Some(TsysXmlStatus::Pass), Some(TsysXmlTransactionState::Voided)) => RefundStatus::Failure,
        (Some(TsysXmlStatus::Fail), _) => RefundStatus::Failure,
        // Unknown / missing transactionState (including Authorized/Captured
        // pre-return states) — stay Pending so HS keeps polling.
        _ => {
            tracing::warn!(
                "tsys_xml: RSync response missing or unrecognized transactionState; defaulting to Pending"
            );
            RefundStatus::Pending
        }
    }
}

impl TryFrom<ResponseRouterData<TsysXmlTransactionInquiryResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TsysXmlTransactionInquiryResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let response = &item.response;

        let refund_status = map_rsync_status(response);

        if matches!(refund_status, RefundStatus::Failure) {
            return Ok(Self {
                resource_common_data: RefundFlowData {
                    status: refund_status,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(ErrorResponse {
                    status_code: item.http_code,
                    code: response
                        .response_code
                        .clone()
                        .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string()),
                    message: response
                        .response_message
                        .clone()
                        .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string()),
                    reason: response.response_message.clone(),
                    attempt_status: None,
                    connector_transaction_id: response.transaction_id.clone(),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: response.response_message.clone(),
                }),
                ..router_data.clone()
            });
        }

        // Success / Pending: prefer the response's transactionID; fall back to
        // whichever id we sent so the caller never loses the reference.
        let connector_refund_id = match response.transaction_id.clone() {
            Some(id) => id,
            None => {
                if !router_data.request.connector_refund_id.is_empty() {
                    router_data.request.connector_refund_id.clone()
                } else if !router_data.request.connector_transaction_id.is_empty() {
                    router_data.request.connector_transaction_id.clone()
                } else {
                    return Err(crate::utils::response_deserialization_fail(
                        item.http_code,
                        "tsys_xml: RSync response and request both missing transactionID.",
                    )
                    .into());
                }
            }
        };

        let refunds_response_data = RefundsResponseData {
            connector_refund_id,
            refund_status,
            status_code: item.http_code,
        };

        Ok(Self {
            resource_common_data: RefundFlowData {
                status: refund_status,
                ..router_data.resource_common_data.clone()
            },
            response: Ok(refunds_response_data),
            ..router_data.clone()
        })
    }
}

// =============================================================================
// VOID — request transformer
// =============================================================================
//
// TransIT `<Void>` accepts an optional `<transactionAmount>`:
//   * Omitted   → full void of the prior auth.
//   * Provided  → partial void (cert script Step 7) — the prior auth is reduced
//     by that amount.
//
// `PaymentVoidData` carries an `Option<MinorUnit>` `amount` field. When set
// alongside `currency`, we convert via the StringMajorUnit converter and emit
// it; otherwise we omit `<transactionAmount>` so TransIT treats this as a full
// void.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TsysXmlRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for TsysXmlVoidRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: TsysXmlRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = TsysXmlAuthType::try_from(&router_data.connector_config)?;

        let transaction_id = router_data.request.connector_transaction_id.clone();

        // Partial-void support: if both `amount` and `currency` are present on
        // PaymentVoidData, convert to a major-unit string and emit
        // `<transactionAmount>`; otherwise omit so TransIT performs a full
        // void.
        let transaction_amount = match (router_data.request.amount, router_data.request.currency) {
            (Some(amount), Some(currency)) => {
                Some(super::TsysXmlAmountConvertor::convert(amount, currency)?)
            }
            _ => None,
        };

        // Cert script Step 7: voidReason is required. Derive from
        // `cancellation_reason`, fall back to a sensible default, cap at 80
        // chars to stay within TSYS' field bounds.
        let void_reason = {
            let raw = router_data
                .request
                .cancellation_reason
                .clone()
                .unwrap_or_else(|| "POST_AUTH_USER_DECLINE".to_string());
            if raw.len() > 80 {
                raw.chars().take(80).collect()
            } else {
                raw
            }
        };

        Ok(Self {
            device_id: auth.device_id,
            transaction_key: auth.transaction_key,
            developer_id: auth.developer_id,
            transaction_id,
            transaction_amount,
            void_reason,
        })
    }
}

// =============================================================================
// VOID — response transformer
// =============================================================================

/// Map TransIT Void (`<status>` + `<responseCode>`) to `AttemptStatus` per
/// tech spec § Status Mappings.
///
/// - `PASS` + `A0000` → `Voided` (full void)
/// - `PASS` + `A0002` → `Voided` (partial void — the auth is reduced; at the
///   auth lifecycle level the state is still "voided" from UCS's perspective)
/// - `FAIL` (any code) → `VoidFailed`
/// - Anything else → `VoidFailed` (fail closed)
fn map_void_status(response: &TsysXmlVoidResponse) -> AttemptStatus {
    match (response.status.as_ref(), response.response_code.as_deref()) {
        (Some(TsysXmlStatus::Pass), Some("A0000")) => AttemptStatus::Voided,
        (Some(TsysXmlStatus::Pass), Some("A0002")) => AttemptStatus::Voided,
        (Some(TsysXmlStatus::Fail), _) => AttemptStatus::VoidFailed,
        _ => AttemptStatus::VoidFailed,
    }
}

impl TryFrom<ResponseRouterData<TsysXmlVoidResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<TsysXmlVoidResponse, Self>) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let response = &item.response;

        let status = map_void_status(response);

        if matches!(status, AttemptStatus::VoidFailed) {
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(ErrorResponse {
                    status_code: item.http_code,
                    code: response
                        .response_code
                        .clone()
                        .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string()),
                    message: response
                        .response_message
                        .clone()
                        .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string()),
                    reason: response.response_message.clone(),
                    attempt_status: Some(AttemptStatus::VoidFailed),
                    connector_transaction_id: response.transaction_id.clone(),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: response.response_message.clone(),
                }),
                ..router_data.clone()
            });
        }

        // Success path: prefer response's transactionID; fall back to the auth
        // txn id we sent (TransIT echoes the same id).
        let connector_txn_id = match response.transaction_id.clone() {
            Some(id) => id,
            None => {
                let id = router_data.request.connector_transaction_id.clone();
                if id.is_empty() {
                    return Err(crate::utils::response_deserialization_fail(
                        item.http_code,
                        "tsys_xml: Void response missing <transactionID> and request had none.",
                    )
                    .into());
                }
                id
            }
        };

        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(connector_txn_id.clone()),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            connector_response_reference_id: Some(connector_txn_id),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data.clone()
            },
            response: Ok(payments_response_data),
            ..router_data.clone()
        })
    }
}

// =============================================================================
// CreateConnectorCustomer — request transformer (`<AddCustomer>`)
// =============================================================================
//
// Sources:
//   - first/last name: split `ConnectorCustomerData.name` on first whitespace.
//     No whitespace -> entire string goes to firstName, lastName defaults to
//     "-" (TSYS' XSD requires both fields).
//   - addressLine1 / zip: PaymentFlowData.address.billing_address.
//   - card data: `ConnectorCustomerData` does NOT carry payment_method_data in
//     this repo. PR-1 fails closed via `MissingRequiredField` so the live-test
//     phase identifies the right HS-side bridge before iterating.
//
// `expirationDate` in `<AddCustomer>` is MMYYYY (6 digits) — different from
// Sale/Auth's MMYY.

fn split_full_name(full: &str) -> (String, String) {
    let trimmed = full.trim();
    if trimmed.is_empty() {
        return ("-".to_string(), "-".to_string());
    }
    match trimmed.split_once(char::is_whitespace) {
        Some((first, rest)) => {
            let last = rest.trim();
            (
                first.to_string(),
                if last.is_empty() {
                    "-".to_string()
                } else {
                    last.to_string()
                },
            )
        }
        None => (trimmed.to_string(), "-".to_string()),
    }
}

#[allow(dead_code)]
fn format_add_customer_expiration(card: &Card<impl PaymentMethodDataTypes>) -> Secret<String> {
    // AddCustomer wants MMYYYY (6 digits). Normalize 2-digit years up to 4-digit
    // by prefixing "20" (TransIT only supports cards expiring this century).
    let month_raw = card.card_exp_month.peek().clone();
    let year_raw = card.card_exp_year.peek().clone();
    let month = if month_raw.len() == 1 {
        format!("0{month_raw}")
    } else {
        month_raw
    };
    let year_full = if year_raw.len() == 2 {
        format!("20{year_raw}")
    } else {
        year_raw
    };
    Secret::new(format!("{month}{year_full}"))
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TsysXmlRouterData<
            RouterDataV2<
                CreateConnectorCustomer,
                PaymentFlowData,
                ConnectorCustomerData,
                ConnectorCustomerResponse,
            >,
            T,
        >,
    > for TsysXmlAddCustomerRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: TsysXmlRouterData<
            RouterDataV2<
                CreateConnectorCustomer,
                PaymentFlowData,
                ConnectorCustomerData,
                ConnectorCustomerResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = TsysXmlAuthType::try_from(&router_data.connector_config)?;

        // Name — required by AddCustomer XSD. Split on the first whitespace; if
        // no whitespace at all, lastName defaults to "-".
        let name_secret = router_data.request.name.clone().ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "ConnectorCustomerData.name",
                context: Default::default(),
            })
        })?;
        let (first_name, last_name) = split_full_name(name_secret.peek().as_str());

        // Billing address — supplies addressLine1 + zip in both personalDetails
        // and walletDetails per the AddCustomer body shape.
        let billing = router_data
            .resource_common_data
            .address
            .get_payment_billing()
            .and_then(|b| b.address.as_ref());
        let address_line1 = billing.and_then(|a| a.line1.clone()).ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "billing.address.line1",
                context: Default::default(),
            })
        })?;
        let zip = billing.and_then(|a| a.zip.clone()).ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "billing.address.zip",
                context: Default::default(),
            })
        })?;

        // `ConnectorCustomerData` is non-generic and lacks `payment_method_data`
        // in this repo; we cannot populate the mandatory <walletDetails>
        // <cardDetails> block without it. Fail closed with the precise field
        // name so the live-test phase identifies the right HS-side bridge.
        let (card_number, expiration_date) = extract_add_customer_card::<T>(router_data)?;

        Ok(Self {
            device_id: auth.device_id,
            transaction_key: auth.transaction_key,
            personal_details: TsysXmlPersonalDetails {
                first_name: Secret::new(first_name),
                last_name: Secret::new(last_name),
                address_line1: address_line1.clone(),
                zip: zip.clone(),
            },
            wallet_details: TsysXmlAddCustomerWalletDetails {
                card_details: TsysXmlAddCustomerCardDetails {
                    card_number,
                    expiration_date,
                },
                address_line1,
                zip,
                payment_sequence: "1".to_string(),
            },
            developer_id: auth.developer_id,
        })
    }
}

/// Pull card data for `<AddCustomer>` from any HS-side surface we recognize.
///
/// `ConnectorCustomerData` does not carry `payment_method_data` in this repo
/// today, so we surface `MissingRequiredField` explicitly. The live-test phase
/// will identify the right HS-side bridge (likely a generic variant of
/// `ConnectorCustomerData` or a `connector_feature_data` payload).
fn extract_add_customer_card<
    T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize,
>(
    _router_data: &RouterDataV2<
        CreateConnectorCustomer,
        PaymentFlowData,
        ConnectorCustomerData,
        ConnectorCustomerResponse,
    >,
) -> Result<(Secret<String>, Secret<String>), Report<IntegrationError>> {
    Err(IntegrationError::MissingRequiredField {
        field_name: "ConnectorCustomerData.payment_method_data (card)",
        context: Default::default(),
    }
    .into())
}

// =============================================================================
// CreateConnectorCustomer — response transformer
// =============================================================================

impl TryFrom<ResponseRouterData<TsysXmlAddCustomerResponse, Self>>
    for RouterDataV2<
        CreateConnectorCustomer,
        PaymentFlowData,
        ConnectorCustomerData,
        ConnectorCustomerResponse,
    >
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TsysXmlAddCustomerResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let response = &item.response;

        let is_success = matches!(response.status, Some(TsysXmlStatus::Pass))
            && response.response_code.as_deref() == Some("A0000");

        if !is_success {
            return Ok(Self {
                response: Err(ErrorResponse {
                    status_code: item.http_code,
                    code: response
                        .response_code
                        .clone()
                        .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string()),
                    message: response
                        .response_message
                        .clone()
                        .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string()),
                    reason: response.response_message.clone(),
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: response.response_message.clone(),
                }),
                ..router_data.clone()
            });
        }

        let customer_code = response.customer_code.clone().ok_or_else(|| {
            crate::utils::response_deserialization_fail(
                item.http_code,
                "tsys_xml: AddCustomerResponse missing <customerCode>; confirm API contract.",
            )
        })?;
        let wallet_id = response
            .wallet_details
            .as_ref()
            .and_then(|w| w.wallet_id.clone())
            .ok_or_else(|| {
                crate::utils::response_deserialization_fail(
                    item.http_code,
                    "tsys_xml: AddCustomerResponse missing <walletDetails><walletID>; confirm API contract.",
                )
            })?;

        // Stash the Path B mandate id (`cust:CCC:WWW`) on
        // `PaymentFlowData.reference_id` so the next Authorize call can pick it
        // up. `ConnectorCustomerResponse` only carries `connector_customer_id`,
        // so we use the generic reference_id slot to surface walletID.
        let path_b_mandate_id = format!("cust:{customer_code}:{wallet_id}");

        Ok(Self {
            response: Ok(ConnectorCustomerResponse {
                connector_customer_id: customer_code,
            }),
            resource_common_data: PaymentFlowData {
                reference_id: Some(path_b_mandate_id),
                ..router_data.resource_common_data.clone()
            },
            ..router_data.clone()
        })
    }
}

// =============================================================================
// SetupMandate — request transformer (`<CardAuthentication>`, zero-dollar CIT)
// =============================================================================

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TsysXmlRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for TsysXmlCardAuthenticationRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: TsysXmlRouterData<
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
        let auth = TsysXmlAuthType::try_from(&router_data.connector_config)?;

        let card = match &router_data.request.payment_method_data {
            PaymentMethodData::Card(card) => card,
            _ => {
                return Err(IntegrationError::NotSupported {
                    message: "Selected payment method".to_string(),
                    connector: "tsys_xml",
                    context: Default::default(),
                }
                .into());
            }
        };

        let billing = router_data
            .resource_common_data
            .address
            .get_payment_billing()
            .and_then(|b| b.address.as_ref());
        let address_line1 = billing.and_then(|a| a.line1.clone()).ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "billing.address.line1",
                context: Default::default(),
            })
        })?;
        let zip = billing.and_then(|a| a.zip.clone()).ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "billing.address.zip",
                context: Default::default(),
            })
        })?;

        let channel = router_data.request.payment_channel.clone();
        let card_data_source = match channel {
            Some(PaymentChannel::TelephoneOrder) => TsysXmlCardDataSource::Phone,
            Some(PaymentChannel::MailOrder) => TsysXmlCardDataSource::Mail,
            Some(PaymentChannel::Ecommerce) | None => TsysXmlCardDataSource::Internet,
        };

        // Reuse the Authorize metadata overrides so terminalData is consistent
        // across CIT verify and the subsequent MIT call.
        let merchant_metadata = match router_data.request.metadata.as_ref() {
            Some(meta) => serde_json::from_value::<TsysXmlMerchantMetadata>(meta.clone().expose())
                .change_context(IntegrationError::InvalidDataFormat {
                    field_name: "connector_metadata.tsys_xml",
                    context: Default::default(),
                })?,
            None => TsysXmlMerchantMetadata::default(),
        };
        let merchant_inner = merchant_metadata.tsys_xml.unwrap_or_default();
        let terminal_overrides = merchant_inner.terminal_data.unwrap_or_default();
        // CardAuthentication uses the e-commerce terminalData baseline per cert
        // (the recurring presets explicitly do NOT apply to Card Authentications).
        // But: MC CIT requires `cardholderPresentDetail=CARDHOLDER_NOT_PRESENT_
        // RECURRING_TRANSACTION` on the CIT in a recurring/subscription series,
        // and the `citStatusIndicator` (C102/C103/C104) when present.
        //
        // SetupMandate has no `recurring_mandate_payment_data` (no prior MIT)
        // — pass None so installment-counter guards never fire on CIT setup.
        let card_network = card.card_network.clone();
        let recurring_context = compute_recurring_context(
            router_data.request.mit_category.clone(),
            None,
            card_network.as_ref(),
        )?;
        let cit_status_indicator = if matches!(card_network, Some(CardNetwork::Mastercard)) {
            recurring_context.mc_cit_status_indicator
        } else {
            None
        };

        let terminal_capability = terminal_overrides
            .terminal_capability
            .unwrap_or(TsysXmlTerminalCapability::KeyedEntryOnly);
        let terminal_operating_environment = terminal_overrides
            .terminal_operating_environment
            .unwrap_or(TsysXmlTerminalOperatingEnvironment::NoTerminal);
        let cardholder_authentication_method = terminal_overrides
            .cardholder_authentication_method
            .unwrap_or(TsysXmlCardholderAuthenticationMethod::NotAuthenticated);
        let terminal_authentication_capability = terminal_overrides
            .terminal_authentication_capability
            .unwrap_or(TsysXmlTerminalAuthenticationCapability::NoCapability);
        let terminal_output_capability = terminal_overrides
            .terminal_output_capability
            .unwrap_or(TsysXmlTerminalOutputCapability::None);
        let max_pin_length = terminal_overrides
            .max_pin_length
            .unwrap_or(TsysXmlMaxPinLength::NotSupported);
        let terminal_card_capture_capability = terminal_overrides
            .terminal_card_capture_capability
            .unwrap_or(TsysXmlTerminalCardCaptureCapability::NoCapability);
        let cardholder_present_detail = terminal_overrides
            .cardholder_present_detail
            .unwrap_or_else(|| {
                // MC CIT in a recurring series: force RECURRING_TRANSACTION on
                // the CIT (cert: "MasterCard requires you to set
                // cardholderPresentDetail as CARDHOLDER_NOT_PRESENT_RECURRING_
                // TRANSACTION in both the CIT … and the subsequent MIT").
                if recurring_context.enabled
                    && matches!(card_network, Some(CardNetwork::Mastercard))
                {
                    return TsysXmlCardholderPresentDetail::CardholderNotPresentRecurringTransaction;
                }
                match channel {
                    Some(PaymentChannel::TelephoneOrder) => {
                        TsysXmlCardholderPresentDetail::CardholderNotPresentPhoneTransaction
                    }
                    Some(PaymentChannel::MailOrder) => {
                        TsysXmlCardholderPresentDetail::CardholderNotPresentMailTransaction
                    }
                    _ => TsysXmlCardholderPresentDetail::CardholderNotPresentElectronicCommerce,
                }
            });
        let card_present_detail = terminal_overrides
            .card_present_detail
            .unwrap_or(TsysXmlCardPresentDetail::CardNotPresent);
        let card_data_input_mode =
            terminal_overrides
                .card_data_input_mode
                .unwrap_or_else(|| match channel {
                    Some(PaymentChannel::Ecommerce) | None => {
                        TsysXmlCardDataInputMode::PanEntryElectronicCommerceIncludingRemoteChip
                    }
                    _ => TsysXmlCardDataInputMode::KeyEnteredInput,
                });
        let cardholder_authentication_entity = terminal_overrides
            .cardholder_authentication_entity
            .unwrap_or(TsysXmlCardholderAuthenticationEntity::NotAuthenticated);
        let card_data_output_capability = terminal_overrides
            .card_data_output_capability
            .unwrap_or(TsysXmlCardDataOutputCapability::None);

        Ok(Self {
            device_id: auth.device_id,
            transaction_key: auth.transaction_key,
            card_data_source,
            card_number: Secret::new(card.card_number.peek().to_string()),
            expiration_date: format_expiration_date(card),
            address_line1,
            zip,
            external_reference_id: router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            card_on_file: TsysXmlCardOnFile::Y,
            cit_status_indicator,
            developer_id: auth.developer_id,
            terminal_capability,
            terminal_operating_environment,
            cardholder_authentication_method,
            terminal_authentication_capability,
            terminal_output_capability,
            max_pin_length,
            terminal_card_capture_capability,
            cardholder_present_detail,
            card_present_detail,
            card_data_input_mode,
            cardholder_authentication_entity,
            card_data_output_capability,
        })
    }
}

// =============================================================================
// SetupMandate — response transformer
// =============================================================================

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<TsysXmlCardAuthenticationResponse, Self>>
    for RouterDataV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    >
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TsysXmlCardAuthenticationResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let response = &item.response;

        let is_success = matches!(response.status, Some(TsysXmlStatus::Pass))
            && response.response_code.as_deref() == Some("A0000");

        if !is_success {
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: AttemptStatus::Failure,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(ErrorResponse {
                    status_code: item.http_code,
                    code: response
                        .response_code
                        .clone()
                        .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string()),
                    message: response
                        .response_message
                        .clone()
                        .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string()),
                    reason: response.response_message.clone(),
                    attempt_status: Some(AttemptStatus::Failure),
                    connector_transaction_id: response.transaction_id.clone(),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: response.response_message.clone(),
                }),
                ..router_data.clone()
            });
        }

        // Prefer cardTransactionIdentifier (the actual NTID); fall back to
        // transactionID if the cert sandbox forgets to emit it.
        let ntid_source = response
            .card_transaction_identifier
            .clone()
            .or_else(|| response.transaction_id.clone())
            .ok_or_else(|| {
                crate::utils::response_deserialization_fail(
                    item.http_code,
                    "tsys_xml: CardAuthenticationResponse missing both <cardTransactionIdentifier> and <transactionID>; confirm API contract.",
                )
            })?;

        let path_a_mandate_id = format!("ntid:{ntid_source}");
        let mandate_reference = Box::new(MandateReference {
            connector_mandate_id: Some(path_a_mandate_id),
            payment_method_id: None,
            connector_mandate_request_reference_id: None,
        });

        let connector_txn_id = response
            .transaction_id
            .clone()
            .unwrap_or_else(|| ntid_source.clone());

        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(connector_txn_id.clone()),
            redirection_data: None,
            mandate_reference: Some(mandate_reference),
            connector_metadata: None,
            network_txn_id: response.auth_code.clone(),
            connector_response_reference_id: Some(connector_txn_id),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                // Card verified — Authorized is the closest non-charged status.
                status: AttemptStatus::Authorized,
                ..router_data.resource_common_data.clone()
            },
            response: Ok(payments_response_data),
            ..router_data.clone()
        })
    }
}

// =============================================================================
// REPEAT PAYMENT — request transformer
// =============================================================================
//
// TransIT does not expose a separate "RecurringCharge" endpoint; MIT replays
// fire the same `<Sale>` (auto-capture) or `<Auth>` (manual capture) XML body
// against the same POST `/` endpoint. We translate `RepeatPaymentData` into a
// synthetic `PaymentsAuthorizeData` so the existing Authorize TryFrom (and its
// `decode_mandate_dispatch` logic) handles Path A (NTID) and Path B (vault)
// without duplication.
fn repeat_payment_data_to_authorize<T: PaymentMethodDataTypes>(
    req: &RepeatPaymentData<T>,
) -> PaymentsAuthorizeData<T> {
    // RepeatPaymentData carries `mandate_reference: MandateReferenceId` directly;
    // wrap it into the `MandateIds` shape Authorize expects.
    let mandate_ids = MandateIds {
        mandate_id: None,
        mandate_reference_id: Some(req.mandate_reference.clone()),
    };

    PaymentsAuthorizeData {
        payment_method_data: req.payment_method_data.clone(),
        amount: req.minor_amount,
        order_tax_amount: None,
        email: req.email.clone(),
        customer_name: None,
        currency: req.currency,
        confirm: true,
        billing_descriptor: req.billing_descriptor.clone(),
        capture_method: req.capture_method,
        router_return_url: req.router_return_url.clone(),
        webhook_url: req.webhook_url.clone(),
        complete_authorize_url: None,
        mandate_id: Some(mandate_ids),
        setup_future_usage: None,
        // MIT — explicitly off-session per the spec.
        off_session: Some(true),
        browser_info: req.browser_info.clone(),
        order_category: None,
        session_token: None,
        access_token: None,
        customer_acceptance: None,
        enrolled_for_3ds: None,
        related_transaction_id: None,
        payment_experience: None,
        payment_method_type: req.payment_method_type,
        customer_id: None,
        request_incremental_authorization: None,
        metadata: req.metadata.clone(),
        authentication_data: req.authentication_data.clone(),
        split_payments: req.split_payments.clone(),
        minor_amount: req.minor_amount,
        merchant_order_id: req.merchant_order_id.clone(),
        shipping_cost: req.shipping_cost,
        merchant_account_id: req.merchant_account_id.as_ref().map(|s| s.peek().clone()),
        integrity_object: None,
        merchant_config_currency: req.merchant_configured_currency,
        all_keys_required: None,
        request_extended_authorization: None,
        enable_overcapture: None,
        setup_mandate_details: None,
        connector_feature_data: req.connector_feature_data.clone(),
        connector_testing_data: req.connector_testing_data.clone(),
        // MIT replay — channel inferred from the original CIT; default to
        // Ecommerce so terminalData defaults match the typical recurring case.
        payment_channel: None,
        enable_partial_authorization: req.enable_partial_authorization,
        locale: req.locale.clone(),
        redirect_response: None,
        threeds_method_comp_ind: None,
        continue_redirection_url: None,
        tokenization: None,
        // Pipe HS-native MIT fields through so the synthesized Authorize body
        // engages recurring/installment mode without any metadata shim.
        mit_category: req.mit_category.clone(),
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TsysXmlRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for TsysXmlRepeatPaymentRequest<T>
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: TsysXmlRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let TsysXmlRouterData {
            connector,
            router_data,
        } = item;

        // Project the RepeatPayment RouterDataV2 onto an Authorize-shaped one so
        // the existing TryFrom (which encodes all of Path A / Path B / CIT logic)
        // can build the wire body unchanged.
        let synthetic_request = repeat_payment_data_to_authorize(&router_data.request);

        let synthetic_router_data: RouterDataV2<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<T>,
            PaymentsResponseData,
        > = RouterDataV2 {
            flow: std::marker::PhantomData,
            resource_common_data: router_data.resource_common_data.clone(),
            connector_config: router_data.connector_config.clone(),
            request: synthetic_request,
            response: Err(ErrorResponse::default()),
        };

        let synthetic_wrapper = TsysXmlRouterData {
            connector,
            router_data: synthetic_router_data,
        };

        let inner = TsysXmlAuthorizeRequest::<T>::try_from(synthetic_wrapper)?;
        Ok(Self(inner))
    }
}

// =============================================================================
// REPEAT PAYMENT — response transformer
// =============================================================================
//
// Response shape is identical to Authorize (Sale / Auth response). We reuse
// `map_authorize_status` and the same success/failure surface; only the
// `RouterDataV2` flow phantom differs.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<TsysXmlRepeatPaymentResponse, Self>>
    for RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TsysXmlRepeatPaymentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let response = &item.response;
        let body = response.body();

        // Reuse the Authorize status mapper by projecting onto the Authorize
        // response enum (wire shape is identical per tech spec).
        let authorize_view = response.as_authorize();
        let status = map_authorize_status(&authorize_view);

        if matches!(status, AttemptStatus::Failure) {
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(ErrorResponse {
                    status_code: item.http_code,
                    code: body
                        .response_code
                        .clone()
                        .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string()),
                    message: body
                        .response_message
                        .clone()
                        .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string()),
                    reason: body.response_message.clone(),
                    attempt_status: Some(AttemptStatus::Failure),
                    connector_transaction_id: body.transaction_id.clone(),
                    network_decline_code: body.host_response_code.clone(),
                    network_advice_code: None,
                    network_error_message: body.response_message.clone(),
                }),
                ..router_data.clone()
            });
        }

        let transaction_id = body.transaction_id.clone().ok_or_else(|| {
            crate::utils::response_deserialization_fail(
                item.http_code,
                "tsys_xml: success response missing <transactionID>; confirm API contract.",
            )
        })?;

        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(transaction_id.clone()),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: body.auth_code.clone(),
            connector_response_reference_id: Some(transaction_id),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data.clone()
            },
            response: Ok(payments_response_data),
            ..router_data.clone()
        })
    }
}
