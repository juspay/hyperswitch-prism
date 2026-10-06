use common_enums::PayoutStatus;
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE, X_EXTERNAL_VAULT_METADATA},
    types::{AmountConvertor, StringMajorUnit, StringMajorUnitForConnector},
};
use domain_types::{
    connector_flow::PayoutTransfer,
    errors::{ConnectorError, IntegrationError},
    payouts::{
        payout_method_data::PayoutMethodData,
        payouts_types::{PayoutFlowData, PayoutTransferRequest, PayoutTransferResponse},
    },
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
};
use error_stack::{Report, ResultExt};
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use crate::{
    connectors::nuvei::transformers::{NuveiAuthType, NuveiPaymentStatus, NuveiTransactionStatus},
    types::ResponseRouterData,
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiPayoutRequest {
    merchant_id: Secret<String>,
    merchant_site_id: Secret<String>,
    client_request_id: String,
    client_unique_id: String,
    amount: StringMajorUnit,
    currency: String,
    user_token_id: common_utils::id_type::CustomerId,
    time_stamp: common_utils::date_time::DateTime<common_utils::date_time::YYYYMMDDHHmmss>,
    checksum: Secret<String>,
    url_details: NuveiPayoutUrlDetails,
    #[serde(flatten)]
    payout_method: NuveiPayoutMethod,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct NuveiPayoutUrlDetails {
    notification_url: String,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum NuveiPayoutMethod {
    Card {
        #[serde(rename = "cardData")]
        card_data: NuveiPayoutCard,
    },
    Passthrough {
        #[serde(rename = "userPaymentOption")]
        user_payment_option: NuveiPayoutPaymentOption,
    },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct NuveiPayoutCard {
    card_number: NuveiPayoutCardNumber,
    card_holder_name: Secret<String>,
    expiration_month: Secret<String>,
    expiration_year: Secret<String>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum NuveiPayoutCardNumber {
    Card(cards::CardNumber),
    Proxy(Secret<String>),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct NuveiPayoutPaymentOption {
    user_payment_option_id: Secret<String>,
}

type NuveiPayoutRouterData =
    RouterDataV2<PayoutTransfer, PayoutFlowData, PayoutTransferRequest, PayoutTransferResponse>;

fn missing(field_name: &'static str) -> Report<IntegrationError> {
    IntegrationError::MissingRequiredField {
        field_name,
        context: Default::default(),
    }
    .into()
}

impl TryFrom<&NuveiPayoutRouterData> for NuveiPayoutRequest {
    type Error = Report<IntegrationError>;

    fn try_from(data: &NuveiPayoutRouterData) -> Result<Self, Self::Error> {
        let auth = NuveiAuthType::try_from(&data.connector_config)?;
        let ConnectorSpecificConfig::Nuvei {
            merchant_id,
            merchant_site_id,
            ..
        } = &data.connector_config
        else {
            return Err(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            }
            .into());
        };
        let request = &data.request;
        let amount = StringMajorUnitForConnector
            .convert(request.amount, request.destination_currency)
            .change_context(IntegrationError::InvalidDataFormat {
                field_name: "amount",
                context: Default::default(),
            })?;
        let currency = request.destination_currency.to_string();
        let time_stamp = NuveiAuthType::get_timestamp();
        let reference = &data.resource_common_data.connector_request_reference_id;
        if reference.trim().is_empty() {
            return Err(missing("merchant_payout_id"));
        }
        let user_token_id = request
            .customer
            .as_ref()
            .ok_or_else(|| missing("customer"))?
            .get_merchant_customer_id()?;
        let notification_url = request
            .webhook_url
            .clone()
            .filter(|url| !url.trim().is_empty())
            .ok_or_else(|| missing("webhook_url"))?;
        let payout_method = match request
            .payout_method_data
            .as_ref()
            .ok_or_else(|| missing("payout_method_data"))?
        {
            PayoutMethodData::Card(card) => NuveiPayoutMethod::Card {
                card_data: NuveiPayoutCard {
                    card_number: NuveiPayoutCardNumber::Card(card.card_number.clone()),
                    card_holder_name: card
                        .card_holder_name
                        .clone()
                        .filter(|name| !name.peek().trim().is_empty())
                        .ok_or_else(|| missing("payout_method_data.card.card_holder_name"))?,
                    expiration_month: card.expiry_month.clone(),
                    expiration_year: card.expiry_year.clone(),
                },
            },
            PayoutMethodData::CardProxy(card) => {
                if !data
                    .resource_common_data
                    .vault_headers
                    .as_ref()
                    .is_some_and(|headers| headers.contains_key(X_EXTERNAL_VAULT_METADATA))
                {
                    return Err(missing(X_EXTERNAL_VAULT_METADATA));
                }
                NuveiPayoutMethod::Card {
                    card_data: NuveiPayoutCard {
                        card_number: NuveiPayoutCardNumber::Proxy(Secret::new(
                            "{{$card_number}}".to_owned(),
                        )),
                        card_holder_name: card
                            .card_holder_name
                            .clone()
                            .filter(|name| !name.peek().trim().is_empty())
                            .ok_or_else(|| {
                                missing("payout_method_data.card_proxy.card_holder_name")
                            })?,
                        expiration_month: card.expiry_month.clone(),
                        expiration_year: card.expiry_year.clone(),
                    },
                }
            }
            PayoutMethodData::Passthrough(token) => NuveiPayoutMethod::Passthrough {
                user_payment_option: NuveiPayoutPaymentOption {
                    user_payment_option_id: token.psp_token.clone(),
                },
            },
            _ => {
                return Err(IntegrationError::NotSupported {
                    message: "Payout method".to_owned(),
                    connector: "nuvei",
                    context: Default::default(),
                }
                .into())
            }
        };
        // Nuvei signs only these non-card fields, so vault substitution does not invalidate the checksum.
        let checksum = auth.generate_checksum(&[
            merchant_id.peek(),
            merchant_site_id.peek(),
            reference,
            &amount.get_amount_as_string(),
            &currency,
            &time_stamp.to_string(),
        ]);
        Ok(Self {
            merchant_id: merchant_id.clone(),
            merchant_site_id: merchant_site_id.clone(),
            client_request_id: reference.clone(),
            client_unique_id: reference.clone(),
            amount,
            currency,
            user_token_id,
            time_stamp,
            checksum: Secret::new(checksum),
            url_details: NuveiPayoutUrlDetails { notification_url },
            payout_method,
        })
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiPayoutResponse {
    status: NuveiPaymentStatus,
    transaction_status: Option<NuveiTransactionStatus>,
    transaction_id: Option<String>,
    err_code: Option<i64>,
    reason: Option<String>,
    gw_error_code: Option<i64>,
    gw_error_reason: Option<String>,
}

impl NuveiPayoutResponse {
    pub(super) fn error_response(&self, status_code: u16) -> ErrorResponse {
        let reason = self
            .reason
            .as_ref()
            .filter(|reason| !reason.is_empty())
            .or_else(|| {
                self.gw_error_reason
                    .as_ref()
                    .filter(|reason| !reason.is_empty())
            })
            .cloned();
        ErrorResponse {
            status_code,
            code: self
                .err_code
                .filter(|code| *code != 0)
                .or_else(|| self.gw_error_code.filter(|code| *code != 0))
                .map(|code| code.to_string())
                .unwrap_or_else(|| NO_ERROR_CODE.to_owned()),
            message: reason
                .clone()
                .unwrap_or_else(|| NO_ERROR_MESSAGE.to_owned()),
            reason,
            attempt_status: Some(FlowStatus::Payout(PayoutStatus::Failure)),
            connector_transaction_id: self.transaction_id.clone(),
            typed_connector_response: crate::connectors::macros::serialize_typed_connector_payload(
                self,
                "typed_connector_response",
            ),
            ..Default::default()
        }
    }
}

impl TryFrom<ResponseRouterData<NuveiPayoutResponse, Self>> for NuveiPayoutRouterData {
    type Error = Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<NuveiPayoutResponse, Self>) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            mut router_data,
            http_code,
        } = item;
        let status = match response.status {
            NuveiPaymentStatus::Failed | NuveiPaymentStatus::Error => PayoutStatus::Failure,
            NuveiPaymentStatus::Processing => PayoutStatus::Pending,
            NuveiPaymentStatus::Success => match response.transaction_status {
                Some(NuveiTransactionStatus::Approved) => PayoutStatus::Success,
                Some(NuveiTransactionStatus::Declined | NuveiTransactionStatus::Error) => {
                    PayoutStatus::Failure
                }
                Some(NuveiTransactionStatus::Pending | NuveiTransactionStatus::Processing) => {
                    PayoutStatus::Pending
                }
                Some(NuveiTransactionStatus::Redirect) => PayoutStatus::Ineligible,
                None => {
                    return Err(crate::utils::response_handling_fail_for_connector(
                        http_code, "nuvei",
                    )
                    .into())
                }
            },
        };
        router_data.response = if status == PayoutStatus::Failure {
            Err(response.error_response(http_code))
        } else {
            let transaction_id = response
                .transaction_id
                .as_ref()
                .filter(|id| !id.is_empty())
                .cloned();
            if status == PayoutStatus::Success && transaction_id.is_none() {
                return Err(
                    crate::utils::response_handling_fail_for_connector(http_code, "nuvei").into(),
                );
            }
            Ok(PayoutTransferResponse {
                merchant_payout_id: router_data.request.merchant_payout_id.clone(),
                payout_status: status,
                connector_payout_id: transaction_id,
                status_code: http_code,
            })
        };
        Ok(router_data)
    }
}
