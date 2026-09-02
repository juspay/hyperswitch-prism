use super::frm_types::{
    FrmChargebackReceivedRequest, FrmFlowData, FrmPaymentOutcomeRequest, FrmRefundProcessedRequest,
    MerchantDetails, PostRiskCheckRequest, PostRiskCheckResponse, PreRiskCheckRequest,
    PreRiskCheckResponse,
};
use crate::{
    connector_types::{
        ConnectorResponseHeaders, CustomerInfo, RawConnectorRequestResponse,
        ServerAuthenticationTokenResponseData,
    },
    errors::IntegrationError,
    mandates::MandateAmountData,
    payment_address::{OrderDetailsWithAmount, PaymentAddress},
    router_request_types::BrowserInformation,
    types::{Connectors, PaymentMethodDataAction},
    utils::{extract_merchant_id_from_metadata, ForeignFrom, ForeignTryFrom},
};
use common_enums::{AttemptStatus, FrmDecision, PaymentMethodType};
use common_utils::{
    pii::Email,
    types::{MinorUnit, Money},
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, Secret};

// ── MerchantDetails conversion ────────────────────────────────────────────────

impl ForeignFrom<grpc_api_types::payments::MerchantDetails> for MerchantDetails {
    fn foreign_from(value: grpc_api_types::payments::MerchantDetails) -> Self {
        Self {
            merchant_id: value.merchant_id,
            merchant_category_code: value.merchant_category_code,
        }
    }
}

// ── FrmDecision conversions ───────────────────────────────────────────────────

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

// ── FrmFlowData conversions ───────────────────────────────────────────────────

impl
    ForeignTryFrom<(
        grpc_api_types::frm::FrmServicePreRiskCheckRequest,
        Connectors,
        &common_utils::metadata::MaskedMetadata,
    )> for FrmFlowData
{
    type Error = IntegrationError;

    fn foreign_try_from(
        (value, connectors, metadata): (
            grpc_api_types::frm::FrmServicePreRiskCheckRequest,
            Connectors,
            &common_utils::metadata::MaskedMetadata,
        ),
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let merchant_id = extract_merchant_id_from_metadata(metadata)?;

        let access_token = value
            .state
            .as_ref()
            .and_then(|state| state.access_token.as_ref())
            .map(ServerAuthenticationTokenResponseData::foreign_try_from)
            .transpose()?;

        Ok(Self {
            merchant_id,
            connectors: connectors.into(),
            access_token,
            raw_connector_response: None,
            raw_connector_request: None,
            typed_connector_request: None,
            typed_connector_response: None,
            connector_response_headers: None,
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
        (value, connectors, metadata): (
            grpc_api_types::frm::FrmServicePostRiskCheckRequest,
            Connectors,
            &common_utils::metadata::MaskedMetadata,
        ),
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let merchant_id = extract_merchant_id_from_metadata(metadata)?;

        let access_token = value
            .state
            .as_ref()
            .and_then(|state| state.access_token.as_ref())
            .map(ServerAuthenticationTokenResponseData::foreign_try_from)
            .transpose()?;

        Ok(Self {
            merchant_id,
            connectors: connectors.into(),
            access_token,
            raw_connector_response: None,
            raw_connector_request: None,
            typed_connector_request: None,
            typed_connector_response: None,
            connector_response_headers: None,
        })
    }
}

impl
    ForeignTryFrom<(
        grpc_api_types::payments::NotifyConnectorRequest,
        Connectors,
        &common_utils::metadata::MaskedMetadata,
    )> for FrmFlowData
{
    type Error = IntegrationError;

    fn foreign_try_from(
        (value, connectors, metadata): (
            grpc_api_types::payments::NotifyConnectorRequest,
            Connectors,
            &common_utils::metadata::MaskedMetadata,
        ),
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let merchant_id = extract_merchant_id_from_metadata(metadata)?;

        let access_token = value
            .state
            .as_ref()
            .and_then(|state| state.access_token.as_ref())
            .map(ServerAuthenticationTokenResponseData::foreign_try_from)
            .transpose()?;

        Ok(Self {
            merchant_id,
            connectors: connectors.into(),
            access_token,
            raw_connector_response: None,
            raw_connector_request: None,
            typed_connector_request: None,
            typed_connector_response: None,
            connector_response_headers: None,
        })
    }
}

// ── PreRiskCheckRequest / PostRiskCheckRequest conversions ────────────────────

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
            let grpc_currency = grpc_api_types::payments::Currency::try_from(amount.currency)
                .change_context(IntegrationError::InvalidDataFormat {
                    field_name: "currency",
                    context: crate::errors::IntegrationErrorContext {
                        additional_context: Some(
                            "Invalid currency in pre-risk check request".to_owned(),
                        ),
                        ..Default::default()
                    },
                })?;
            common_enums::Currency::foreign_try_from(grpc_currency)?
        };

        let customer_info = value
            .customer_info
            .map(CustomerInfo::foreign_try_from)
            .transpose()
            .change_context(IntegrationError::InvalidDataFormat {
                field_name: "customer_info",
                context: crate::errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Failed to parse customer info in pre-risk check".to_owned(),
                    ),
                    ..Default::default()
                },
            })?;

        let payment_method_type = value
            .payment_method
            .clone()
            .and_then(|pm| Option::<PaymentMethodType>::foreign_try_from(pm).ok())
            .flatten();

        let payment_method = value
            .payment_method
            .map(|pm| {
                // grpc_api_types::frm re-exports the same proto types as
                // grpc_api_types::payments, so we can reuse the shared
                // PaymentMethodDataAction pipeline directly.
                let payments_pm = grpc_api_types::payments::PaymentMethod {
                    payment_method: pm.payment_method,
                };
                let action =
                    PaymentMethodDataAction::get_payment_method_data_action(payments_pm.clone())
                        .change_context(IntegrationError::InvalidDataFormat {
                            field_name: "payment_method",
                            context: crate::errors::IntegrationErrorContext {
                                additional_context: Some(
                                    "Failed to parse payment method in pre-risk check".to_owned(),
                                ),
                                ..Default::default()
                            },
                        })?;
                action
                    .into_default_pci_payment_method_data(Some(payments_pm))
                    .change_context(IntegrationError::InvalidDataFormat {
                        field_name: "payment_method",
                        context: crate::errors::IntegrationErrorContext {
                            additional_context: Some(
                                "Failed to parse payment method in pre-risk check".to_owned(),
                            ),
                            ..Default::default()
                        },
                    })
            })
            .transpose()?;

        let browser_info = value
            .browser_info
            .map(BrowserInformation::foreign_try_from)
            .transpose()
            .change_context(IntegrationError::InvalidDataFormat {
                field_name: "browser_info",
                context: crate::errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Failed to parse browser info in pre-risk check".to_owned(),
                    ),
                    ..Default::default()
                },
            })?;

        let order_details = (!value.order_details.is_empty())
            .then(|| {
                value
                    .order_details
                    .into_iter()
                    .map(OrderDetailsWithAmount::foreign_try_from)
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()
            .change_context(IntegrationError::InvalidDataFormat {
                field_name: "order_details",
                context: crate::errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Failed to parse order details in pre-risk check".to_owned(),
                    ),
                    ..Default::default()
                },
            })?;

        let address = value
            .address
            .map(PaymentAddress::foreign_try_from)
            .transpose()
            .change_context(IntegrationError::InvalidDataFormat {
                field_name: "address",
                context: crate::errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Failed to parse address in pre-risk check".to_owned(),
                    ),
                    ..Default::default()
                },
            })?;

        let mandate_details = value
            .mandate_details
            .map(MandateAmountData::foreign_try_from)
            .transpose()?;

        Ok(Self {
            amount: Money {
                amount: MinorUnit::new(amount.minor_amount),
                currency,
            },
            customer_info,
            payment_method,
            browser_info,
            merchant_transaction_id: value.merchant_transaction_id,
            order_details,
            address,
            metadata: value.metadata,
            connector_feature_data: value.connector_feature_data,
            test_mode: value.test_mode,
            mandate_details,
            merchant_details: value.merchant_details.map(MerchantDetails::foreign_from),
            payment_method_type,
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
            let grpc_currency = grpc_api_types::payments::Currency::try_from(amount.currency)
                .change_context(IntegrationError::InvalidDataFormat {
                    field_name: "currency",
                    context: crate::errors::IntegrationErrorContext {
                        additional_context: Some(
                            "Invalid currency in post-risk check request".to_owned(),
                        ),
                        ..Default::default()
                    },
                })?;
            common_enums::Currency::foreign_try_from(grpc_currency)?
        };

        let payment_status = value.payment_status.and_then(|status| {
            grpc_api_types::payments::PaymentStatus::try_from(status)
                .ok()
                .and_then(|payment_status| AttemptStatus::foreign_try_from(payment_status).ok())
        });

        let payment_connector = value
            .payment_connector
            .and_then(|c| grpc_api_types::payments::Connector::try_from(c).ok());

        let customer_info = value
            .customer_info
            .map(CustomerInfo::foreign_try_from)
            .transpose()
            .change_context(IntegrationError::InvalidDataFormat {
                field_name: "customer_info",
                context: crate::errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Failed to parse customer info in post-risk check".to_owned(),
                    ),
                    ..Default::default()
                },
            })?;

        let payment_method = value
            .payment_method
            .map(|pm| {
                let payments_pm = grpc_api_types::payments::PaymentMethod {
                    payment_method: pm.payment_method,
                };
                let action =
                    PaymentMethodDataAction::get_payment_method_data_action(payments_pm.clone())
                        .change_context(IntegrationError::InvalidDataFormat {
                            field_name: "payment_method",
                            context: crate::errors::IntegrationErrorContext {
                                additional_context: Some(
                                    "Failed to parse payment method in post-risk check".to_owned(),
                                ),
                                ..Default::default()
                            },
                        })?;
                action
                    .into_default_pci_payment_method_data(Some(payments_pm))
                    .change_context(IntegrationError::InvalidDataFormat {
                        field_name: "payment_method",
                        context: crate::errors::IntegrationErrorContext {
                            additional_context: Some(
                                "Failed to parse payment method in post-risk check".to_owned(),
                            ),
                            ..Default::default()
                        },
                    })
            })
            .transpose()?;

        let order_details = (!value.order_details.is_empty())
            .then(|| {
                value
                    .order_details
                    .into_iter()
                    .map(OrderDetailsWithAmount::foreign_try_from)
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()
            .change_context(IntegrationError::InvalidDataFormat {
                field_name: "order_details",
                context: crate::errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Failed to parse order details in post-risk check".to_owned(),
                    ),
                    ..Default::default()
                },
            })?;

        let address = value
            .address
            .map(PaymentAddress::foreign_try_from)
            .transpose()
            .change_context(IntegrationError::InvalidDataFormat {
                field_name: "address",
                context: crate::errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Failed to parse address in post-risk check".to_owned(),
                    ),
                    ..Default::default()
                },
            })?;

        Ok(Self {
            amount: Money {
                amount: MinorUnit::new(amount.minor_amount),
                currency,
            },
            customer_info,
            payment_method,
            merchant_transaction_id: value.merchant_transaction_id,
            order_details,
            metadata: value.metadata,
            connector_feature_data: value.connector_feature_data,
            test_mode: value.test_mode,
            payment_status,
            connector_transaction_id: value.connector_transaction_id,
            payment_connector,
            address,
        })
    }
}

// ── frm:: type conversions ────────────────────────────────────────────────────
// After the lib.rs namespace unification, grpc_api_types::frm::* re-exports the same
// proto-generated types as grpc_api_types::payments::*, so most conversions are handled
// by the payments:: impls in types.rs. Only types whose target differs from the
// payments:: equivalents are kept here.

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
            .map(|email| email.expose().parse::<Email>())
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
            date_of_birth: None,
        })
    }
}

// ── FRM Notification ForeignTryFrom ──────────────────────────────────────────

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
            let grpc_currency = grpc_api_types::payments::Currency::try_from(amount.currency)
                .change_context(IntegrationError::InvalidDataFormat {
                    field_name: "currency",
                    context: crate::errors::IntegrationErrorContext {
                        additional_context: Some(
                            "Invalid currency in FRM payment outcome".to_owned(),
                        ),
                        ..Default::default()
                    },
                })?;
            common_enums::Currency::foreign_try_from(grpc_currency)?
        };

        let payment_details = match value.notification_type {
            Some(grpc_api_types::payments::frm_notification_content::NotificationType::PaymentDetails(pd)) => pd,
            _ => return Err(error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "payment_details",
                context: crate::errors::IntegrationErrorContext {
                    additional_context: Some("Payment details required for FRM payment outcome".to_owned()),
                    ..Default::default()
                },
            })),
        };

        let payment_status = payment_details
            .payment_status
            .try_into()
            .ok()
            .and_then(|status| AttemptStatus::foreign_try_from(status).ok());

        let frm_decision = value.frm_decision.and_then(|decision| {
            grpc_api_types::frm::FrmDecision::try_from(decision)
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
            merchant_transaction_id: payment_details.merchant_transaction_id,
            frm_decision,
            merchant_details: value.merchant_details.map(MerchantDetails::foreign_from),
            connector_feature_data: value.connector_feature_data,
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
            let grpc_currency = grpc_api_types::payments::Currency::try_from(amount.currency)
                .change_context(IntegrationError::InvalidDataFormat {
                    field_name: "currency",
                    context: crate::errors::IntegrationErrorContext {
                        additional_context: Some(
                            "Invalid currency in FRM refund processed".to_owned(),
                        ),
                        ..Default::default()
                    },
                })?;
            common_enums::Currency::foreign_try_from(grpc_currency)?
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

        let frm_decision = value.frm_decision.and_then(|decision| {
            grpc_api_types::frm::FrmDecision::try_from(decision)
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
            merchant_details: value.merchant_details.map(MerchantDetails::foreign_from),
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
            let grpc_currency = grpc_api_types::payments::Currency::try_from(amount.currency)
                .change_context(IntegrationError::InvalidDataFormat {
                    field_name: "currency",
                    context: crate::errors::IntegrationErrorContext {
                        additional_context: Some("Invalid currency in FRM chargeback".to_owned()),
                        ..Default::default()
                    },
                })?;
            common_enums::Currency::foreign_try_from(grpc_currency)?
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

        let frm_decision = value.frm_decision.and_then(|decision| {
            grpc_api_types::frm::FrmDecision::try_from(decision)
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

// ── Response generation functions ─────────────────────────────────────────────

pub fn generate_pre_risk_check_response(
    router_data_v2: crate::router_data_v2::RouterDataV2<
        crate::connector_flow::PreRiskCheck,
        super::frm_types::FrmFlowData,
        super::frm_types::PreRiskCheckRequest,
        super::frm_types::PreRiskCheckResponse,
    >,
) -> Result<
    grpc_api_types::frm::FrmServicePreRiskCheckResponse,
    error_stack::Report<crate::errors::ConnectorError>,
> {
    let raw_connector_response = router_data_v2
        .resource_common_data
        .get_raw_connector_response();
    let typed_connector_response = router_data_v2
        .resource_common_data
        .get_typed_connector_response()
        .map(Secret::new);
    let raw_connector_request = router_data_v2
        .resource_common_data
        .get_raw_connector_request();
    let typed_connector_request = router_data_v2
        .resource_common_data
        .get_typed_connector_request()
        .map(Secret::new);
    let response_headers = router_data_v2
        .resource_common_data
        .get_connector_response_headers_as_map();

    let response = match router_data_v2.response {
        Ok(PreRiskCheckResponse {
            frm_decision,
            risk_score,
            reason,
            frm_transaction_id,
            status_code,
        }) => {
            let grpc_frm_decision = frm_decision
                .map(grpc_api_types::frm::FrmDecision::foreign_from)
                .unwrap_or(grpc_api_types::frm::FrmDecision::Unspecified);

            grpc_api_types::frm::FrmServicePreRiskCheckResponse {
                frm_decision: Some(grpc_frm_decision as i32),
                risk_score,
                reason,
                frm_transaction_id,
                status_code: status_code.into(),
                error: None,
                raw_connector_request,
                typed_connector_request,
                raw_connector_response,
                typed_connector_response,
                response_headers,
            }
        }
        Err(err) => grpc_api_types::frm::FrmServicePreRiskCheckResponse {
            frm_decision: Some(grpc_api_types::frm::FrmDecision::Unspecified as i32),
            risk_score: None,
            reason: None,
            frm_transaction_id: None,
            status_code: err.status_code.into(),
            error: Some(grpc_api_types::frm::ErrorInfo {
                unified_details: None,
                connector_details: Some(grpc_api_types::frm::ConnectorErrorDetails {
                    code: Some(err.code),
                    message: Some(err.message.clone()),
                    reason: None,
                    connector_transaction_id: err.connector_transaction_id.clone(),
                    status: None,
                }),
                issuer_details: None,
            }),
            raw_connector_request,
            typed_connector_request,
            raw_connector_response,
            typed_connector_response,
            response_headers,
        },
    };
    Ok(response)
}

pub fn generate_post_risk_check_response(
    router_data_v2: crate::router_data_v2::RouterDataV2<
        crate::connector_flow::PostRiskCheck,
        super::frm_types::FrmFlowData,
        super::frm_types::PostRiskCheckRequest,
        super::frm_types::PostRiskCheckResponse,
    >,
) -> Result<
    grpc_api_types::frm::FrmServicePostRiskCheckResponse,
    error_stack::Report<crate::errors::ConnectorError>,
> {
    let raw_connector_response = router_data_v2
        .resource_common_data
        .get_raw_connector_response();
    let typed_connector_response = router_data_v2
        .resource_common_data
        .get_typed_connector_response()
        .map(Secret::new);
    let raw_connector_request = router_data_v2
        .resource_common_data
        .get_raw_connector_request();
    let typed_connector_request = router_data_v2
        .resource_common_data
        .get_typed_connector_request()
        .map(Secret::new);
    let response_headers = router_data_v2
        .resource_common_data
        .get_connector_response_headers_as_map();

    let response = match router_data_v2.response {
        Ok(PostRiskCheckResponse {
            frm_decision,
            risk_score,
            reason,
            frm_transaction_id,
            status_code,
        }) => {
            let grpc_frm_decision = frm_decision
                .map(grpc_api_types::frm::FrmDecision::foreign_from)
                .unwrap_or(grpc_api_types::frm::FrmDecision::Unspecified);

            grpc_api_types::frm::FrmServicePostRiskCheckResponse {
                frm_decision: Some(grpc_frm_decision as i32),
                risk_score,
                reason,
                frm_transaction_id,
                status_code: status_code.into(),
                error: None,
                raw_connector_request,
                typed_connector_request,
                raw_connector_response,
                typed_connector_response,
                response_headers,
            }
        }
        Err(err) => grpc_api_types::frm::FrmServicePostRiskCheckResponse {
            frm_decision: Some(grpc_api_types::frm::FrmDecision::Unspecified as i32),
            risk_score: None,
            reason: None,
            frm_transaction_id: None,
            status_code: err.status_code.into(),
            error: Some(grpc_api_types::frm::ErrorInfo {
                unified_details: None,
                connector_details: Some(grpc_api_types::frm::ConnectorErrorDetails {
                    code: Some(err.code),
                    message: Some(err.message.clone()),
                    reason: None,
                    connector_transaction_id: err.connector_transaction_id.clone(),
                    status: None,
                }),
                issuer_details: None,
            }),
            raw_connector_request,
            typed_connector_request,
            raw_connector_response,
            typed_connector_response,
            response_headers,
        },
    };
    Ok(response)
}

pub fn generate_frm_payment_outcome_response(
    router_data_v2: crate::router_data_v2::RouterDataV2<
        crate::connector_flow::FrmPaymentOutcome,
        super::frm_types::FrmFlowData,
        super::frm_types::FrmPaymentOutcomeRequest,
        super::frm_types::FrmPaymentOutcomeResponse,
    >,
) -> Result<
    grpc_api_types::payments::NotifyConnectorResponse,
    error_stack::Report<crate::errors::ConnectorError>,
> {
    match router_data_v2.response {
        Ok(response) => Ok(grpc_api_types::payments::NotifyConnectorResponse {
            status_code: response.status_code.into(),
            error: None,
        }),
        Err(e) => Ok(grpc_api_types::payments::NotifyConnectorResponse {
            status_code: e.status_code.into(),
            error: Some(grpc_api_types::payments::ErrorInfo {
                unified_details: None,
                connector_details: Some(grpc_api_types::payments::ConnectorErrorDetails {
                    code: Some(e.code),
                    message: Some(e.message.clone()),
                    reason: e.reason.clone(),
                    connector_transaction_id: e.connector_transaction_id.clone(),
                    status: None,
                }),
                issuer_details: None,
            }),
        }),
    }
}

pub fn generate_frm_refund_processed_response(
    router_data_v2: crate::router_data_v2::RouterDataV2<
        crate::connector_flow::FrmRefundProcessed,
        super::frm_types::FrmFlowData,
        super::frm_types::FrmRefundProcessedRequest,
        super::frm_types::FrmRefundProcessedResponse,
    >,
) -> Result<
    grpc_api_types::payments::NotifyConnectorResponse,
    error_stack::Report<crate::errors::ConnectorError>,
> {
    match router_data_v2.response {
        Ok(response) => Ok(grpc_api_types::payments::NotifyConnectorResponse {
            status_code: response.status_code.into(),
            error: None,
        }),
        Err(e) => Ok(grpc_api_types::payments::NotifyConnectorResponse {
            status_code: e.status_code.into(),
            error: Some(grpc_api_types::payments::ErrorInfo {
                unified_details: None,
                connector_details: Some(grpc_api_types::payments::ConnectorErrorDetails {
                    code: Some(e.code),
                    message: Some(e.message.clone()),
                    reason: e.reason.clone(),
                    connector_transaction_id: e.connector_transaction_id.clone(),
                    status: None,
                }),
                issuer_details: None,
            }),
        }),
    }
}

pub fn generate_frm_chargeback_received_response(
    router_data_v2: crate::router_data_v2::RouterDataV2<
        crate::connector_flow::FrmChargebackReceived,
        super::frm_types::FrmFlowData,
        super::frm_types::FrmChargebackReceivedRequest,
        super::frm_types::FrmChargebackReceivedResponse,
    >,
) -> Result<
    grpc_api_types::payments::NotifyConnectorResponse,
    error_stack::Report<crate::errors::ConnectorError>,
> {
    match router_data_v2.response {
        Ok(response) => Ok(grpc_api_types::payments::NotifyConnectorResponse {
            status_code: response.status_code.into(),
            error: None,
        }),
        Err(e) => Ok(grpc_api_types::payments::NotifyConnectorResponse {
            status_code: e.status_code.into(),
            error: Some(grpc_api_types::payments::ErrorInfo {
                unified_details: None,
                connector_details: Some(grpc_api_types::payments::ConnectorErrorDetails {
                    code: Some(e.code),
                    message: Some(e.message.clone()),
                    reason: e.reason.clone(),
                    connector_transaction_id: e.connector_transaction_id.clone(),
                    status: None,
                }),
                issuer_details: None,
            }),
        }),
    }
}
