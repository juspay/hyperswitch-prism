use crate::{
    connector_types::{ConnectorResponseHeaders, CustomerInfo, RawConnectorRequestResponse},
    errors::IntegrationError,
    payment_address::{
        Address, AddressDetails, OrderDetailsWithAmount, PaymentAddress, PhoneDetails,
    },
    payment_method_data::{Card, DefaultPCIHolder, PaymentMethodData, RawCardNumber},
    router_request_types::BrowserInformation,
    types::Connectors,
    utils::{extract_merchant_id_from_metadata, ForeignFrom, ForeignTryFrom},
};
use common_enums::{AttemptStatus, CardNetwork, CountryAlpha2, FrmDecision};
use common_utils::{
    pii::Email,
    types::{MinorUnit, Money},
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, Secret};

impl ForeignFrom<grpc_api_types::frm::FrmDecision> for FrmDecision {
    fn foreign_from(value: grpc_api_types::frm::FrmDecision) -> Self {
        match value {
            grpc_api_types::frm::FrmDecision::Approve => Self::Approve,
            grpc_api_types::frm::FrmDecision::Reject => Self::Reject,
            grpc_api_types::frm::FrmDecision::Unspecified
            | grpc_api_types::frm::FrmDecision::Review => Self::Review,
            grpc_api_types::frm::FrmDecision::Error => Self::Error,
        }
    }
}

impl ForeignFrom<FrmDecision> for grpc_api_types::frm::FrmDecision {
    fn foreign_from(value: FrmDecision) -> Self {
        match value {
            FrmDecision::Approve => Self::Approve,
            FrmDecision::Reject => Self::Reject,
            FrmDecision::Review => Self::Review,
            FrmDecision::Error => Self::Error,
        }
    }
}

impl
    ForeignTryFrom<(
        grpc_api_types::frm::FrmServicePreRiskCheckRequest,
        Connectors,
        &common_utils::metadata::MaskedMetadata,
    )> for FrmFlowData
{
    type Error = IntegrationError;

    fn foreign_try_from(
        (_value, connectors, metadata): (
            grpc_api_types::frm::FrmServicePreRiskCheckRequest,
            Connectors,
            &common_utils::metadata::MaskedMetadata,
        ),
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let merchant_id = extract_merchant_id_from_metadata(metadata)?;

        Ok(Self {
            merchant_id,
            connectors,
            raw_connector_response: None,
            raw_connector_request: None,
            connector_response_headers: None,
        })
    }
}

impl ForeignTryFrom<grpc_api_types::frm::FrmServicePreRiskCheckRequest> for PreRiskCheckRequest {
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::frm::FrmServicePreRiskCheckRequest,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let amount = value.amount.ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "amount",
                context: crate::errors::IntegrationErrorContext {
                    additional_context: Some("Amount is required for pre-risk check".to_owned()),
                    ..Default::default()
                },
            })
        })?;

        let currency = {
            let curr = grpc_api_types::payments::Currency::try_from(amount.currency)
                .change_context(IntegrationError::InvalidDataFormat {
                    field_name: "currency",
                    context: crate::errors::IntegrationErrorContext {
                        additional_context: Some(
                            "Invalid currency in pre-risk check request".to_owned(),
                        ),
                        ..Default::default()
                    },
                })?;
            common_enums::Currency::foreign_try_from(curr)?
        };

        Ok(Self {
            amount: Money {
                amount: MinorUnit::new(amount.minor_amount),
                currency,
            },
            customer_info: value
                .customer_info
                .and_then(|ci| CustomerInfo::foreign_try_from(ci).ok()),
            payment_method: value
                .payment_method
                .and_then(|pm| PaymentMethodData::<DefaultPCIHolder>::foreign_try_from(pm).ok()),
            browser_info: value
                .browser_info
                .and_then(|bi| BrowserInformation::foreign_try_from(bi).ok()),
            merchant_transaction_id: value.merchant_transaction_id,
            order_details: {
                let details: Vec<OrderDetailsWithAmount> = value
                    .order_details
                    .into_iter()
                    .filter_map(|d| OrderDetailsWithAmount::foreign_try_from(d).ok())
                    .collect();
                if details.is_empty() {
                    None
                } else {
                    Some(details)
                }
            },
            address: value.address.and_then(|addr| {
                Address::foreign_try_from(addr)
                    .ok()
                    .map(|a| PaymentAddress::new(None, Some(a.clone()), Some(a), Some(false)))
            }),
            metadata: value.metadata,
            connector_feature_data: value.connector_feature_data,
            test_mode: value.test_mode,
        })
    }
}

impl
    ForeignTryFrom<(
        grpc_api_types::frm::FrmServicePostRiskCheckRequest,
        Connectors,
        &common_utils::metadata::MaskedMetadata,
    )> for FrmFlowData
{
    type Error = IntegrationError;

    fn foreign_try_from(
        (_value, connectors, metadata): (
            grpc_api_types::frm::FrmServicePostRiskCheckRequest,
            Connectors,
            &common_utils::metadata::MaskedMetadata,
        ),
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let merchant_id = extract_merchant_id_from_metadata(metadata)?;

        Ok(Self {
            merchant_id,
            connectors,
            raw_connector_response: None,
            raw_connector_request: None,
            connector_response_headers: None,
        })
    }
}

impl ForeignTryFrom<grpc_api_types::frm::FrmServicePostRiskCheckRequest> for PostRiskCheckRequest {
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::frm::FrmServicePostRiskCheckRequest,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let amount = value.amount.ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "amount",
                context: crate::errors::IntegrationErrorContext {
                    additional_context: Some("Amount is required for post-risk check".to_owned()),
                    ..Default::default()
                },
            })
        })?;

        let currency = {
            let curr = grpc_api_types::payments::Currency::try_from(amount.currency)
                .change_context(IntegrationError::InvalidDataFormat {
                    field_name: "currency",
                    context: crate::errors::IntegrationErrorContext {
                        additional_context: Some(
                            "Invalid currency in post-risk check request".to_owned(),
                        ),
                        ..Default::default()
                    },
                })?;
            common_enums::Currency::foreign_try_from(curr)?
        };

        let payment_status = value.payment_status.and_then(|status| {
            grpc_api_types::payments::PaymentStatus::try_from(status)
                .ok()
                .and_then(|payment_status| AttemptStatus::foreign_try_from(payment_status).ok())
        });

        let payment_connector = value
            .payment_connector
            .and_then(|c| grpc_api_types::payments::Connector::try_from(c).ok());

        Ok(Self {
            amount: Money {
                amount: MinorUnit::new(amount.minor_amount),
                currency,
            },
            customer_info: value
                .customer_info
                .and_then(|ci| CustomerInfo::foreign_try_from(ci).ok()),
            payment_method: value
                .payment_method
                .and_then(|pm| PaymentMethodData::<DefaultPCIHolder>::foreign_try_from(pm).ok()),
            merchant_transaction_id: value.merchant_transaction_id,
            order_details: {
                let details: Vec<OrderDetailsWithAmount> = value
                    .order_details
                    .into_iter()
                    .filter_map(|d| OrderDetailsWithAmount::foreign_try_from(d).ok())
                    .collect();
                if details.is_empty() {
                    None
                } else {
                    Some(details)
                }
            },
            metadata: value.metadata,
            connector_feature_data: value.connector_feature_data,
            test_mode: value.test_mode,
            payment_status,
            connector_transaction_id: value.connector_transaction_id,
            payment_connector,
        })
    }
}

// ── frm:: type conversions ────────────────────────────────────────────────────
// These mirror the payments:: equivalents but target grpc_api_types::frm types,
// which are separate Rust types even though they originate from the same proto package.

impl ForeignTryFrom<grpc_api_types::frm::Customer> for CustomerInfo {
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::frm::Customer,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let customer_id = value
            .id
            .map(|id| common_utils::id_type::CustomerId::try_from(std::borrow::Cow::from(id)))
            .transpose()
            .change_context(IntegrationError::InvalidDataFormat {
                field_name: "customer_info.id",
                context: crate::errors::IntegrationErrorContext {
                    additional_context: Some("Failed to parse customer ID".to_owned()),
                    ..Default::default()
                },
            })?;

        let customer_email = value
            .email
            .map(|e| e.expose().parse::<Email>())
            .transpose()
            .map_err(|_| {
                error_stack::report!(IntegrationError::InvalidDataFormat {
                    field_name: "customer_info.email",
                    context: crate::errors::IntegrationErrorContext {
                        additional_context: Some("Invalid customer email format".to_owned()),
                        ..Default::default()
                    },
                })
            })?;

        Ok(Self {
            customer_id,
            customer_email,
            customer_name: value.name.map(Into::into),
            first_name: value.first_name.map(Into::into),
            last_name: value.last_name.map(Into::into),
            customer_phone_number: value.phone_number,
            customer_phone_country_code: value.phone_country_code,
            salutation: value.salutation,
        })
    }
}

impl ForeignTryFrom<grpc_api_types::frm::BrowserInformation> for BrowserInformation {
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::frm::BrowserInformation,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        Ok(Self {
            color_depth: value.color_depth.map(|cd| cd as u8),
            java_enabled: value.java_enabled,
            java_script_enabled: value.java_script_enabled,
            language: value.language,
            screen_height: value.screen_height,
            screen_width: value.screen_width,
            time_zone: value.time_zone_offset_minutes,
            ip_address: value.ip_address.and_then(|ip| ip.parse().ok()),
            accept_header: value.accept_header,
            user_agent: value.user_agent,
            os_type: value.os_type,
            os_version: value.os_version,
            device_model: value.device_model,
            accept_language: value.accept_language,
            referer: value.referer,
        })
    }
}

impl ForeignTryFrom<grpc_api_types::frm::OrderDetailsWithAmount> for OrderDetailsWithAmount {
    type Error = IntegrationError;

    fn foreign_try_from(
        item: grpc_api_types::frm::OrderDetailsWithAmount,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        Ok(Self {
            product_name: item.product_name,
            quantity: u16::try_from(item.quantity).change_context(
                IntegrationError::InvalidDataFormat {
                    field_name: "order_details.quantity",
                    context: crate::errors::IntegrationErrorContext {
                        additional_context: Some(
                            "Quantity value is out of range for u16".to_owned(),
                        ),
                        ..Default::default()
                    },
                },
            )?,
            amount: MinorUnit::new(item.amount),
            tax_rate: item.tax_rate,
            total_tax_amount: item.total_tax_amount.map(MinorUnit::new),
            requires_shipping: item.requires_shipping,
            product_img_link: item.product_img_link,
            product_id: item.product_id,
            category: item.category,
            sub_category: item.sub_category,
            brand: item.brand,
            description: item.description,
            unit_of_measure: item.unit_of_measure,
            // Convert frm::ProductType i32 to payments::ProductType via shared integer values.
            product_type: item
                .product_type
                .and_then(|pt| grpc_api_types::payments::ProductType::try_from(pt).ok())
                .filter(|pt| !matches!(pt, grpc_api_types::payments::ProductType::Unspecified))
                .map(common_enums::ProductType::foreign_from),
            product_tax_code: item.product_tax_code,
            commodity_code: item.commodity_code,
            sku: item.sku,
            upc: item.upc,
            unit_discount_amount: item.unit_discount_amount.map(MinorUnit::new),
            total_amount: None,
        })
    }
}

impl ForeignTryFrom<grpc_api_types::frm::Address> for AddressDetails {
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::frm::Address,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        // Convert frm::CountryAlpha2 to payments::CountryAlpha2 via shared proto integer values,
        // then use the existing ForeignTryFrom impl for the domain CountryAlpha2.
        let country_code_i32 = value.country_alpha2_code() as i32;
        let payments_country = grpc_api_types::payments::CountryAlpha2::try_from(country_code_i32)
            .unwrap_or(grpc_api_types::payments::CountryAlpha2::Unspecified);
        let country = if matches!(
            payments_country,
            grpc_api_types::payments::CountryAlpha2::Unspecified
        ) {
            None
        } else {
            Some(CountryAlpha2::foreign_try_from(payments_country)?)
        };

        Ok(Self {
            country,
            city: value.city,
            line1: value.line1,
            line2: value.line2,
            line3: value.line3,
            zip: value.zip_code,
            state: value.state,
            first_name: value.first_name,
            last_name: value.last_name,
            origin_zip: None,
        })
    }
}

impl ForeignTryFrom<grpc_api_types::frm::Address> for Address {
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::frm::Address,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let email = value
            .email
            .clone()
            .map(|e| e.expose().parse::<Email>())
            .transpose()
            .map_err(|_| {
                error_stack::report!(IntegrationError::InvalidDataFormat {
                    field_name: "address.email",
                    context: crate::errors::IntegrationErrorContext {
                        additional_context: Some("Invalid email".to_owned()),
                        ..Default::default()
                    },
                })
            })?;

        Ok(Self {
            address: Some(AddressDetails::foreign_try_from(value.clone())?),
            phone: value.phone_number.map(|number| PhoneDetails {
                number: Some(number),
                country_code: value.phone_country_code,
            }),
            email,
        })
    }
}

impl ForeignTryFrom<grpc_api_types::frm::CardDetails> for Card<DefaultPCIHolder> {
    type Error = IntegrationError;

    fn foreign_try_from(
        card: grpc_api_types::frm::CardDetails,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        // Convert frm::CardNetwork to payments::CardNetwork via shared proto integer values,
        // then use the existing ForeignTryFrom impl for the domain CardNetwork.
        let frm_card_network = card.card_network();
        let card_network = if matches!(
            frm_card_network,
            grpc_api_types::frm::CardNetwork::Unspecified
        ) {
            None
        } else {
            let payments_cn =
                grpc_api_types::payments::CardNetwork::try_from(frm_card_network as i32)
                    .unwrap_or(grpc_api_types::payments::CardNetwork::Unspecified);
            Some(CardNetwork::foreign_try_from(payments_cn)?)
        };

        Ok(Self {
            card_number: RawCardNumber(card.card_number.ok_or_else(|| {
                error_stack::report!(IntegrationError::InvalidDataFormat {
                    field_name: "card.card_number",
                    context: crate::errors::IntegrationErrorContext {
                        additional_context: Some("Missing card number".to_owned()),
                        ..Default::default()
                    },
                })
            })?),
            card_exp_month: card.card_exp_month.ok_or_else(|| {
                error_stack::report!(IntegrationError::InvalidDataFormat {
                    field_name: "card.card_exp_month",
                    context: crate::errors::IntegrationErrorContext {
                        additional_context: Some("Missing card expiry month".to_owned()),
                        ..Default::default()
                    },
                })
            })?,
            card_exp_year: card.card_exp_year.ok_or_else(|| {
                error_stack::report!(IntegrationError::InvalidDataFormat {
                    field_name: "card.card_exp_year",
                    context: crate::errors::IntegrationErrorContext {
                        additional_context: Some("Missing card expiry year".to_owned()),
                        ..Default::default()
                    },
                })
            })?,
            card_cvc: card.card_cvc.ok_or_else(|| {
                error_stack::report!(IntegrationError::InvalidDataFormat {
                    field_name: "card.card_cvc",
                    context: crate::errors::IntegrationErrorContext {
                        additional_context: Some("Missing CVC".to_owned()),
                        ..Default::default()
                    },
                })
            })?,
            card_issuer: card.card_issuer,
            card_network,
            card_type: card.card_type,
            card_issuing_country: card.card_issuing_country_alpha2,
            bank_code: card.bank_code,
            nick_name: card.nick_name.map(Into::into),
            card_holder_name: card.card_holder_name,
            co_badged_card_data: None,
        })
    }
}

impl ForeignTryFrom<grpc_api_types::frm::PaymentMethod> for PaymentMethodData<DefaultPCIHolder> {
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::frm::PaymentMethod,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        match value.payment_method {
            Some(grpc_api_types::frm::payment_method::PaymentMethod::Card(card)) => Ok(
                PaymentMethodData::Card(Card::<DefaultPCIHolder>::foreign_try_from(card)?),
            ),
            _ => Err(error_stack::report!(IntegrationError::NotImplemented(
                "Non-card payment method conversion for FRM".to_owned(),
                Default::default()
            ))),
        }
    }
}

#[derive(Debug, Clone)]
pub struct FrmFlowData {
    pub merchant_id: common_utils::id_type::MerchantId,
    pub connectors: Connectors,
    pub raw_connector_response: Option<Secret<String>>,
    pub raw_connector_request: Option<Secret<String>>,
    pub connector_response_headers: Option<http::HeaderMap>,
}

impl RawConnectorRequestResponse for FrmFlowData {
    fn set_raw_connector_response(&mut self, response: Option<Secret<String>>) {
        self.raw_connector_response = response;
    }

    fn get_raw_connector_response(&self) -> Option<Secret<String>> {
        self.raw_connector_response.clone()
    }

    fn get_raw_connector_request(&self) -> Option<Secret<String>> {
        self.raw_connector_request.clone()
    }

    fn set_raw_connector_request(&mut self, request: Option<Secret<String>>) {
        self.raw_connector_request = request;
    }
}

impl ConnectorResponseHeaders for FrmFlowData {
    fn set_connector_response_headers(&mut self, headers: Option<http::HeaderMap>) {
        self.connector_response_headers = headers;
    }

    fn get_connector_response_headers(&self) -> Option<&http::HeaderMap> {
        self.connector_response_headers.as_ref()
    }
}

/// Request data for pre-risk check
#[derive(Debug, Clone)]
pub struct PreRiskCheckRequest {
    pub amount: Money,
    pub customer_info: Option<CustomerInfo>,
    pub payment_method: Option<PaymentMethodData<DefaultPCIHolder>>,
    pub browser_info: Option<BrowserInformation>,
    pub merchant_transaction_id: Option<String>,
    pub order_details: Option<Vec<OrderDetailsWithAmount>>,
    pub address: Option<crate::payment_address::PaymentAddress>,
    pub metadata: Option<Secret<String>>,
    pub connector_feature_data: Option<Secret<String>>,
    pub test_mode: Option<bool>,
}

/// Response data for pre-risk check
#[derive(Debug, Clone)]
pub struct PreRiskCheckResponse {
    pub frm_decision: Option<FrmDecision>,
    pub risk_score: Option<i32>,
    pub reason: Option<String>,
    pub frm_transaction_id: Option<String>,
    pub status_code: u16,
}

/// Request data for post-risk check
#[derive(Debug, Clone)]
pub struct PostRiskCheckRequest {
    pub amount: Money,
    pub customer_info: Option<CustomerInfo>,
    pub payment_method: Option<PaymentMethodData<DefaultPCIHolder>>,
    pub merchant_transaction_id: Option<String>,
    pub order_details: Option<Vec<OrderDetailsWithAmount>>,
    pub metadata: Option<Secret<String>>,
    pub connector_feature_data: Option<Secret<String>>,
    pub test_mode: Option<bool>,
    pub payment_status: Option<AttemptStatus>,
    pub connector_transaction_id: Option<String>,
    pub payment_connector: Option<grpc_api_types::payments::Connector>,
}

/// Response data for post-risk check
#[derive(Debug, Clone)]
pub struct PostRiskCheckResponse {
    pub frm_decision: Option<FrmDecision>,
    pub risk_score: Option<i32>,
    pub reason: Option<String>,
    pub frm_transaction_id: Option<String>,
    pub status_code: u16,
}

// ── FRM Notification Requests ─────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct FrmPaymentOutcomeRequest {
    pub connector_transaction_id: Option<String>,
    pub amount: Money,
    pub frm_transaction_id: Option<String>,
    pub payment_status: Option<AttemptStatus>,
    pub merchant_transaction_id: Option<String>,
    pub frm_decision: Option<FrmDecision>,
}

#[derive(Debug, Clone)]
pub struct FrmRefundProcessedRequest {
    pub connector_transaction_id: Option<String>,
    pub amount: Money,
    pub frm_transaction_id: Option<String>,
    pub connector_refund_id: Option<String>,
    pub merchant_refund_id: Option<String>,
    pub refund_reason: Option<String>,
    pub frm_decision: Option<FrmDecision>,
}

#[derive(Debug, Clone)]
pub struct FrmChargebackReceivedRequest {
    pub connector_transaction_id: Option<String>,
    pub amount: Money,
    pub frm_transaction_id: Option<String>,
    pub connector_dispute_id: Option<String>,
    pub merchant_dispute_id: Option<String>,
    pub chargeback_reason: Option<String>,
    pub frm_decision: Option<FrmDecision>,
}

// ── FRM Notification Responses ────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct FrmPaymentOutcomeResponse {
    pub status_code: u16,
}

#[derive(Debug, Clone)]
pub struct FrmRefundProcessedResponse {
    pub status_code: u16,
}

#[derive(Debug, Clone)]
pub struct FrmChargebackReceivedResponse {
    pub status_code: u16,
}

// ── ForeignTryFrom conversions ────────────────────────────────────────

impl ForeignTryFrom<grpc_api_types::payments::FrmNotificationContent> for FrmPaymentOutcomeRequest {
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::payments::FrmNotificationContent,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let amount = value.amount.ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "amount",
                context: crate::errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Amount is required for FRM payment outcome".to_owned()
                    ),
                    ..Default::default()
                },
            })
        })?;

        let currency = {
            let curr = grpc_api_types::payments::Currency::try_from(amount.currency)
                .change_context(IntegrationError::InvalidDataFormat {
                    field_name: "currency",
                    context: crate::errors::IntegrationErrorContext {
                        additional_context: Some(
                            "Invalid currency in FRM payment outcome".to_owned(),
                        ),
                        ..Default::default()
                    },
                })?;
            common_enums::Currency::foreign_try_from(curr)?
        };

        let payment_success = match value.notification_type {
            Some(grpc_api_types::payments::frm_notification_content::NotificationType::PaymentSuccess(ps)) => ps,
            _ => return Err(error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "payment_success",
                context: crate::errors::IntegrationErrorContext {
                    additional_context: Some("Payment success details required".to_owned()),
                    ..Default::default()
                },
            })),
        };

        let payment_status = payment_success
            .payment_status
            .try_into()
            .ok()
            .and_then(|status| AttemptStatus::foreign_try_from(status).ok());

        let frm_decision = value.frm_decision.and_then(|d| {
            grpc_api_types::frm::FrmDecision::try_from(d)
                .ok()
                .map(FrmDecision::foreign_from)
        });

        Ok(Self {
            connector_transaction_id: value.connector_transaction_id,
            amount: Money {
                amount: MinorUnit::new(amount.minor_amount),
                currency,
            },
            frm_transaction_id: value.frm_transaction_id,
            payment_status,
            merchant_transaction_id: payment_success.merchant_transaction_id,
            frm_decision,
        })
    }
}

impl ForeignTryFrom<grpc_api_types::payments::FrmNotificationContent>
    for FrmRefundProcessedRequest
{
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::payments::FrmNotificationContent,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let amount = value.amount.ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "amount",
                context: crate::errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Amount is required for FRM refund processed".to_owned()
                    ),
                    ..Default::default()
                },
            })
        })?;

        let currency = {
            let curr = grpc_api_types::payments::Currency::try_from(amount.currency)
                .change_context(IntegrationError::InvalidDataFormat {
                    field_name: "currency",
                    context: crate::errors::IntegrationErrorContext {
                        additional_context: Some(
                            "Invalid currency in FRM refund processed".to_owned(),
                        ),
                        ..Default::default()
                    },
                })?;
            common_enums::Currency::foreign_try_from(curr)?
        };

        let refund = match value.notification_type {
            Some(grpc_api_types::payments::frm_notification_content::NotificationType::Refund(
                r,
            )) => r,
            _ => {
                return Err(error_stack::report!(
                    IntegrationError::MissingRequiredField {
                        field_name: "refund",
                        context: crate::errors::IntegrationErrorContext {
                            additional_context: Some("Refund details required".to_owned()),
                            ..Default::default()
                        },
                    }
                ))
            }
        };

        let frm_decision = value.frm_decision.and_then(|d| {
            grpc_api_types::frm::FrmDecision::try_from(d)
                .ok()
                .map(FrmDecision::foreign_from)
        });

        Ok(Self {
            connector_transaction_id: value.connector_transaction_id,
            amount: Money {
                amount: MinorUnit::new(amount.minor_amount),
                currency,
            },
            frm_transaction_id: value.frm_transaction_id,
            connector_refund_id: refund.connector_refund_id,
            merchant_refund_id: refund.merchant_refund_id,
            refund_reason: refund.refund_reason,
            frm_decision,
        })
    }
}

impl ForeignTryFrom<grpc_api_types::payments::FrmNotificationContent>
    for FrmChargebackReceivedRequest
{
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::payments::FrmNotificationContent,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let amount = value.amount.ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "amount",
                context: crate::errors::IntegrationErrorContext {
                    additional_context: Some("Amount is required for FRM chargeback".to_owned()),
                    ..Default::default()
                },
            })
        })?;

        let currency = {
            let curr = grpc_api_types::payments::Currency::try_from(amount.currency)
                .change_context(IntegrationError::InvalidDataFormat {
                    field_name: "currency",
                    context: crate::errors::IntegrationErrorContext {
                        additional_context: Some("Invalid currency in FRM chargeback".to_owned()),
                        ..Default::default()
                    },
                })?;
            common_enums::Currency::foreign_try_from(curr)?
        };

        let chargeback = match value.notification_type {
            Some(
                grpc_api_types::payments::frm_notification_content::NotificationType::Chargeback(c),
            ) => c,
            _ => {
                return Err(error_stack::report!(
                    IntegrationError::MissingRequiredField {
                        field_name: "chargeback",
                        context: crate::errors::IntegrationErrorContext {
                            additional_context: Some("Chargeback details required".to_owned()),
                            ..Default::default()
                        },
                    }
                ))
            }
        };

        let frm_decision = value.frm_decision.and_then(|d| {
            grpc_api_types::frm::FrmDecision::try_from(d)
                .ok()
                .map(FrmDecision::foreign_from)
        });

        Ok(Self {
            connector_transaction_id: value.connector_transaction_id,
            amount: Money {
                amount: MinorUnit::new(amount.minor_amount),
                currency,
            },
            frm_transaction_id: value.frm_transaction_id,
            connector_dispute_id: chargeback.connector_dispute_id,
            merchant_dispute_id: chargeback.merchant_dispute_id,
            chargeback_reason: chargeback.chargeback_reason,
            frm_decision,
        })
    }
}

// ── FRM Notification ForeignTryFrom ─────────────────────────────────────

impl
    ForeignTryFrom<(
        grpc_api_types::payments::NotifyConnectorRequest,
        Connectors,
        &common_utils::metadata::MaskedMetadata,
    )> for FrmFlowData
{
    type Error = IntegrationError;

    fn foreign_try_from(
        (_value, connectors, metadata): (
            grpc_api_types::payments::NotifyConnectorRequest,
            Connectors,
            &common_utils::metadata::MaskedMetadata,
        ),
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let merchant_id = extract_merchant_id_from_metadata(metadata)?;

        Ok(Self {
            merchant_id,
            connectors,
            raw_connector_response: None,
            raw_connector_request: None,
            connector_response_headers: None,
        })
    }
}

// ForeignTryFrom for NotifyConnectorRequest -> FrmPaymentOutcomeRequest
impl ForeignTryFrom<grpc_api_types::payments::NotifyConnectorRequest> for FrmPaymentOutcomeRequest {
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::payments::NotifyConnectorRequest,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let notify_content = value.content.ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "content",
                context: crate::errors::IntegrationErrorContext {
                    additional_context: Some("NotifyConnector content required".to_owned()),
                    ..Default::default()
                },
            })
        })?;

        let frm_content = match notify_content.content {
            Some(grpc_api_types::payments::notify_connector_content::Content::FrmNotification(
                frm,
            )) => frm,
            _ => {
                return Err(error_stack::report!(
                    IntegrationError::MissingRequiredField {
                        field_name: "frm_notification",
                        context: crate::errors::IntegrationErrorContext {
                            additional_context: Some(
                                "FRM notification content required".to_owned()
                            ),
                            ..Default::default()
                        },
                    }
                ))
            }
        };

        Self::foreign_try_from(frm_content)
    }
}

impl ForeignTryFrom<grpc_api_types::payments::NotifyConnectorRequest>
    for FrmRefundProcessedRequest
{
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::payments::NotifyConnectorRequest,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let notify_content = value.content.ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "content",
                context: crate::errors::IntegrationErrorContext {
                    additional_context: Some("NotifyConnector content required".to_owned()),
                    ..Default::default()
                },
            })
        })?;

        let frm_content = match notify_content.content {
            Some(grpc_api_types::payments::notify_connector_content::Content::FrmNotification(
                frm,
            )) => frm,
            _ => {
                return Err(error_stack::report!(
                    IntegrationError::MissingRequiredField {
                        field_name: "frm_notification",
                        context: crate::errors::IntegrationErrorContext {
                            additional_context: Some(
                                "FRM notification content required".to_owned()
                            ),
                            ..Default::default()
                        },
                    }
                ))
            }
        };

        Self::foreign_try_from(frm_content)
    }
}

impl ForeignTryFrom<grpc_api_types::payments::NotifyConnectorRequest>
    for FrmChargebackReceivedRequest
{
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::payments::NotifyConnectorRequest,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let notify_content = value.content.ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "content",
                context: crate::errors::IntegrationErrorContext {
                    additional_context: Some("NotifyConnector content required".to_owned()),
                    ..Default::default()
                },
            })
        })?;

        let frm_content = match notify_content.content {
            Some(grpc_api_types::payments::notify_connector_content::Content::FrmNotification(
                frm,
            )) => frm,
            _ => {
                return Err(error_stack::report!(
                    IntegrationError::MissingRequiredField {
                        field_name: "frm_notification",
                        context: crate::errors::IntegrationErrorContext {
                            additional_context: Some(
                                "FRM notification content required".to_owned()
                            ),
                            ..Default::default()
                        },
                    }
                ))
            }
        };

        Self::foreign_try_from(frm_content)
    }
}
