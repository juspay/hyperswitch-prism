//! DEPRECATED - delete this entire file on or after 2026-10-23.
//!
//! Pre-typed transport for the 3DS authenticate request: the acquirer / merchant
//! objects and the challenge preference arrived as JSON inside the request's
//! `connector_feature_data` blob (`NetceteraMeta`) instead of the typed fields.
//! Nothing here is reachable once callers send the typed 3DS request fields.
//!
//! Removal: delete this file, its `mod` line in `netcetera.rs`, and the
//! `legacy_merchant_side(..)` arm of the `match` in
//! `transformers::NetceteraAuthenticateRequest::try_from`. Everything else stays.
//!
//! Gate: Loki reports zero `netcetera_legacy_3ds_transport=true` over 7 days.

use domain_types::{
    connector_types::{PaymentFlowData, PaymentsAuthenticateData},
    errors::IntegrationError,
    payment_method_data::PaymentMethodDataTypes,
};

use super::{netcetera_types, transformers::MerchantSideObjects};

/// True when the caller sent none of the typed 3DS request fields, i.e. it
/// predates the typed contract and carries those values in the JSON blobs.
pub(super) fn is_legacy_caller<T: PaymentMethodDataTypes>(
    request: &PaymentsAuthenticateData<T>,
) -> bool {
    !request.uses_typed_three_ds_contract()
}

/// Build the AReq merchant-side objects from the deprecated
/// `connector_feature_data` blob. Byte-for-byte the pre-typed behaviour,
/// including the blob `notification_url` taking precedence over the request's
/// `return_url`, and `force_3ds_challenge = true` mapping to challenge
/// indicator 04.
///
/// Returns `None` when the caller carries no blob at all, so the caller falls
/// through to the typed path and no legacy-transport warning is emitted.
pub(super) fn legacy_merchant_side<T: PaymentMethodDataTypes>(
    request: &PaymentsAuthenticateData<T>,
    common_data: &PaymentFlowData,
) -> Result<Option<MerchantSideObjects>, error_stack::Report<IntegrationError>> {
    if !is_legacy_caller(request) || common_data.connector_feature_data.is_none() {
        return Ok(None);
    }

    let netcetera_meta: netcetera_types::NetceteraMeta =
        crate::utils::to_connector_meta_from_secret(common_data.connector_feature_data.clone())?;

    tracing::warn!(
        netcetera_legacy_3ds_transport = true,
        "Netcetera AReq built from the deprecated connector_feature_data blob"
    );

    let return_url = common_data
        .return_url
        .as_ref()
        .and_then(|u| url::Url::parse(u).ok());

    Ok(Some(MerchantSideObjects {
        acquirer: Some(netcetera_meta.to_acquirer_data()),
        merchant: Some(netcetera_meta.to_merchant_data(return_url.clone())),
        three_ds_requestor_url: netcetera_meta.notification_url.clone().or(return_url),
        challenge_indicator: netcetera_meta
            .force_3ds_challenge
            .filter(|force| *force)
            .map(|_| {
                netcetera_types::ThreeDSRequestorChallengeIndicator::ChallengeRequestedMandate
            }),
    }))
}
