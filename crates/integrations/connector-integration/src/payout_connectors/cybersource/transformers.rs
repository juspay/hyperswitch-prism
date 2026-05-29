use cards::CardNumber;
use domain_types::{
    connector_flow::PayoutTransfer,
    errors::{ConnectorError, IntegrationError},
    payouts::payouts_types::{
        PayoutFlowData, PayoutTransferRequest, PayoutTransferResponse,
    },
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
};
use error_stack::ResultExt;
use hyperswitch_masking::Secret;
use serde::{Deserialize, Serialize};

use crate::{
    connectors::cybersource::transformers::{
        Amount, ClientReferenceInformation, OrderInformation,
    },
    types::ResponseRouterData,
    utils::CybersourceCardTypeCode,
};

pub struct CybersourceAuthType {
    pub(super) api_key: Secret<String>,
    pub(super) merchant_account: Secret<String>,
    pub(super) api_secret: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for CybersourceAuthType {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        if let ConnectorSpecificConfig::Cybersource {
            api_key,
            merchant_account,
            api_secret,
            ..
        } = auth_type
        {
            Ok(Self {
                api_key: api_key.to_owned(),
                merchant_account: merchant_account.to_owned(),
                api_secret: api_secret.to_owned(),
            })
        } else {
            Err(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            })?
        }
    }
}

fn card_issuer_to_string(card_issuer: domain_types::utils::CardIssuer) -> String {
    use domain_types::utils::CardIssuer;
    let card_type = match card_issuer {
        CardIssuer::AmericanExpress => "003",
        CardIssuer::Master => "002",
        CardIssuer::Maestro => "042",
        CardIssuer::Visa => "001",
        CardIssuer::Discover => "004",
        CardIssuer::DinersClub => "005",
        CardIssuer::CarteBlanche => "006",
        CardIssuer::JCB => "007",
        CardIssuer::CartesBancaires => "036",
        CardIssuer::UnionPay => "062",
    };
    card_type.to_string()
}

// ===== PAYOUT TRANSFER (PoFulfill) =====

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CybersourcePayoutFulfillRequest {
    client_reference_information: ClientReferenceInformation,
    order_information: OrderInformation,
    recipient_information: CybersourceRecipientInfo,
    sender_information: CybersourceSenderInfo,
    processing_information: CybersourceProcessingInfo,
    payment_information: CybersourcePayoutPaymentInformation,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CybersourceRecipientInfo {
    first_name: Secret<String>,
    last_name: Secret<String>,
    address1: Secret<String>,
    locality: String,
    administrative_area: Secret<String>,
    postal_code: Secret<String>,
    country: common_enums::CountryAlpha2,
    phone_number: Option<Secret<String>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CybersourceSenderInfo {
    reference_number: String,
    account: CybersourceAccountInfo,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CybersourceAccountInfo {
    funds_source: CybersourcePayoutFundSourceType,
}

#[derive(Debug, Serialize)]
pub enum CybersourcePayoutFundSourceType {
    #[serde(rename = "05")]
    Disbursement,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CybersourceProcessingInfo {
    business_application_id: CybersourcePayoutBusinessType,
}

#[derive(Debug, Serialize)]
pub enum CybersourcePayoutBusinessType {
    #[serde(rename = "PP")]
    PersonToPerson,
    #[allow(dead_code)]
    #[serde(rename = "AA")]
    AccountToAccount,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum CybersourcePayoutPaymentInformation {
    Card(Box<CybersourcePayoutCardPaymentInformation>),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CybersourcePayoutCardPaymentInformation {
    card: CybersourcePayoutCard,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CybersourcePayoutCard {
    number: CardNumber,
    expiration_month: Secret<String>,
    expiration_year: Secret<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    card_type: Option<String>,
}

pub(super) struct CybersourcePayoutContext {
    pub total_amount: common_utils::types::StringMajorUnit,
}

impl
    TryFrom<(
        &RouterDataV2<PayoutTransfer, PayoutFlowData, PayoutTransferRequest, PayoutTransferResponse>,
        CybersourcePayoutContext,
    )> for CybersourcePayoutFulfillRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        (router_data, ctx): (
            &RouterDataV2<
                PayoutTransfer,
                PayoutFlowData,
                PayoutTransferRequest,
                PayoutTransferResponse,
            >,
            CybersourcePayoutContext,
        ),
    ) -> Result<Self, Self::Error> {
        let request = &router_data.request;

        let client_reference_information = ClientReferenceInformation {
            code: Some(
                router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
            ),
        };

        let order_information = OrderInformation {
            amount_details: Amount {
                total_amount: ctx.total_amount,
                currency: request.destination_currency,
            },
        };

        let recipient_information = build_recipient_info(request)?;

        let sender_information = CybersourceSenderInfo {
            reference_number: router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            account: CybersourceAccountInfo {
                funds_source: CybersourcePayoutFundSourceType::Disbursement,
            },
        };

        let processing_information = CybersourceProcessingInfo {
            business_application_id: CybersourcePayoutBusinessType::PersonToPerson,
        };

        let card = request
            .payout_method_data
            .as_ref()
            .ok_or_else(|| IntegrationError::MissingRequiredField {
                field_name: "payout_method_data",
                context: Default::default(),
            })?
            .get_card()?;
        let card_type = card
            .card_network
            .as_ref()
            .and_then(|network| network.cybersource_type_code().map(str::to_string))
            .or_else(|| {
                domain_types::utils::get_card_issuer(&card.card_number.get_card_no())
                    .ok()
                    .map(card_issuer_to_string)
            });
        let payment_information = CybersourcePayoutPaymentInformation::Card(Box::new(
            CybersourcePayoutCardPaymentInformation {
                card: CybersourcePayoutCard {
                    number: card.card_number.clone(),
                    expiration_month: card.expiry_month.clone(),
                    expiration_year: card.expiry_year.clone(),
                    card_type,
                },
            },
        ));

        Ok(Self {
            client_reference_information,
            order_information,
            recipient_information,
            sender_information,
            processing_information,
            payment_information,
        })
    }
}

fn build_recipient_info(
    request: &PayoutTransferRequest,
) -> Result<CybersourceRecipientInfo, error_stack::Report<IntegrationError>> {
    let first_name = request.get_billing_first_name().change_context(
        IntegrationError::MissingRequiredField {
            field_name: "address.billing_address.address.first_name",
            context: Default::default(),
        },
    )?;
    let last_name =
        request
            .get_billing_last_name()
            .change_context(IntegrationError::MissingRequiredField {
                field_name: "address.billing_address.address.last_name",
                context: Default::default(),
            })?;
    let address1 = request.get_optional_billing_line1().ok_or_else(|| {
        IntegrationError::MissingRequiredField {
            field_name: "address.billing_address.address.line1",
            context: Default::default(),
        }
    })?;
    let locality = request.get_optional_billing_city().ok_or_else(|| {
        IntegrationError::MissingRequiredField {
            field_name: "address.billing_address.address.city",
            context: Default::default(),
        }
    })?;
    let administrative_area_raw = request.get_optional_billing_state().ok_or_else(|| {
        IntegrationError::MissingRequiredField {
            field_name: "address.billing_address.address.state",
            context: Default::default(),
        }
    })?;
    // Cybersource rejects administrativeArea > 20 chars on the recipient side, same as billing.
    let administrative_area = crate::utils::truncate_secret_string(&administrative_area_raw, 20);
    let postal_code = request.get_optional_billing_zip().ok_or_else(|| {
        IntegrationError::MissingRequiredField {
            field_name: "address.billing_address.address.zip",
            context: Default::default(),
        }
    })?;
    let country = request.get_optional_billing_country().ok_or_else(|| {
        IntegrationError::MissingRequiredField {
            field_name: "address.billing_address.address.country",
            context: Default::default(),
        }
    })?;
    let phone_number = request.get_optional_billing_phone();

    Ok(CybersourceRecipientInfo {
        first_name,
        last_name,
        address1,
        locality,
        administrative_area,
        postal_code,
        country,
        phone_number,
    })
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CybersourceFulfillResponse {
    id: String,
    status: CybersourcePayoutStatus,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CybersourcePayoutStatus {
    Accepted,
    Declined,
    InvalidRequest,
}

fn map_payout_status(status: &CybersourcePayoutStatus) -> common_enums::PayoutStatus {
    match status {
        CybersourcePayoutStatus::Accepted => common_enums::PayoutStatus::Success,
        CybersourcePayoutStatus::Declined | CybersourcePayoutStatus::InvalidRequest => {
            common_enums::PayoutStatus::Failure
        }
    }
}

impl TryFrom<ResponseRouterData<CybersourceFulfillResponse, Self>>
    for RouterDataV2<PayoutTransfer, PayoutFlowData, PayoutTransferRequest, PayoutTransferResponse>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<CybersourceFulfillResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        Ok(Self {
            response: Ok(PayoutTransferResponse {
                merchant_payout_id: item.router_data.request.merchant_payout_id.clone(),
                payout_status: map_payout_status(&response.status),
                connector_payout_id: Some(response.id.clone()),
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}
