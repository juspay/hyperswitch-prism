use crate::{
    connector_flow::{
        SurchargeCalculate, SurchargePaymentSucceeded, SurchargeRefundComplete,
        SurchargeRefundInit, SurchargeRefundSucceeded,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    router_data_v2::RouterDataV2,
    surcharge::surcharge_types::{
        SurchargeCalculateRequest, SurchargeCalculateResponse, SurchargeFlowData,
        SurchargePaymentSucceededRequest, SurchargePaymentSucceededResponse,
        SurchargeRefundCompleteRequest, SurchargeRefundCompleteResponse,
        SurchargeRefundInitRequest, SurchargeRefundInitResponse, SurchargeRefundSucceededRequest,
        SurchargeRefundSucceededResponse, SurchargeStrategy,
    },
    types::Connectors,
    utils::{
        extract_connector_request_reference_id, extract_merchant_id_from_metadata, ForeignTryFrom,
    },
};
use common_utils::{metadata::MaskedMetadata, types::MinorUnit};
use error_stack::ResultExt;

fn missing_refund_field(field_name: &'static str) -> error_stack::Report<IntegrationError> {
    error_stack::report!(IntegrationError::MissingRequiredField {
        field_name,
        context: IntegrationErrorContext::default(),
    })
}

fn surcharge_flow_data(
    reference: &Option<String>,
    connectors: Connectors,
    metadata: &MaskedMetadata,
) -> Result<SurchargeFlowData, error_stack::Report<IntegrationError>> {
    Ok(SurchargeFlowData {
        merchant_id: extract_merchant_id_from_metadata(metadata)?,
        connector_request_reference_id: extract_connector_request_reference_id(reference),
        connectors: connectors.into(),
        raw_connector_response: None,
        raw_connector_request: None,
        typed_connector_request: None,
        typed_connector_response: None,
        connector_response_headers: None,
    })
}

impl
    ForeignTryFrom<(
        grpc_api_types::surcharge::SurchargeServiceSurchargeRefundInitRequest,
        Connectors,
        &MaskedMetadata,
    )> for SurchargeFlowData
{
    type Error = IntegrationError;
    fn foreign_try_from(
        (value, connectors, metadata): (
            grpc_api_types::surcharge::SurchargeServiceSurchargeRefundInitRequest,
            Connectors,
            &MaskedMetadata,
        ),
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        surcharge_flow_data(&value.merchant_surcharge_refund_id, connectors, metadata)
    }
}

impl
    ForeignTryFrom<(
        grpc_api_types::surcharge::SurchargeServiceSurchargeRefundCompleteRequest,
        Connectors,
        &MaskedMetadata,
    )> for SurchargeFlowData
{
    type Error = IntegrationError;
    fn foreign_try_from(
        (value, connectors, metadata): (
            grpc_api_types::surcharge::SurchargeServiceSurchargeRefundCompleteRequest,
            Connectors,
            &MaskedMetadata,
        ),
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        surcharge_flow_data(&value.merchant_surcharge_refund_id, connectors, metadata)
    }
}

impl ForeignTryFrom<grpc_api_types::surcharge::SurchargeServiceSurchargeRefundInitRequest>
    for SurchargeRefundInitRequest
{
    type Error = IntegrationError;
    fn foreign_try_from(
        value: grpc_api_types::surcharge::SurchargeServiceSurchargeRefundInitRequest,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        if value.connector_surcharge_id.trim().is_empty() {
            return Err(missing_refund_field("connector_surcharge_id"));
        }
        let amount = value.amount.ok_or_else(|| missing_refund_field("amount"))?;
        if amount.minor_amount <= 0 {
            return Err(error_stack::report!(IntegrationError::InvalidDataFormat {
                field_name: "amount",
                context: IntegrationErrorContext::default(),
            }));
        }
        let currency = grpc_api_types::payments::Currency::try_from(amount.currency)
            .change_context(IntegrationError::InvalidDataFormat {
                field_name: "currency",
                context: IntegrationErrorContext::default(),
            })?;
        Ok(Self {
            connector_surcharge_id: value.connector_surcharge_id,
            amount: MinorUnit::new(amount.minor_amount),
            currency: common_enums::Currency::foreign_try_from(currency)?,
        })
    }
}

impl ForeignTryFrom<grpc_api_types::surcharge::SurchargeServiceSurchargeRefundCompleteRequest>
    for SurchargeRefundCompleteRequest
{
    type Error = IntegrationError;
    fn foreign_try_from(
        value: grpc_api_types::surcharge::SurchargeServiceSurchargeRefundCompleteRequest,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        if value.connector_surcharge_refund_id.trim().is_empty() {
            return Err(missing_refund_field("connector_surcharge_refund_id"));
        }
        Ok(Self {
            connector_surcharge_refund_id: value.connector_surcharge_refund_id,
        })
    }
}

fn surcharge_refund_error(
    e: crate::router_data::ErrorResponse,
) -> grpc_api_types::surcharge::ErrorInfo {
    grpc_api_types::surcharge::ErrorInfo {
        unified_details: None,
        connector_details: Some(grpc_api_types::surcharge::ConnectorErrorDetails {
            code: Some(e.code),
            message: Some(e.message),
            reason: e.reason,
            connector_transaction_id: e.connector_transaction_id,
            status: None,
        }),
        issuer_details: None,
    }
}

fn refund_money(
    amount: MinorUnit,
    currency: common_enums::Currency,
) -> Result<grpc_api_types::surcharge::Money, error_stack::Report<ConnectorError>> {
    Ok(grpc_api_types::surcharge::Money {
        minor_amount: amount.get_amount_as_i64(),
        currency: grpc_api_types::payments::Currency::foreign_try_from(currency)?.into(),
    })
}

pub fn generate_surcharge_refund_init_response(
    data: RouterDataV2<
        SurchargeRefundInit,
        SurchargeFlowData,
        SurchargeRefundInitRequest,
        SurchargeRefundInitResponse,
    >,
) -> Result<
    grpc_api_types::surcharge::SurchargeServiceSurchargeRefundInitResponse,
    error_stack::Report<ConnectorError>,
> {
    let reference = Some(data.resource_common_data.connector_request_reference_id);
    match data.response {
        Ok(response) => {
            let currency = response.currency.unwrap_or(data.request.currency);
            Ok(
                grpc_api_types::surcharge::SurchargeServiceSurchargeRefundInitResponse {
                    merchant_surcharge_refund_id: reference,
                    connector_surcharge_refund_id: Some(response.connector_surcharge_refund_id),
                    refund_amount: Some(refund_money(response.refund_amount, currency)?),
                    original_transaction_fee: response
                        .original_transaction_fee
                        .map(|amount| refund_money(amount, currency))
                        .transpose()?,
                    previously_refunded_transaction_fees: response
                        .previously_refunded_transaction_fees
                        .map(|amount| refund_money(amount, currency))
                        .transpose()?,
                    status_code: 200,
                    error: None,
                },
            )
        }
        Err(error) => Ok(
            grpc_api_types::surcharge::SurchargeServiceSurchargeRefundInitResponse {
                merchant_surcharge_refund_id: reference,
                connector_surcharge_refund_id: None,
                refund_amount: None,
                original_transaction_fee: None,
                previously_refunded_transaction_fees: None,
                status_code: error.status_code.into(),
                error: Some(surcharge_refund_error(error)),
            },
        ),
    }
}

pub fn generate_surcharge_refund_complete_response(
    data: RouterDataV2<
        SurchargeRefundComplete,
        SurchargeFlowData,
        SurchargeRefundCompleteRequest,
        SurchargeRefundCompleteResponse,
    >,
) -> Result<
    grpc_api_types::surcharge::SurchargeServiceSurchargeRefundCompleteResponse,
    error_stack::Report<ConnectorError>,
> {
    let reference = Some(data.resource_common_data.connector_request_reference_id);
    match data.response {
        Ok(response) => Ok(
            grpc_api_types::surcharge::SurchargeServiceSurchargeRefundCompleteResponse {
                merchant_surcharge_refund_id: reference,
                connector_surcharge_refund_id: Some(response.connector_surcharge_refund_id),
                status_code: 200,
                error: None,
            },
        ),
        Err(error) => Ok(
            grpc_api_types::surcharge::SurchargeServiceSurchargeRefundCompleteResponse {
                merchant_surcharge_refund_id: reference,
                connector_surcharge_refund_id: None,
                status_code: error.status_code.into(),
                error: Some(surcharge_refund_error(error)),
            },
        ),
    }
}
impl
    ForeignTryFrom<(
        grpc_api_types::surcharge::SurchargeServiceCalculateRequest,
        Connectors,
        &MaskedMetadata,
    )> for SurchargeFlowData
{
    type Error = IntegrationError;

    fn foreign_try_from(
        (value, connectors, metadata): (
            grpc_api_types::surcharge::SurchargeServiceCalculateRequest,
            Connectors,
            &MaskedMetadata,
        ),
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let merchant_id = extract_merchant_id_from_metadata(metadata)?;

        Ok(Self {
            merchant_id,
            connector_request_reference_id: extract_connector_request_reference_id(
                &value.merchant_surcharge_id,
            ),
            connectors: connectors.into(),
            raw_connector_response: None,
            raw_connector_request: None,
            typed_connector_request: None,
            typed_connector_response: None,
            connector_response_headers: None,
        })
    }
}

impl ForeignTryFrom<grpc_api_types::surcharge::SurchargeServiceCalculateRequest>
    for SurchargeCalculateRequest
{
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::surcharge::SurchargeServiceCalculateRequest,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let amount = value.amount.ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "amount",
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "Amount is required for surcharge calculation".to_owned()
                    ),
                    ..Default::default()
                },
            })
        })?;

        let currency = {
            let curr = grpc_api_types::payments::Currency::try_from(amount.currency)
                .change_context(IntegrationError::InvalidDataFormat {
                    field_name: "currency",
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "Invalid currency in surcharge request".to_owned(),
                        ),
                        ..Default::default()
                    },
                })?;
            common_enums::Currency::foreign_try_from(curr)?
        };

        let country = value
            .country
            .map(|country| {
                let country = grpc_api_types::payments::CountryAlpha2::try_from(country)
                    .change_context(IntegrationError::InvalidDataFormat {
                        field_name: "country",
                        context: IntegrationErrorContext {
                            additional_context: Some("Invalid country code".to_owned()),
                            ..Default::default()
                        },
                    })?;
                common_enums::CountryAlpha2::foreign_try_from(country)
            })
            .transpose()?;

        let surcharge_strategy = value.surcharge_strategy.map(|surcharge_strategy| {
            let grpc_strategy =
                grpc_api_types::surcharge::SurchargeStrategy::try_from(surcharge_strategy)
                    .unwrap_or(grpc_api_types::surcharge::SurchargeStrategy::Unspecified);
            SurchargeStrategy::from(grpc_strategy)
        });

        let postal_code = value.postal_code.ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "postal_code",
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "Postal code is required for surcharge calculation".to_owned()
                    ),
                    ..Default::default()
                },
            })
        })?;

        Ok(Self {
            amount: MinorUnit::new(amount.minor_amount),
            currency,
            previous_connector_surcharge_id: value.previous_connector_surcharge_id,
            surcharge_strategy,
            card_bin: value.card_bin,
            postal_code,
            country,
        })
    }
}

impl ForeignTryFrom<grpc_api_types::payments::NotifyConnectorRequest>
    for SurchargePaymentSucceededRequest
{
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::payments::NotifyConnectorRequest,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let connector_surcharge_id = value
            .content
            .and_then(|c| match c.content {
                Some(grpc_api_types::payments::notify_connector_content::Content::SurchargeContent(details)) => {
                    Some(details.connector_surcharge_id)
                }
                _ => None,
            })
            .ok_or_else(|| error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "connector_surcharge_id",
                context: IntegrationErrorContext {
                    additional_context: Some("connector_surcharge_id is required for surcharge payment succeeded notification".to_owned()),
                    ..Default::default()
                },
            }))?;

        Ok(Self {
            connector_surcharge_id,
        })
    }
}

impl ForeignTryFrom<grpc_api_types::payments::NotifyConnectorRequest>
    for SurchargeRefundSucceededRequest
{
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::payments::NotifyConnectorRequest,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let connector_surcharge_id = value
            .content
            .and_then(|c| match c.content {
                Some(grpc_api_types::payments::notify_connector_content::Content::SurchargeContent(details)) => {
                    Some(details.connector_surcharge_id)
                }
                _ => None,
            })
            .ok_or_else(|| error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "connector_surcharge_id",
                context: IntegrationErrorContext {
                    additional_context: Some("connector_surcharge_id is required for surcharge refund succeeded notification".to_owned()),
                    ..Default::default()
                },
            }))?;

        Ok(Self {
            connector_surcharge_id,
        })
    }
}

impl
    ForeignTryFrom<(
        grpc_api_types::payments::NotifyConnectorRequest,
        Connectors,
        &MaskedMetadata,
    )> for SurchargeFlowData
{
    type Error = IntegrationError;

    fn foreign_try_from(
        (value, connectors, metadata): (
            grpc_api_types::payments::NotifyConnectorRequest,
            Connectors,
            &MaskedMetadata,
        ),
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let merchant_id = extract_merchant_id_from_metadata(metadata)?;

        Ok(Self {
            merchant_id,
            connector_request_reference_id: extract_connector_request_reference_id(&Some(
                value.event_id,
            )),
            connectors: connectors.into(),
            raw_connector_response: None,
            raw_connector_request: None,
            typed_connector_request: None,
            typed_connector_response: None,
            connector_response_headers: None,
        })
    }
}

pub fn generate_surcharge_calculate_response(
    router_data_v2: RouterDataV2<
        SurchargeCalculate,
        SurchargeFlowData,
        SurchargeCalculateRequest,
        SurchargeCalculateResponse,
    >,
) -> Result<
    grpc_api_types::surcharge::SurchargeServiceCalculateResponse,
    error_stack::Report<ConnectorError>,
> {
    let surcharge_response = router_data_v2.response;
    match surcharge_response {
        Ok(response) => {
            let surcharge_amount = grpc_api_types::surcharge::Money {
                minor_amount: response.surcharge_amount.get_amount_as_i64(),
                currency: grpc_api_types::payments::Currency::foreign_try_from(response.currency)?
                    .into(),
            };

            Ok(
                grpc_api_types::surcharge::SurchargeServiceCalculateResponse {
                    merchant_surcharge_id: Some(
                        router_data_v2
                            .resource_common_data
                            .connector_request_reference_id
                            .clone(),
                    ),
                    surcharge_amount: Some(surcharge_amount),
                    surcharge_percentage: Some(response.surcharge_rate_percent),
                    connector_surcharge_id: Some(response.connector_surcharge_id),
                    status_code: 200,
                    error: None,
                },
            )
        }
        Err(e) => Ok(
            grpc_api_types::surcharge::SurchargeServiceCalculateResponse {
                merchant_surcharge_id: None,
                surcharge_amount: None,
                surcharge_percentage: None,
                connector_surcharge_id: None,
                status_code: e.status_code.into(),
                error: Some(grpc_api_types::surcharge::ErrorInfo {
                    unified_details: None,
                    connector_details: Some(grpc_api_types::surcharge::ConnectorErrorDetails {
                        code: Some(e.code),
                        message: Some(e.message.clone()),
                        reason: e.reason.clone(),
                        connector_transaction_id: e.connector_transaction_id.clone(),
                        status: None,
                    }),
                    issuer_details: None,
                }),
            },
        ),
    }
}

pub fn generate_surcharge_payment_succeeded_response(
    router_data_v2: RouterDataV2<
        SurchargePaymentSucceeded,
        SurchargeFlowData,
        SurchargePaymentSucceededRequest,
        SurchargePaymentSucceededResponse,
    >,
) -> Result<grpc_api_types::payments::NotifyConnectorResponse, error_stack::Report<ConnectorError>>
{
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

pub fn generate_surcharge_refund_succeeded_response(
    router_data_v2: RouterDataV2<
        SurchargeRefundSucceeded,
        SurchargeFlowData,
        SurchargeRefundSucceededRequest,
        SurchargeRefundSucceededResponse,
    >,
) -> Result<grpc_api_types::payments::NotifyConnectorResponse, error_stack::Report<ConnectorError>>
{
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
