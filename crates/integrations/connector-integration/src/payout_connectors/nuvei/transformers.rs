use common_enums::PayoutStatus;
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    types::{AmountConvertor, StringMajorUnit, StringMajorUnitForConnector},
};
use domain_types::{
    connector_flow::PayoutTransfer,
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    payment_method_data::PaymentMethodDataTypes,
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
use std::fmt::Debug;

use super::NuveiPayoutsRouterData;

use crate::{
    connectors::nuvei::transformers::{NuveiAuthType, NuveiPaymentStatus, NuveiTransactionStatus},
    types::ResponseRouterData,
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiPayoutRequest<T: PaymentMethodDataTypes> {
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
    payout_method: NuveiPayoutMethod<T>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct NuveiPayoutUrlDetails {
    notification_url: String,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum NuveiPayoutMethod<T: PaymentMethodDataTypes> {
    Card {
        #[serde(rename = "cardData")]
        card_data: NuveiPayoutCard<T>,
    },
    Passthrough {
        #[serde(rename = "userPaymentOption")]
        user_payment_option: NuveiPayoutPaymentOption,
    },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct NuveiPayoutCard<T: PaymentMethodDataTypes> {
    card_number: T::Inner,
    card_holder_name: Secret<String>,
    expiration_month: Secret<String>,
    expiration_year: Secret<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct NuveiPayoutPaymentOption {
    user_payment_option_id: Secret<String>,
}

type NuveiPayoutRouterData<T> =
    RouterDataV2<PayoutTransfer, PayoutFlowData, PayoutTransferRequest<T>, PayoutTransferResponse>;

fn missing(field_name: &'static str) -> Report<IntegrationError> {
    IntegrationError::MissingRequiredField {
        field_name,
        context: nuvei_context(
            format!("Nuvei payout request is missing `{field_name}`"),
            format!("Provide `{field_name}` in the payout request"),
        ),
    }
    .into()
}

fn nuvei_context(
    additional_context: impl Into<String>,
    suggested_action: impl Into<String>,
) -> IntegrationErrorContext {
    IntegrationErrorContext {
        additional_context: Some(additional_context.into()),
        suggested_action: Some(suggested_action.into()),
        doc_url: None,
    }
}

fn validate_payout_currency(
    source_currency: common_enums::Currency,
    destination_currency: common_enums::Currency,
) -> Result<common_enums::Currency, Report<IntegrationError>> {
    match source_currency == destination_currency {
        true => Ok(source_currency),
        false => Err(IntegrationError::NotSupported {
            message: "Cross-currency payouts".to_owned(),
            connector: "nuvei",
            context: nuvei_context(
                format!(
                    "Nuvei payout source currency {source_currency} differs from destination currency {destination_currency}"
                ),
                "Use the same source and destination currency for Nuvei payouts",
            ),
        }
        .into()),
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<NuveiPayoutsRouterData<NuveiPayoutRouterData<T>, T>> for NuveiPayoutRequest<T>
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: NuveiPayoutsRouterData<NuveiPayoutRouterData<T>, T>,
    ) -> Result<Self, Self::Error> {
        let data = &item.router_data;
        let auth = NuveiAuthType::try_from(&data.connector_config)?;
        let (merchant_id, merchant_site_id) = match &data.connector_config {
            ConnectorSpecificConfig::Nuvei {
                merchant_id,
                merchant_site_id,
                ..
            } => Ok((merchant_id, merchant_site_id)),
            _ => Err(Report::new(IntegrationError::FailedToObtainAuthType {
                context: nuvei_context(
                    "Nuvei payout received connector credentials for a different connector",
                    "Configure Nuvei merchant credentials for this payout connector",
                ),
            })),
        }?;
        let request = &data.request;
        let payout_currency =
            validate_payout_currency(request.source_currency, request.destination_currency)?;
        let amount = StringMajorUnitForConnector
            .convert(request.amount, payout_currency)
            .change_context(IntegrationError::InvalidDataFormat {
                field_name: "amount",
                context: nuvei_context(
                    "Nuvei payout amount could not be converted to major currency units",
                    "Provide an amount valid for the source currency",
                ),
            })?;
        let currency = payout_currency.to_string();
        let time_stamp = NuveiAuthType::get_timestamp();
        let reference = match &data.resource_common_data.connector_request_reference_id {
            reference if !reference.trim().is_empty() => Ok(reference),
            _ => Err(missing("merchant_payout_id")),
        }?;
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
            PayoutMethodData::Card(card) => Ok(NuveiPayoutMethod::Card {
                card_data: NuveiPayoutCard {
                    card_number: card.card_number.clone(),
                    card_holder_name: card
                        .card_holder_name
                        .clone()
                        .filter(|name| !name.peek().trim().is_empty())
                        .ok_or_else(|| missing("payout_method_data.card.card_holder_name"))?,
                    expiration_month: card.expiry_month.clone(),
                    expiration_year: card.expiry_year.clone(),
                },
            }),
            PayoutMethodData::Passthrough(token) => Ok(NuveiPayoutMethod::Passthrough {
                user_payment_option: NuveiPayoutPaymentOption {
                    user_payment_option_id: token.psp_token.clone(),
                },
            }),
            _ => Err(Report::new(IntegrationError::NotSupported {
                message: "Payout method".to_owned(),
                connector: "nuvei",
                context: nuvei_context(
                    "Nuvei payout transfer supports card and passthrough payout methods",
                    "Use card or passthrough payout method data",
                ),
            })),
        }?;
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
            attempt_status: (status_code < 500)
                .then_some(FlowStatus::Payout(PayoutStatus::Failure)),
            connector_transaction_id: self.transaction_id.clone(),
            typed_connector_response: crate::connectors::macros::serialize_typed_connector_payload(
                self,
                "typed_connector_response",
            ),
            ..Default::default()
        }
    }

    fn payout_status(&self, status_code: u16) -> Result<PayoutStatus, Report<ConnectorError>> {
        match self.status {
            NuveiPaymentStatus::Failed | NuveiPaymentStatus::Error => Ok(PayoutStatus::Failure),
            NuveiPaymentStatus::Processing => Ok(PayoutStatus::Pending),
            NuveiPaymentStatus::Success => match self.transaction_status {
                Some(NuveiTransactionStatus::Approved) => Ok(PayoutStatus::Success),
                Some(
                    NuveiTransactionStatus::Declined
                    | NuveiTransactionStatus::Error
                    | NuveiTransactionStatus::Redirect,
                ) => Ok(PayoutStatus::Failure),
                Some(NuveiTransactionStatus::Pending | NuveiTransactionStatus::Processing) => {
                    Ok(PayoutStatus::Pending)
                }
                None => Err(Report::new(
                    crate::utils::response_handling_fail_for_connector(status_code, "nuvei"),
                )),
            },
        }
    }
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<NuveiPayoutResponse, Self>>
    for NuveiPayoutRouterData<T>
{
    type Error = Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<NuveiPayoutResponse, Self>) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            mut router_data,
            http_code,
        } = item;
        let status = response.payout_status(http_code)?;
        router_data.response = match status {
            PayoutStatus::Failure => Err(response.error_response(http_code)),
            status => {
                let transaction_id = response
                    .transaction_id
                    .as_ref()
                    .filter(|id| !id.is_empty())
                    .cloned();
                let connector_payout_id = match (status, transaction_id) {
                    (PayoutStatus::Success, None) => Err(Report::new(
                        crate::utils::response_handling_fail_for_connector(http_code, "nuvei"),
                    )),
                    (_, transaction_id) => Ok(transaction_id),
                }?;
                Ok(PayoutTransferResponse {
                    merchant_payout_id: router_data.request.merchant_payout_id.clone(),
                    payout_status: status,
                    connector_payout_id,
                    status_code: http_code,
                })
            }
        };
        Ok(router_data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(
        status: NuveiPaymentStatus,
        transaction_status: Option<NuveiTransactionStatus>,
    ) -> NuveiPayoutResponse {
        NuveiPayoutResponse {
            status,
            transaction_status,
            transaction_id: Some("txn_123".to_owned()),
            err_code: None,
            reason: None,
            gw_error_code: None,
            gw_error_reason: None,
        }
    }

    #[test]
    fn payout_currency_rejects_cross_currency_requests() {
        assert_eq!(
            validate_payout_currency(common_enums::Currency::USD, common_enums::Currency::USD)
                .expect("matching currency should be accepted"),
            common_enums::Currency::USD
        );
        assert!(
            validate_payout_currency(common_enums::Currency::USD, common_enums::Currency::EUR)
                .is_err()
        );
    }

    #[test]
    fn payout_checksum_uses_the_signed_nuvei_fields_in_order() {
        let config = ConnectorSpecificConfig::Nuvei {
            merchant_id: Secret::new("merchant".to_owned()),
            merchant_site_id: Secret::new("site".to_owned()),
            merchant_secret: Secret::new("secret".to_owned()),
            base_url: None,
        };
        let auth = NuveiAuthType::try_from(&config).expect("valid Nuvei credentials");

        assert_eq!(
            auth.generate_checksum(&[
                "merchant",
                "site",
                "payout_123",
                "10.00",
                "USD",
                "20261007123000"
            ]),
            "8501f7dd724473e8a4149c9e227684e221b82b6fa40816add3e1064b36c21eee"
        );
    }

    #[test]
    fn redirect_is_a_terminal_failure_for_server_to_server_payouts() {
        let response = response(
            NuveiPaymentStatus::Success,
            Some(NuveiTransactionStatus::Redirect),
        );
        assert_eq!(
            response.payout_status(200).expect("status should map"),
            PayoutStatus::Failure
        );
    }

    #[test]
    fn server_errors_do_not_mark_the_payout_failed() {
        let response = response(NuveiPaymentStatus::Error, None);
        assert_eq!(response.error_response(500).attempt_status, None);
        assert_eq!(
            response.error_response(400).attempt_status,
            Some(FlowStatus::Payout(PayoutStatus::Failure))
        );
    }
}
