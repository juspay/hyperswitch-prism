use common_enums::AttemptStatus;
use common_utils::{
    crypto::{self, GenerateDigest},
    request::Method,
};
use domain_types::{
    connector_flow::Authorize,
    connector_types::{PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData, ResponseId},
    errors,
    payment_method_data::PaymentMethodDataTypes,
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use crate::{connectors::robokassa::RobokassaRouterData, types::ResponseRouterData};

/// Path that renders the Robokassa hosted payment page. The customer's browser is
/// redirected here (GET) with the signed parameters.
const ROBOKASSA_PAYMENT_PATH: &str = "/Merchant/Index.aspx";

// =============================================================================
// AUTH
// =============================================================================
// Robokassa authenticates each request by a `SignatureValue` (MD5 by default) over
// an ordered, colon-separated string that includes Password #1. Password #2 is used
// to verify the asynchronous ResultURL notification.
#[derive(Debug, Clone)]
pub struct RobokassaAuthType {
    /// MerchantLogin — the shop identifier sent in the request.
    pub merchant_login: Secret<String>,
    /// Password #1 — used to sign the outgoing payment request.
    pub password1: Secret<String>,
    /// Password #2 — used to verify the ResultURL (webhook) notification.
    pub password2: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for RobokassaAuthType {
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Robokassa {
                api_key,
                key1,
                api_secret,
                ..
            } => Ok(Self {
                merchant_login: api_key.to_owned(),
                password1: key1.to_owned(),
                password2: api_secret.to_owned(),
            }),
            _ => Err(error_stack::report!(
                errors::IntegrationError::FailedToObtainAuthType {
                    context: errors::IntegrationErrorContext::default()
                }
            )),
        }
    }
}

// =============================================================================
// ERROR RESPONSE
// =============================================================================
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RobokassaErrorResponse {
    #[serde(default)]
    pub code: String,
    #[serde(default)]
    pub message: String,
}

// =============================================================================
// SIGNATURE
// =============================================================================
/// Compute the Robokassa payment-request signature:
/// `MD5(MerchantLogin:OutSum:InvId:Password#1)` (hex, lowercase).
///
/// `OutSum` is the major-unit decimal string (e.g. `990.00`) and `InvId` the
/// numeric order reference. Robokassa supports stronger hashes (SHA-256/384/512)
/// per account; this integration uses the default MD5.
fn compute_signature(
    merchant_login: &str,
    out_sum: &str,
    inv_id: &str,
    password1: &str,
) -> Result<String, error_stack::Report<errors::IntegrationError>> {
    let signature_payload = format!("{merchant_login}:{out_sum}:{inv_id}:{password1}");
    let digest = crypto::Md5
        .generate_digest(signature_payload.as_bytes())
        .change_context(errors::IntegrationError::RequestEncodingFailed {
            context: errors::IntegrationErrorContext::default(),
        })?;
    Ok(hex::encode(digest))
}

/// Robokassa expects the amount as a major-unit decimal string. Build "n.dd" from
/// the minor-unit integer amount carried in the request.
fn format_out_sum(minor_amount: i64) -> String {
    format!("{}.{:02}", minor_amount / 100, (minor_amount % 100).abs())
}

// =============================================================================
// REQUEST
// =============================================================================
/// Parameters that form the Robokassa hosted-page redirect. Robokassa has no
/// server-to-server "create payment" API for the redirect flow — the merchant
/// builds this signed parameter set and redirects the customer to `Index.aspx`.
#[derive(Debug, Clone)]
pub struct RobokassaRedirectParams {
    pub merchant_login: String,
    pub out_sum: String,
    pub inv_id: String,
    pub description: Option<String>,
    pub culture: Option<String>,
    pub is_test: Option<String>,
    pub signature_value: String,
}

impl RobokassaRedirectParams {
    /// Build the signed redirect parameters from the authorize request.
    fn build<T: PaymentMethodDataTypes>(
        router_data: &RouterDataV2<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<T>,
            PaymentsResponseData,
        >,
    ) -> Result<Self, error_stack::Report<errors::IntegrationError>> {
        let auth = RobokassaAuthType::try_from(&router_data.connector_config)?;
        let merchant_login = auth.merchant_login.expose();
        let out_sum = format_out_sum(router_data.request.minor_amount.get_amount_as_i64());
        let inv_id = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();
        let signature_value =
            compute_signature(&merchant_login, &out_sum, &inv_id, auth.password1.peek())?;

        let is_test = if router_data.resource_common_data.test_mode == Some(true) {
            Some("1".to_string())
        } else {
            None
        };

        Ok(Self {
            merchant_login,
            out_sum,
            inv_id,
            description: router_data.resource_common_data.description.clone(),
            culture: Some("en".to_string()),
            is_test,
            signature_value,
        })
    }

    /// Build the form fields for the hosted-page redirect.
    fn into_form_fields(self) -> std::collections::HashMap<String, String> {
        let mut fields = std::collections::HashMap::new();
        fields.insert("MerchantLogin".to_string(), self.merchant_login);
        fields.insert("OutSum".to_string(), self.out_sum);
        fields.insert("InvId".to_string(), self.inv_id);
        if let Some(description) = self.description {
            fields.insert("Description".to_string(), description);
        }
        if let Some(culture) = self.culture {
            fields.insert("Culture".to_string(), culture);
        }
        if let Some(is_test) = self.is_test {
            fields.insert("IsTest".to_string(), is_test);
        }
        fields.insert("SignatureValue".to_string(), self.signature_value);
        fields
    }
}

/// Body type for the macro framework. Robokassa's redirect flow carries no body to
/// the API; this serialises to an empty form, and the redirect is built from the
/// request in the response transformer.
#[derive(Debug, Serialize)]
pub struct RobokassaPaymentsRequest {}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        RobokassaRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for RobokassaPaymentsRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: RobokassaRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // Validate that the signed redirect can be built (auth present, signature
        // computable) before the flow proceeds.
        RobokassaRedirectParams::build(&item.router_data)?;
        Ok(Self {})
    }
}

// =============================================================================
// RESPONSE
// =============================================================================
/// Robokassa's hosted page returns HTML, not a structured body. The connector's
/// `preprocess_response_bytes` neutralises the body to `{}`, which deserialises
/// into this empty struct. The meaningful result — the signed redirect — is
/// rebuilt from the original request in the `TryFrom` below.
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct RobokassaPaymentsResponse {}

impl<T: PaymentMethodDataTypes>
    TryFrom<
        ResponseRouterData<
            RobokassaPaymentsResponse,
            RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        >,
    >
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<
            RobokassaPaymentsResponse,
            RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        >,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response: _,
            router_data,
            http_code,
        } = item;

        // Rebuild the signed redirect parameters from the original request so the
        // SDK can open the hosted payment page.
        let params = RobokassaRedirectParams::build(&router_data).change_context(
            crate::utils::response_handling_fail_for_connector(http_code, "robokassa"),
        )?;
        let inv_id = params.inv_id.clone();

        let endpoint = format!(
            "{}{}",
            router_data
                .resource_common_data
                .connectors
                .robokassa
                .base_url
                .trim_end_matches('/'),
            ROBOKASSA_PAYMENT_PATH
        );

        let redirection_data = RedirectForm::Form {
            endpoint,
            method: Method::Get,
            form_fields: params.into_form_fields(),
        };

        // The hosted-page redirect has been handed off; the payment is awaiting the
        // customer to complete it on Robokassa's page (confirmed later via the
        // ResultURL notification / PSync).
        let status = AttemptStatus::AuthenticationPending;

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(inv_id),
                redirection_data: Some(Box::new(redirection_data)),
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: None,
                incremental_authorization_allowed: None,
                status_code: http_code,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}
