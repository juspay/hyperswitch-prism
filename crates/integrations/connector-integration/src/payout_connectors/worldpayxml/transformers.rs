use domain_types::{
    connector_flow::{PayoutGet, PayoutTransfer, PayoutVoid},
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    payouts::payouts_types::{
        PayoutFlowData, PayoutGetRequest, PayoutGetResponse, PayoutTransferRequest,
        PayoutTransferResponse, PayoutVoidRequest, PayoutVoidResponse,
    },
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
};
use error_stack::Report;
use hyperswitch_masking::{PeekInterface, Secret};

use crate::{
    connectors::worldpayxml::{requests, responses, WorldpayxmlAmountConvertor},
    types::ResponseRouterData,
};

pub(super) const API_VERSION: &str = "1.4";
pub(super) const DEFAULT_PAYMENT_DESCRIPTION: &str = "Payment";

#[derive(Debug, Clone)]
pub struct WorldpayxmlAuthType {
    pub api_username: Secret<String>,
    pub api_password: Secret<String>,
    pub merchant_code: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for WorldpayxmlAuthType {
    type Error = Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Worldpayxml {
                api_username,
                api_password,
                merchant_code,
                ..
            } => Ok(Self {
                api_username: api_username.to_owned(),
                api_password: api_password.to_owned(),
                merchant_code: merchant_code.to_owned(),
            }),
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            }
            .into()),
        }
    }
}

fn map_worldpayxml_payout_status(
    last_event: &responses::WorldpayxmlLastEvent,
) -> common_enums::PayoutStatus {
    use responses::WorldpayxmlLastEvent;
    match last_event {
        WorldpayxmlLastEvent::Authorised
        | WorldpayxmlLastEvent::Captured
        | WorldpayxmlLastEvent::PushApproved
        | WorldpayxmlLastEvent::SettledByMerchant => common_enums::PayoutStatus::Success,
        WorldpayxmlLastEvent::PushRequested | WorldpayxmlLastEvent::PushPending => {
            common_enums::PayoutStatus::Pending
        }
        WorldpayxmlLastEvent::Cancelled => common_enums::PayoutStatus::Cancelled,
        WorldpayxmlLastEvent::SentForRefund | WorldpayxmlLastEvent::Refunded => {
            common_enums::PayoutStatus::Reversed
        }
        WorldpayxmlLastEvent::Refused
        | WorldpayxmlLastEvent::RefundFailed
        | WorldpayxmlLastEvent::PushRefused
        | WorldpayxmlLastEvent::Expired
        | WorldpayxmlLastEvent::Error => common_enums::PayoutStatus::Failure,
        // Exhaustiveness only: the shared lastEvent enum gained variants for the payment flows.
        // Every mapping above is unchanged from before those variants existed.
        _ => common_enums::PayoutStatus::Pending,
    }
}

fn worldpayxml_amount_exponent(
    currency: common_enums::Currency,
) -> Result<String, Report<IntegrationError>> {
    currency
        .number_of_digits_after_decimal_point()
        .map(|digits| digits.to_string())
        .map_err(|_| {
            IntegrationError::InvalidDataFormat {
                field_name: "currency",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Use an ISO 4217 currency Worldpay accepts (e.g. GBP, USD, EUR)."
                            .to_string(),
                    ),
                    doc_url: None,
                    additional_context: Some(format!(
                        "Currency {currency:?} has no known minor-unit exponent"
                    )),
                },
            }
            .into()
        })
}

// ----- PayoutTransfer (PoFulfill) request -----
impl
    TryFrom<
        &RouterDataV2<
            PayoutTransfer,
            PayoutFlowData,
            PayoutTransferRequest,
            PayoutTransferResponse,
        >,
    > for requests::WorldpayxmlPayoutTransferRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        router_data: &RouterDataV2<
            PayoutTransfer,
            PayoutFlowData,
            PayoutTransferRequest,
            PayoutTransferResponse,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = WorldpayxmlAuthType::try_from(&router_data.connector_config)?;
        let request = &router_data.request;

        let card = request
            .payout_method_data
            .as_ref()
            .ok_or_else(|| IntegrationError::MissingRequiredField {
                field_name: "payout_method_data",
                context: Default::default(),
            })?
            .get_card()?;

        let formatted_year = crate::utils::pad_expiry_year_to_four_digits(&card.expiry_year);

        let card_holder_name = crate::utils::build_card_holder_name(
            &card.card_holder_name,
            router_data.request.get_billing_first_name().ok(),
            router_data.request.get_billing_last_name().ok(),
        );

        let billing_address = match (
            router_data.request.get_optional_billing_line1(),
            router_data.request.get_optional_billing_city(),
            router_data.request.get_optional_billing_zip(),
            router_data.request.get_optional_billing_country(),
        ) {
            (Some(line1), Some(city), Some(zip), Some(country)) => {
                Some(requests::WorldpayxmlAddress {
                    first_name: router_data.request.get_billing_first_name().ok(),
                    last_name: router_data.request.get_billing_last_name().ok(),
                    address1: Some(line1),
                    address2: None,
                    address3: None,
                    postal_code: Some(zip),
                    city: Some(city),
                    state: router_data.request.get_optional_billing_state(),
                    country_code: Some(country),
                    telephone_number: None,
                })
            }
            _ => None,
        };

        let payment_method = requests::WorldpayxmlPayoutPaymentMethod::FastAccessSsl(
            requests::WorldpayxmlFastAccess {
                recipient: requests::WorldpayxmlPayoutRecipient {
                    payment_instrument: requests::WorldpayxmlPayoutPaymentInstrument {
                        card_details: requests::WorldpayxmlPayoutCardDetails {
                            card_number: Secret::new(card.card_number.peek().to_string()),
                            expiry_date: requests::WorldpayxmlExpiryDate {
                                date: requests::WorldpayxmlDate {
                                    month: card.expiry_month.clone(),
                                    year: formatted_year,
                                },
                            },
                            card_holder_name,
                        },
                    },
                    address: billing_address,
                },
                purpose_of_payment: None,
            },
        );

        let converted_amount =
            WorldpayxmlAmountConvertor::convert(request.amount, request.destination_currency)?;

        Ok(Self {
            version: API_VERSION.to_string(),
            merchant_code: auth.merchant_code,
            submit: requests::WorldpayxmlPayoutSubmit {
                order: requests::WorldpayxmlPayoutOrder {
                    order_code: router_data
                        .resource_common_data
                        .connector_request_reference_id
                        .clone(),
                    description: router_data
                        .resource_common_data
                        .description
                        .clone()
                        .unwrap_or_else(|| DEFAULT_PAYMENT_DESCRIPTION.to_string()),
                    amount: requests::WorldpayxmlAmount {
                        value: converted_amount,
                        currency_code: request.destination_currency,
                        exponent: worldpayxml_amount_exponent(request.destination_currency)?,
                    },
                    payment_details: requests::WorldpayxmlPayoutPaymentDetails { payment_method },
                },
            },
        })
    }
}

// ----- PayoutTransfer response -----
impl TryFrom<ResponseRouterData<responses::WorldpayxmlPayoutTransferResponse, Self>>
    for RouterDataV2<PayoutTransfer, PayoutFlowData, PayoutTransferRequest, PayoutTransferResponse>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<responses::WorldpayxmlPayoutTransferResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        if let Some(error) = &response.reply.error {
            return Ok(Self {
                response: Err(crate::utils::build_error_response(
                    error.code.clone(),
                    error.message.clone(),
                    item.http_code,
                    None,
                )),
                ..router_data.clone()
            });
        }

        let order_status = response.reply.order_status.as_ref().ok_or(
            crate::utils::response_deserialization_fail(
                item.http_code,
                "worldpayxml: payout response missing orderStatus.",
            ),
        )?;

        if let Some(error) = &order_status.error {
            return Ok(Self {
                response: Err(crate::utils::build_error_response(
                    error.code.clone(),
                    error.message.clone(),
                    item.http_code,
                    Some(order_status.order_code.clone()),
                )),
                ..router_data.clone()
            });
        }

        let payment =
            order_status
                .payment
                .as_ref()
                .ok_or(crate::utils::response_deserialization_fail(
                    item.http_code,
                    "worldpayxml: payout response missing payment.",
                ))?;

        Ok(Self {
            response: Ok(PayoutTransferResponse {
                merchant_payout_id: router_data.request.merchant_payout_id.clone(),
                payout_status: map_worldpayxml_payout_status(&payment.last_event),
                connector_payout_id: Some(order_status.order_code.clone()),
                status_code: item.http_code,
            }),
            ..router_data.clone()
        })
    }
}

// ----- PayoutGet (PoSync) request -----
impl TryFrom<&RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>>
    for requests::WorldpayxmlPayoutGetRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        router_data: &RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
    ) -> Result<Self, Self::Error> {
        let auth = WorldpayxmlAuthType::try_from(&router_data.connector_config)?;

        let order_code = router_data.request.connector_payout_id.clone().ok_or(
            IntegrationError::MissingRequiredField {
                field_name: "connector_payout_id",
                context: Default::default(),
            },
        )?;

        Ok(Self {
            version: API_VERSION.to_string(),
            merchant_code: auth.merchant_code,
            inquiry: requests::WorldpayxmlInquiry {
                order_inquiry: requests::WorldpayxmlOrderInquiry { order_code },
            },
        })
    }
}

// ----- PayoutGet response -----
impl TryFrom<ResponseRouterData<responses::WorldpayxmlPayoutGetResponse, Self>>
    for RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<responses::WorldpayxmlPayoutGetResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        if let Some(error) = &response.reply.error {
            return Ok(Self {
                response: Err(crate::utils::build_error_response(
                    error.code.clone(),
                    error.message.clone(),
                    item.http_code,
                    None,
                )),
                ..router_data.clone()
            });
        }

        let order_status = response.reply.order_status.as_ref().ok_or(
            crate::utils::response_deserialization_fail(
                item.http_code,
                "worldpayxml: payout sync response missing orderStatus.",
            ),
        )?;

        if let Some(error) = &order_status.error {
            return Ok(Self {
                response: Err(crate::utils::build_error_response(
                    error.code.clone(),
                    error.message.clone(),
                    item.http_code,
                    Some(order_status.order_code.clone()),
                )),
                ..router_data.clone()
            });
        }

        let payment =
            order_status
                .payment
                .as_ref()
                .ok_or(crate::utils::response_deserialization_fail(
                    item.http_code,
                    "worldpayxml: payout sync response missing payment.",
                ))?;

        Ok(Self {
            response: Ok(PayoutGetResponse {
                merchant_payout_id: router_data.request.merchant_payout_id.clone(),
                payout_status: map_worldpayxml_payout_status(&payment.last_event),
                connector_payout_id: Some(order_status.order_code.clone()),
                status_code: item.http_code,
            }),
            ..router_data.clone()
        })
    }
}

// ----- PayoutVoid (PoCancel) request -----
impl TryFrom<&RouterDataV2<PayoutVoid, PayoutFlowData, PayoutVoidRequest, PayoutVoidResponse>>
    for requests::WorldpayxmlPayoutVoidRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        router_data: &RouterDataV2<
            PayoutVoid,
            PayoutFlowData,
            PayoutVoidRequest,
            PayoutVoidResponse,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = WorldpayxmlAuthType::try_from(&router_data.connector_config)?;

        let order_code = router_data.request.connector_payout_id.clone().ok_or(
            IntegrationError::MissingRequiredField {
                field_name: "connector_payout_id",
                context: Default::default(),
            },
        )?;

        Ok(Self {
            version: API_VERSION.to_string(),
            merchant_code: auth.merchant_code,
            modify: requests::WorldpayxmlPayoutVoidModify {
                order_modification: requests::WorldpayxmlPayoutCancelOrderModification {
                    order_code,
                    cancel_refund: requests::WorldpayxmlCancelRefund {},
                },
            },
        })
    }
}

// ----- PayoutVoid response -----
impl TryFrom<ResponseRouterData<responses::WorldpayxmlPayoutVoidResponse, Self>>
    for RouterDataV2<PayoutVoid, PayoutFlowData, PayoutVoidRequest, PayoutVoidResponse>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<responses::WorldpayxmlPayoutVoidResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        if let Some(error) = &response.reply.error {
            return Ok(Self {
                response: Err(crate::utils::build_error_response(
                    error.code.clone(),
                    error.message.clone(),
                    item.http_code,
                    None,
                )),
                ..router_data.clone()
            });
        }

        let ok = response
            .reply
            .ok
            .as_ref()
            .ok_or(crate::utils::response_deserialization_fail(
                item.http_code,
                "worldpayxml: payout cancel response missing ok element.",
            ))?;

        Ok(Self {
            response: Ok(PayoutVoidResponse {
                merchant_payout_id: router_data.request.merchant_payout_id.clone(),
                payout_status: common_enums::PayoutStatus::Pending,
                connector_payout_id: Some(ok.cancel_received.order_code.clone()),
                status_code: item.http_code,
            }),
            ..router_data.clone()
        })
    }
}
