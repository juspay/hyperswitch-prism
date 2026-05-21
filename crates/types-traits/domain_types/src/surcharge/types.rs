use crate::{
    connector_flow::SurchargeCalculate,
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    router_data_v2::RouterDataV2,
    surcharge::surcharge_types::{
        SurchargeCalculateRequest, SurchargeCalculateResponse, SurchargeFlowData, SurchargeStrategy,
    },
    types::Connectors,
    utils::{
        extract_connector_request_reference_id, extract_merchant_id_from_metadata, ForeignTryFrom,
    },
};

use common_utils::{metadata::MaskedMetadata, types::MinorUnit};
use error_stack::ResultExt;
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
            connectors,
            raw_connector_response: None,
            raw_connector_request: None,
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

            match grpc_strategy {
                grpc_api_types::surcharge::SurchargeStrategy::Apply => SurchargeStrategy::Apply,
                grpc_api_types::surcharge::SurchargeStrategy::Waive => SurchargeStrategy::Waive,
                grpc_api_types::surcharge::SurchargeStrategy::Unspecified => {
                    SurchargeStrategy::Unspecified
                }
            }
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
                    merchant_surcharge_id: response.connector_response_reference_id,
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
                    }),
                    issuer_details: None,
                }),
            },
        ),
    }
}
