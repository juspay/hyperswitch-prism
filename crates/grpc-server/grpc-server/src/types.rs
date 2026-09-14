//! gRPC ↔ domain type conversions for the gRPC server layer.

use domain_types::{errors::IntegrationError, utils::ForeignTryFrom};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};

/// Structured card payload serialized into the injector [`injector::TokenData`].
///
/// This mirrors the `CardTokenData` helper used by the payment Authorize flow so the
/// 3DS auth flows (pre/auth/post authenticate) build identical token data when the
/// request carries a vault-aliased card proxy.
#[derive(Debug, serde::Serialize)]
struct ProxyCardTokenData {
    card_number: Secret<String>,
    card_cvc: Secret<String>,
    card_exp_month: Secret<String>,
    card_exp_year: Secret<String>,
}

/// Newtype over the injector's (foreign) [`injector::TokenData`] so the local
/// [`ForeignTryFrom`] trait can be implemented for it (orphan rule). Used by the 3DS
/// auth flows to build injector token data from a gRPC card proxy, mirroring the
/// Authorize flow's private `ToTokenData` conversion. The resulting token data carries
/// the vault token values (card number alias, cvc, expiry) that the external-services
/// injector substitutes into the connector request template when `token_data` is `Some`.
pub struct InjectorTokenData(pub injector::TokenData);

impl ForeignTryFrom<&grpc_api_types::payments::ProxyCardDetails> for InjectorTokenData {
    type Error = IntegrationError;

    fn foreign_try_from(
        proxy_card_details: &grpc_api_types::payments::ProxyCardDetails,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let card_data = ProxyCardTokenData {
            card_number: Secret::new(
                proxy_card_details
                    .card_number
                    .as_ref()
                    .map(|cn| cn.peek().to_owned())
                    .filter(|cn| !cn.is_empty())
                    .ok_or_else(|| {
                        error_stack::report!(IntegrationError::MissingRequiredField {
                            field_name: "card_number",
                            context: Default::default(),
                        })
                    })?,
            ),
            card_cvc: Secret::new(
                proxy_card_details
                    .card_cvc
                    .as_ref()
                    .map(|cvc| cvc.clone().expose().to_string())
                    .filter(|cvc| !cvc.is_empty())
                    .ok_or_else(|| {
                        error_stack::report!(IntegrationError::MissingRequiredField {
                            field_name: "card_cvc",
                            context: Default::default(),
                        })
                    })?,
            ),
            card_exp_month: Secret::new(
                proxy_card_details
                    .card_exp_month
                    .as_ref()
                    .map(|em| em.clone().expose().to_string())
                    .filter(|em| !em.is_empty())
                    .ok_or_else(|| {
                        error_stack::report!(IntegrationError::MissingRequiredField {
                            field_name: "card_exp_month",
                            context: Default::default(),
                        })
                    })?,
            ),
            card_exp_year: Secret::new(
                proxy_card_details
                    .card_exp_year
                    .as_ref()
                    .map(|ey| ey.clone().expose().to_string())
                    .filter(|ey| !ey.is_empty())
                    .ok_or_else(|| {
                        error_stack::report!(IntegrationError::MissingRequiredField {
                            field_name: "card_exp_year",
                            context: Default::default(),
                        })
                    })?,
            ),
        };

        let card_json = serde_json::to_value(card_data).change_context(
            IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            },
        )?;

        Ok(Self(injector::TokenData {
            specific_token_data: common_utils::SecretSerdeValue::new(card_json),
        }))
    }
}
