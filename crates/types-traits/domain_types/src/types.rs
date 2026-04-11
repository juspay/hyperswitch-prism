use core::result::Result;
use std::{borrow::Cow, collections::HashMap, fmt::Debug, str::FromStr};

use crate::{
    connector_flow::MandateRevoke,
    connector_types::{self, ConnectorEnum},
    payment_method_data::SamsungPayWalletCredentials,
    utils::extract_connector_request_reference_id,
};
use common_enums::{
    CaptureMethod, CardNetwork, CountryAlpha2, FutureUsage, PaymentMethod, PaymentMethodType,
    SamsungPayCardBrand,
};
use common_utils::config_patch::Patch;
use common_utils::{
    consts::{self, NO_ERROR_CODE, X_EXTERNAL_VAULT_METADATA},
    id_type::CustomerId,
    metadata::MaskedMetadata,
    pii::Email,
    Method, SecretSerdeValue,
};
use error_stack::{report, ResultExt};
use grpc_api_types::payments::{
    self as grpc_payment_types, ConnectorState, DisputeResponse, DisputeServiceAcceptResponse,
    DisputeServiceDefendRequest, DisputeServiceDefendResponse,
    DisputeServiceSubmitEvidenceResponse,
    MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest,
    MerchantAuthenticationServiceCreateClientAuthenticationTokenResponse,
    MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse,
    MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenResponse,
    PaymentMethodAuthenticationServiceAuthenticateResponse,
    PaymentMethodAuthenticationServicePostAuthenticateResponse,
    PaymentMethodAuthenticationServicePreAuthenticateResponse, PaymentServiceAuthorizeRequest,
    PaymentServiceAuthorizeResponse, PaymentServiceCaptureResponse,
    PaymentServiceCreateOrderResponse, PaymentServiceGetResponse,
    PaymentServiceIncrementalAuthorizationRequest, PaymentServiceIncrementalAuthorizationResponse,
    PaymentServiceReverseResponse, PaymentServiceSetupRecurringRequest,
    PaymentServiceSetupRecurringResponse, PaymentServiceVoidRequest, PaymentServiceVoidResponse,
    RecurringPaymentServiceRevokeRequest, RecurringPaymentServiceRevokeResponse, RefundResponse,
};
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};
use tracing::info;
use utoipa::ToSchema;

/// Extract vault-related headers from gRPC metadata
fn extract_headers_from_metadata(
    metadata: &MaskedMetadata,
) -> Option<HashMap<String, Secret<String>>> {
    let mut vault_headers = HashMap::new();

    if let Some(vault_creds) = metadata.get(X_EXTERNAL_VAULT_METADATA) {
        vault_headers.insert(X_EXTERNAL_VAULT_METADATA.to_string(), vault_creds);
    }

    if vault_headers.is_empty() {
        None
    } else {
        Some(vault_headers)
    }
}

fn convert_optional_country_alpha2(
    value: grpc_api_types::payments::CountryAlpha2,
) -> Result<Option<CountryAlpha2>, error_stack::Report<IntegrationError>> {
    if matches!(value, grpc_api_types::payments::CountryAlpha2::Unspecified) {
        Ok(None)
    } else {
        CountryAlpha2::foreign_try_from(value).map(Some)
    }
}

impl ForeignTryFrom<grpc_api_types::payments::PazeDecryptedData>
    for router_data::PazeDecryptedData
{
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::payments::PazeDecryptedData,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let token = value.token.ok_or(IntegrationError::MissingRequiredField {
            field_name: "payment_method.paze.decrypted_data.token",
            context: IntegrationErrorContext::default(),
        })?;
        let billing_address =
            value
                .billing_address
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "payment_method.paze.decrypted_data.billing_address",
                    context: IntegrationErrorContext::default(),
                })?;
        let consumer = value
            .consumer
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "payment_method.paze.decrypted_data.consumer",
                context: IntegrationErrorContext::default(),
            })?;

        let consumer_country_code = convert_optional_country_alpha2(consumer.country_code())?;

        let email_address = Email::try_from(
            consumer
                .email_address
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "payment_method.paze.decrypted_data.consumer.email_address",
                    context: IntegrationErrorContext::default(),
                })?
                .expose(),
        )
        .change_context(IntegrationError::InvalidDataFormat {
            field_name: "payment_method.paze.decrypted_data.consumer.email_address",
            context: IntegrationErrorContext {
                additional_context: Some(
                    "Invalid Paze consumer email in payment_method".to_string(),
                ),
                ..Default::default()
            },
        })?;

        let mobile_number = consumer
            .mobile_number
            .map(
                |mobile_number| -> Result<_, error_stack::Report<IntegrationError>> {
                    Ok(router_data::PazePhoneNumber {
                        country_code: mobile_number
                            .country_code
                            .ok_or(IntegrationError::MissingRequiredField { field_name: "payment_method.paze.decrypted_data.consumer.mobile_number.country_code", context: IntegrationErrorContext::default() })?,
                        phone_number: mobile_number
                            .phone_number
                            .ok_or(IntegrationError::MissingRequiredField { field_name: "payment_method.paze.decrypted_data.consumer.mobile_number.phone_number", context: IntegrationErrorContext::default() })?,
                    })
                },
            )
            .transpose()?;

        let grpc_payment_card_network =
            grpc_api_types::payments::CardNetwork::try_from(value.payment_card_network)
                .change_context(IntegrationError::InvalidDataFormat {
                    field_name: "payment_method.paze.decrypted_data.payment_card_network",
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "Invalid Paze payment card network in payment_method".to_string(),
                        ),
                        ..Default::default()
                    },
                })?;

        let payment_card_network = CardNetwork::foreign_try_from(grpc_payment_card_network)?;

        let dynamic_data = value
            .dynamic_data
            .into_iter()
            .map(|dynamic_data| router_data::PazeDynamicData {
                dynamic_data_value: dynamic_data.dynamic_data_value,
                dynamic_data_type: dynamic_data.dynamic_data_type,
                dynamic_data_expiration: dynamic_data.dynamic_data_expiration,
            })
            .collect();

        let billing_country_code = convert_optional_country_alpha2(billing_address.country_code())?;

        Ok(Self {
            client_id: value
                .client_id
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "payment_method.paze.decrypted_data.client_id",
                    context: IntegrationErrorContext::default(),
                })?,
            profile_id: value.profile_id,
            token: router_data::PazeToken {
                payment_token: token.payment_token.ok_or(
                    IntegrationError::MissingRequiredField {
                        field_name: "payment_method.paze.decrypted_data.token.payment_token",
                        context: IntegrationErrorContext::default(),
                    },
                )?,
                token_expiration_month: token.token_expiration_month.ok_or(
                    IntegrationError::MissingRequiredField {
                        field_name:
                            "payment_method.paze.decrypted_data.token.token_expiration_month",
                        context: IntegrationErrorContext::default(),
                    },
                )?,
                token_expiration_year: token.token_expiration_year.ok_or(
                    IntegrationError::MissingRequiredField {
                        field_name:
                            "payment_method.paze.decrypted_data.token.token_expiration_year",
                        context: IntegrationErrorContext::default(),
                    },
                )?,
                payment_account_reference: token.payment_account_reference.ok_or(
                    IntegrationError::MissingRequiredField {
                        field_name:
                            "payment_method.paze.decrypted_data.token.payment_account_reference",
                        context: IntegrationErrorContext::default(),
                    },
                )?,
            },
            payment_card_network,
            dynamic_data,
            billing_address: router_data::PazeAddress {
                name: billing_address.name,
                line1: billing_address.line1,
                line2: billing_address.line2,
                line3: billing_address.line3,
                city: billing_address.city,
                state: billing_address.state,
                zip: billing_address.zip,
                country_code: billing_country_code,
            },
            consumer: router_data::PazeConsumer {
                first_name: consumer.first_name,
                last_name: consumer.last_name,
                full_name: consumer
                    .full_name
                    .ok_or(IntegrationError::MissingRequiredField {
                        field_name: "payment_method.paze.decrypted_data.consumer.full_name",
                        context: IntegrationErrorContext::default(),
                    })?,
                email_address,
                mobile_number,
                country_code: consumer_country_code,
                language_code: consumer.language_code,
            },
            eci: value.eci,
        })
    }
}

impl ForeignTryFrom<(Secret<String>, &'static str)> for SecretSerdeValue {
    type Error = IntegrationError;

    fn foreign_try_from(
        (secret, field_name): (Secret<String>, &'static str),
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let raw = secret.expose();
        serde_json::from_str(&raw).map(Self::new).change_context(
            IntegrationError::InvalidDataFormat {
                field_name,
                context: IntegrationErrorContext::default(),
            },
        )
    }
}

// For decoding connector feature data and Engine trait - base64 crate no longer needed here
use crate::{
    connector_flow::{
        Accept, Authenticate, Authorize, Capture, ClientAuthenticationToken,
        CreateConnectorCustomer, CreateOrder, DefendDispute, IncrementalAuthorization, PSync,
        PaymentMethodToken, PostAuthenticate, PreAuthenticate, RSync, Refund, RepeatPayment,
        ServerAuthenticationToken, ServerSessionAuthenticationToken, SetupMandate, SubmitEvidence,
        Void, VoidPC,
    },
    connector_types::{
        AcceptDisputeData, ApplePayPaymentRequest, ApplePaySessionResponse, BillingDescriptor,
        ClientAuthenticationTokenData, ClientAuthenticationTokenRequestData, ConnectorCustomerData,
        ConnectorMandateReferenceId, ConnectorResponseHeaders,
        ConnectorSpecificClientAuthenticationResponse, ContinueRedirectionResponse, CustomerInfo,
        DisputeDefendData, DisputeFlowData, DisputeResponseData, DisputeWebhookDetailsResponse,
        GpayAllowedPaymentMethods, GpayBillingAddressFormat, GpayClientAuthenticationResponse,
        L2L3Data, MandateReferenceId, MandateRevokeRequestData, MultipleCaptureRequestData,
        NetworkTokenWithNTIRef, NextActionCall, OrderInfo, PaymentCreateOrderData,
        PaymentCreateOrderResponse, PaymentFlowData, PaymentMethodTokenResponse,
        PaymentMethodTokenizationData, PaymentVoidData, PaymentsAuthenticateData,
        PaymentsAuthorizeData, PaymentsCaptureData, PaymentsIncrementalAuthorizationData,
        PaymentsPostAuthenticateData, PaymentsPreAuthenticateData, PaymentsResponseData,
        PaymentsSyncData, PaypalFlow, PaypalTransactionInfo, RawConnectorRequestResponse,
        RedirectDetailsResponse, RefundFlowData, RefundSyncData, RefundWebhookDetailsResponse,
        RefundsData, RefundsResponseData, RepeatPaymentData, ResponseId,
        ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData,
        ServerSessionAuthenticationTokenRequestData, ServerSessionAuthenticationTokenResponseData,
        SetupMandateRequestData, SubmitEvidenceData, TaxInfo, WebhookDetailsResponse,
    },
    errors::{
        ConnectorError, IntegrationError, IntegrationErrorContext,
        ResponseTransformationErrorContext,
    },
    mandates::{self, MandateData},
    payment_address::{
        Address, AddressDetails, OrderDetailsWithAmount, PaymentAddress, PhoneDetails,
    },
    payment_method_data,
    payment_method_data::{
        DefaultPCIHolder, PaymentMethodData, PaymentMethodDataTypes, RawCardNumber,
        VaultTokenHolder,
    },
    router_data::{
        self, AdditionalPaymentMethodConnectorResponse, ConnectorResponseData,
        ConnectorSpecificConfig, RecurringMandatePaymentData,
    },
    router_data_v2::RouterDataV2,
    router_request_types,
    router_request_types::BrowserInformation,
    router_response_types,
    utils::{extract_merchant_id_from_metadata, ForeignFrom, ForeignTryFrom},
};

#[derive(
    Clone,
    serde::Deserialize,
    serde::Serialize,
    Debug,
    Default,
    PartialEq,
    config_patch_derive::Patch,
)]
pub struct Connectors {
    // Added pub
    pub adyen: ConnectorParams,
    pub forte: ConnectorParams,
    pub razorpay: ConnectorParams,
    pub razorpayv2: ConnectorParams,
    pub fiserv: ConnectorParams,
    pub elavon: ConnectorParams, // Add your connector params
    pub xendit: ConnectorParams,
    pub ppro: ConnectorParams,
    pub checkout: ConnectorParams,
    pub authorizedotnet: ConnectorParams, // Add your connector params
    pub mifinity: ConnectorParams,
    pub phonepe: ConnectorParams,
    pub cashfree: ConnectorParams,
    pub paytm: ConnectorParams,
    pub fiuu: ConnectorParams,
    pub payu: ConnectorParams,
    pub cashtocode: ConnectorParams,
    pub novalnet: ConnectorParams,
    pub nexinets: ConnectorParams,
    pub noon: ConnectorParams,
    pub braintree: ConnectorParams,
    pub volt: ConnectorParams,
    pub calida: ConnectorParams,
    pub cryptopay: ConnectorParams,
    pub helcim: ConnectorParams,
    pub dlocal: ConnectorParams,
    pub placetopay: ConnectorParams,
    pub rapyd: ConnectorParams,
    pub aci: ConnectorParams,
    pub trustpay: ConnectorParamsWithMoreUrls,
    pub stripe: ConnectorParams,
    pub cybersource: ConnectorParams,
    pub worldpay: ConnectorParams,
    pub worldpayvantiv: ConnectorParams,
    pub multisafepay: ConnectorParams,
    pub payload: ConnectorParams,
    pub fiservemea: ConnectorParams,
    pub paysafe: ConnectorParams,
    pub datatrans: ConnectorParams,
    pub bluesnap: ConnectorParams,
    pub authipay: ConnectorParams,
    pub bamboraapac: ConnectorParams,
    pub silverflow: ConnectorParams,
    pub celero: ConnectorParams,
    pub paypal: ConnectorParams,
    pub stax: ConnectorParams,
    pub billwerk: ConnectorParams,
    pub hipay: ConnectorParams,
    pub trustpayments: ConnectorParams,
    pub globalpay: ConnectorParams,
    pub nuvei: ConnectorParams,
    pub iatapay: ConnectorParams,
    pub jpmorgan: ConnectorParams,
    pub nmi: ConnectorParams,
    pub shift4: ConnectorParams,
    pub paybox: ConnectorParams,
    pub barclaycard: ConnectorParams,
    pub redsys: ConnectorParams,
    pub nexixpay: ConnectorParams,
    pub mollie: ConnectorParams,
    pub airwallex: ConnectorParams,
    pub worldpayxml: ConnectorParams,
    pub tsys: ConnectorParams,
    pub bankofamerica: ConnectorParams,
    pub powertranz: ConnectorParams,
    pub getnet: ConnectorParams,
    pub bambora: ConnectorParams,
    pub payme: ConnectorParams,
    pub revolut: ConnectorParams,
    pub gigadat: ConnectorParams,
    pub loonio: ConnectorParams,
    pub wellsfargo: ConnectorParams,
    pub hyperpg: ConnectorParams,
    pub zift: ConnectorParams,
    pub revolv3: ConnectorParams,
    pub fiservcommercehub: ConnectorParams,
    pub truelayer: ConnectorParams,
    pub peachpayments: ConnectorParams,
    pub finix: ConnectorParams,
    pub trustly: ConnectorParams,
    pub itaubank: ConnectorParams,
}

#[derive(Clone, Deserialize, Serialize, Debug, Default, PartialEq, config_patch_derive::Patch)]
pub struct ConnectorParams {
    /// base url
    #[serde(default)]
    pub base_url: String,
    #[serde(default)]
    pub dispute_base_url: Option<String>,
    #[serde(default)]
    pub secondary_base_url: Option<String>,
    #[serde(default)]
    pub third_base_url: Option<String>,
}

impl ConnectorParams {
    pub fn new(base_url: String, dispute_base_url: Option<String>) -> Self {
        Self {
            base_url,
            dispute_base_url,
            secondary_base_url: None,
            third_base_url: None,
        }
    }

    /// Patch this ConnectorParams with resolved URLs from superposition.
    ///
    /// Only non-empty resolved URLs will override the existing values.
    /// This allows superposition to selectively override specific URLs
    /// while keeping static config values for others.
    pub fn patch_with_resolved_urls(
        &self,
        base_url: Option<String>,
        dispute_base_url: Option<String>,
        secondary_base_url: Option<String>,
        third_base_url: Option<String>,
    ) -> Self {
        Self {
            base_url: base_url.unwrap_or_else(|| self.base_url.clone()),
            dispute_base_url: dispute_base_url.or(self.dispute_base_url.clone()),
            secondary_base_url: secondary_base_url.or(self.secondary_base_url.clone()),
            third_base_url: third_base_url.or(self.third_base_url.clone()),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, Default, PartialEq, config_patch_derive::Patch)]
pub struct ConnectorParamsWithMoreUrls {
    /// base url
    pub base_url: String,
    /// base url for bank redirects
    pub base_url_bank_redirects: String,
}

// Trait to provide access to connectors field
pub trait HasConnectors {
    fn connectors(&self) -> &Connectors;
}

impl HasConnectors for PaymentFlowData {
    fn connectors(&self) -> &Connectors {
        &self.connectors
    }
}

impl HasConnectors for RefundFlowData {
    fn connectors(&self) -> &Connectors {
        &self.connectors
    }
}

impl HasConnectors for DisputeFlowData {
    fn connectors(&self) -> &Connectors {
        &self.connectors
    }
}

impl Connectors {
    /// Patch the specified connector's URL configuration with resolved URLs from superposition.
    ///
    /// This method creates a new `Connectors` instance with the specified connector's
    /// `ConnectorParams` updated with the resolved URLs. All other connectors remain unchanged.
    ///
    /// This implementation leverages the `config_patch` framework to apply selective patches
    /// to individual connector fields, avoiding manual match arms for each connector.
    ///
    /// # Arguments
    /// * `connector` - The connector enum variant
    /// * `urls` - The resolved URLs from superposition configuration
    ///
    /// # Returns
    /// `Ok(Connectors)` - A new `Connectors` instance with the patched connector params.
    /// `Err(IntegrationError)` - If the connector is not supported for URL patching.
    ///
    /// # Example
    /// ```ignore
    /// let urls = ConnectorUrls {
    ///     base_url: Some("https://api.stripe.com/".to_string()),
    ///     ..Default::default()
    /// };
    /// let patched = connectors.patch_connector_urls(&ConnectorEnum::Stripe, &urls)?;
    /// ```
    pub fn patch_connector_urls(
        &self,
        connector: &ConnectorEnum,
        urls: &common_utils::superposition_config::ConnectorUrls,
    ) -> Result<Self, IntegrationError> {
        let mut patched = self.clone();

        // Create a patch for ConnectorParams with the resolved URLs
        let params_patch = ConnectorParamsPatch {
            base_url: urls.base_url.clone(),
            dispute_base_url: Some(urls.dispute_base_url.clone()),
            secondary_base_url: Some(urls.secondary_base_url.clone()),
            third_base_url: Some(urls.third_base_url.clone()),
        };

        // Apply the patch to the appropriate connector field
        // Using the config_patch framework, missing fields in the patch mean "no change"
        match connector {
            ConnectorEnum::Stripe => {
                patched.stripe.apply(params_patch);
            }
            ConnectorEnum::Adyen => {
                patched.adyen.apply(params_patch);
            }
            ConnectorEnum::Paypal => {
                patched.paypal.apply(params_patch);
            }
            ConnectorEnum::Braintree => {
                patched.braintree.apply(params_patch);
            }
            ConnectorEnum::Checkout => {
                patched.checkout.apply(params_patch);
            }
            ConnectorEnum::Cybersource => {
                patched.cybersource.apply(params_patch);
            }
            ConnectorEnum::Revolut => {
                patched.revolut.apply(params_patch);
            }
            ConnectorEnum::Worldpay => {
                patched.worldpay.apply(params_patch);
            }
            ConnectorEnum::Trustpay => {
                // TrustPay uses ConnectorParamsWithMoreUrls which has different fields
                let trustpay_patch = ConnectorParamsWithMoreUrlsPatch {
                    base_url: urls.base_url.clone(),
                    base_url_bank_redirects: urls.base_url_bank_redirects.clone(),
                };
                patched.trustpay.apply(trustpay_patch);
            }
            _ => {
                // Connector not supported for URL patching - return error
                return Err(IntegrationError::InvalidDataFormat {
                    field_name: "connector",
                    context: IntegrationErrorContext {
                        additional_context: Some(format!(
                            "Connector '{}' is not supported for dynamic URL patching from superposition. \
                             Supported connectors: stripe, adyen, paypal, braintree, checkout, cybersource, revolut, worldpay, trustpay",
                            connector
                        )),
                        ..Default::default()
                    }
                });
            }
        }

        Ok(patched)
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq, Hash, config_patch_derive::Patch)]
pub struct Proxy {
    pub http_url: Option<String>,
    pub https_url: Option<String>,
    pub idle_pool_connection_timeout: Option<u64>,
    pub bypass_proxy_urls: Vec<String>,
    pub mitm_proxy_enabled: bool,
    pub mitm_ca_cert: Option<String>,
}

impl Proxy {
    pub fn cache_key(&self, should_bypass_proxy: bool) -> Option<Self> {
        // Return Some(self) if there's an actual proxy configuration
        // let sbp = self.bypass_proxy_urls.contains(&url.to_string());
        if should_bypass_proxy || (self.http_url.is_none() && self.https_url.is_none()) {
            None
        } else {
            Some(self.clone())
        }
    }

    pub fn is_proxy_configured(&self, should_bypass_proxy: bool) -> bool {
        should_bypass_proxy || (self.http_url.is_none() && self.https_url.is_none())
    }
}

impl ForeignTryFrom<grpc_api_types::payments::CaptureMethod> for CaptureMethod {
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::payments::CaptureMethod,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        match value {
            grpc_api_types::payments::CaptureMethod::Automatic => Ok(Self::Automatic),
            grpc_api_types::payments::CaptureMethod::Manual => Ok(Self::Manual),
            grpc_api_types::payments::CaptureMethod::ManualMultiple => Ok(Self::ManualMultiple),
            grpc_api_types::payments::CaptureMethod::Scheduled => Ok(Self::Scheduled),
            _ => Ok(Self::Automatic),
        }
    }
}

impl ForeignTryFrom<grpc_api_types::payments::ThreeDsCompletionIndicator>
    for connector_types::ThreeDsCompletionIndicator
{
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::payments::ThreeDsCompletionIndicator,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        match value {
            grpc_api_types::payments::ThreeDsCompletionIndicator::Success => Ok(Self::Success),
            grpc_api_types::payments::ThreeDsCompletionIndicator::Failure => Ok(Self::Failure),
            _ => Ok(Self::NotAvailable),
        }
    }
}

impl ForeignTryFrom<grpc_api_types::payments::CardNetwork> for CardNetwork {
    type Error = IntegrationError;

    fn foreign_try_from(
        network: grpc_api_types::payments::CardNetwork,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        match network {
            grpc_api_types::payments::CardNetwork::Visa => Ok(Self::Visa),
            grpc_api_types::payments::CardNetwork::Mastercard => Ok(Self::Mastercard),
            grpc_api_types::payments::CardNetwork::Amex => Ok(Self::AmericanExpress),
            grpc_api_types::payments::CardNetwork::Jcb => Ok(Self::JCB),
            grpc_api_types::payments::CardNetwork::Diners => Ok(Self::DinersClub),
            grpc_api_types::payments::CardNetwork::Discover => Ok(Self::Discover),
            grpc_api_types::payments::CardNetwork::CartesBancaires => Ok(Self::CartesBancaires),
            grpc_api_types::payments::CardNetwork::Unionpay => Ok(Self::UnionPay),
            grpc_api_types::payments::CardNetwork::Rupay => Ok(Self::RuPay),
            grpc_api_types::payments::CardNetwork::Maestro => Ok(Self::Maestro),
            grpc_api_types::payments::CardNetwork::InteracCard => Ok(Self::Interac),
            grpc_api_types::payments::CardNetwork::Star => Ok(Self::Star),
            grpc_api_types::payments::CardNetwork::Pulse => Ok(Self::Pulse),
            grpc_api_types::payments::CardNetwork::Accel => Ok(Self::Accel),
            grpc_api_types::payments::CardNetwork::Nyce => Ok(Self::Nyce),
            grpc_api_types::payments::CardNetwork::Unspecified => {
                Err(IntegrationError::InvalidDataFormat {
                    field_name: "card_network",
                    context: IntegrationErrorContext {
                        additional_context: Some("Card network must be specified".to_string()),
                        ..Default::default()
                    },
                }
                .into())
            }
        }
    }
}

impl ForeignTryFrom<grpc_api_types::payments::Tokenization> for common_enums::Tokenization {
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::payments::Tokenization,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        match value {
            grpc_api_types::payments::Tokenization::SkipPsp => Ok(Self::SkipPsp),
            grpc_api_types::payments::Tokenization::TokenizeAtPsp => Ok(Self::TokenizeAtPsp),
            grpc_api_types::payments::Tokenization::Unspecified => {
                Err(IntegrationError::InvalidDataFormat {
                    field_name: "tokenization",
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "Tokenization strategy must be specified".to_string(),
                        ),
                        ..Default::default()
                    },
                }
                .into())
            }
        }
    }
}

// Helper functions for Samsung Pay credential validation
/// Trims a string and returns None if empty, Some(trimmed) otherwise
fn trim_and_check_empty(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

/// Validates a 4-digit string (for card_last_four_digits and dpan_last_four_digits)
fn validate_last_four_digits(
    value: &str,
    field_name: &str,
) -> Result<String, error_stack::Report<IntegrationError>> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(IntegrationError::InvalidDataFormat {
            field_name: "samsung_pay_last_four_digits",
            context: IntegrationErrorContext {
                additional_context: Some(format!("Samsung Pay {} cannot be empty", field_name)),
                ..Default::default()
            },
        }
        .into());
    }
    if trimmed.len() != 4 {
        return Err(IntegrationError::InvalidDataFormat {
            field_name: "samsung_pay_last_four_digits",
            context: IntegrationErrorContext {
                additional_context: Some(format!(
                    "Samsung Pay {} must be 4 characters",
                    field_name
                )),
                ..Default::default()
            },
        }
        .into());
    }
    Ok(trimmed.to_string())
}

impl ForeignTryFrom<grpc_api_types::payments::samsung_wallet::PaymentCredential>
    for SamsungPayWalletCredentials
{
    type Error = IntegrationError;

    fn foreign_try_from(
        credential: grpc_api_types::payments::samsung_wallet::PaymentCredential,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        // Validate card_last_four_digits
        let last_four_raw = credential
            .card_last_four_digits
            .as_ref()
            .map(|s| s.clone().expose())
            .ok_or_else(|| IntegrationError::MissingRequiredField {
                field_name: "card_last_four_digits",
                context: IntegrationErrorContext::default(),
            })?;

        let last_four = validate_last_four_digits(&last_four_raw, "card_last_four_digits")?;

        // Validate DPAN last four digits if present
        if let Some(dpan_secret) = credential.dpan_last_four_digits.as_ref() {
            let dpan_raw = dpan_secret.clone().expose();
            validate_last_four_digits(&dpan_raw, "dpan_last_four_digits")?;
        }

        // Validate token_data
        let token_data = credential.token_data.as_ref().ok_or_else(|| {
            IntegrationError::MissingRequiredField {
                field_name: "token_data",
                context: IntegrationErrorContext::default(),
            }
        })?;

        if trim_and_check_empty(&token_data.version).is_none() {
            return Err(IntegrationError::MissingRequiredField {
                field_name: "token_version",
                context: IntegrationErrorContext::default(),
            }
            .into());
        }

        let raw_token =
            token_data
                .data
                .clone()
                .ok_or_else(|| IntegrationError::MissingRequiredField {
                    field_name: "token_data",
                    context: IntegrationErrorContext::default(),
                })?;

        if trim_and_check_empty(raw_token.peek()).is_none() {
            return Err(IntegrationError::MissingRequiredField {
                field_name: "token_data",
                context: IntegrationErrorContext::default(),
            }
            .into());
        }

        let card_brand = SamsungPayCardBrand::foreign_try_from(credential.card_brand())?;
        Ok(Self {
            method: credential.method,
            recurring_payment: credential.recurring_payment,
            card_brand,
            dpan_last_four_digits: credential
                .dpan_last_four_digits
                .as_ref()
                .map(|s| s.clone().expose()),
            card_last_four_digits: last_four.to_string(),
            token_data: payment_method_data::SamsungPayTokenData {
                three_ds_type: token_data.r#type.clone(),
                version: token_data.version.clone(),
                data: raw_token,
            },
        })
    }
}

impl ForeignTryFrom<grpc_api_types::payments::CardNetwork> for SamsungPayCardBrand {
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::payments::CardNetwork,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        match value {
            grpc_api_types::payments::CardNetwork::Visa => Ok(SamsungPayCardBrand::Visa),
            grpc_api_types::payments::CardNetwork::Mastercard => {
                Ok(SamsungPayCardBrand::MasterCard)
            }
            grpc_api_types::payments::CardNetwork::Amex => Ok(SamsungPayCardBrand::Amex),
            grpc_api_types::payments::CardNetwork::Discover => Ok(SamsungPayCardBrand::Discover),
            _ => Ok(SamsungPayCardBrand::Unknown),
        }
    }
}

impl ForeignTryFrom<grpc_api_types::payments::PaymentExperience>
    for common_enums::PaymentExperience
{
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::payments::PaymentExperience,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        match value {
            grpc_api_types::payments::PaymentExperience::RedirectToUrl => Ok(Self::RedirectToUrl),
            grpc_api_types::payments::PaymentExperience::InvokeSdkClient => {
                Ok(Self::InvokeSdkClient)
            }
            grpc_api_types::payments::PaymentExperience::DisplayQrCode => Ok(Self::DisplayQrCode),
            grpc_api_types::payments::PaymentExperience::OneClick => Ok(Self::OneClick),
            grpc_api_types::payments::PaymentExperience::LinkWallet => Ok(Self::LinkWallet),
            grpc_api_types::payments::PaymentExperience::InvokePaymentApp => {
                Ok(Self::InvokePaymentApp)
            }
            grpc_api_types::payments::PaymentExperience::DisplayWaitScreen => {
                Ok(Self::DisplayWaitScreen)
            }
            grpc_api_types::payments::PaymentExperience::CollectOtp => Ok(Self::CollectOtp),
            grpc_api_types::payments::PaymentExperience::Unspecified => {
                Err(IntegrationError::InvalidDataFormat {
                    field_name: "payment_experience",
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "Payment experience must be specified".to_string(),
                        ),
                        ..Default::default()
                    },
                }
                .into())
            }
        }
    }
}

// Helper function to extract and convert UPI source from gRPC type
fn convert_upi_source(
    source_option: Option<i32>,
) -> Result<Option<payment_method_data::UpiSource>, error_stack::Report<IntegrationError>> {
    source_option
        .map(|source| {
            grpc_api_types::payments::UpiSource::try_from(source)
                .map_err(|_| {
                    error_stack::report!(IntegrationError::InvalidDataFormat {
                        field_name: "payment_method.upi.source",
                        context: IntegrationErrorContext {
                            additional_context: Some("Invalid UPI source value".to_string()),
                            ..Default::default()
                        }
                    })
                })
                .and_then(payment_method_data::UpiSource::foreign_try_from)
        })
        .transpose()
}

impl ForeignTryFrom<grpc_api_types::payments::UpiSource> for payment_method_data::UpiSource {
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::payments::UpiSource,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        match value {
            grpc_api_types::payments::UpiSource::UpiCc => Ok(Self::UpiCc),
            grpc_api_types::payments::UpiSource::UpiCl => Ok(Self::UpiCl),
            grpc_api_types::payments::UpiSource::UpiAccount => Ok(Self::UpiAccount),
            grpc_api_types::payments::UpiSource::UpiCcCl => Ok(Self::UpiCcCl),
            grpc_api_types::payments::UpiSource::UpiPpi => Ok(Self::UpiPpi),
            grpc_api_types::payments::UpiSource::UpiVoucher => Ok(Self::UpiVoucher),
        }
    }
}

impl ForeignFrom<payment_method_data::UpiSource> for grpc_api_types::payments::UpiSource {
    fn foreign_from(value: payment_method_data::UpiSource) -> Self {
        match value {
            payment_method_data::UpiSource::UpiCc => Self::UpiCc,
            payment_method_data::UpiSource::UpiCl => Self::UpiCl,
            payment_method_data::UpiSource::UpiAccount => Self::UpiAccount,
            payment_method_data::UpiSource::UpiCcCl => Self::UpiCcCl,
            payment_method_data::UpiSource::UpiPpi => Self::UpiPpi,
            payment_method_data::UpiSource::UpiVoucher => Self::UpiVoucher,
        }
    }
}

impl<
        T: PaymentMethodDataTypes
            + Default
            + Debug
            + Send
            + Eq
            + PartialEq
            + Serialize
            + serde::de::DeserializeOwned
            + Clone
            + CardConversionHelper<T>,
    > ForeignTryFrom<grpc_api_types::payments::PaymentMethod> for PaymentMethodData<T>
{
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::payments::PaymentMethod,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        tracing::info!("PaymentMethod data received: {:?}", value);

        match value.payment_method {
            Some(data) => match data {
                // ============================================================================
                // CARD METHODS
                // ============================================================================
                grpc_api_types::payments::payment_method::PaymentMethod::Card(card_details) => {
                    let card = payment_method_data::Card::<T>::foreign_try_from(card_details)?;
                    Ok(Self::Card(card))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::CardProxy(
                    card_details,
                ) => {
                    let card = payment_method_data::Card::<T>::foreign_try_from(card_details)?;
                    Ok(Self::Card(card))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::CardRedirect(
                    card_redirect,
                ) => {
                    let card_redirect_data = match card_redirect.r#type() {
                        grpc_api_types::payments::card_redirect::CardRedirectType::Knet => {
                            payment_method_data::CardRedirectData::Knet {}
                        }
                        grpc_api_types::payments::card_redirect::CardRedirectType::Benefit => {
                            payment_method_data::CardRedirectData::Benefit {}
                        }
                        grpc_api_types::payments::card_redirect::CardRedirectType::MomoAtm => {
                            payment_method_data::CardRedirectData::MomoAtm {}
                        }
                        grpc_api_types::payments::card_redirect::CardRedirectType::CardRedirect => {
                            payment_method_data::CardRedirectData::CardRedirect {}
                        }
                        grpc_api_types::payments::card_redirect::CardRedirectType::Unspecified => {
                            return Err(report!(IntegrationError::InvalidDataFormat { field_name: "payment_method.card_redirect.type", context: IntegrationErrorContext { additional_context: Some("Card redirect type cannot be unspecified".to_string()), ..Default::default() } }))
                        }
                    };
                    Ok(Self::CardRedirect(card_redirect_data))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::Token(token) => {
                    Ok(Self::PaymentMethodToken(payment_method_data::PaymentMethodToken {
                        token: token
                            .token
                            .ok_or_else(|| report!(IntegrationError::MissingRequiredField {
                                field_name: "payment_method.token.token",
                                context: Default::default(),
                            }))?,
                    }))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::UpiCollect(
                    upi_collect,
                ) => {
                    let upi_source = convert_upi_source(upi_collect.upi_source)?;
                    Ok(PaymentMethodData::Upi(
                        payment_method_data::UpiData::UpiCollect(
                            payment_method_data::UpiCollectData {
                                vpa_id: upi_collect.vpa_id.map(|vpa| vpa.expose().into()),
                                upi_source,
                            },
                        ),
                    ))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::UpiIntent(upi_intent) => {
                    let upi_source = convert_upi_source(upi_intent.upi_source)?;
                    Ok(PaymentMethodData::Upi(
                        payment_method_data::UpiData::UpiIntent(
                            payment_method_data::UpiIntentData {
                                upi_source,
                                app_name: upi_intent.app_name,
                            },
                        ),
                    ))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::UpiQr(upi_qr) => {
                    let upi_source = convert_upi_source(upi_qr.upi_source)?;
                    Ok(PaymentMethodData::Upi(
                        crate::payment_method_data::UpiData::UpiQr(
                            crate::payment_method_data::UpiQrData { upi_source },
                        ),
                    ))
                }
                // ============================================================================
                // REWARD METHODS - Flattened direct variants
                // ============================================================================
                grpc_api_types::payments::payment_method::PaymentMethod::ClassicReward(_) => {
                    Ok(Self::Reward)
                }
                grpc_api_types::payments::payment_method::PaymentMethod::EVoucher(_) => {
                    Ok(Self::Reward)
                }
                // ============================================================================
                // DIGITAL WALLETS - Direct conversions
                // ============================================================================
                grpc_api_types::payments::payment_method::PaymentMethod::Bluecode(_) => Ok(
                    Self::Wallet(payment_method_data::WalletData::BluecodeRedirect {}),
                ),
                grpc_api_types::payments::payment_method::PaymentMethod::RevolutPay(_) => {
                    Ok(Self::Wallet(payment_method_data::WalletData::RevolutPay(
                        payment_method_data::RevolutPayData {},
                    )))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::AliPayRedirect(_) => Ok(
                    Self::Wallet(payment_method_data::WalletData::AliPayRedirect(
                        payment_method_data::AliPayRedirection {},
                    )),
                ),
                grpc_api_types::payments::payment_method::PaymentMethod::AliPayHk(_) => Ok(
                    Self::Wallet(payment_method_data::WalletData::AliPayHkRedirect(
                        payment_method_data::AliPayHkRedirection {},
                    )),
                ),
                grpc_api_types::payments::payment_method::PaymentMethod::GcashRedirect(_) => Ok(
                    Self::Wallet(payment_method_data::WalletData::GcashRedirect(
                        payment_method_data::GcashRedirection {},
                    )),
                ),
                grpc_api_types::payments::payment_method::PaymentMethod::DanaRedirect(_) => Ok(
                    Self::Wallet(payment_method_data::WalletData::DanaRedirect {}),
                ),
                grpc_api_types::payments::payment_method::PaymentMethod::GoPayRedirect(_) => Ok(
                    Self::Wallet(payment_method_data::WalletData::GoPayRedirect(
                        payment_method_data::GoPayRedirection {},
                    )),
                ),
                grpc_api_types::payments::payment_method::PaymentMethod::KakaoPayRedirect(_) => Ok(
                    Self::Wallet(payment_method_data::WalletData::KakaoPayRedirect(
                        payment_method_data::KakaoPayRedirection {},
                    )),
                ),
                grpc_api_types::payments::payment_method::PaymentMethod::MbWayRedirect(_) => Ok(
                    Self::Wallet(payment_method_data::WalletData::MbWayRedirect(Box::new(
                        payment_method_data::MbWayRedirection {},
                    ))),
                ),
                grpc_api_types::payments::payment_method::PaymentMethod::MomoRedirect(_) => Ok(
                    Self::Wallet(payment_method_data::WalletData::MomoRedirect(
                        payment_method_data::MomoRedirection {},
                    )),
                ),
                grpc_api_types::payments::payment_method::PaymentMethod::TouchNGoRedirect(_) => Ok(
                    Self::Wallet(payment_method_data::WalletData::TouchNGoRedirect(
                        Box::new(payment_method_data::TouchNGoRedirection {}),
                    )),
                ),
                grpc_api_types::payments::payment_method::PaymentMethod::TwintRedirect(_) => Ok(
                    Self::Wallet(payment_method_data::WalletData::TwintRedirect {}),
                ),
                grpc_api_types::payments::payment_method::PaymentMethod::VippsRedirect(_) => Ok(
                    Self::Wallet(payment_method_data::WalletData::VippsRedirect {}),
                ),
                grpc_api_types::payments::payment_method::PaymentMethod::SwishQr(_) => Ok(
                    Self::Wallet(payment_method_data::WalletData::SwishQr(
                        payment_method_data::SwishQrData {},
                    )),
                ),
                grpc_api_types::payments::payment_method::PaymentMethod::AmazonPayRedirect(_) => {
                    Ok(Self::Wallet(
                        payment_method_data::WalletData::AmazonPayRedirect(Box::new(
                            payment_method_data::AmazonPayRedirectData {},
                        )),
                    ))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::CashappQr(_) => {
                    Ok(Self::Wallet(payment_method_data::WalletData::CashappQr(
                        Box::new(payment_method_data::CashappQr {}),
                    )))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::WeChatPayQr(_) => {
                    Ok(Self::Wallet(payment_method_data::WalletData::WeChatPayQr(
                        Box::new(payment_method_data::WeChatPayQr {}),
                    )))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::MbWay(_) => {
                    Ok(Self::Wallet(payment_method_data::WalletData::MbWay(
                        payment_method_data::MbWayData {},
                    )))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::Satispay(_) => {
                    Ok(Self::Wallet(payment_method_data::WalletData::Satispay(
                        payment_method_data::SatispayData {},
                    )))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::Wero(_) => {
                    Ok(Self::Wallet(payment_method_data::WalletData::Wero(
                        payment_method_data::WeroData {},
                    )))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::WeChatPayRedirect(_) => {
                    Ok(Self::Wallet(payment_method_data::WalletData::WeChatPayRedirect(
                        Box::new(payment_method_data::WeChatPayRedirection {}),
                    )))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::Mifinity(
                    mifinity_data,
                ) => Ok(Self::Wallet(payment_method_data::WalletData::Mifinity(
                    payment_method_data::MifinityData {
                        date_of_birth: Secret::<time::Date>::foreign_try_from(
                            mifinity_data
                                .date_of_birth
                                .ok_or(IntegrationError::InvalidDataFormat { field_name: "payment_method.mifinity.date_of_birth", context: IntegrationErrorContext { additional_context: Some("Missing Date of Birth".to_string()), ..Default::default() } })?
                                .expose(),
                        )?,
                        language_preference: mifinity_data.language_preference,
                    },
                ))),
                grpc_api_types::payments::payment_method::PaymentMethod::ApplePay(apple_wallet) => {
                    let payment_data = apple_wallet.payment_data.ok_or_else(|| {
                        IntegrationError::InvalidDataFormat { field_name: "payment_method.apple_pay.payment_data", context: IntegrationErrorContext { additional_context: Some("Apple Pay payment data is required".to_string()), ..Default::default() } }
                    })?;

                    let applepay_payment_data = match payment_data.payment_data {
                                Some(grpc_api_types::payments::apple_wallet::payment_data::PaymentData::EncryptedData(encrypted_data)) => {
                                    Ok(payment_method_data::ApplePayPaymentData::Encrypted(encrypted_data))
                                },
                                Some(grpc_api_types::payments::apple_wallet::payment_data::PaymentData::DecryptedData(decrypted_data)) => {
                                    let payment_data = payment_method_data::ApplePayWalletData::validate_decrypted_payment_data(
                                        decrypted_data.payment_data,
                                    )?;

                                    Ok(payment_method_data::ApplePayPaymentData::Decrypted(
                                        payment_method_data::ApplePayDecryptedData {
                                            application_primary_account_number: payment_method_data::ApplePayWalletData::validate_decrypted_primary_account_number(
                                                decrypted_data.application_primary_account_number,
                                            )?,
                                            application_expiration_month: payment_method_data::ApplePayWalletData::validate_decrypted_expiration_month(
                                                decrypted_data.application_expiration_month,
                                            )?,
                                            application_expiration_year: payment_method_data::ApplePayWalletData::validate_decrypted_expiration_year(
                                                decrypted_data.application_expiration_year,
                                            )?,
                                            payment_data,
                                        }
                                    ))
                                },
                                None => Err(report!(IntegrationError::InvalidDataFormat { field_name: "payment_method.apple_pay.payment_data.payment_data", context: IntegrationErrorContext { additional_context: Some("Apple Pay payment data is required".to_string()), ..Default::default() } }))
                            }?;

                    let payment_method = apple_wallet.payment_method.ok_or_else(|| {
                        IntegrationError::InvalidDataFormat { field_name: "payment_method.apple_pay.payment_method", context: IntegrationErrorContext { additional_context: Some("Apple Pay payment method is required".to_string()), ..Default::default() } }
                    })?;

                    let wallet_data = payment_method_data::ApplePayWalletData {
                        payment_data: applepay_payment_data,
                        payment_method: payment_method_data::ApplepayPaymentMethod {
                            display_name: payment_method.display_name,
                            network: payment_method.network,
                            pm_type: payment_method.r#type,
                        },
                        transaction_identifier: apple_wallet.transaction_identifier,
                    };
                    Ok(Self::Wallet(payment_method_data::WalletData::ApplePay(
                        wallet_data,
                    )))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::GooglePay(
                    google_wallet,
                ) => {
                    let info = google_wallet.info.ok_or_else(|| {
                        IntegrationError::InvalidDataFormat { field_name: "payment_method.google_pay.info", context: IntegrationErrorContext { additional_context: Some("Google Pay payment method info is required".to_string()), ..Default::default() } }
                    })?;

                    let tokenization_data = google_wallet.tokenization_data.ok_or_else(|| {
                        IntegrationError::InvalidDataFormat { field_name: "payment_method.google_pay.tokenization_data", context: IntegrationErrorContext { additional_context: Some("Google Pay tokenization data is required".to_string()), ..Default::default() } }
                    })?;

                    // Handle the new oneof tokenization_data structure
                    let gpay_tokenization_data = match tokenization_data.tokenization_data {
                                Some(grpc_api_types::payments::google_wallet::tokenization_data::TokenizationData::DecryptedData(decrypt_data)) => {
                                    Ok(payment_method_data::GpayTokenizationData::Decrypted(
                                        payment_method_data::GooglePayDecryptedData {
                                            card_exp_month: payment_method_data::GooglePayWalletData::validate_decrypted_card_exp_month(
                                                decrypt_data.card_exp_month,
                                            )?,
                                            card_exp_year: payment_method_data::GooglePayWalletData::validate_decrypted_card_exp_year(
                                                decrypt_data.card_exp_year,
                                            )?,
                                            application_primary_account_number: payment_method_data::GooglePayWalletData::validate_decrypted_primary_account_number(
                                                decrypt_data.application_primary_account_number,
                                            )?,
                                            cryptogram: decrypt_data.cryptogram,
                                            eci_indicator: decrypt_data.eci_indicator,
                                        }
                                    ))
                                },
                                Some(grpc_api_types::payments::google_wallet::tokenization_data::TokenizationData::EncryptedData(encrypted_data)) => {
                                    Ok(payment_method_data::GpayTokenizationData::Encrypted(
                                        payment_method_data::GpayEncryptedTokenizationData {
                                            token_type: encrypted_data.token_type,
                                            token: encrypted_data.token,
                                        }
                                    ))
                                },
                                None => Err(report!(IntegrationError::InvalidDataFormat { field_name: "payment_method.google_pay.tokenization_data.tokenization_data", context: IntegrationErrorContext { additional_context: Some("Google Pay tokenization data variant is required".to_string()), ..Default::default() } }))
                            }?;

                    let wallet_data = payment_method_data::GooglePayWalletData {
                        pm_type: google_wallet.r#type,
                        description: google_wallet.description,
                        info: payment_method_data::GooglePayPaymentMethodInfo {
                            card_network: info.card_network,
                            card_details: info.card_details,
                            assurance_details: info.assurance_details.map(|details| {
                                payment_method_data::GooglePayAssuranceDetails {
                                    card_holder_authenticated: details.card_holder_authenticated,
                                    account_verified: details.account_verified,
                                }
                            }),
                        },
                        tokenization_data: gpay_tokenization_data,
                    };
                    Ok(Self::Wallet(payment_method_data::WalletData::GooglePay(
                        wallet_data,
                    )))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::GooglePayThirdPartySdk(
                    google_pay_sdk_wallet,
                ) => Ok(Self::Wallet(
                    payment_method_data::WalletData::GooglePayThirdPartySdk(Box::new(
                        payment_method_data::GooglePayThirdPartySdkData {
                            token: google_pay_sdk_wallet.token.map(|t| Secret::new(t.expose())),
                        },
                    )),
                )),
                grpc_api_types::payments::payment_method::PaymentMethod::ApplePayThirdPartySdk(
                    apple_pay_sdk_wallet,
                ) => Ok(Self::Wallet(
                    payment_method_data::WalletData::ApplePayThirdPartySdk(Box::new(
                        payment_method_data::ApplePayThirdPartySdkData {
                            token: apple_pay_sdk_wallet.token.map(|t| Secret::new(t.expose())),
                        },
                    )),
                )),
                grpc_api_types::payments::payment_method::PaymentMethod::PaypalSdk(
                    paypal_sdk_wallet,
                ) => Ok(Self::Wallet(payment_method_data::WalletData::PaypalSdk(
                    payment_method_data::PayPalWalletData {
                        token: paypal_sdk_wallet
                            .token
                            .ok_or_else(|| {
                                IntegrationError::InvalidDataFormat { field_name: "payment_method.paypal_sdk.token", context: IntegrationErrorContext { additional_context: Some("PayPal SDK token is required".to_string()), ..Default::default() } }
                            })?
                            .expose(),
                    },
                ))),
                grpc_api_types::payments::payment_method::PaymentMethod::PaypalRedirect(
                    paypal_redirect,
                ) => Ok(Self::Wallet(
                    payment_method_data::WalletData::PaypalRedirect(
                        payment_method_data::PaypalRedirection {
                            email: match paypal_redirect.email {
                                Some(ref email_str) => Some(
                                    Email::try_from(email_str.clone().expose()).change_context(
                                        IntegrationError::InvalidDataFormat { field_name: "payment_method.paypal_redirect.email", context: IntegrationErrorContext { additional_context: Some("Invalid email".to_string()), ..Default::default() } },
                                    )?,
                                ),
                                None => None,
                            },
                        },
                    ),
                )),
                grpc_api_types::payments::payment_method::PaymentMethod::Paze(paze_wallet) => {
                    let paze_wallet_data = match paze_wallet.paze_data {
                        Some(grpc_api_types::payments::paze_wallet::PazeData::CompleteResponse(
                            complete_response,
                        )) => payment_method_data::PazeWalletData::CompleteResponse(
                            complete_response,
                        ),
                        Some(grpc_api_types::payments::paze_wallet::PazeData::DecryptedData(
                            decrypted_data,
                        )) => payment_method_data::PazeWalletData::Decrypted(Box::new(
                            router_data::PazeDecryptedData::foreign_try_from(decrypted_data)?,
                        )),
                        None => {
                            return Err(report!(IntegrationError::MissingRequiredField { field_name: "payment_method.paze.paze_data", context: IntegrationErrorContext::default() }))
                        }
                    };

                    Ok(Self::Wallet(payment_method_data::WalletData::Paze(Box::new(
                        paze_wallet_data,
                    ))))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::SamsungPay(
                    samsung_pay,
                ) => {
                let credential = samsung_pay
                    .payment_credential
                    .ok_or_else(|| {
                        IntegrationError::InvalidDataFormat { field_name: "payment_method.samsung_pay.payment_credential", context: IntegrationErrorContext { additional_context: Some("Samsung Pay payment credential is required".to_string()), ..Default::default() } }
                    })?;

                let domain_credential =
                    payment_method_data::SamsungPayWalletCredentials::foreign_try_from(
                        credential,
                    )?;

                Ok(Self::Wallet(
                    payment_method_data::WalletData::SamsungPay(Box::new(
                        payment_method_data::SamsungPayWalletData {
                            payment_credential: domain_credential,
                        },
                    ))))
                }
                // ============================================================================
                // BANK TRANSFERS - Direct variants
                // ============================================================================
                grpc_api_types::payments::payment_method::PaymentMethod::InstantBankTransfer(_) => {
                    Ok(Self::BankTransfer(Box::new(
                        payment_method_data::BankTransferData::InstantBankTransfer {},
                    )))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::AchBankTransfer(_) => {
                    Ok(Self::BankTransfer(Box::new(
                        payment_method_data::BankTransferData::AchBankTransfer {},
                    )))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::SepaBankTransfer(_) => {
                    Ok(Self::BankTransfer(Box::new(
                        payment_method_data::BankTransferData::SepaBankTransfer {},
                    )))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::BacsBankTransfer(_) => {
                    Ok(Self::BankTransfer(Box::new(
                        payment_method_data::BankTransferData::BacsBankTransfer {},
                    )))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::MultibancoBankTransfer(
                    _,
                ) => Ok(Self::BankTransfer(Box::new(
                    payment_method_data::BankTransferData::MultibancoBankTransfer {},
                ))),
                grpc_api_types::payments::payment_method::PaymentMethod::InstantBankTransferFinland(_) => {
                    Ok(Self::BankTransfer(Box::new(
                        payment_method_data::BankTransferData::InstantBankTransferFinland {},
                    )))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::InstantBankTransferPoland(_) => {
                    Ok(Self::BankTransfer(Box::new(
                        payment_method_data::BankTransferData::InstantBankTransferPoland {},
                    )))
                }
                // ============================================================================
                // ONLINE BANKING - Direct variants
                // ============================================================================
                grpc_api_types::payments::payment_method::PaymentMethod::OpenBankingUk(
                    open_banking_uk,
                ) => Ok(Self::BankRedirect(
                    payment_method_data::BankRedirectData::OpenBankingUk {
                        issuer: open_banking_uk
                            .issuer
                            .and_then(|i| common_enums::BankNames::from_str(&i).ok()),
                        country: open_banking_uk
                            .country
                            .and_then(|c| CountryAlpha2::from_str(&c).ok()),
                    },
                )),
                grpc_api_types::payments::payment_method::PaymentMethod::OpenBanking(_) => {
                    Ok(PaymentMethodData::BankRedirect(
                        payment_method_data::BankRedirectData::OpenBanking {},
                    ))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::LocalBankRedirect(_) => {
                    Ok(PaymentMethodData::BankRedirect(
                        payment_method_data::BankRedirectData::LocalBankRedirect {},
                    ))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::Bizum(_) => {
                    Ok(PaymentMethodData::BankRedirect(
                        payment_method_data::BankRedirectData::Bizum {},
                    ))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::Eft(eft) => {
                    Ok(PaymentMethodData::BankRedirect(
                        payment_method_data::BankRedirectData::Eft {
                            provider: eft.provider,
                        },
                    ))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::OnlineBankingFpx(fpx) => {
                    Ok(Self::BankRedirect(
                        payment_method_data::BankRedirectData::OnlineBankingFpx {
                            issuer: common_enums::BankNames::foreign_try_from(fpx.issuer())?,
                        },
                    ))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::Ideal(ideal) => Ok(
                    Self::BankRedirect(payment_method_data::BankRedirectData::Ideal {
                        bank_name: match ideal.bank_name() {
                            grpc_payment_types::BankNames::Unspecified => None,
                            _ => Some(common_enums::BankNames::foreign_try_from(
                                ideal.bank_name(),
                            )?),
                        },
                    }),
                ),
                grpc_api_types::payments::payment_method::PaymentMethod::Giropay(giropay) => Ok(
                    Self::BankRedirect(payment_method_data::BankRedirectData::Giropay {
                        country: match giropay.country() {
                            grpc_payment_types::CountryAlpha2::Unspecified => None,
                            _ => Some(CountryAlpha2::foreign_try_from(giropay.country())?),
                        },
                        bank_account_bic: giropay.bank_account_bic,
                        bank_account_iban: giropay.bank_account_iban,
                    }),
                ),
                grpc_api_types::payments::payment_method::PaymentMethod::Eps(eps) => Ok(
                    Self::BankRedirect(payment_method_data::BankRedirectData::Eps {
                        bank_name: match eps.bank_name() {
                            grpc_payment_types::BankNames::Unspecified => None,
                            _ => Some(common_enums::BankNames::foreign_try_from(eps.bank_name())?),
                        },
                        country: match eps.country() {
                            grpc_payment_types::CountryAlpha2::Unspecified => None,
                            _ => Some(CountryAlpha2::foreign_try_from(eps.country())?),
                        },
                    }),
                ),
                grpc_api_types::payments::payment_method::PaymentMethod::Sofort(sofort) => Ok(
                    Self::BankRedirect(payment_method_data::BankRedirectData::Sofort {
                        country: match sofort.country() {
                            grpc_payment_types::CountryAlpha2::Unspecified => None,
                            _ => Some(CountryAlpha2::foreign_try_from(sofort.country())?),
                        },
                        preferred_language: sofort.preferred_language,
                    }),
                ),
                grpc_api_types::payments::payment_method::PaymentMethod::Przelewy24(przelewy24) => {
                    Ok(Self::BankRedirect(
                        payment_method_data::BankRedirectData::Przelewy24 {
                            bank_name: match przelewy24.bank_name() {
                                grpc_payment_types::BankNames::Unspecified => None,
                                _ => Some(common_enums::BankNames::foreign_try_from(
                                    przelewy24.bank_name(),
                                )?),
                            },
                        },
                    ))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::BancontactCard(
                    bancontactcard,
                ) => Ok(Self::BankRedirect(
                    payment_method_data::BankRedirectData::BancontactCard {
                        card_number: bancontactcard.card_number,
                        card_exp_month: bancontactcard.card_exp_month,
                        card_exp_year: bancontactcard.card_exp_year,
                        card_holder_name: bancontactcard.card_holder_name,
                    },
                )),
                grpc_payment_types::payment_method::PaymentMethod::Blik(blik) => Ok(
                    Self::BankRedirect(payment_method_data::BankRedirectData::Blik {
                        blik_code: blik.blik_code,
                    }),
                ),
                grpc_payment_types::payment_method::PaymentMethod::Interac(interac) => Ok(
                    Self::BankRedirect(payment_method_data::BankRedirectData::Interac {
                        country: match interac.country() {
                            grpc_payment_types::CountryAlpha2::Unspecified => None,
                            _ => Some(CountryAlpha2::foreign_try_from(interac.country())?),
                        },
                        email: match interac.email {
                            Some(ref email_str) => Some(
                                Email::try_from(email_str.clone().expose()).change_context(
                                    IntegrationError::InvalidDataFormat { field_name: "unknown", context: IntegrationErrorContext { additional_context: Some("Invalid email for Interac".to_string()), ..Default::default() } },
                                )?,
                            ),
                            None => None,
                        },
                    }),
                ),
                grpc_payment_types::payment_method::PaymentMethod::OnlineBankingThailand(online_banking_thailand) => Ok(
                    Self::BankRedirect(payment_method_data::BankRedirectData::OnlineBankingThailand {
                        issuer: common_enums::BankNames::foreign_try_from(online_banking_thailand.issuer())?,
                    }),
                ),
                grpc_payment_types::payment_method::PaymentMethod::OnlineBankingCzechRepublic(online_banking_czech_republic) => Ok(
                    Self::BankRedirect(payment_method_data::BankRedirectData::OnlineBankingCzechRepublic {
                        issuer: common_enums::BankNames::foreign_try_from(online_banking_czech_republic.issuer())?,
                    }),
                ),
                grpc_payment_types::payment_method::PaymentMethod::OnlineBankingPoland(online_banking_poland) => Ok(
                    Self::BankRedirect(payment_method_data::BankRedirectData::OnlineBankingPoland {
                        issuer: common_enums::BankNames::foreign_try_from(online_banking_poland.issuer())?,
                    }),
                ),
                grpc_payment_types::payment_method::PaymentMethod::OnlineBankingSlovakia(online_banking_slovakia) => Ok(
                    Self::BankRedirect(payment_method_data::BankRedirectData::OnlineBankingSlovakia {
                        issuer: common_enums::BankNames::foreign_try_from(online_banking_slovakia.issuer())?,
                    }),
                ),
                grpc_payment_types::payment_method::PaymentMethod::OnlineBankingFinland(online_banking_finland) => Ok(
                    Self::BankRedirect(payment_method_data::BankRedirectData::OnlineBankingFinland {
                        email: match online_banking_finland.email {
                                Some(ref email_str) => Some(
                                    Email::try_from(email_str.clone().expose()).change_context(
                                        IntegrationError::InvalidDataFormat { field_name: "unknown", context: IntegrationErrorContext { additional_context: Some("Invalid email".to_string()), ..Default::default() } },
                                    )?,
                                ),
                                None => None,
                            },
                    }),
                ),
                // ============================================================================
                // MOBILE PAYMENTS - Direct variants
                // ============================================================================
                grpc_api_types::payments::payment_method::PaymentMethod::DuitNow(_) => {
                    Ok(Self::RealTimePayment(Box::new(
                        payment_method_data::RealTimePaymentData::DuitNow {},
                    )))
                }
                // ============================================================================
                // BUY NOW, PAY LATER - Direct variants
                // ============================================================================
                grpc_api_types::payments::payment_method::PaymentMethod::Affirm(_) => Ok(
                    Self::PayLater(payment_method_data::PayLaterData::AffirmRedirect {}),
                ),
                grpc_api_types::payments::payment_method::PaymentMethod::AfterpayClearpay(_) => Ok(
                    Self::PayLater(payment_method_data::PayLaterData::AfterpayClearpayRedirect {}),
                ),
                grpc_api_types::payments::payment_method::PaymentMethod::Klarna(_) => Ok(
                    Self::PayLater(payment_method_data::PayLaterData::KlarnaRedirect {}),
                ),
                // ============================================================================
                // DIRECT DEBIT - Direct variants
                // ============================================================================
                grpc_api_types::payments::payment_method::PaymentMethod::Ach(ach) => Ok(
                    Self::BankDebit(payment_method_data::BankDebitData::AchBankDebit {
                        bank_name: match ach.bank_name() {
                            grpc_payment_types::BankNames::Unspecified => None,
                            _ => Some(common_enums::BankNames::foreign_try_from(ach.bank_name())?),
                        },
                        bank_type: match ach.bank_type() {
                            grpc_payment_types::BankType::Unspecified => None,
                            _ => Some(common_enums::BankType::foreign_try_from(ach.bank_type())?),
                        },
                        bank_holder_type: match ach.bank_holder_type() {
                            grpc_payment_types::BankHolderType::Unspecified => None,
                            _ => Some(common_enums::BankHolderType::foreign_try_from(
                                ach.bank_holder_type(),
                            )?),
                        },
                        card_holder_name: ach.card_holder_name,
                        bank_account_holder_name: ach.bank_account_holder_name,
                        account_number: ach.account_number.ok_or(
                            IntegrationError::InvalidDataFormat { field_name: "unknown", context: IntegrationErrorContext { additional_context: Some("ACH account number is required".to_string()), ..Default::default() } },
                        )?,
                        routing_number: ach.routing_number.ok_or(
                            IntegrationError::InvalidDataFormat { field_name: "unknown", context: IntegrationErrorContext { additional_context: Some("ACH routing number is required".to_string()), ..Default::default() } },
                        )?,
                    }),
                ),
                grpc_api_types::payments::payment_method::PaymentMethod::Sepa(sepa) => Ok(
                    Self::BankDebit(payment_method_data::BankDebitData::SepaBankDebit {
                        iban: sepa
                            .iban
                            .ok_or(IntegrationError::InvalidDataFormat { field_name: "unknown", context: IntegrationErrorContext { additional_context: Some("SEPA IBAN is required".to_string()), ..Default::default() } })?,
                        bank_account_holder_name: sepa.bank_account_holder_name,
                    }),
                ),
                grpc_api_types::payments::payment_method::PaymentMethod::Bacs(bacs) => Ok(
                    Self::BankDebit(payment_method_data::BankDebitData::BacsBankDebit {
                        account_number: bacs.account_number.ok_or(
                            IntegrationError::InvalidDataFormat { field_name: "unknown", context: IntegrationErrorContext { additional_context: Some("BACS account number is required".to_string()), ..Default::default() } },
                        )?,
                        sort_code: bacs.sort_code.ok_or(
                            IntegrationError::MissingRequiredField {
                                field_name: "payment_method.bacs.sort_code",
                                context: IntegrationErrorContext::default(),
                            },
                        )?,
                        bank_account_holder_name: bacs.bank_account_holder_name,
                    }),
                ),
                grpc_api_types::payments::payment_method::PaymentMethod::Becs(becs) => Ok(
                    Self::BankDebit(payment_method_data::BankDebitData::BecsBankDebit {
                        account_number: becs.account_number.ok_or(
                            IntegrationError::InvalidDataFormat { field_name: "unknown", context: IntegrationErrorContext { additional_context: Some("BECS account number is required".to_string()), ..Default::default() } },
                        )?,
                        bsb_number: becs.bsb_number.ok_or(
                            IntegrationError::MissingRequiredField {
                                field_name: "payment_method.becs.bsb_number",
                                context: IntegrationErrorContext::default(),
                            },
                        )?,
                        bank_account_holder_name: becs.bank_account_holder_name,
                    }),
                ),
                grpc_api_types::payments::payment_method::PaymentMethod::SepaGuaranteedDebit(sepa_guaranteed_bank_debit) => Ok(
                    Self::BankDebit(payment_method_data::BankDebitData::SepaGuaranteedBankDebit {
                        iban: sepa_guaranteed_bank_debit
                            .iban
                            .ok_or(IntegrationError::InvalidDataFormat { field_name: "unknown", context: IntegrationErrorContext { additional_context: Some("SEPA guaranteed IBAN is required".to_string()), ..Default::default() } })?,
                        bank_account_holder_name: sepa_guaranteed_bank_debit.bank_account_holder_name,
                    }),
                ),
                // ============================================================================
                // CRYPTOCURRENCY - Direct variant
                // ============================================================================
                grpc_api_types::payments::payment_method::PaymentMethod::Crypto(
                    crypto_currency,
                ) => Ok(Self::Crypto(payment_method_data::CryptoData {
                    pay_currency: crypto_currency.pay_currency,
                    network: crypto_currency.network,
                })),

                // ============================================================================
                // NETWORK TRANSACTION METHODS - New variants for recurring payments
                // ============================================================================
                grpc_api_types::payments::payment_method::PaymentMethod::CardDetailsForNetworkTransactionId(
                    card_details_for_nti,
                ) => {
                    let card_number = card_details_for_nti.card_number
                        .ok_or_else(|| IntegrationError::InvalidDataFormat { field_name: "unknown", context: IntegrationErrorContext { additional_context: Some("Missing card number for network transaction ID".to_string()), ..Default::default() } })?;

                    Ok(Self::CardDetailsForNetworkTransactionId(
                        payment_method_data::CardDetailsForNetworkTransactionId {
                            card_number,
                            card_exp_month: card_details_for_nti.card_exp_month.ok_or_else(|| IntegrationError::InvalidDataFormat { field_name: "unknown", context: IntegrationErrorContext { additional_context: Some("Missing card expiration month".to_string()), ..Default::default() } })?,
                            card_exp_year: card_details_for_nti.card_exp_year.ok_or_else(|| IntegrationError::InvalidDataFormat { field_name: "unknown", context: IntegrationErrorContext { additional_context: Some("Missing card expiration year".to_string()), ..Default::default() } })?,
                            card_issuer: card_details_for_nti.card_issuer,
                            card_network: card_details_for_nti
                                .card_network
                                .and_then(|network_i32| grpc_payment_types::CardNetwork::try_from(network_i32).ok())
                                .and_then(|network| CardNetwork::foreign_try_from(network).ok()),
                            card_type: card_details_for_nti.card_type,
                            card_issuing_country: card_details_for_nti.card_issuing_country,
                            bank_code: card_details_for_nti.bank_code,
                            nick_name: card_details_for_nti.nick_name,
                            card_holder_name: card_details_for_nti.card_holder_name,
                        },
                    ))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::NetworkToken(
                    network_token_data,
                ) => {
                    let token_number = network_token_data.token_number
                        .ok_or_else(|| IntegrationError::InvalidDataFormat { field_name: "unknown", context: IntegrationErrorContext { additional_context: Some("Missing network token".to_string()), ..Default::default() } })?;

                    Ok(Self::NetworkToken(payment_method_data::NetworkTokenData {
                        token_number,
                        token_exp_month: network_token_data.token_exp_month.ok_or_else(|| IntegrationError::InvalidDataFormat { field_name: "unknown", context: IntegrationErrorContext { additional_context: Some("Missing token expiration month".to_string()), ..Default::default() } })?,
                        token_exp_year: network_token_data.token_exp_year.ok_or_else(|| IntegrationError::InvalidDataFormat { field_name: "unknown", context: IntegrationErrorContext { additional_context: Some("Missing token expiration year".to_string()), ..Default::default() } })?,
                        token_cryptogram: network_token_data.token_cryptogram,
                        card_issuer: network_token_data.card_issuer,
                        card_network: network_token_data
                            .card_network
                            .and_then(|network_i32| grpc_payment_types::CardNetwork::try_from(network_i32).ok())
                            .and_then(|network| CardNetwork::foreign_try_from(network).ok()),
                        card_type: network_token_data
                            .card_type,
                        card_issuing_country: network_token_data
                            .card_issuing_country,
                        bank_code: network_token_data.bank_code,
                        nick_name: network_token_data.nick_name,
                        eci: network_token_data.eci,
                    }))
                }

                grpc_payment_types::payment_method::PaymentMethod::DecryptedWalletTokenDetailsForNetworkTransactionId(
                    decrypted_wallet_token_details_for_nti,
                ) => {
                    let decrypted_token = decrypted_wallet_token_details_for_nti.decrypted_token
                        .ok_or_else(|| IntegrationError::InvalidDataFormat { field_name: "unknown", context: IntegrationErrorContext { additional_context: Some("Missing decrypted wallet token".to_string()), ..Default::default() } })?;

                    Ok(Self::DecryptedWalletTokenDetailsForNetworkTransactionId(
                        payment_method_data::DecryptedWalletTokenDetailsForNetworkTransactionId {
                            decrypted_token,
                            token_exp_month: decrypted_wallet_token_details_for_nti.token_exp_month.ok_or_else(|| IntegrationError::InvalidDataFormat { field_name: "unknown", context: IntegrationErrorContext { additional_context: Some("Missing decrypted token expiration month".to_string()), ..Default::default() } })?,
                            token_exp_year: decrypted_wallet_token_details_for_nti.token_exp_year.ok_or_else(|| IntegrationError::InvalidDataFormat { field_name: "unknown", context: IntegrationErrorContext { additional_context: Some("Missing decrypted token expiration year".to_string()), ..Default::default() } })?,
                            card_holder_name: decrypted_wallet_token_details_for_nti.card_holder_name,
                            eci: decrypted_wallet_token_details_for_nti.eci,
                            token_source: decrypted_wallet_token_details_for_nti.token_source
                                .and_then(|source_i32| grpc_api_types::payments::TokenSource::try_from(source_i32).ok())
                                .and_then(|source| payment_method_data::TokenSource::foreign_try_from(source).ok()),

                        },
                    ))
                }

                // ============================================================================
                // BANK REDIRECT - Trustly
                // ============================================================================
                grpc_api_types::payments::payment_method::PaymentMethod::Trustly(trustly_data) => {
                    let country = match trustly_data.country() {
                        grpc_payment_types::CountryAlpha2::Unspecified => None,
                        country_code => Some(CountryAlpha2::foreign_try_from(country_code)?),
                    };
                    Ok(Self::BankRedirect(
                        payment_method_data::BankRedirectData::Trustly { country },
                    ))
                }

                // ============================================================================
                // INDONESIAN BANK TRANSFERS - Doku Integration
                // ============================================================================
                grpc_api_types::payments::payment_method::PaymentMethod::Pix(pix_data) => {
                    // Parse expiry_date from ISO 8601 string if provided
                    let expiry_date = pix_data
                        .expiry_date
                        .as_ref()
                        .and_then(|date_str| {
                            time::PrimitiveDateTime::parse(
                                date_str,
                                &time::format_description::well_known::Iso8601::DEFAULT,
                            )
                            .ok()
                        });

                    Ok(Self::BankTransfer(Box::new(
                        payment_method_data::BankTransferData::Pix {
                            pix_key: pix_data.pix_key,
                            cpf: pix_data.cpf,
                            cnpj: pix_data.cnpj,
                            source_bank_account_id: None,
                            destination_bank_account_id: None,
                            expiry_date,
                        },
                    )))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::PermataBankTransfer(_) => {
                    Ok(Self::BankTransfer(Box::new(
                        payment_method_data::BankTransferData::PermataBankTransfer {},
                    )))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::BcaBankTransfer(_) => {
                    Ok(Self::BankTransfer(Box::new(
                        payment_method_data::BankTransferData::BcaBankTransfer {},
                    )))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::BniVaBankTransfer(_) => {
                    Ok(Self::BankTransfer(Box::new(
                        payment_method_data::BankTransferData::BniVaBankTransfer {},
                    )))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::BriVaBankTransfer(_) => {
                    Ok(Self::BankTransfer(Box::new(
                        payment_method_data::BankTransferData::BriVaBankTransfer {},
                    )))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::CimbVaBankTransfer(_) => {
                    Ok(Self::BankTransfer(Box::new(
                        payment_method_data::BankTransferData::CimbVaBankTransfer {},
                    )))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::DanamonVaBankTransfer(_) => {
                    Ok(Self::BankTransfer(Box::new(
                        payment_method_data::BankTransferData::DanamonVaBankTransfer {},
                    )))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::MandiriVaBankTransfer(_) => {
                    Ok(Self::BankTransfer(Box::new(
                        payment_method_data::BankTransferData::MandiriVaBankTransfer {},
                    )))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::Pse(_) => {
                    Ok(Self::BankTransfer(Box::new(
                        payment_method_data::BankTransferData::Pse {},
                    )))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::LocalBankTransfer(local_bank_transfer) => {
                    Ok(Self::BankTransfer(Box::new(
                        payment_method_data::BankTransferData::LocalBankTransfer {
                            bank_code: local_bank_transfer.bank_code,
                        },
                    )))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::IndonesianBankTransfer(indonesian_bank_transfer) => {
                    Ok(Self::BankTransfer(Box::new(
                        payment_method_data::BankTransferData::IndonesianBankTransfer {
                            bank_name: match indonesian_bank_transfer.bank_name() {
                                grpc_payment_types::BankNames::Unspecified => None,
                                _ => Some(common_enums::BankNames::foreign_try_from(
                                    indonesian_bank_transfer.bank_name(),
                                )?),
                            },
                        },
                    )))
                }

                grpc_api_types::payments::payment_method::PaymentMethod::Givex(givex_data) => {
                    Ok(Self::GiftCard(Box::new(
                        payment_method_data::GiftCardData::Givex(payment_method_data::GiftCardDetails {
                            number: givex_data.number.ok_or_else(|| IntegrationError::InvalidDataFormat { field_name: "unknown", context: IntegrationErrorContext { additional_context: Some("Missing Givex gift card number".to_string()), ..Default::default() } })?,
                            cvc: givex_data.cvc.ok_or_else(|| IntegrationError::InvalidDataFormat { field_name: "unknown", context: IntegrationErrorContext { additional_context: Some("Missing Givex gift card CVC".to_string()), ..Default::default() } })?,
                        }),
                    )))
                }

                grpc_api_types::payments::payment_method::PaymentMethod::PaySafeCard(_) => {
                    Ok(Self::GiftCard(Box::new(
                        payment_method_data::GiftCardData::PaySafeCard {},
                    )))
                }

                // ============================================================================
                // VOUCHER PAYMENT METHODS
                // ============================================================================
                grpc_api_types::payments::payment_method::PaymentMethod::Boleto(boleto) => {
                    Ok(Self::Voucher(payment_method_data::VoucherData::Boleto(Box::new(
                        payment_method_data::BoletoVoucherData {
                            social_security_number: boleto.social_security_number.map(Secret::new),
                        },
                    ))))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::Efecty(_) => {
                    Ok(Self::Voucher(payment_method_data::VoucherData::Efecty))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::PagoEfectivo(_) => {
                    Ok(Self::Voucher(payment_method_data::VoucherData::PagoEfectivo))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::RedCompra(_) => {
                    Ok(Self::Voucher(payment_method_data::VoucherData::RedCompra))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::RedPagos(_) => {
                    Ok(Self::Voucher(payment_method_data::VoucherData::RedPagos))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::Alfamart(_) => {
                    Ok(Self::Voucher(payment_method_data::VoucherData::Alfamart(Box::new(
                        payment_method_data::AlfamartVoucherData {},
                    ))))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::Indomaret(_) => {
                    Ok(Self::Voucher(payment_method_data::VoucherData::Indomaret(Box::new(
                        payment_method_data::IndomaretVoucherData {},
                    ))))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::Oxxo(_) => {
                    Ok(Self::Voucher(payment_method_data::VoucherData::Oxxo))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::SevenEleven(_) => {
                    Ok(Self::Voucher(payment_method_data::VoucherData::SevenEleven(Box::new(
                        payment_method_data::JCSVoucherData {},
                    ))))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::Lawson(_) => {
                    Ok(Self::Voucher(payment_method_data::VoucherData::Lawson(Box::new(
                        payment_method_data::JCSVoucherData {},
                    ))))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::MiniStop(_) => {
                    Ok(Self::Voucher(payment_method_data::VoucherData::MiniStop(Box::new(
                        payment_method_data::JCSVoucherData {},
                    ))))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::FamilyMart(_) => {
                    Ok(Self::Voucher(payment_method_data::VoucherData::FamilyMart(Box::new(
                        payment_method_data::JCSVoucherData {},
                    ))))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::Seicomart(_) => {
                    Ok(Self::Voucher(payment_method_data::VoucherData::Seicomart(Box::new(
                        payment_method_data::JCSVoucherData {},
                    ))))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::PayEasy(_) => {
                    Ok(Self::Voucher(payment_method_data::VoucherData::PayEasy(Box::new(
                        payment_method_data::JCSVoucherData {},
                    ))))
                }

                grpc_api_types::payments::payment_method::PaymentMethod::Netbanking(nb) => {
                    let grpc_bank = grpc_api_types::payments::BankNames::try_from(nb.issuer)
                        .unwrap_or_default();
                    let issuer = common_enums::BankNames::foreign_try_from(grpc_bank)?;
                    Ok(Self::BankRedirect(
                        crate::payment_method_data::BankRedirectData::Netbanking { issuer },
                    ))
                }

                _ => Err(report!(IntegrationError::InvalidDataFormat { field_name: "unknown", context: IntegrationErrorContext { additional_context: Some("This payment method type is not yet supported".to_string()), ..Default::default() } })),
            },
            None => Err(IntegrationError::InvalidDataFormat { field_name: "unknown", context: IntegrationErrorContext { additional_context: Some("Payment method data is required".to_string()), ..Default::default() } }
            .into()),
        }
    }
}

impl ForeignTryFrom<grpc_api_types::payments::TokenSource> for payment_method_data::TokenSource {
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::payments::TokenSource,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        match value {
            grpc_api_types::payments::TokenSource::Googlepay => Ok(Self::GooglePay),
            grpc_api_types::payments::TokenSource::Applepay => Ok(Self::ApplePay),
            grpc_api_types::payments::TokenSource::Unspecified => {
                Err(report!(IntegrationError::InvalidDataFormat {
                    field_name: "unknown",
                    context: IntegrationErrorContext {
                        additional_context: Some("Token source is required".to_string()),
                        ..Default::default()
                    }
                }))
            }
        }
    }
}

impl ForeignTryFrom<grpc_api_types::payments::BankType> for common_enums::BankType {
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::payments::BankType,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        match value {
            grpc_api_types::payments::BankType::Checking => Ok(common_enums::BankType::Checking),
            grpc_api_types::payments::BankType::Savings => Ok(common_enums::BankType::Savings),
            grpc_api_types::payments::BankType::Unspecified => {
                Err(IntegrationError::InvalidDataFormat {
                    field_name: "unknown",
                    context: IntegrationErrorContext {
                        additional_context: Some("Invalid bank type".to_string()),
                        ..Default::default()
                    },
                })?
            }
        }
    }
}

impl ForeignTryFrom<grpc_api_types::payments::BankHolderType> for common_enums::BankHolderType {
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::payments::BankHolderType,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        match value {
            grpc_api_types::payments::BankHolderType::Personal => {
                Ok(common_enums::BankHolderType::Personal)
            }
            grpc_api_types::payments::BankHolderType::Business => {
                Ok(common_enums::BankHolderType::Business)
            }
            grpc_api_types::payments::BankHolderType::Unspecified => {
                Err(IntegrationError::InvalidDataFormat {
                    field_name: "unknown",
                    context: IntegrationErrorContext {
                        additional_context: Some("Invalid bank holder type".to_string()),
                        ..Default::default()
                    },
                })?
            }
        }
    }
}

impl ForeignTryFrom<grpc_api_types::payments::PaymentMethodType> for Option<PaymentMethodType> {
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::payments::PaymentMethodType,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        match value {
            grpc_api_types::payments::PaymentMethodType::Unspecified => Ok(None),
            grpc_api_types::payments::PaymentMethodType::Credit => {
                Ok(Some(PaymentMethodType::Card))
            }
            grpc_api_types::payments::PaymentMethodType::Debit => Ok(Some(PaymentMethodType::Card)),
            grpc_api_types::payments::PaymentMethodType::UpiCollect => {
                Ok(Some(PaymentMethodType::UpiCollect))
            }
            grpc_api_types::payments::PaymentMethodType::UpiIntent => {
                Ok(Some(PaymentMethodType::UpiIntent))
            }
            grpc_api_types::payments::PaymentMethodType::UpiQr => {
                Ok(Some(PaymentMethodType::UpiIntent))
            } // UpiQr not yet implemented, fallback to UpiIntent
            grpc_api_types::payments::PaymentMethodType::ClassicReward => {
                Ok(Some(PaymentMethodType::ClassicReward))
            }
            grpc_api_types::payments::PaymentMethodType::Evoucher => {
                Ok(Some(PaymentMethodType::Evoucher))
            }
            grpc_api_types::payments::PaymentMethodType::ApplePay => {
                Ok(Some(PaymentMethodType::ApplePay))
            }
            grpc_api_types::payments::PaymentMethodType::GooglePay => {
                Ok(Some(PaymentMethodType::GooglePay))
            }
            grpc_api_types::payments::PaymentMethodType::AmazonPay => {
                Ok(Some(PaymentMethodType::AmazonPay))
            }
            grpc_api_types::payments::PaymentMethodType::RevolutPay => {
                Ok(Some(PaymentMethodType::RevolutPay))
            }
            grpc_api_types::payments::PaymentMethodType::Paze => Ok(Some(PaymentMethodType::Paze)),
            grpc_api_types::payments::PaymentMethodType::PayPal => {
                Ok(Some(PaymentMethodType::Paypal))
            }
            grpc_api_types::payments::PaymentMethodType::WeChatPay => {
                Ok(Some(PaymentMethodType::WeChatPay))
            }
            grpc_api_types::payments::PaymentMethodType::AliPay => {
                Ok(Some(PaymentMethodType::AliPay))
            }
            grpc_api_types::payments::PaymentMethodType::Cashapp => {
                Ok(Some(PaymentMethodType::Cashapp))
            }
            grpc_api_types::payments::PaymentMethodType::SepaBankTransfer => {
                Ok(Some(PaymentMethodType::SepaBankTransfer))
            }
            grpc_api_types::payments::PaymentMethodType::InstantBankTransfer => {
                Ok(Some(PaymentMethodType::InstantBankTransfer))
            }
            grpc_api_types::payments::PaymentMethodType::InstantBankTransferFinland => {
                Ok(Some(PaymentMethodType::InstantBankTransferFinland))
            }
            grpc_api_types::payments::PaymentMethodType::InstantBankTransferPoland => {
                Ok(Some(PaymentMethodType::InstantBankTransferPoland))
            }
            grpc_api_types::payments::PaymentMethodType::NetworkToken => {
                Ok(Some(PaymentMethodType::NetworkToken))
            }
            grpc_api_types::payments::PaymentMethodType::MbWay => {
                Ok(Some(PaymentMethodType::MbWay))
            }
            grpc_api_types::payments::PaymentMethodType::Satispay => {
                Ok(Some(PaymentMethodType::Satispay))
            }
            grpc_api_types::payments::PaymentMethodType::Wero => Ok(Some(PaymentMethodType::Wero)),
            grpc_api_types::payments::PaymentMethodType::Netbanking => {
                Ok(Some(PaymentMethodType::Netbanking))
            }
            _ => Err(IntegrationError::InvalidDataFormat {
                field_name: "unknown",
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "This payment method type is not yet supported".to_string(),
                    ),
                    ..Default::default()
                },
            }
            .into()),
        }
    }
}

impl ForeignTryFrom<grpc_api_types::payments::PaymentMethod> for Option<PaymentMethodType> {
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::payments::PaymentMethod,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        match value.payment_method {
            Some(data) => match data {
                // ============================================================================
                // CARD METHODS
                // ============================================================================
                grpc_api_types::payments::payment_method::PaymentMethod::Card(_) => {
                    Ok(Some(PaymentMethodType::Card))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::CardProxy(_) => {
                    Ok(Some(PaymentMethodType::Card))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::CardRedirect(card_redirect) => {
                match card_redirect.r#type() {
                        grpc_api_types::payments::card_redirect::CardRedirectType::Knet => {
                            Ok(Some(PaymentMethodType::Knet))
                        }
                        grpc_api_types::payments::card_redirect::CardRedirectType::Benefit => {
                            Ok(Some(PaymentMethodType::Benefit))
                        }
                        grpc_api_types::payments::card_redirect::CardRedirectType::MomoAtm => {
                            Ok(Some(PaymentMethodType::MomoAtm))
                        }
                        grpc_api_types::payments::card_redirect::CardRedirectType::CardRedirect => {
                            Ok(Some(PaymentMethodType::CardRedirect))
                        }
                        grpc_api_types::payments::card_redirect::CardRedirectType::Unspecified => {
                            Err(report!(IntegrationError::InvalidDataFormat { field_name: "unknown", context: IntegrationErrorContext { additional_context: Some("Card redirect type cannot be unspecified".to_string()), ..Default::default() } }))
                        }
                    }
                }
                grpc_api_types::payments::payment_method::PaymentMethod::Token(_) => {
                    Ok(None)
                },
                grpc_api_types::payments::payment_method::PaymentMethod::UpiCollect(_) => Ok(Some(PaymentMethodType::UpiCollect)),
                grpc_api_types::payments::payment_method::PaymentMethod::UpiIntent(_) => Ok(Some(PaymentMethodType::UpiIntent)),
                grpc_api_types::payments::payment_method::PaymentMethod::UpiQr(_) => Ok(Some(PaymentMethodType::UpiIntent)), // UpiQr not yet implemented, fallback to UpiIntent
                // ============================================================================
                // REWARD METHODS - Flattened direct variants
                // ============================================================================
                grpc_api_types::payments::payment_method::PaymentMethod::ClassicReward(_) => {
                    Ok(Some(PaymentMethodType::ClassicReward))
                },
                grpc_api_types::payments::payment_method::PaymentMethod::EVoucher(_) => {
                    Ok(Some(PaymentMethodType::Evoucher))
                },
                // ============================================================================
                // DIGITAL WALLETS - PaymentMethodType mappings
                // ============================================================================
                grpc_api_types::payments::payment_method::PaymentMethod::ApplePay(_) => Ok(Some(PaymentMethodType::ApplePay)),
                grpc_api_types::payments::payment_method::PaymentMethod::GooglePay(_) => Ok(Some(PaymentMethodType::GooglePay)),
                grpc_api_types::payments::payment_method::PaymentMethod::ApplePayThirdPartySdk(_) => Ok(Some(PaymentMethodType::ApplePay)),
                grpc_api_types::payments::payment_method::PaymentMethod::GooglePayThirdPartySdk(_) => Ok(Some(PaymentMethodType::GooglePay)),
                grpc_api_types::payments::payment_method::PaymentMethod::PaypalSdk(_) => Ok(Some(PaymentMethodType::Paypal)),
                grpc_api_types::payments::payment_method::PaymentMethod::AmazonPayRedirect(_) => Ok(Some(PaymentMethodType::AmazonPay)),
                grpc_api_types::payments::payment_method::PaymentMethod::CashappQr(_) => Ok(Some(PaymentMethodType::Cashapp)),
                grpc_api_types::payments::payment_method::PaymentMethod::PaypalRedirect(_) => Ok(Some(PaymentMethodType::Paypal)),
                grpc_api_types::payments::payment_method::PaymentMethod::WeChatPayQr(_) => Ok(Some(PaymentMethodType::WeChatPay)),
                grpc_api_types::payments::payment_method::PaymentMethod::WeChatPayRedirect(_) => Ok(Some(PaymentMethodType::WeChatPay)),
                grpc_api_types::payments::payment_method::PaymentMethod::AliPayRedirect(_) => Ok(Some(PaymentMethodType::AliPay)),
                grpc_api_types::payments::payment_method::PaymentMethod::RevolutPay(_) => Ok(Some(PaymentMethodType::RevolutPay)),
                grpc_api_types::payments::payment_method::PaymentMethod::Mifinity(_) => Ok(Some(PaymentMethodType::Mifinity)),
                grpc_api_types::payments::payment_method::PaymentMethod::Bluecode(_) => Ok(Some(PaymentMethodType::Bluecode)),
                grpc_api_types::payments::payment_method::PaymentMethod::Paze(_) => Ok(Some(PaymentMethodType::Paze)),
                grpc_api_types::payments::payment_method::PaymentMethod::AliPayHk(_) => Ok(Some(PaymentMethodType::AliPayHk)),
                grpc_api_types::payments::payment_method::PaymentMethod::DanaRedirect(_) => Ok(Some(PaymentMethodType::Dana)),
                grpc_api_types::payments::payment_method::PaymentMethod::GcashRedirect(_) => Ok(Some(PaymentMethodType::Gcash)),
                grpc_api_types::payments::payment_method::PaymentMethod::GoPayRedirect(_) => Ok(Some(PaymentMethodType::GoPay)),
                grpc_api_types::payments::payment_method::PaymentMethod::KakaoPayRedirect(_) => Ok(Some(PaymentMethodType::KakaoPay)),
                grpc_api_types::payments::payment_method::PaymentMethod::MbWayRedirect(_) => Ok(Some(PaymentMethodType::MbWay)),
                grpc_api_types::payments::payment_method::PaymentMethod::MomoRedirect(_) => Ok(Some(PaymentMethodType::Momo)),
                grpc_api_types::payments::payment_method::PaymentMethod::TouchNGoRedirect(_) => Ok(Some(PaymentMethodType::TouchNGo)),
                grpc_api_types::payments::payment_method::PaymentMethod::TwintRedirect(_) => Ok(Some(PaymentMethodType::Twint)),
                grpc_api_types::payments::payment_method::PaymentMethod::VippsRedirect(_) => Ok(Some(PaymentMethodType::Vipps)),
                grpc_api_types::payments::payment_method::PaymentMethod::SwishQr(_) => Ok(Some(PaymentMethodType::Swish)),
                grpc_api_types::payments::payment_method::PaymentMethod::SamsungPay(_) => Ok(Some(PaymentMethodType::SamsungPay)),
                grpc_api_types::payments::payment_method::PaymentMethod::MbWay(_) => Ok(Some(PaymentMethodType::MbWay)),
                grpc_api_types::payments::payment_method::PaymentMethod::Satispay(_) => Ok(Some(PaymentMethodType::Satispay)),
                grpc_api_types::payments::payment_method::PaymentMethod::Wero(_) => Ok(Some(PaymentMethodType::Wero)),
                // ============================================================================
                // BANK TRANSFERS - PaymentMethodType mappings
                // ============================================================================
                grpc_api_types::payments::payment_method::PaymentMethod::InstantBankTransfer(_) => Ok(Some(PaymentMethodType::InstantBankTransfer)),
                grpc_api_types::payments::payment_method::PaymentMethod::AchBankTransfer(_) => Ok(Some(PaymentMethodType::Ach)),
                grpc_api_types::payments::payment_method::PaymentMethod::SepaBankTransfer(_) => Ok(Some(PaymentMethodType::SepaBankTransfer)),
                grpc_api_types::payments::payment_method::PaymentMethod::BacsBankTransfer(_) => Ok(Some(PaymentMethodType::Bacs)),
                grpc_api_types::payments::payment_method::PaymentMethod::MultibancoBankTransfer(_) => Ok(Some(PaymentMethodType::Multibanco)),
                grpc_api_types::payments::payment_method::PaymentMethod::InstantBankTransferFinland(_) => Ok(Some(PaymentMethodType::InstantBankTransferFinland)),
                grpc_api_types::payments::payment_method::PaymentMethod::InstantBankTransferPoland(_) => Ok(Some(PaymentMethodType::InstantBankTransferPoland)),
                grpc_api_types::payments::payment_method::PaymentMethod::LocalBankTransfer(_) => Ok(Some(PaymentMethodType::LocalBankTransfer)),
                grpc_api_types::payments::payment_method::PaymentMethod::IndonesianBankTransfer(_) => Ok(Some(PaymentMethodType::IndonesianBankTransfer)),
                // ============================================================================
                // ONLINE BANKING - PaymentMethodType mappings
                // ============================================================================
                grpc_api_types::payments::payment_method::PaymentMethod::OpenBankingUk(_) => Ok(Some(PaymentMethodType::OpenBankingUk)),
                grpc_api_types::payments::payment_method::PaymentMethod::OpenBanking(_) => Ok(Some(PaymentMethodType::OpenBanking)),
                grpc_api_types::payments::payment_method::PaymentMethod::OnlineBankingFpx(_) => Ok(Some(PaymentMethodType::OnlineBankingFpx)),
                grpc_api_types::payments::payment_method::PaymentMethod::Ideal(_) => Ok(Some(PaymentMethodType::Ideal)),
                grpc_api_types::payments::payment_method::PaymentMethod::Giropay(_) => Ok(Some(PaymentMethodType::Giropay)),
                grpc_api_types::payments::payment_method::PaymentMethod::Eps(_) => Ok(Some(PaymentMethodType::Eps)),
                grpc_api_types::payments::payment_method::PaymentMethod::Przelewy24(_) => Ok(Some(PaymentMethodType::Przelewy24)),
                grpc_api_types::payments::payment_method::PaymentMethod::BancontactCard(_) => Ok(Some(PaymentMethodType::BancontactCard)),
                grpc_api_types::payments::payment_method::PaymentMethod::Blik(_) => Ok(Some(PaymentMethodType::Blik)),
                grpc_api_types::payments::payment_method::PaymentMethod::Sofort(_) => Ok(Some(PaymentMethodType::Sofort)),
                grpc_api_types::payments::payment_method::PaymentMethod::Bizum(_) => Ok(Some(PaymentMethodType::Bizum)),
                grpc_api_types::payments::payment_method::PaymentMethod::Eft(_) => Ok(Some(PaymentMethodType::Eft)),
                // ============================================================================
                // MOBILE & CRYPTO PAYMENTS - PaymentMethodType mappings
                // ============================================================================
                grpc_api_types::payments::payment_method::PaymentMethod::DuitNow(_) => Ok(Some(PaymentMethodType::DuitNow)),
                grpc_api_types::payments::payment_method::PaymentMethod::Crypto(_) => Ok(Some(PaymentMethodType::CryptoCurrency)),
                // ============================================================================
                // BUY NOW, PAY LATER - PaymentMethodType mappings
                // ============================================================================
                grpc_api_types::payments::payment_method::PaymentMethod::Affirm(_) => Ok(Some(PaymentMethodType::Affirm)),
                grpc_api_types::payments::payment_method::PaymentMethod::AfterpayClearpay(_) => Ok(Some(PaymentMethodType::AfterpayClearpay)),
                grpc_api_types::payments::payment_method::PaymentMethod::Klarna(_) => Ok(Some(PaymentMethodType::Klarna)),
                // ============================================================================
                // DIRECT DEBIT - PaymentMethodType mappings
                // ============================================================================
                grpc_api_types::payments::payment_method::PaymentMethod::Ach(_) => Ok(Some(PaymentMethodType::Ach)),
                grpc_api_types::payments::payment_method::PaymentMethod::Sepa(_) => Ok(Some(PaymentMethodType::Sepa)),
                grpc_api_types::payments::payment_method::PaymentMethod::Bacs(_) => Ok(Some(PaymentMethodType::Bacs)),
                grpc_api_types::payments::payment_method::PaymentMethod::Becs(_) => Ok(Some(PaymentMethodType::Becs)),
                grpc_api_types::payments::payment_method::PaymentMethod::SepaGuaranteedDebit(_) => Ok(Some(PaymentMethodType::SepaGuaranteedDebit)),
                // ============================================================================
                // NETWORK TRANSACTION METHODS - recurring payments
                // ============================================================================
                grpc_api_types::payments::payment_method::PaymentMethod::CardDetailsForNetworkTransactionId(_) => Ok(Some(PaymentMethodType::Card)),
                grpc_api_types::payments::payment_method::PaymentMethod::NetworkToken(_) => Ok(Some(PaymentMethodType::Card)),
                grpc_payment_types::payment_method::PaymentMethod::DecryptedWalletTokenDetailsForNetworkTransactionId(_) => Ok(Some(PaymentMethodType::NetworkToken)),
                // ============================================================================
                // GIFT CARDS
                // ============================================================================
                grpc_api_types::payments::payment_method::PaymentMethod::Givex(_) => {
                    Ok(Some(PaymentMethodType::Givex))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::PaySafeCard(_) => {
                    Ok(Some(PaymentMethodType::PaySafeCard))
                }
                // ============================================================================
                // VOUCHER PAYMENT METHODS
                // ============================================================================
                grpc_api_types::payments::payment_method::PaymentMethod::Boleto(_) => {
                    Ok(Some(PaymentMethodType::Boleto))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::Efecty(_) => {
                    Ok(Some(PaymentMethodType::Efecty))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::PagoEfectivo(_) => {
                    Ok(Some(PaymentMethodType::PagoEfectivo))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::RedCompra(_) => {
                    Ok(Some(PaymentMethodType::RedCompra))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::RedPagos(_) => {
                    Ok(Some(PaymentMethodType::RedPagos))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::Alfamart(_) => {
                    Ok(Some(PaymentMethodType::Alfamart))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::Indomaret(_) => {
                    Ok(Some(PaymentMethodType::Indomaret))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::Oxxo(_) => {
                    Ok(Some(PaymentMethodType::Oxxo))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::SevenEleven(_) => {
                    Ok(Some(PaymentMethodType::SevenEleven))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::Lawson(_) => {
                    Ok(Some(PaymentMethodType::Lawson))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::MiniStop(_) => {
                    Ok(Some(PaymentMethodType::MiniStop))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::FamilyMart(_) => {
                    Ok(Some(PaymentMethodType::FamilyMart))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::Seicomart(_) => {
                    Ok(Some(PaymentMethodType::Seicomart))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::PayEasy(_) => {
                    Ok(Some(PaymentMethodType::PayEasy))
                }
                // ============================================================================
                // ONLINE BANKING PAYMENT METHODS
                // ============================================================================
                grpc_api_types::payments::payment_method::PaymentMethod::OnlineBankingThailand(_) => {
                    Ok(Some(PaymentMethodType::OnlineBankingThailand))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::OnlineBankingCzechRepublic(_) => {
                    Ok(Some(PaymentMethodType::OnlineBankingCzechRepublic))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::OnlineBankingFinland(_) => {
                    Ok(Some(PaymentMethodType::OnlineBankingFinland))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::OnlineBankingPoland(_) => {
                    Ok(Some(PaymentMethodType::OnlineBankingPoland))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::OnlineBankingSlovakia(_) => {
                    Ok(Some(PaymentMethodType::OnlineBankingSlovakia))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::OpenBankingPis(_) => {
                    Ok(Some(PaymentMethodType::OpenBankingPIS))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::LocalBankRedirect(_) => {
                    Ok(Some(PaymentMethodType::LocalBankRedirect))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::Trustly(_) => {
                    Ok(Some(PaymentMethodType::Trustly))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::Pse(_) => {
                    Ok(Some(PaymentMethodType::Pse))
                }
                grpc_api_types::payments::payment_method::PaymentMethod::Interac(_) => {
                    Ok(Some(PaymentMethodType::Interac))
                }
                // ============================================================================
                // INDONESIAN BANK TRANSFERS - PaymentMethodType mappings
                // ============================================================================
                grpc_api_types::payments::payment_method::PaymentMethod::Pix(_) => Ok(Some(PaymentMethodType::Pix)),
                grpc_api_types::payments::payment_method::PaymentMethod::PermataBankTransfer(_) => Ok(Some(PaymentMethodType::PermataBankTransfer)),
                grpc_api_types::payments::payment_method::PaymentMethod::BcaBankTransfer(_) => Ok(Some(PaymentMethodType::BcaBankTransfer)),
                grpc_api_types::payments::payment_method::PaymentMethod::BniVaBankTransfer(_) => Ok(Some(PaymentMethodType::BniVa)),
                grpc_api_types::payments::payment_method::PaymentMethod::BriVaBankTransfer(_) => Ok(Some(PaymentMethodType::BriVa)),
                grpc_api_types::payments::payment_method::PaymentMethod::CimbVaBankTransfer(_) => Ok(Some(PaymentMethodType::CimbVa)),
                grpc_api_types::payments::payment_method::PaymentMethod::DanamonVaBankTransfer(_) => Ok(Some(PaymentMethodType::DanamonVa)),
                grpc_api_types::payments::payment_method::PaymentMethod::MandiriVaBankTransfer(_) => Ok(Some(PaymentMethodType::MandiriVa)),
                grpc_api_types::payments::payment_method::PaymentMethod::Netbanking(_) => Ok(Some(PaymentMethodType::Netbanking)),
            },
            None => Err(IntegrationError::InvalidDataFormat { field_name: "unknown", context: IntegrationErrorContext { additional_context: Some("Payment method data is required".to_string()), ..Default::default() } }
            .into()),
        }
    }
}

// Helper trait for generic card conversion
pub trait CardConversionHelper<T: PaymentMethodDataTypes> {
    fn convert_card_details(
        card: grpc_api_types::payments::CardDetails,
    ) -> Result<payment_method_data::Card<T>, error_stack::Report<IntegrationError>>;
}

// Implementation for DefaultPCIHolder
impl CardConversionHelper<Self> for DefaultPCIHolder {
    fn convert_card_details(
        card: grpc_api_types::payments::CardDetails,
    ) -> Result<payment_method_data::Card<Self>, error_stack::Report<IntegrationError>> {
        let card_network = match card.card_network() {
            grpc_api_types::payments::CardNetwork::Unspecified => None,
            _ => Some(CardNetwork::foreign_try_from(card.card_network())?),
        };
        Ok(payment_method_data::Card {
            card_number: RawCardNumber::<Self>(card.card_number.ok_or(
                IntegrationError::InvalidDataFormat {
                    field_name: "unknown",
                    context: IntegrationErrorContext {
                        additional_context: Some("Missing card number".to_string()),
                        ..Default::default()
                    },
                },
            )?),
            card_exp_month: card
                .card_exp_month
                .ok_or(IntegrationError::InvalidDataFormat {
                    field_name: "unknown",
                    context: IntegrationErrorContext {
                        additional_context: Some("Missing Card Expiry Month".to_string()),
                        ..Default::default()
                    },
                })?,
            card_exp_year: card
                .card_exp_year
                .ok_or(IntegrationError::InvalidDataFormat {
                    field_name: "unknown",
                    context: IntegrationErrorContext {
                        additional_context: Some("Missing Card Expiry Year".to_string()),
                        ..Default::default()
                    },
                })?,
            card_cvc: card.card_cvc.ok_or(IntegrationError::InvalidDataFormat {
                field_name: "unknown",
                context: IntegrationErrorContext {
                    additional_context: Some("Missing CVC".to_string()),
                    ..Default::default()
                },
            })?,
            card_issuer: card.card_issuer,
            card_network,
            card_type: card.card_type,
            card_issuing_country: card.card_issuing_country_alpha2,
            bank_code: card.bank_code,
            nick_name: card.nick_name.map(|name| name.into()),
            card_holder_name: card.card_holder_name,
            co_badged_card_data: None,
        })
    }
}

// Implementation for VaultTokenHolder
impl CardConversionHelper<Self> for VaultTokenHolder {
    fn convert_card_details(
        card: grpc_api_types::payments::CardDetails,
    ) -> Result<payment_method_data::Card<Self>, error_stack::Report<IntegrationError>> {
        Ok(payment_method_data::Card {
            card_number: RawCardNumber(
                card.card_number
                    .ok_or(IntegrationError::InvalidDataFormat {
                        field_name: "unknown",
                        context: IntegrationErrorContext {
                            additional_context: Some("Missing card number".to_string()),
                            ..Default::default()
                        },
                    })
                    .map(|cn| cn.get_card_no())?,
            ),
            card_exp_month: card
                .card_exp_month
                .ok_or(IntegrationError::InvalidDataFormat {
                    field_name: "unknown",
                    context: IntegrationErrorContext {
                        additional_context: Some("Missing Card Expiry Month".to_string()),
                        ..Default::default()
                    },
                })?,
            card_exp_year: card
                .card_exp_year
                .ok_or(IntegrationError::InvalidDataFormat {
                    field_name: "unknown",
                    context: IntegrationErrorContext {
                        additional_context: Some("Missing Card Expiry Year".to_string()),
                        ..Default::default()
                    },
                })?,
            card_cvc: card.card_cvc.ok_or(IntegrationError::InvalidDataFormat {
                field_name: "unknown",
                context: IntegrationErrorContext {
                    additional_context: Some("Missing CVC".to_string()),
                    ..Default::default()
                },
            })?,
            card_issuer: card.card_issuer,
            card_network: None,
            card_type: card.card_type,
            card_issuing_country: card.card_issuing_country_alpha2,
            bank_code: card.bank_code,
            nick_name: card.nick_name.map(|name| name.into()),
            card_holder_name: card.card_holder_name,
            co_badged_card_data: None,
        })
    }
}

// Generic ForeignTryFrom implementation using the helper trait
impl<T> ForeignTryFrom<grpc_api_types::payments::CardDetails> for payment_method_data::Card<T>
where
    T: PaymentMethodDataTypes
        + Default
        + Debug
        + Send
        + Eq
        + PartialEq
        + Serialize
        + serde::de::DeserializeOwned
        + Clone
        + CardConversionHelper<T>,
{
    type Error = IntegrationError;
    fn foreign_try_from(
        card: grpc_api_types::payments::CardDetails,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        T::convert_card_details(card)
    }
}

impl ForeignTryFrom<grpc_api_types::payments::Currency> for common_enums::Currency {
    type Error = IntegrationError;
    fn foreign_try_from(
        value: grpc_api_types::payments::Currency,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        match value {
            grpc_api_types::payments::Currency::Aed => Ok(Self::AED),
            grpc_api_types::payments::Currency::All => Ok(Self::ALL),
            grpc_api_types::payments::Currency::Amd => Ok(Self::AMD),
            grpc_api_types::payments::Currency::Ang => Ok(Self::ANG),
            grpc_api_types::payments::Currency::Aoa => Ok(Self::AOA),
            grpc_api_types::payments::Currency::Ars => Ok(Self::ARS),
            grpc_api_types::payments::Currency::Aud => Ok(Self::AUD),
            grpc_api_types::payments::Currency::Awg => Ok(Self::AWG),
            grpc_api_types::payments::Currency::Azn => Ok(Self::AZN),
            grpc_api_types::payments::Currency::Bam => Ok(Self::BAM),
            grpc_api_types::payments::Currency::Bbd => Ok(Self::BBD),
            grpc_api_types::payments::Currency::Bdt => Ok(Self::BDT),
            grpc_api_types::payments::Currency::Bgn => Ok(Self::BGN),
            grpc_api_types::payments::Currency::Bhd => Ok(Self::BHD),
            grpc_api_types::payments::Currency::Bif => Ok(Self::BIF),
            grpc_api_types::payments::Currency::Bmd => Ok(Self::BMD),
            grpc_api_types::payments::Currency::Bnd => Ok(Self::BND),
            grpc_api_types::payments::Currency::Bob => Ok(Self::BOB),
            grpc_api_types::payments::Currency::Brl => Ok(Self::BRL),
            grpc_api_types::payments::Currency::Bsd => Ok(Self::BSD),
            grpc_api_types::payments::Currency::Bwp => Ok(Self::BWP),
            grpc_api_types::payments::Currency::Byn => Ok(Self::BYN),
            grpc_api_types::payments::Currency::Bzd => Ok(Self::BZD),
            grpc_api_types::payments::Currency::Cad => Ok(Self::CAD),
            grpc_api_types::payments::Currency::Chf => Ok(Self::CHF),
            grpc_api_types::payments::Currency::Clp => Ok(Self::CLP),
            grpc_api_types::payments::Currency::Cny => Ok(Self::CNY),
            grpc_api_types::payments::Currency::Cop => Ok(Self::COP),
            grpc_api_types::payments::Currency::Crc => Ok(Self::CRC),
            grpc_api_types::payments::Currency::Cup => Ok(Self::CUP),
            grpc_api_types::payments::Currency::Cve => Ok(Self::CVE),
            grpc_api_types::payments::Currency::Czk => Ok(Self::CZK),
            grpc_api_types::payments::Currency::Djf => Ok(Self::DJF),
            grpc_api_types::payments::Currency::Dkk => Ok(Self::DKK),
            grpc_api_types::payments::Currency::Dop => Ok(Self::DOP),
            grpc_api_types::payments::Currency::Dzd => Ok(Self::DZD),
            grpc_api_types::payments::Currency::Egp => Ok(Self::EGP),
            grpc_api_types::payments::Currency::Etb => Ok(Self::ETB),
            grpc_api_types::payments::Currency::Eur => Ok(Self::EUR),
            grpc_api_types::payments::Currency::Fjd => Ok(Self::FJD),
            grpc_api_types::payments::Currency::Fkp => Ok(Self::FKP),
            grpc_api_types::payments::Currency::Gbp => Ok(Self::GBP),
            grpc_api_types::payments::Currency::Gel => Ok(Self::GEL),
            grpc_api_types::payments::Currency::Ghs => Ok(Self::GHS),
            grpc_api_types::payments::Currency::Gip => Ok(Self::GIP),
            grpc_api_types::payments::Currency::Gmd => Ok(Self::GMD),
            grpc_api_types::payments::Currency::Gnf => Ok(Self::GNF),
            grpc_api_types::payments::Currency::Gtq => Ok(Self::GTQ),
            grpc_api_types::payments::Currency::Gyd => Ok(Self::GYD),
            grpc_api_types::payments::Currency::Hkd => Ok(Self::HKD),
            grpc_api_types::payments::Currency::Hnl => Ok(Self::HNL),
            grpc_api_types::payments::Currency::Hrk => Ok(Self::HRK),
            grpc_api_types::payments::Currency::Htg => Ok(Self::HTG),
            grpc_api_types::payments::Currency::Huf => Ok(Self::HUF),
            grpc_api_types::payments::Currency::Idr => Ok(Self::IDR),
            grpc_api_types::payments::Currency::Ils => Ok(Self::ILS),
            grpc_api_types::payments::Currency::Inr => Ok(Self::INR),
            grpc_api_types::payments::Currency::Iqd => Ok(Self::IQD),
            grpc_api_types::payments::Currency::Jmd => Ok(Self::JMD),
            grpc_api_types::payments::Currency::Jod => Ok(Self::JOD),
            grpc_api_types::payments::Currency::Jpy => Ok(Self::JPY),
            grpc_api_types::payments::Currency::Kes => Ok(Self::KES),
            grpc_api_types::payments::Currency::Kgs => Ok(Self::KGS),
            grpc_api_types::payments::Currency::Khr => Ok(Self::KHR),
            grpc_api_types::payments::Currency::Kmf => Ok(Self::KMF),
            grpc_api_types::payments::Currency::Krw => Ok(Self::KRW),
            grpc_api_types::payments::Currency::Kwd => Ok(Self::KWD),
            grpc_api_types::payments::Currency::Kyd => Ok(Self::KYD),
            grpc_api_types::payments::Currency::Kzt => Ok(Self::KZT),
            grpc_api_types::payments::Currency::Lak => Ok(Self::LAK),
            grpc_api_types::payments::Currency::Lbp => Ok(Self::LBP),
            grpc_api_types::payments::Currency::Lkr => Ok(Self::LKR),
            grpc_api_types::payments::Currency::Lrd => Ok(Self::LRD),
            grpc_api_types::payments::Currency::Lsl => Ok(Self::LSL),
            grpc_api_types::payments::Currency::Lyd => Ok(Self::LYD),
            grpc_api_types::payments::Currency::Mad => Ok(Self::MAD),
            grpc_api_types::payments::Currency::Mdl => Ok(Self::MDL),
            grpc_api_types::payments::Currency::Mga => Ok(Self::MGA),
            grpc_api_types::payments::Currency::Mkd => Ok(Self::MKD),
            grpc_api_types::payments::Currency::Mmk => Ok(Self::MMK),
            grpc_api_types::payments::Currency::Mnt => Ok(Self::MNT),
            grpc_api_types::payments::Currency::Mop => Ok(Self::MOP),
            grpc_api_types::payments::Currency::Mru => Ok(Self::MRU),
            grpc_api_types::payments::Currency::Mur => Ok(Self::MUR),
            grpc_api_types::payments::Currency::Mvr => Ok(Self::MVR),
            grpc_api_types::payments::Currency::Mwk => Ok(Self::MWK),
            grpc_api_types::payments::Currency::Mxn => Ok(Self::MXN),
            grpc_api_types::payments::Currency::Myr => Ok(Self::MYR),
            grpc_api_types::payments::Currency::Mzn => Ok(Self::MZN),
            grpc_api_types::payments::Currency::Nad => Ok(Self::NAD),
            grpc_api_types::payments::Currency::Ngn => Ok(Self::NGN),
            grpc_api_types::payments::Currency::Nio => Ok(Self::NIO),
            grpc_api_types::payments::Currency::Nok => Ok(Self::NOK),
            grpc_api_types::payments::Currency::Npr => Ok(Self::NPR),
            grpc_api_types::payments::Currency::Nzd => Ok(Self::NZD),
            grpc_api_types::payments::Currency::Omr => Ok(Self::OMR),
            grpc_api_types::payments::Currency::Pab => Ok(Self::PAB),
            grpc_api_types::payments::Currency::Pen => Ok(Self::PEN),
            grpc_api_types::payments::Currency::Pgk => Ok(Self::PGK),
            grpc_api_types::payments::Currency::Php => Ok(Self::PHP),
            grpc_api_types::payments::Currency::Pkr => Ok(Self::PKR),
            grpc_api_types::payments::Currency::Pln => Ok(Self::PLN),
            grpc_api_types::payments::Currency::Pyg => Ok(Self::PYG),
            grpc_api_types::payments::Currency::Qar => Ok(Self::QAR),
            grpc_api_types::payments::Currency::Ron => Ok(Self::RON),
            grpc_api_types::payments::Currency::Rsd => Ok(Self::RSD),
            grpc_api_types::payments::Currency::Rub => Ok(Self::RUB),
            grpc_api_types::payments::Currency::Rwf => Ok(Self::RWF),
            grpc_api_types::payments::Currency::Sar => Ok(Self::SAR),
            grpc_api_types::payments::Currency::Sbd => Ok(Self::SBD),
            grpc_api_types::payments::Currency::Scr => Ok(Self::SCR),
            grpc_api_types::payments::Currency::Sek => Ok(Self::SEK),
            grpc_api_types::payments::Currency::Sgd => Ok(Self::SGD),
            grpc_api_types::payments::Currency::Shp => Ok(Self::SHP),
            grpc_api_types::payments::Currency::Sle => Ok(Self::SLE),
            grpc_api_types::payments::Currency::Sll => Ok(Self::SLL),
            grpc_api_types::payments::Currency::Sos => Ok(Self::SOS),
            grpc_api_types::payments::Currency::Srd => Ok(Self::SRD),
            grpc_api_types::payments::Currency::Ssp => Ok(Self::SSP),
            grpc_api_types::payments::Currency::Stn => Ok(Self::STN),
            grpc_api_types::payments::Currency::Svc => Ok(Self::SVC),
            grpc_api_types::payments::Currency::Szl => Ok(Self::SZL),
            grpc_api_types::payments::Currency::Thb => Ok(Self::THB),
            grpc_api_types::payments::Currency::Tnd => Ok(Self::TND),
            grpc_api_types::payments::Currency::Top => Ok(Self::TOP),
            grpc_api_types::payments::Currency::Try => Ok(Self::TRY),
            grpc_api_types::payments::Currency::Ttd => Ok(Self::TTD),
            grpc_api_types::payments::Currency::Twd => Ok(Self::TWD),
            grpc_api_types::payments::Currency::Tzs => Ok(Self::TZS),
            grpc_api_types::payments::Currency::Uah => Ok(Self::UAH),
            grpc_api_types::payments::Currency::Ugx => Ok(Self::UGX),
            grpc_api_types::payments::Currency::Usd => Ok(Self::USD),
            grpc_api_types::payments::Currency::Uyu => Ok(Self::UYU),
            grpc_api_types::payments::Currency::Uzs => Ok(Self::UZS),
            grpc_api_types::payments::Currency::Ves => Ok(Self::VES),
            grpc_api_types::payments::Currency::Vnd => Ok(Self::VND),
            grpc_api_types::payments::Currency::Vuv => Ok(Self::VUV),
            grpc_api_types::payments::Currency::Wst => Ok(Self::WST),
            grpc_api_types::payments::Currency::Xaf => Ok(Self::XAF),
            grpc_api_types::payments::Currency::Xcd => Ok(Self::XCD),
            grpc_api_types::payments::Currency::Xof => Ok(Self::XOF),
            grpc_api_types::payments::Currency::Xpf => Ok(Self::XPF),
            grpc_api_types::payments::Currency::Yer => Ok(Self::YER),
            grpc_api_types::payments::Currency::Zar => Ok(Self::ZAR),
            grpc_api_types::payments::Currency::Zmw => Ok(Self::ZMW),
            _ => Err(report!(IntegrationError::InvalidDataFormat {
                field_name: "currency",
                context: IntegrationErrorContext {
                    additional_context: Some(format!("Currency {value:?} is not supported")),
                    ..Default::default()
                }
            })),
        }
    }
}

impl<
        T: PaymentMethodDataTypes
            + Default
            + Debug
            + Send
            + Eq
            + PartialEq
            + Serialize
            + serde::de::DeserializeOwned
            + Clone
            + CardConversionHelper<T>,
    > ForeignTryFrom<PaymentServiceAuthorizeRequest> for PaymentsAuthorizeData<T>
{
    type Error = IntegrationError;

    fn foreign_try_from(
        value: PaymentServiceAuthorizeRequest,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let amount = match value.amount {
            Some(amount) => amount,
            None => {
                return Err(report!(IntegrationError::MissingRequiredField {
                    field_name: "amount",
                    context: IntegrationErrorContext::default(),
                }));
            }
        };
        let email: Option<Email> = match value.customer.clone().and_then(|customer| customer.email)
        {
            Some(ref email_str) => {
                Some(Email::try_from(email_str.clone().expose()).map_err(|_| {
                    error_stack::Report::new(IntegrationError::InvalidDataFormat {
                        field_name: "unknown",
                        context: IntegrationErrorContext {
                            additional_context: Some("Invalid email".to_string()),
                            ..Default::default()
                        },
                    })
                })?)
            }
            None => None,
        };
        let merchant_config_currency = common_enums::Currency::foreign_try_from(amount.currency())?;

        let connector_feature_data = value
            .clone()
            .connector_feature_data
            .map(|m| ForeignTryFrom::foreign_try_from((m, "feature_data")))
            .transpose()?;
        let merchant_account_id = connector_feature_data
            .as_ref()
            .and_then(|m: &SecretSerdeValue| m.peek().get("merchant_account_id"))
            .and_then(|v| v.as_str())
            .map(str::to_string);

        let setup_future_usage = match value.setup_future_usage() {
            grpc_payment_types::FutureUsage::Unspecified => None,
            _ => Some(FutureUsage::foreign_try_from(value.setup_future_usage())?),
        };

        let customer_acceptance = value.customer_acceptance.clone();
        let authentication_data = value
            .authentication_data
            .clone()
            .map(router_request_types::AuthenticationData::try_from)
            .transpose()?;

        let access_token = value
            .state
            .as_ref()
            .and_then(|state| state.access_token.as_ref())
            .map(ServerAuthenticationTokenResponseData::foreign_try_from)
            .transpose()?;

        let shipping_cost = Some(common_utils::types::MinorUnit::new(value.shipping_cost()));
        // Connector testing data should be sent as a separate field (for adyen) (to be implemented)
        // For now, set to None as Hyperswitch needs to be updated to send this data properly
        let connector_testing_data: Option<Secret<serde_json::Value>> = None;

        let billing_descriptor = value
            .billing_descriptor
            .as_ref()
            .map(|descriptor| {
                BillingDescriptor::from((
                    descriptor,
                    value.statement_descriptor_name.clone(),
                    value.statement_descriptor_suffix.clone(),
                ))
            })
            .or_else(|| {
                // Only build a fallback if at least one descriptor exists
                if value.statement_descriptor_name.is_some()
                    || value.statement_descriptor_suffix.is_some()
                {
                    Some(BillingDescriptor {
                        name: None,
                        city: None,
                        phone: None,
                        reference: None,
                        statement_descriptor: value.statement_descriptor_name.clone(),
                        statement_descriptor_suffix: value.statement_descriptor_suffix.clone(),
                    })
                } else {
                    None
                }
            });

        let payment_channel = match value.payment_channel() {
            grpc_payment_types::PaymentChannel::Unspecified => None,
            _ => Some(common_enums::PaymentChannel::foreign_try_from(
                value.payment_channel(),
            )?),
        };
        let tokenization = match value.tokenization_strategy {
            None => None,
            Some(_) => Some(common_enums::Tokenization::foreign_try_from(
                value.tokenization_strategy(),
            )?),
        };

        Ok(Self {
            authentication_data,
            capture_method: Some(CaptureMethod::foreign_try_from(value.capture_method())?),
            payment_method_data: PaymentMethodData::<T>::foreign_try_from(
                value.payment_method.clone().ok_or_else(|| {
                    IntegrationError::InvalidDataFormat {
                        field_name: "unknown",
                        context: IntegrationErrorContext {
                            additional_context: Some("Payment method data is required".to_string()),
                            ..Default::default()
                        },
                    }
                })?,
            )?,
            amount: common_utils::types::MinorUnit::new(amount.minor_amount),
            currency: common_enums::Currency::foreign_try_from(amount.currency())?,
            confirm: true,
            webhook_url: value.webhook_url.clone(),
            browser_info: value
                .browser_info
                .as_ref()
                .cloned()
                .map(BrowserInformation::foreign_try_from)
                .transpose()?,
            payment_method_type: <Option<PaymentMethodType>>::foreign_try_from(
                value.payment_method.clone().ok_or_else(|| {
                    IntegrationError::InvalidDataFormat {
                        field_name: "unknown",
                        context: IntegrationErrorContext {
                            additional_context: Some("Payment method data is required".to_string()),
                            ..Default::default()
                        },
                    }
                })?,
            )?,
            minor_amount: common_utils::types::MinorUnit::new(amount.minor_amount),
            email,
            customer_name: value
                .customer
                .as_ref()
                .and_then(|customer| customer.name.clone()),
            billing_descriptor,
            router_return_url: value.return_url.clone(),
            complete_authorize_url: value.complete_authorize_url,
            setup_future_usage,
            mandate_id: None,
            off_session: value.off_session,
            order_category: value.order_category,
            session_token: None,
            access_token,
            customer_acceptance: customer_acceptance
                .map(mandates::CustomerAcceptance::foreign_try_from)
                .transpose()?,
            enrolled_for_3ds: value.enrolled_for_3ds,
            related_transaction_id: None,
            payment_experience: None,
            customer_id: value
                .customer
                .and_then(|customer| customer.id)
                .map(|customer_id| CustomerId::try_from(Cow::from(customer_id)))
                .transpose()
                .change_context(IntegrationError::InvalidDataFormat {
                    field_name: "unknown",
                    context: IntegrationErrorContext {
                        additional_context: Some("Failed to parse Customer Id".to_string()),
                        ..Default::default()
                    },
                })?,
            request_incremental_authorization: value.request_incremental_authorization,
            metadata: value
                .metadata
                .map(|m| ForeignTryFrom::foreign_try_from((m, "metadata")))
                .transpose()?,
            merchant_order_id: value.merchant_order_id,
            order_tax_amount: None,
            shipping_cost,
            merchant_account_id,
            integrity_object: None,
            merchant_config_currency: Some(merchant_config_currency),
            all_keys_required: None, // Field not available in new proto structure
            split_payments: None,
            enable_overcapture: None,
            setup_mandate_details: value
                .setup_mandate_details
                .map(MandateData::foreign_try_from)
                .transpose()?,
            request_extended_authorization: value.request_extended_authorization,
            connector_feature_data,
            connector_testing_data,
            payment_channel,
            enable_partial_authorization: value.enable_partial_authorization,
            locale: value.locale.clone(),
            continue_redirection_url: value
                .continue_redirection_url
                .map(|url_str| {
                    url::Url::parse(&url_str).change_context(IntegrationError::InvalidDataFormat {
                        field_name: "continue_redirection_url",
                        context: IntegrationErrorContext::default(),
                    })
                })
                .transpose()?,
            redirect_response: value
                .redirection_response
                .map(|rr| ContinueRedirectionResponse {
                    params: rr.params.map(Secret::new),
                    payload: Some(Secret::new(serde_json::Value::Object(
                        rr.payload
                            .into_iter()
                            .map(|(k, v)| (k, serde_json::Value::String(v)))
                            .collect(),
                    ))),
                }),
            threeds_method_comp_ind: value.threeds_completion_indicator.and_then(|i| {
                grpc_api_types::payments::ThreeDsCompletionIndicator::try_from(i)
                    .ok()
                    .and_then(|e| {
                        connector_types::ThreeDsCompletionIndicator::foreign_try_from(e).ok()
                    })
            }),
            tokenization,
        })
    }
}

impl ForeignTryFrom<grpc_api_types::payments::PaymentAddress> for PaymentAddress {
    type Error = IntegrationError;
    fn foreign_try_from(
        value: grpc_api_types::payments::PaymentAddress,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let shipping = match value.shipping_address {
            Some(address) => Some(Address::foreign_try_from(address)?),
            None => None,
        };

        let billing = match value.billing_address.clone() {
            Some(address) => Some(Address::foreign_try_from(address)?),
            None => None,
        };

        let payment_method_billing = match value.billing_address {
            Some(address) => Some(Address::foreign_try_from(address)?),
            None => None,
        };

        Ok(Self::new(
            shipping,
            billing,
            payment_method_billing,
            Some(false), // should_unify_address set to false
        ))
    }
}

impl ForeignTryFrom<grpc_api_types::payments::Address> for Address {
    type Error = IntegrationError;
    fn foreign_try_from(
        value: grpc_api_types::payments::Address,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let email = match value.email.clone() {
            Some(email) => Some(Email::from_str(&email.expose()).change_context(
                IntegrationError::InvalidDataFormat {
                    field_name: "unknown",
                    context: IntegrationErrorContext {
                        additional_context: Some("Invalid email".to_string()),
                        ..Default::default()
                    },
                },
            )?),
            None => None,
        };
        Ok(Self {
            address: Some(AddressDetails::foreign_try_from(value.clone())?),
            phone: value.phone_number.map(|phone_number| PhoneDetails {
                number: Some(phone_number),
                country_code: value.phone_country_code,
            }),
            email,
        })
    }
}

impl ForeignTryFrom<common_enums::Currency> for grpc_api_types::payments::Currency {
    type Error = ConnectorError;

    fn foreign_try_from(
        currency: common_enums::Currency,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let grpc_currency = Self::from_str_name(&currency.to_string()).ok_or_else(|| {
            ConnectorError::UnexpectedResponseError {
                context: ResponseTransformationErrorContext {
                    http_status_code: None,
                    additional_context: Some(
                        "Failed to parse Currency from connector response".to_string(),
                    ),
                },
            }
        })?;
        Ok(grpc_currency)
    }
}

impl ForeignTryFrom<CountryAlpha2> for grpc_api_types::payments::CountryAlpha2 {
    type Error = ConnectorError;

    fn foreign_try_from(country: CountryAlpha2) -> Result<Self, error_stack::Report<Self::Error>> {
        let grpc_country = Self::from_str_name(&country.to_string()).ok_or_else(|| {
            ConnectorError::UnexpectedResponseError {
                context: ResponseTransformationErrorContext {
                    http_status_code: None,
                    additional_context: Some(
                        "Failed to parse CountryAlpha2 from connector response".to_string(),
                    ),
                },
            }
        })?;
        Ok(grpc_country)
    }
}

impl ForeignTryFrom<grpc_api_types::payments::CountryAlpha2> for CountryAlpha2 {
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::payments::CountryAlpha2,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        match value {
            grpc_api_types::payments::CountryAlpha2::Us => Ok(Self::US),
            grpc_api_types::payments::CountryAlpha2::Af => Ok(Self::AF),
            grpc_api_types::payments::CountryAlpha2::Ax => Ok(Self::AX),
            grpc_api_types::payments::CountryAlpha2::Al => Ok(Self::AL),
            grpc_api_types::payments::CountryAlpha2::Dz => Ok(Self::DZ),
            grpc_api_types::payments::CountryAlpha2::As => Ok(Self::AS),
            grpc_api_types::payments::CountryAlpha2::Ad => Ok(Self::AD),
            grpc_api_types::payments::CountryAlpha2::Ao => Ok(Self::AO),
            grpc_api_types::payments::CountryAlpha2::Ai => Ok(Self::AI),
            grpc_api_types::payments::CountryAlpha2::Aq => Ok(Self::AQ),
            grpc_api_types::payments::CountryAlpha2::Ag => Ok(Self::AG),
            grpc_api_types::payments::CountryAlpha2::Ar => Ok(Self::AR),
            grpc_api_types::payments::CountryAlpha2::Am => Ok(Self::AM),
            grpc_api_types::payments::CountryAlpha2::Aw => Ok(Self::AW),
            grpc_api_types::payments::CountryAlpha2::Au => Ok(Self::AU),
            grpc_api_types::payments::CountryAlpha2::At => Ok(Self::AT),
            grpc_api_types::payments::CountryAlpha2::Az => Ok(Self::AZ),
            grpc_api_types::payments::CountryAlpha2::Bs => Ok(Self::BS),
            grpc_api_types::payments::CountryAlpha2::Bh => Ok(Self::BH),
            grpc_api_types::payments::CountryAlpha2::Bd => Ok(Self::BD),
            grpc_api_types::payments::CountryAlpha2::Bb => Ok(Self::BB),
            grpc_api_types::payments::CountryAlpha2::By => Ok(Self::BY),
            grpc_api_types::payments::CountryAlpha2::Be => Ok(Self::BE),
            grpc_api_types::payments::CountryAlpha2::Bz => Ok(Self::BZ),
            grpc_api_types::payments::CountryAlpha2::Bj => Ok(Self::BJ),
            grpc_api_types::payments::CountryAlpha2::Bm => Ok(Self::BM),
            grpc_api_types::payments::CountryAlpha2::Bt => Ok(Self::BT),
            grpc_api_types::payments::CountryAlpha2::Bo => Ok(Self::BO),
            grpc_api_types::payments::CountryAlpha2::Bq => Ok(Self::BQ),
            grpc_api_types::payments::CountryAlpha2::Ba => Ok(Self::BA),
            grpc_api_types::payments::CountryAlpha2::Bw => Ok(Self::BW),
            grpc_api_types::payments::CountryAlpha2::Bv => Ok(Self::BV),
            grpc_api_types::payments::CountryAlpha2::Br => Ok(Self::BR),
            grpc_api_types::payments::CountryAlpha2::Io => Ok(Self::IO),
            grpc_api_types::payments::CountryAlpha2::Bn => Ok(Self::BN),
            grpc_api_types::payments::CountryAlpha2::Bg => Ok(Self::BG),
            grpc_api_types::payments::CountryAlpha2::Bf => Ok(Self::BF),
            grpc_api_types::payments::CountryAlpha2::Bi => Ok(Self::BI),
            grpc_api_types::payments::CountryAlpha2::Kh => Ok(Self::KH),
            grpc_api_types::payments::CountryAlpha2::Cm => Ok(Self::CM),
            grpc_api_types::payments::CountryAlpha2::Ca => Ok(Self::CA),
            grpc_api_types::payments::CountryAlpha2::Cv => Ok(Self::CV),
            grpc_api_types::payments::CountryAlpha2::Ky => Ok(Self::KY),
            grpc_api_types::payments::CountryAlpha2::Cf => Ok(Self::CF),
            grpc_api_types::payments::CountryAlpha2::Td => Ok(Self::TD),
            grpc_api_types::payments::CountryAlpha2::Cl => Ok(Self::CL),
            grpc_api_types::payments::CountryAlpha2::Cn => Ok(Self::CN),
            grpc_api_types::payments::CountryAlpha2::Cx => Ok(Self::CX),
            grpc_api_types::payments::CountryAlpha2::Cc => Ok(Self::CC),
            grpc_api_types::payments::CountryAlpha2::Co => Ok(Self::CO),
            grpc_api_types::payments::CountryAlpha2::Km => Ok(Self::KM),
            grpc_api_types::payments::CountryAlpha2::Cg => Ok(Self::CG),
            grpc_api_types::payments::CountryAlpha2::Cd => Ok(Self::CD),
            grpc_api_types::payments::CountryAlpha2::Ck => Ok(Self::CK),
            grpc_api_types::payments::CountryAlpha2::Cr => Ok(Self::CR),
            grpc_api_types::payments::CountryAlpha2::Ci => Ok(Self::CI),
            grpc_api_types::payments::CountryAlpha2::Hr => Ok(Self::HR),
            grpc_api_types::payments::CountryAlpha2::Cu => Ok(Self::CU),
            grpc_api_types::payments::CountryAlpha2::Cw => Ok(Self::CW),
            grpc_api_types::payments::CountryAlpha2::Cy => Ok(Self::CY),
            grpc_api_types::payments::CountryAlpha2::Cz => Ok(Self::CZ),
            grpc_api_types::payments::CountryAlpha2::Dk => Ok(Self::DK),
            grpc_api_types::payments::CountryAlpha2::Dj => Ok(Self::DJ),
            grpc_api_types::payments::CountryAlpha2::Dm => Ok(Self::DM),
            grpc_api_types::payments::CountryAlpha2::Do => Ok(Self::DO),
            grpc_api_types::payments::CountryAlpha2::Ec => Ok(Self::EC),
            grpc_api_types::payments::CountryAlpha2::Eg => Ok(Self::EG),
            grpc_api_types::payments::CountryAlpha2::Sv => Ok(Self::SV),
            grpc_api_types::payments::CountryAlpha2::Gq => Ok(Self::GQ),
            grpc_api_types::payments::CountryAlpha2::Er => Ok(Self::ER),
            grpc_api_types::payments::CountryAlpha2::Ee => Ok(Self::EE),
            grpc_api_types::payments::CountryAlpha2::Et => Ok(Self::ET),
            grpc_api_types::payments::CountryAlpha2::Fk => Ok(Self::FK),
            grpc_api_types::payments::CountryAlpha2::Fo => Ok(Self::FO),
            grpc_api_types::payments::CountryAlpha2::Fj => Ok(Self::FJ),
            grpc_api_types::payments::CountryAlpha2::Fi => Ok(Self::FI),
            grpc_api_types::payments::CountryAlpha2::Fr => Ok(Self::FR),
            grpc_api_types::payments::CountryAlpha2::Gf => Ok(Self::GF),
            grpc_api_types::payments::CountryAlpha2::Pf => Ok(Self::PF),
            grpc_api_types::payments::CountryAlpha2::Tf => Ok(Self::TF),
            grpc_api_types::payments::CountryAlpha2::Ga => Ok(Self::GA),
            grpc_api_types::payments::CountryAlpha2::Gm => Ok(Self::GM),
            grpc_api_types::payments::CountryAlpha2::Ge => Ok(Self::GE),
            grpc_api_types::payments::CountryAlpha2::De => Ok(Self::DE),
            grpc_api_types::payments::CountryAlpha2::Gh => Ok(Self::GH),
            grpc_api_types::payments::CountryAlpha2::Gi => Ok(Self::GI),
            grpc_api_types::payments::CountryAlpha2::Gr => Ok(Self::GR),
            grpc_api_types::payments::CountryAlpha2::Gl => Ok(Self::GL),
            grpc_api_types::payments::CountryAlpha2::Gd => Ok(Self::GD),
            grpc_api_types::payments::CountryAlpha2::Gp => Ok(Self::GP),
            grpc_api_types::payments::CountryAlpha2::Gu => Ok(Self::GU),
            grpc_api_types::payments::CountryAlpha2::Gt => Ok(Self::GT),
            grpc_api_types::payments::CountryAlpha2::Gg => Ok(Self::GG),
            grpc_api_types::payments::CountryAlpha2::Gn => Ok(Self::GN),
            grpc_api_types::payments::CountryAlpha2::Gw => Ok(Self::GW),
            grpc_api_types::payments::CountryAlpha2::Gy => Ok(Self::GY),
            grpc_api_types::payments::CountryAlpha2::Ht => Ok(Self::HT),
            grpc_api_types::payments::CountryAlpha2::Hm => Ok(Self::HM),
            grpc_api_types::payments::CountryAlpha2::Va => Ok(Self::VA),
            grpc_api_types::payments::CountryAlpha2::Hn => Ok(Self::HN),
            grpc_api_types::payments::CountryAlpha2::Hk => Ok(Self::HK),
            grpc_api_types::payments::CountryAlpha2::Hu => Ok(Self::HU),
            grpc_api_types::payments::CountryAlpha2::Is => Ok(Self::IS),
            grpc_api_types::payments::CountryAlpha2::In => Ok(Self::IN),
            grpc_api_types::payments::CountryAlpha2::Id => Ok(Self::ID),
            grpc_api_types::payments::CountryAlpha2::Ir => Ok(Self::IR),
            grpc_api_types::payments::CountryAlpha2::Iq => Ok(Self::IQ),
            grpc_api_types::payments::CountryAlpha2::Ie => Ok(Self::IE),
            grpc_api_types::payments::CountryAlpha2::Im => Ok(Self::IM),
            grpc_api_types::payments::CountryAlpha2::Il => Ok(Self::IL),
            grpc_api_types::payments::CountryAlpha2::It => Ok(Self::IT),
            grpc_api_types::payments::CountryAlpha2::Jm => Ok(Self::JM),
            grpc_api_types::payments::CountryAlpha2::Jp => Ok(Self::JP),
            grpc_api_types::payments::CountryAlpha2::Je => Ok(Self::JE),
            grpc_api_types::payments::CountryAlpha2::Jo => Ok(Self::JO),
            grpc_api_types::payments::CountryAlpha2::Kz => Ok(Self::KZ),
            grpc_api_types::payments::CountryAlpha2::Ke => Ok(Self::KE),
            grpc_api_types::payments::CountryAlpha2::Ki => Ok(Self::KI),
            grpc_api_types::payments::CountryAlpha2::Kp => Ok(Self::KP),
            grpc_api_types::payments::CountryAlpha2::Kr => Ok(Self::KR),
            grpc_api_types::payments::CountryAlpha2::Kw => Ok(Self::KW),
            grpc_api_types::payments::CountryAlpha2::Kg => Ok(Self::KG),
            grpc_api_types::payments::CountryAlpha2::La => Ok(Self::LA),
            grpc_api_types::payments::CountryAlpha2::Lv => Ok(Self::LV),
            grpc_api_types::payments::CountryAlpha2::Lb => Ok(Self::LB),
            grpc_api_types::payments::CountryAlpha2::Ls => Ok(Self::LS),
            grpc_api_types::payments::CountryAlpha2::Lr => Ok(Self::LR),
            grpc_api_types::payments::CountryAlpha2::Ly => Ok(Self::LY),
            grpc_api_types::payments::CountryAlpha2::Li => Ok(Self::LI),
            grpc_api_types::payments::CountryAlpha2::Lt => Ok(Self::LT),
            grpc_api_types::payments::CountryAlpha2::Lu => Ok(Self::LU),
            grpc_api_types::payments::CountryAlpha2::Mo => Ok(Self::MO),
            grpc_api_types::payments::CountryAlpha2::Mk => Ok(Self::MK),
            grpc_api_types::payments::CountryAlpha2::Mg => Ok(Self::MG),
            grpc_api_types::payments::CountryAlpha2::Mw => Ok(Self::MW),
            grpc_api_types::payments::CountryAlpha2::My => Ok(Self::MY),
            grpc_api_types::payments::CountryAlpha2::Mv => Ok(Self::MV),
            grpc_api_types::payments::CountryAlpha2::Ml => Ok(Self::ML),
            grpc_api_types::payments::CountryAlpha2::Mt => Ok(Self::MT),
            grpc_api_types::payments::CountryAlpha2::Mh => Ok(Self::MH),
            grpc_api_types::payments::CountryAlpha2::Mq => Ok(Self::MQ),
            grpc_api_types::payments::CountryAlpha2::Mr => Ok(Self::MR),
            grpc_api_types::payments::CountryAlpha2::Mu => Ok(Self::MU),
            grpc_api_types::payments::CountryAlpha2::Yt => Ok(Self::YT),
            grpc_api_types::payments::CountryAlpha2::Mx => Ok(Self::MX),
            grpc_api_types::payments::CountryAlpha2::Fm => Ok(Self::FM),
            grpc_api_types::payments::CountryAlpha2::Md => Ok(Self::MD),
            grpc_api_types::payments::CountryAlpha2::Mc => Ok(Self::MC),
            grpc_api_types::payments::CountryAlpha2::Mn => Ok(Self::MN),
            grpc_api_types::payments::CountryAlpha2::Me => Ok(Self::ME),
            grpc_api_types::payments::CountryAlpha2::Ms => Ok(Self::MS),
            grpc_api_types::payments::CountryAlpha2::Ma => Ok(Self::MA),
            grpc_api_types::payments::CountryAlpha2::Mz => Ok(Self::MZ),
            grpc_api_types::payments::CountryAlpha2::Mm => Ok(Self::MM),
            grpc_api_types::payments::CountryAlpha2::Na => Ok(Self::NA),
            grpc_api_types::payments::CountryAlpha2::Nr => Ok(Self::NR),
            grpc_api_types::payments::CountryAlpha2::Np => Ok(Self::NP),
            grpc_api_types::payments::CountryAlpha2::Nl => Ok(Self::NL),
            grpc_api_types::payments::CountryAlpha2::Nc => Ok(Self::NC),
            grpc_api_types::payments::CountryAlpha2::Nz => Ok(Self::NZ),
            grpc_api_types::payments::CountryAlpha2::Ni => Ok(Self::NI),
            grpc_api_types::payments::CountryAlpha2::Ne => Ok(Self::NE),
            grpc_api_types::payments::CountryAlpha2::Ng => Ok(Self::NG),
            grpc_api_types::payments::CountryAlpha2::Nu => Ok(Self::NU),
            grpc_api_types::payments::CountryAlpha2::Nf => Ok(Self::NF),
            grpc_api_types::payments::CountryAlpha2::Mp => Ok(Self::MP),
            grpc_api_types::payments::CountryAlpha2::No => Ok(Self::NO),
            grpc_api_types::payments::CountryAlpha2::Om => Ok(Self::OM),
            grpc_api_types::payments::CountryAlpha2::Pk => Ok(Self::PK),
            grpc_api_types::payments::CountryAlpha2::Pw => Ok(Self::PW),
            grpc_api_types::payments::CountryAlpha2::Ps => Ok(Self::PS),
            grpc_api_types::payments::CountryAlpha2::Pa => Ok(Self::PA),
            grpc_api_types::payments::CountryAlpha2::Pg => Ok(Self::PG),
            grpc_api_types::payments::CountryAlpha2::Py => Ok(Self::PY),
            grpc_api_types::payments::CountryAlpha2::Pe => Ok(Self::PE),
            grpc_api_types::payments::CountryAlpha2::Ph => Ok(Self::PH),
            grpc_api_types::payments::CountryAlpha2::Pn => Ok(Self::PN),
            grpc_api_types::payments::CountryAlpha2::Pl => Ok(Self::PL),
            grpc_api_types::payments::CountryAlpha2::Pt => Ok(Self::PT),
            grpc_api_types::payments::CountryAlpha2::Pr => Ok(Self::PR),
            grpc_api_types::payments::CountryAlpha2::Qa => Ok(Self::QA),
            grpc_api_types::payments::CountryAlpha2::Re => Ok(Self::RE),
            grpc_api_types::payments::CountryAlpha2::Ro => Ok(Self::RO),
            grpc_api_types::payments::CountryAlpha2::Ru => Ok(Self::RU),
            grpc_api_types::payments::CountryAlpha2::Rw => Ok(Self::RW),
            grpc_api_types::payments::CountryAlpha2::Bl => Ok(Self::BL),
            grpc_api_types::payments::CountryAlpha2::Sh => Ok(Self::SH),
            grpc_api_types::payments::CountryAlpha2::Kn => Ok(Self::KN),
            grpc_api_types::payments::CountryAlpha2::Lc => Ok(Self::LC),
            grpc_api_types::payments::CountryAlpha2::Mf => Ok(Self::MF),
            grpc_api_types::payments::CountryAlpha2::Pm => Ok(Self::PM),
            grpc_api_types::payments::CountryAlpha2::Vc => Ok(Self::VC),
            grpc_api_types::payments::CountryAlpha2::Ws => Ok(Self::WS),
            grpc_api_types::payments::CountryAlpha2::Sm => Ok(Self::SM),
            grpc_api_types::payments::CountryAlpha2::St => Ok(Self::ST),
            grpc_api_types::payments::CountryAlpha2::Sa => Ok(Self::SA),
            grpc_api_types::payments::CountryAlpha2::Sn => Ok(Self::SN),
            grpc_api_types::payments::CountryAlpha2::Rs => Ok(Self::RS),
            grpc_api_types::payments::CountryAlpha2::Sc => Ok(Self::SC),
            grpc_api_types::payments::CountryAlpha2::Sl => Ok(Self::SL),
            grpc_api_types::payments::CountryAlpha2::Sg => Ok(Self::SG),
            grpc_api_types::payments::CountryAlpha2::Sx => Ok(Self::SX),
            grpc_api_types::payments::CountryAlpha2::Sk => Ok(Self::SK),
            grpc_api_types::payments::CountryAlpha2::Si => Ok(Self::SI),
            grpc_api_types::payments::CountryAlpha2::Sb => Ok(Self::SB),
            grpc_api_types::payments::CountryAlpha2::So => Ok(Self::SO),
            grpc_api_types::payments::CountryAlpha2::Za => Ok(Self::ZA),
            grpc_api_types::payments::CountryAlpha2::Gs => Ok(Self::GS),
            grpc_api_types::payments::CountryAlpha2::Ss => Ok(Self::SS),
            grpc_api_types::payments::CountryAlpha2::Es => Ok(Self::ES),
            grpc_api_types::payments::CountryAlpha2::Lk => Ok(Self::LK),
            grpc_api_types::payments::CountryAlpha2::Sd => Ok(Self::SD),
            grpc_api_types::payments::CountryAlpha2::Sr => Ok(Self::SR),
            grpc_api_types::payments::CountryAlpha2::Sj => Ok(Self::SJ),
            grpc_api_types::payments::CountryAlpha2::Sz => Ok(Self::SZ),
            grpc_api_types::payments::CountryAlpha2::Se => Ok(Self::SE),
            grpc_api_types::payments::CountryAlpha2::Ch => Ok(Self::CH),
            grpc_api_types::payments::CountryAlpha2::Sy => Ok(Self::SY),
            grpc_api_types::payments::CountryAlpha2::Tw => Ok(Self::TW),
            grpc_api_types::payments::CountryAlpha2::Tj => Ok(Self::TJ),
            grpc_api_types::payments::CountryAlpha2::Tz => Ok(Self::TZ),
            grpc_api_types::payments::CountryAlpha2::Th => Ok(Self::TH),
            grpc_api_types::payments::CountryAlpha2::Tl => Ok(Self::TL),
            grpc_api_types::payments::CountryAlpha2::Tg => Ok(Self::TG),
            grpc_api_types::payments::CountryAlpha2::Tk => Ok(Self::TK),
            grpc_api_types::payments::CountryAlpha2::To => Ok(Self::TO),
            grpc_api_types::payments::CountryAlpha2::Tt => Ok(Self::TT),
            grpc_api_types::payments::CountryAlpha2::Tn => Ok(Self::TN),
            grpc_api_types::payments::CountryAlpha2::Tr => Ok(Self::TR),
            grpc_api_types::payments::CountryAlpha2::Tm => Ok(Self::TM),
            grpc_api_types::payments::CountryAlpha2::Tc => Ok(Self::TC),
            grpc_api_types::payments::CountryAlpha2::Tv => Ok(Self::TV),
            grpc_api_types::payments::CountryAlpha2::Ug => Ok(Self::UG),
            grpc_api_types::payments::CountryAlpha2::Ua => Ok(Self::UA),
            grpc_api_types::payments::CountryAlpha2::Ae => Ok(Self::AE),
            grpc_api_types::payments::CountryAlpha2::Gb => Ok(Self::GB),
            grpc_api_types::payments::CountryAlpha2::Um => Ok(Self::UM),
            grpc_api_types::payments::CountryAlpha2::Uy => Ok(Self::UY),
            grpc_api_types::payments::CountryAlpha2::Uz => Ok(Self::UZ),
            grpc_api_types::payments::CountryAlpha2::Vu => Ok(Self::VU),
            grpc_api_types::payments::CountryAlpha2::Ve => Ok(Self::VE),
            grpc_api_types::payments::CountryAlpha2::Vn => Ok(Self::VN),
            grpc_api_types::payments::CountryAlpha2::Vg => Ok(Self::VG),
            grpc_api_types::payments::CountryAlpha2::Vi => Ok(Self::VI),
            grpc_api_types::payments::CountryAlpha2::Wf => Ok(Self::WF),
            grpc_api_types::payments::CountryAlpha2::Eh => Ok(Self::EH),
            grpc_api_types::payments::CountryAlpha2::Ye => Ok(Self::YE),
            grpc_api_types::payments::CountryAlpha2::Zm => Ok(Self::ZM),
            grpc_api_types::payments::CountryAlpha2::Zw => Ok(Self::ZW),
            grpc_api_types::payments::CountryAlpha2::Unspecified => Ok(Self::US), // Default to US if unspecified
        }
    }
}

impl ForeignTryFrom<grpc_api_types::payments::Address> for AddressDetails {
    type Error = IntegrationError;
    fn foreign_try_from(
        value: grpc_api_types::payments::Address,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let country_code = value.country_alpha2_code();
        let country = if matches!(
            country_code,
            grpc_api_types::payments::CountryAlpha2::Unspecified
        ) {
            None
        } else {
            Some(CountryAlpha2::foreign_try_from(country_code)?)
        };

        Ok(Self {
            country,
            city: value.city,
            line1: value.line1,
            line2: value.line2,
            line3: value.line3,
            zip: value.zip_code,
            state: value.state,
            first_name: value.first_name,
            last_name: value.last_name,
            origin_zip: None,
        })
    }
}

impl ForeignTryFrom<grpc_api_types::payments::OrderDetailsWithAmount> for OrderDetailsWithAmount {
    type Error = IntegrationError;

    fn foreign_try_from(
        item: grpc_api_types::payments::OrderDetailsWithAmount,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        Ok(Self {
            product_name: item.product_name,
            quantity: u16::try_from(item.quantity).change_context(
                IntegrationError::InvalidDataFormat {
                    field_name: "unknown",
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "Quantity value is out of range for u16".to_string(),
                        ),
                        ..Default::default()
                    },
                },
            )?,
            amount: common_utils::types::MinorUnit::new(item.amount),
            tax_rate: item.tax_rate,
            total_tax_amount: item
                .total_tax_amount
                .map(common_utils::types::MinorUnit::new),
            requires_shipping: item.requires_shipping,
            product_img_link: item.product_img_link,
            product_id: item.product_id,
            category: item.category,
            sub_category: item.sub_category,
            brand: item.brand,
            description: item.description,
            unit_of_measure: item.unit_of_measure,
            product_type: item
                .product_type
                .and_then(|pt| grpc_api_types::payments::ProductType::try_from(pt).ok())
                .map(|grpc_product_type| {
                    common_enums::ProductType::foreign_from(grpc_product_type)
                }),
            product_tax_code: item.product_tax_code,
            commodity_code: None,
            sku: None,
            upc: None,
            unit_discount_amount: None,
            total_amount: None,
        })
    }
}

impl
    ForeignTryFrom<(
        grpc_payment_types::MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest,
        Connectors,
        &MaskedMetadata,
    )> for PaymentFlowData
{
    type Error = IntegrationError;

    fn foreign_try_from(
        (value, connectors, metadata): (
            grpc_payment_types::MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest,
            Connectors,
            &MaskedMetadata,
        ),
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        // For access token creation operations, address information is typically not available or required
        let address: PaymentAddress = PaymentAddress::new(
            None,        // shipping
            None,        // billing
            None,        // payment_method_billing
            Some(false), // should_unify_address = false for access token operations
        );

        let merchant_id_from_header = extract_merchant_id_from_metadata(metadata)?;

        let connector_feature_data = value
            .connector_feature_data
            .map(|m| ForeignTryFrom::foreign_try_from((m, "feature_data")))
            .transpose()?;

        Ok(Self {
            merchant_id: merchant_id_from_header,
            payment_id: "IRRELEVANT_PAYMENT_ID".to_string(),
            attempt_id: "IRRELEVANT_ATTEMPT_ID".to_string(),
            status: common_enums::AttemptStatus::Pending,
            payment_method: PaymentMethod::Card, // Default for access token operations
            address,
            auth_type: common_enums::AuthenticationType::default(),
            connector_request_reference_id: extract_connector_request_reference_id(
                &value.merchant_access_token_id,
            ), // No request_ref_id available for access token requests
            customer_id: None,
            connector_customer: None,
            description: None,
            return_url: None,
            connector_feature_data,
            amount_captured: None,
            minor_amount_captured: None,
            minor_amount_capturable: None,
            amount: None,
            access_token: None,
            session_token: None,
            reference_id: None,
            connector_order_id: None,
            preprocessing_id: None,
            connector_api_version: None,
            test_mode: value.test_mode,
            connector_http_status_code: None,
            external_latency: None,
            connectors,
            raw_connector_response: None,
            raw_connector_request: None,
            connector_response_headers: None,
            vault_headers: None,
            connector_response: None,
            recurring_mandate_payment_data: None,
            order_details: None,
            minor_amount_authorized: None,
            l2_l3_data: None,
        })
    }
}

// PhoneDetails conversion removed - phone info is now embedded in Address

impl ForeignTryFrom<(PaymentServiceAuthorizeRequest, Connectors, &MaskedMetadata)>
    for PaymentFlowData
{
    type Error = IntegrationError;

    fn foreign_try_from(
        (value, connectors, metadata): (
            PaymentServiceAuthorizeRequest,
            Connectors,
            &MaskedMetadata,
        ),
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let address = match &value.address {
            // Borrow value.address
            Some(address_value) => {
                // address_value is &grpc_api_types::payments::PaymentAddress
                PaymentAddress::foreign_try_from(
                    (*address_value).clone(), // Clone the grpc_api_types::payments::PaymentAddress
                )?
            }
            None => {
                return Err(IntegrationError::InvalidDataFormat {
                    field_name: "unknown",
                    context: IntegrationErrorContext {
                        additional_context: Some("Address is required".to_string()),
                        ..Default::default()
                    },
                })?
            }
        };

        let l2_l3_data = value
            .l2_l3_data
            .as_ref()
            .map(|l2_l3| L2L3Data::foreign_try_from((l2_l3, &address, value.customer.as_ref())))
            .transpose()?;

        let merchant_id_from_header = extract_merchant_id_from_metadata(metadata)?;

        // Extract specific headers for vault and other integrations
        let vault_headers = extract_headers_from_metadata(metadata);

        let connector_feature_data = value
            .connector_feature_data
            .map(|m| ForeignTryFrom::foreign_try_from((m, "feature_data")))
            .transpose()?;

        let order_details = (!value.order_details.is_empty())
            .then(|| {
                value
                    .order_details
                    .into_iter()
                    .map(OrderDetailsWithAmount::foreign_try_from)
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?;

        let access_token = value
            .state
            .as_ref()
            .and_then(|state| state.access_token.as_ref())
            .map(ServerAuthenticationTokenResponseData::foreign_try_from)
            .transpose()?;

        Ok(Self {
            merchant_id: merchant_id_from_header,
            payment_id: "IRRELEVANT_PAYMENT_ID".to_string(),
            attempt_id: "IRRELEVANT_ATTEMPT_ID".to_string(),
            status: common_enums::AttemptStatus::Pending,
            payment_method: PaymentMethod::foreign_try_from(
                value.payment_method.unwrap_or_default(),
            )?, // Use direct enum
            address,
            auth_type: common_enums::AuthenticationType::foreign_try_from(
                grpc_api_types::payments::AuthenticationType::try_from(value.auth_type)
                    .unwrap_or_default(),
            )?, // Use direct enum
            connector_request_reference_id: extract_connector_request_reference_id(
                &value.merchant_transaction_id,
            ),
            customer_id: value
                .customer
                .clone()
                .and_then(|customer| customer.connector_customer_id)
                .map(|customer_id| CustomerId::try_from(Cow::from(customer_id)))
                .transpose()
                .change_context(IntegrationError::InvalidDataFormat {
                    field_name: "unknown",
                    context: IntegrationErrorContext {
                        additional_context: Some("Failed to parse Customer Id".to_string()),
                        ..Default::default()
                    },
                })?,
            connector_customer: value
                .customer
                .and_then(|customer| customer.connector_customer_id),
            description: value.description,
            return_url: value.return_url.clone(),
            connector_feature_data,
            amount_captured: None,
            minor_amount_captured: None,
            minor_amount_capturable: None,
            amount: None,
            access_token,
            session_token: value.session_token,
            reference_id: value.merchant_order_id.clone(),
            connector_order_id: value.connector_order_id,
            preprocessing_id: None,
            connector_api_version: None,
            test_mode: value.test_mode,
            connector_http_status_code: None,
            external_latency: None,
            connectors,
            raw_connector_response: None,
            raw_connector_request: None,
            connector_response_headers: None,
            connector_response: None,
            vault_headers,
            recurring_mandate_payment_data: None,
            order_details,
            l2_l3_data: l2_l3_data.map(Box::new),
            minor_amount_authorized: None,
        })
    }
}

impl
    ForeignTryFrom<(
        grpc_api_types::payments::RecurringPaymentServiceChargeRequest,
        Connectors,
        &MaskedMetadata,
    )> for PaymentFlowData
{
    type Error = IntegrationError;

    fn foreign_try_from(
        (value, connectors, metadata): (
            grpc_api_types::payments::RecurringPaymentServiceChargeRequest,
            Connectors,
            &MaskedMetadata,
        ),
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let address = match &value.address {
            // Borrow value.address
            Some(address_value) => {
                // address_value is &grpc_api_types::payments::PaymentAddress
                PaymentAddress::foreign_try_from(
                    (*address_value).clone(), // Clone the grpc_api_types::payments::PaymentAddress
                )?
            }
            None => {
                // For repeat payment operations, address information is typically not available or required
                PaymentAddress::new(
                    None,        // shipping
                    None,        // billing
                    None,        // payment_method_billing
                    Some(false), // should_unify_address = false for repeat operations
                )
            }
        };

        let l2_l3_data = value
            .l2_l3_data
            .as_ref()
            .map(|l2_l3| L2L3Data::foreign_try_from((l2_l3, &address, value.customer.as_ref())))
            .transpose()?;

        let merchant_id_from_header = extract_merchant_id_from_metadata(metadata)?;

        // Extract access_token from state field
        let access_token = value
            .state
            .as_ref()
            .and_then(|state| state.access_token.as_ref())
            .map(ServerAuthenticationTokenResponseData::foreign_try_from)
            .transpose()?;

        Ok(Self {
            merchant_id: merchant_id_from_header,
            payment_id: "IRRELEVANT_PAYMENT_ID".to_string(),
            attempt_id: "IRRELEVANT_ATTEMPT_ID".to_string(),
            status: common_enums::AttemptStatus::Pending,
            payment_method: PaymentMethod::Card, //TODO
            address,
            auth_type: common_enums::AuthenticationType::default(),
            connector_request_reference_id: extract_connector_request_reference_id(
                &value.merchant_charge_id,
            ),
            customer_id: None,
            connector_customer: value.connector_customer_id,
            description: value.description,
            return_url: None,
            connector_feature_data: value
                .connector_feature_data
                .map(|m| ForeignTryFrom::foreign_try_from((m, "feature_data")))
                .transpose()?,
            amount_captured: None,
            minor_amount_captured: None,
            minor_amount_capturable: None,
            amount: None,
            access_token,
            session_token: None,
            reference_id: None,
            connector_order_id: None,
            preprocessing_id: None,
            connector_api_version: None,
            test_mode: value.test_mode,
            connector_http_status_code: None,
            external_latency: None,
            connectors,
            raw_connector_response: None,
            raw_connector_request: None,
            connector_response_headers: None,
            connector_response: None,
            vault_headers: None,
            recurring_mandate_payment_data: None,
            order_details: None,
            minor_amount_authorized: None,
            l2_l3_data: l2_l3_data.map(Box::new),
        })
    }
}

impl
    ForeignTryFrom<(
        grpc_api_types::payments::PaymentServiceGetRequest,
        Connectors,
        &MaskedMetadata,
    )> for PaymentFlowData
{
    type Error = IntegrationError;

    fn foreign_try_from(
        (value, connectors, metadata): (
            grpc_api_types::payments::PaymentServiceGetRequest,
            Connectors,
            &MaskedMetadata,
        ),
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        // For sync operations, address information is typically not available or required
        let address: PaymentAddress = PaymentAddress::new(
            None,        // shipping
            None,        // billing
            None,        // payment_method_billing
            Some(false), // should_unify_address = false for sync operations
        );

        let merchant_id_from_header = extract_merchant_id_from_metadata(metadata)?;

        let access_token = value
            .state
            .as_ref()
            .and_then(|state| state.access_token.as_ref())
            .map(ServerAuthenticationTokenResponseData::foreign_try_from)
            .transpose()?;

        Ok(Self {
            merchant_id: merchant_id_from_header,
            payment_id: "IRRELEVANT_PAYMENT_ID".to_string(),
            attempt_id: "IRRELEVANT_ATTEMPT_ID".to_string(),
            status: common_enums::AttemptStatus::Pending,
            payment_method: PaymentMethod::Card, //TODO
            address,
            auth_type: common_enums::AuthenticationType::default(),
            connector_request_reference_id: value.merchant_transaction_id.unwrap_or_default(),
            customer_id: None,
            connector_customer: None,
            description: None,
            return_url: None,
            connector_feature_data: value
                .connector_feature_data
                .map(|m| ForeignTryFrom::foreign_try_from((m, "merchant account metadata")))
                .transpose()?,
            amount_captured: None,
            minor_amount_captured: None,
            minor_amount_capturable: None,
            amount: None,
            access_token,
            session_token: None,
            reference_id: value.connector_order_reference_id.clone(),
            connector_order_id: None,
            preprocessing_id: None,
            connector_api_version: None,
            test_mode: value.test_mode,
            connector_http_status_code: None,
            external_latency: None,
            connectors,
            raw_connector_response: None,
            raw_connector_request: None,
            connector_response_headers: None,
            vault_headers: None,
            connector_response: None,
            recurring_mandate_payment_data: None,
            order_details: None,
            minor_amount_authorized: None,
            l2_l3_data: None,
        })
    }
}

impl ForeignTryFrom<(PaymentServiceVoidRequest, Connectors, &MaskedMetadata)> for PaymentFlowData {
    type Error = IntegrationError;

    fn foreign_try_from(
        (value, connectors, metadata): (PaymentServiceVoidRequest, Connectors, &MaskedMetadata),
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        // For void operations, address information is typically not available or required
        // Since this is a PaymentServiceVoidRequest, we use default address values
        let address: PaymentAddress = PaymentAddress::new(
            None,        // shipping
            None,        // billing
            None,        // payment_method_billing
            Some(false), // should_unify_address = false for void operations
        );

        let merchant_id_from_header = extract_merchant_id_from_metadata(metadata)?;
        let access_token = value
            .state
            .as_ref()
            .and_then(|state| state.access_token.as_ref())
            .map(ServerAuthenticationTokenResponseData::foreign_try_from)
            .transpose()?;

        let connector_feature_data = value
            .connector_feature_data
            .map(|m| ForeignTryFrom::foreign_try_from((m, "merchant account metadata")))
            .transpose()?;

        Ok(Self {
            merchant_id: merchant_id_from_header,
            payment_id: "IRRELEVANT_PAYMENT_ID".to_string(),
            attempt_id: "IRRELEVANT_ATTEMPT_ID".to_string(),
            status: common_enums::AttemptStatus::Pending,
            payment_method: PaymentMethod::Card, //TODO
            address,
            auth_type: common_enums::AuthenticationType::default(),
            connector_request_reference_id: extract_connector_request_reference_id(
                &value.merchant_void_id,
            ),
            customer_id: None,
            connector_customer: None,
            description: None,
            return_url: None,
            connector_feature_data,
            amount_captured: None,
            minor_amount_captured: None,
            minor_amount_capturable: None,
            amount: None,
            access_token,
            session_token: None,
            reference_id: None,
            connector_order_id: None,
            preprocessing_id: None,
            connector_api_version: None,
            test_mode: value.test_mode,
            connector_http_status_code: None,
            external_latency: None,
            connectors,
            raw_connector_response: None,
            raw_connector_request: None,
            connector_response_headers: None,
            vault_headers: None,
            connector_response: None,
            recurring_mandate_payment_data: None,
            order_details: None,
            minor_amount_authorized: None,
            l2_l3_data: None,
        })
    }
}

impl ForeignTryFrom<ResponseId> for Option<String> {
    type Error = ConnectorError;
    fn foreign_try_from(
        value: ResponseId,
    ) -> Result<Option<String>, error_stack::Report<Self::Error>> {
        Ok(match value {
            ResponseId::ConnectorTransactionId(id) => Some(id),
            ResponseId::EncodedData(data) => Some(data),
            ResponseId::NoResponseId => None,
        })
    }
}

impl ForeignTryFrom<router_request_types::AuthenticationData>
    for grpc_api_types::payments::AuthenticationData
{
    type Error = IntegrationError;
    fn foreign_try_from(
        value: router_request_types::AuthenticationData,
    ) -> error_stack::Result<Self, Self::Error> {
        use hyperswitch_masking::ExposeInterface;
        let trans_status = value
            .trans_status
            .map(|ts| grpc_api_types::payments::TransactionStatus::foreign_from(ts).into());
        let exemption_indicator = value
            .exemption_indicator
            .map(|ei| grpc_api_types::payments::ExemptionIndicator::foreign_from(ei).into());
        Ok(Self {
            ucaf_collection_indicator: value.ucaf_collection_indicator,
            eci: value.eci,
            cavv: value.cavv.map(|cavv| cavv.expose()),
            threeds_server_transaction_id: value.threeds_server_transaction_id,
            message_version: value.message_version.map(|v| v.to_string()),
            ds_transaction_id: value.ds_trans_id,
            trans_status,
            acs_transaction_id: value.acs_transaction_id,
            connector_transaction_id: value.transaction_id,
            exemption_indicator,
            network_params: value
                .network_params
                .map(grpc_api_types::payments::NetworkParams::foreign_from),
        })
    }
}

impl ForeignFrom<common_enums::TransactionStatus> for grpc_api_types::payments::TransactionStatus {
    fn foreign_from(from: common_enums::TransactionStatus) -> Self {
        match from {
            common_enums::TransactionStatus::Success => Self::Success,
            common_enums::TransactionStatus::Failure => Self::Failure,
            common_enums::TransactionStatus::VerificationNotPerformed => {
                Self::VerificationNotPerformed
            }
            common_enums::TransactionStatus::NotVerified => Self::NotVerified,
            common_enums::TransactionStatus::Rejected => Self::Rejected,
            common_enums::TransactionStatus::ChallengeRequired => Self::ChallengeRequired,
            common_enums::TransactionStatus::ChallengeRequiredDecoupledAuthentication => {
                Self::ChallengeRequiredDecoupledAuthentication
            }
            common_enums::TransactionStatus::InformationOnly => Self::InformationOnly,
        }
    }
}

impl ForeignFrom<grpc_api_types::payments::TransactionStatus> for common_enums::TransactionStatus {
    fn foreign_from(value: grpc_api_types::payments::TransactionStatus) -> Self {
        match value {
            grpc_api_types::payments::TransactionStatus::Success => Self::Success,
            grpc_api_types::payments::TransactionStatus::Failure => Self::Failure,
            grpc_api_types::payments::TransactionStatus::VerificationNotPerformed => Self::VerificationNotPerformed,
            grpc_api_types::payments::TransactionStatus::Unspecified | grpc_api_types::payments::TransactionStatus::NotVerified => Self::NotVerified,
            grpc_api_types::payments::TransactionStatus::Rejected => Self::Rejected,
            grpc_api_types::payments::TransactionStatus::ChallengeRequired => Self::ChallengeRequired,
            grpc_api_types::payments::TransactionStatus::ChallengeRequiredDecoupledAuthentication => Self::ChallengeRequiredDecoupledAuthentication,
            grpc_api_types::payments::TransactionStatus::InformationOnly => Self::InformationOnly,
        }
    }
}

impl ForeignFrom<common_enums::ExemptionIndicator>
    for grpc_api_types::payments::ExemptionIndicator
{
    fn foreign_from(value: common_enums::ExemptionIndicator) -> Self {
        match value {
            common_enums::ExemptionIndicator::LowValue => Self::LowValue,
            common_enums::ExemptionIndicator::TransactionRiskAssessment => {
                Self::TransactionRiskAssessment
            }
            common_enums::ExemptionIndicator::TrustedListing => Self::TrustedListing,
            common_enums::ExemptionIndicator::SecureCorporatePayment => {
                Self::SecureCorporatePayment
            }
            common_enums::ExemptionIndicator::ScaDelegation => Self::ScaDelegation,
            common_enums::ExemptionIndicator::ThreeDsOutage => Self::ThreeDsOutage,
            common_enums::ExemptionIndicator::OutOfScaScope => Self::OutOfScaScope,
            common_enums::ExemptionIndicator::Other => Self::Other,
            common_enums::ExemptionIndicator::LowRiskProgram => Self::LowRiskProgram,
            common_enums::ExemptionIndicator::RecurringOperation => Self::RecurringOperation,
        }
    }
}

impl ForeignFrom<grpc_api_types::payments::ExemptionIndicator>
    for common_enums::ExemptionIndicator
{
    fn foreign_from(value: grpc_api_types::payments::ExemptionIndicator) -> Self {
        match value {
            grpc_api_types::payments::ExemptionIndicator::LowValue => Self::LowValue,
            grpc_api_types::payments::ExemptionIndicator::TransactionRiskAssessment => {
                Self::TransactionRiskAssessment
            }
            grpc_api_types::payments::ExemptionIndicator::TrustedListing => Self::TrustedListing,
            grpc_api_types::payments::ExemptionIndicator::SecureCorporatePayment => {
                Self::SecureCorporatePayment
            }
            grpc_api_types::payments::ExemptionIndicator::ScaDelegation => Self::ScaDelegation,
            grpc_api_types::payments::ExemptionIndicator::ThreeDsOutage => Self::ThreeDsOutage,
            grpc_api_types::payments::ExemptionIndicator::OutOfScaScope => Self::OutOfScaScope,
            grpc_api_types::payments::ExemptionIndicator::Other => Self::Other,
            grpc_api_types::payments::ExemptionIndicator::LowRiskProgram => Self::LowRiskProgram,
            grpc_api_types::payments::ExemptionIndicator::RecurringOperation => {
                Self::RecurringOperation
            }
            grpc_api_types::payments::ExemptionIndicator::Unspecified => Self::Other,
        }
    }
}

impl ForeignFrom<common_enums::CavvAlgorithm> for grpc_api_types::payments::CavvAlgorithm {
    fn foreign_from(value: common_enums::CavvAlgorithm) -> Self {
        match value {
            common_enums::CavvAlgorithm::Zero => Self::Zero,
            common_enums::CavvAlgorithm::One => Self::One,
            common_enums::CavvAlgorithm::Two => Self::Two,
            common_enums::CavvAlgorithm::Three => Self::Three,
            common_enums::CavvAlgorithm::Four => Self::Four,
            common_enums::CavvAlgorithm::A => Self::A,
        }
    }
}

impl ForeignFrom<grpc_api_types::payments::CavvAlgorithm> for common_enums::CavvAlgorithm {
    fn foreign_from(value: grpc_api_types::payments::CavvAlgorithm) -> Self {
        match value {
            grpc_api_types::payments::CavvAlgorithm::Zero => Self::Zero,
            grpc_api_types::payments::CavvAlgorithm::One => Self::One,
            grpc_api_types::payments::CavvAlgorithm::Two => Self::Two,
            grpc_api_types::payments::CavvAlgorithm::Three => Self::Three,
            grpc_api_types::payments::CavvAlgorithm::Four => Self::Four,
            grpc_api_types::payments::CavvAlgorithm::A => Self::A,
            grpc_api_types::payments::CavvAlgorithm::Unspecified => Self::Zero,
        }
    }
}

impl ForeignTryFrom<ConnectorResponseData> for grpc_api_types::payments::ConnectorResponseData {
    type Error = ConnectorError;
    fn foreign_try_from(
        value: ConnectorResponseData,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        Ok(Self {
            additional_payment_method_data: value.additional_payment_method_data.as_ref().map(
                |additional_payment_method_connector_response| {
                    match additional_payment_method_connector_response {
                        AdditionalPaymentMethodConnectorResponse::Card {
                            authentication_data,
                            payment_checks,
                            card_network,
                            domestic_network,
                            auth_code,
                        } => grpc_api_types::payments::AdditionalPaymentMethodConnectorResponse {
                            payment_method_data: Some(
                                grpc_api_types::payments::additional_payment_method_connector_response::PaymentMethodData::Card(
                                    grpc_api_types::payments::CardConnectorResponse {
                                        authentication_data: authentication_data
                                            .as_ref()
                                            .and_then(|data| serde_json::to_vec(data).ok()),
                                        payment_checks: payment_checks
                                            .as_ref()
                                            .and_then(|checks| serde_json::to_vec(checks).ok()),
                                        card_network: card_network.clone(),
                                        domestic_network: domestic_network.clone(),
                                        auth_code: auth_code.clone(),
                                    }
                                )
                            ),
                        },
                        AdditionalPaymentMethodConnectorResponse::Upi { upi_mode } => {
                            grpc_api_types::payments::AdditionalPaymentMethodConnectorResponse {
                                payment_method_data: Some(
                                    grpc_api_types::payments::additional_payment_method_connector_response::PaymentMethodData::Upi(
                                        grpc_api_types::payments::UpiConnectorResponse {
                                            upi_mode: upi_mode.clone().map(|source| {
                                                let proto_source: grpc_api_types::payments::UpiSource = ForeignFrom::foreign_from(source);
                                                proto_source as i32
                                            }),
                                        }
                                    )
                                ),
                            }
                        }
                        AdditionalPaymentMethodConnectorResponse::GooglePay { auth_code } => {
                            grpc_api_types::payments::AdditionalPaymentMethodConnectorResponse {
                                payment_method_data: Some(
                                    grpc_api_types::payments::additional_payment_method_connector_response::PaymentMethodData::GooglePay(
                                        grpc_api_types::payments::GooglePayConnectorResponse {
                                            auth_code: auth_code.clone(),
                                        }
                                    )
                                ),
                            }
                        }
                        AdditionalPaymentMethodConnectorResponse::ApplePay { auth_code } => {
                            grpc_api_types::payments::AdditionalPaymentMethodConnectorResponse {
                                payment_method_data: Some(
                                    grpc_api_types::payments::additional_payment_method_connector_response::PaymentMethodData::ApplePay(
                                        grpc_api_types::payments::ApplePayConnectorResponse {
                                            auth_code: auth_code.clone(),
                                        }
                                    )
                                ),
                            }
                        }
                        AdditionalPaymentMethodConnectorResponse::BankRedirect { interac } => {
                            grpc_api_types::payments::AdditionalPaymentMethodConnectorResponse {
                                payment_method_data: Some(
                                    grpc_api_types::payments::additional_payment_method_connector_response::PaymentMethodData::BankRedirect(
                                        grpc_api_types::payments::BankRedirectConnectorResponse {
                                            interac: interac.clone().map(|interac_info| grpc_api_types::payments::InteracCustomerInfo {
                                                customer_info: interac_info.customer_info.map(|customer_info_details| {
                                                    grpc_api_types::payments::CustomerInfo {
                                                        customer_name: customer_info_details.customer_name,
                                                        customer_email: customer_info_details.customer_email
                                                        .map(|email| Secret::new(email.clone().expose().expose())),
                                                        customer_phone_number: customer_info_details.customer_phone_number,
                                                        customer_bank_id: customer_info_details.customer_bank_id,
                                                        customer_bank_name: customer_info_details.customer_bank_name,
                                                    }
                                                }),
                                            }),
                                        }
                                    )
                                ),
                            }
                        }
                    }
                },
            ),
            extended_authorization_response_data: value
                .get_extended_authorization_response_data()
                .map(|extended_authorization_response_data| {
                    grpc_api_types::payments::ExtendedAuthorizationResponseData {
                        extended_authentication_applied: extended_authorization_response_data
                            .extended_authentication_applied,
                        extended_authorization_last_applied_at:
                            extended_authorization_response_data
                                .extended_authorization_last_applied_at
                                .map(|dt| dt.assume_utc().unix_timestamp()),
                        capture_before: extended_authorization_response_data
                            .capture_before
                            .map(|dt| dt.assume_utc().unix_timestamp()),
                    }
                }),
            is_overcapture_enabled: value.is_overcapture_enabled(),
        })
    }
}

pub fn generate_create_order_response(
    router_data_v2: RouterDataV2<
        CreateOrder,
        PaymentFlowData,
        PaymentCreateOrderData,
        PaymentCreateOrderResponse,
    >,
) -> Result<PaymentServiceCreateOrderResponse, error_stack::Report<ConnectorError>> {
    let transaction_response = router_data_v2.response;
    let status = router_data_v2.resource_common_data.status;
    let grpc_status = grpc_api_types::payments::PaymentStatus::foreign_from(status);
    let raw_connector_response = router_data_v2
        .resource_common_data
        .get_raw_connector_response();
    let raw_connector_request = router_data_v2
        .resource_common_data
        .get_raw_connector_request();
    let response_headers = router_data_v2
        .resource_common_data
        .get_connector_response_headers_as_map();

    let response = match transaction_response {
        Ok(PaymentCreateOrderResponse {
            connector_order_id,
            session_data,
        }) => {
            let grpc_session_data = session_data
                .map(grpc_api_types::payments::ClientAuthenticationTokenData::foreign_try_from)
                .transpose()?;

            PaymentServiceCreateOrderResponse {
                connector_order_id: Some(connector_order_id),
                status: grpc_status.into(),
                error: None,
                status_code: 200,
                response_headers,
                merchant_order_id: None,
                raw_connector_request,
                raw_connector_response,
                session_data: grpc_session_data,
            }
        }
        Err(err) => PaymentServiceCreateOrderResponse {
            connector_order_id: None,
            status: err
                .attempt_status
                .map(grpc_api_types::payments::PaymentStatus::foreign_from)
                .unwrap_or_default()
                .into(),
            error: Some(grpc_api_types::payments::ErrorInfo {
                unified_details: None,
                connector_details: Some(grpc_api_types::payments::ConnectorErrorDetails {
                    code: Some(err.code),
                    message: Some(err.message.clone()),
                    reason: None,
                }),
                issuer_details: None,
            }),
            status_code: err.status_code.into(),
            response_headers,
            merchant_order_id: None,
            raw_connector_request,
            raw_connector_response,
            session_data: None,
        },
    };
    Ok(response)
}

/// Helper function to convert connector_metadata from serde_json::Value to Option<Secret<String>>
/// Serializes the JSON value to a string for transmission via gRPC
fn convert_connector_metadata_to_secret_string(
    connector_metadata: Option<serde_json::Value>,
) -> Option<Secret<String>> {
    connector_metadata.and_then(|value| serde_json::to_string(&value).ok().map(Secret::new))
}

pub fn generate_payment_authorize_response<T: PaymentMethodDataTypes>(
    router_data_v2: RouterDataV2<
        Authorize,
        PaymentFlowData,
        PaymentsAuthorizeData<T>,
        PaymentsResponseData,
    >,
) -> Result<PaymentServiceAuthorizeResponse, error_stack::Report<ConnectorError>> {
    let transaction_response = router_data_v2.response;
    let status = router_data_v2.resource_common_data.status;
    info!("Payment authorize response status: {:?}", status);
    let response_headers = router_data_v2
        .resource_common_data
        .get_connector_response_headers_as_map();
    let grpc_status = grpc_api_types::payments::PaymentStatus::foreign_from(status);
    let raw_connector_response = router_data_v2
        .resource_common_data
        .get_raw_connector_response();
    let raw_connector_request = router_data_v2
        .resource_common_data
        .get_raw_connector_request();

    // Create state if either access token or connector customer is available
    let state = if router_data_v2.resource_common_data.access_token.is_some()
        || router_data_v2
            .resource_common_data
            .connector_customer
            .is_some()
    {
        Some(ConnectorState {
            access_token: router_data_v2
                .resource_common_data
                .access_token
                .as_ref()
                .map(|token_data| grpc_api_types::payments::AccessToken {
                    token: Some(token_data.access_token.clone()),
                    expires_in_seconds: token_data.expires_in,
                    token_type: token_data.token_type.clone(),
                }),
            connector_customer_id: router_data_v2
                .resource_common_data
                .connector_customer
                .clone(),
        })
    } else {
        None
    };

    let connector_response = router_data_v2
        .resource_common_data
        .connector_response
        .as_ref()
        .map(|connector_response_data| {
            grpc_api_types::payments::ConnectorResponseData::foreign_try_from(
                connector_response_data.clone(),
            )
        })
        .transpose()?;

    let response = match transaction_response {
        Ok(response) => match response {
            PaymentsResponseData::TransactionResponse {
                resource_id,
                redirection_data,
                connector_metadata,
                network_txn_id,
                connector_response_reference_id,
                incremental_authorization_allowed,
                mandate_reference,
                status_code,
            } => {
                let mandate_reference_grpc =
                    mandate_reference.map(|m| grpc_api_types::payments::MandateReference {
                        mandate_id_type: Some(grpc_api_types::payments::mandate_reference::MandateIdType::ConnectorMandateId(
                            grpc_payment_types::ConnectorMandateReferenceId {
                                connector_mandate_id: m.connector_mandate_id,
                                payment_method_id: m.payment_method_id,
                                connector_mandate_request_reference_id: m
                                    .connector_mandate_request_reference_id,
                             }
                        )),
                    });
                PaymentServiceAuthorizeResponse {
                    connector_transaction_id: Option::foreign_try_from(resource_id)?,
                    redirection_data: redirection_data
                        .map(|form| grpc_api_types::payments::RedirectForm::foreign_try_from(*form))
                        .transpose()?,
                    connector_feature_data: convert_connector_metadata_to_secret_string(
                        connector_metadata,
                    ),
                    network_transaction_id: network_txn_id,
                    merchant_transaction_id: connector_response_reference_id.clone(),
                    mandate_reference: mandate_reference_grpc,
                    incremental_authorization_allowed,
                    status: grpc_status as i32,
                    error: None,
                    raw_connector_response,
                    raw_connector_request,
                    status_code: status_code as u32,
                    response_headers,
                    state,
                    captured_amount: router_data_v2.resource_common_data.amount_captured,
                    capturable_amount: router_data_v2
                        .resource_common_data
                        .minor_amount_capturable
                        .map(|amount_capturable| amount_capturable.get_amount_as_i64()),
                    authorized_amount: router_data_v2
                        .resource_common_data
                        .minor_amount_authorized
                        .map(|amount_authorized| amount_authorized.get_amount_as_i64()),
                    connector_response,
                }
            }
            _ => {
                return Err(report!(ConnectorError::UnexpectedResponseError {
                    context: ResponseTransformationErrorContext {
                        http_status_code: None,
                        additional_context: Some(
                            "Invalid response type received from connector".to_owned()
                        ),
                    },
                }))
            }
        },
        Err(err) => {
            let status = match err.get_attempt_status_for_grpc(
                err.status_code,
                router_data_v2.resource_common_data.status,
            ) {
                Some(attempt_status) => {
                    grpc_api_types::payments::PaymentStatus::foreign_from(attempt_status)
                }
                None => grpc_api_types::payments::PaymentStatus::Unspecified,
            };

            PaymentServiceAuthorizeResponse {
                connector_transaction_id: err.connector_transaction_id.clone(),
                redirection_data: None,
                network_transaction_id: None,
                merchant_transaction_id: err.connector_transaction_id.clone(),
                mandate_reference: None,
                incremental_authorization_allowed: None,
                status: status as i32,
                error: Some(grpc_api_types::payments::ErrorInfo {
                    unified_details: None,
                    connector_details: Some(grpc_api_types::payments::ConnectorErrorDetails {
                        code: Some(err.code.clone()),
                        reason: err.reason.clone(),
                        message: Some(err.message.clone()),
                    }),
                    issuer_details: Some(grpc_api_types::payments::IssuerErrorDetails {
                        code: None, // To be filled with card network specific error code if available
                        message: err.network_error_message.clone(),
                        network_details: Some(grpc_api_types::payments::NetworkErrorDetails {
                            advice_code: err.network_advice_code,
                            decline_code: err.network_decline_code,
                            error_message: err.network_error_message.clone(),
                        }),
                    }),
                }),
                status_code: err.status_code as u32,
                response_headers,
                raw_connector_response,
                raw_connector_request,
                connector_feature_data: None,
                state,
                captured_amount: None,
                capturable_amount: None,
                authorized_amount: None,
                connector_response,
            }
        }
    };
    Ok(response)
}

// ForeignTryFrom for PaymentMethod gRPC enum to internal enum
impl ForeignTryFrom<grpc_api_types::payments::PaymentMethod> for PaymentMethod {
    type Error = IntegrationError;
    fn foreign_try_from(
        item: grpc_api_types::payments::PaymentMethod,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        match item {
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::Card(_)),
            } => Ok(Self::Card),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::CardProxy(_)),
            } => Ok(Self::Card),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::CardRedirect(_)),
            } => Ok(Self::Card),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::NetworkToken(_)),
            } => Ok(Self::NetworkToken),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::Token(_)),
            } => Ok(Self::Wallet),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::UpiCollect(_)),
            } => Ok(Self::Upi),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::UpiIntent(_)),
            } => Ok(Self::Upi),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::UpiQr(_)),
            } => Ok(Self::Upi),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::ClassicReward(_)),
            } => Ok(Self::Reward),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::EVoucher(_)),
            } => Ok(Self::Reward),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::ApplePay(_)),
            } => Ok(Self::Wallet),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::GooglePay(_)),
            } => Ok(Self::Wallet),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::SamsungPay(_)),
            } => Ok(Self::Wallet),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::ApplePayThirdPartySdk(_)),
            } => Ok(Self::Wallet),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::GooglePayThirdPartySdk(_)),
            } => Ok(Self::Wallet),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::PaypalSdk(_)),
            } => Ok(Self::Wallet),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::AmazonPayRedirect(_)),
            } => Ok(Self::Wallet),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::PaypalRedirect(_)),
            } => Ok(Self::Wallet),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::RevolutPay(_)),
            } => Ok(Self::Wallet),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::Mifinity(_)),
            } => Ok(Self::Wallet),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::Bluecode(_)),
            } => Ok(Self::Wallet),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::CashappQr(_)),
            } => Ok(Self::Wallet),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::WeChatPayQr(_)),
            } => Ok(Self::Wallet),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::WeChatPayRedirect(_)),
            } => Ok(Self::Wallet),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::AliPayRedirect(_)),
            } => Ok(Self::Wallet),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::AliPayHk(_)),
            } => Ok(Self::Wallet),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::GcashRedirect(_)),
            } => Ok(Self::Wallet),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::DanaRedirect(_)),
            } => Ok(Self::Wallet),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::GoPayRedirect(_)),
            } => Ok(Self::Wallet),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::KakaoPayRedirect(_)),
            } => Ok(Self::Wallet),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::MbWayRedirect(_)),
            } => Ok(Self::Wallet),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::MomoRedirect(_)),
            } => Ok(Self::Wallet),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::TouchNGoRedirect(_)),
            } => Ok(Self::Wallet),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::TwintRedirect(_)),
            } => Ok(Self::Wallet),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::VippsRedirect(_)),
            } => Ok(Self::Wallet),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::SwishQr(_)),
            } => Ok(Self::Wallet),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::Wero(_)),
            } => Ok(Self::Wallet),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::MbWay(_)),
            } => Ok(Self::Wallet),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::Satispay(_)),
            } => Ok(Self::Wallet),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::InstantBankTransfer(_)),
            } => Ok(Self::BankTransfer),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::SepaBankTransfer(_)),
            } => Ok(Self::BankTransfer),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::InstantBankTransferPoland(_)),
            } => Ok(Self::BankTransfer),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::InstantBankTransferFinland(_)),
            } => Ok(Self::BankTransfer),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::AchBankTransfer(_)),
            } => Ok(Self::BankTransfer),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::BacsBankTransfer(_)),
            } => Ok(Self::BankTransfer),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::MultibancoBankTransfer(_)),
            } => Ok(Self::BankTransfer),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::PermataBankTransfer(_)),
            } => Ok(Self::BankTransfer),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::BcaBankTransfer(_)),
            } => Ok(Self::BankTransfer),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::BniVaBankTransfer(_)),
            } => Ok(Self::BankTransfer),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::BriVaBankTransfer(_)),
            } => Ok(Self::BankTransfer),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::CimbVaBankTransfer(_)),
            } => Ok(Self::BankTransfer),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::DanamonVaBankTransfer(_)),
            } => Ok(Self::BankTransfer),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::MandiriVaBankTransfer(_)),
            } => Ok(Self::BankTransfer),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::Pix(_)),
            } => Ok(Self::BankTransfer),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::Pse(_)),
            } => Ok(Self::BankTransfer),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::LocalBankTransfer(_)),
            } => Ok(Self::BankTransfer),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::IndonesianBankTransfer(_)),
            } => Ok(Self::BankTransfer),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::Ideal(_)),
            } => Ok(Self::BankRedirect),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::Eps(_)),
            } => Ok(Self::BankRedirect),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::Blik(_)),
            } => Ok(Self::BankRedirect),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::Sofort(_)),
            } => Ok(Self::BankRedirect),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::BancontactCard(_)),
            } => Ok(Self::BankRedirect),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::Bizum(_)),
            } => Ok(Self::BankRedirect),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::Giropay(_)),
            } => Ok(Self::BankRedirect),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::Interac(_)),
            } => Ok(Self::BankRedirect),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::OnlineBankingCzechRepublic(_)),
            } => Ok(Self::BankRedirect),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::OnlineBankingFinland(_)),
            } => Ok(Self::BankRedirect),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::OnlineBankingPoland(_)),
            } => Ok(Self::BankRedirect),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::OnlineBankingSlovakia(_)),
            } => Ok(Self::BankRedirect),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::OpenBankingUk(_)),
            } => Ok(Self::BankRedirect),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::Przelewy24(_)),
            } => Ok(Self::BankRedirect),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::Trustly(_)),
            } => Ok(Self::BankRedirect),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::OnlineBankingFpx(_)),
            } => Ok(Self::BankRedirect),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::OnlineBankingThailand(_)),
            } => Ok(Self::BankRedirect),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::LocalBankRedirect(_)),
            } => Ok(Self::BankRedirect),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::Eft(_)),
            } => Ok(Self::BankRedirect),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::OpenBanking(_)),
            } => Ok(Self::BankRedirect),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::Affirm(_)),
            } => Ok(Self::PayLater),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::AfterpayClearpay(_)),
            } => Ok(Self::PayLater),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::Klarna(_)),
            } => Ok(Self::PayLater),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::Boleto(_)),
            } => Ok(Self::Voucher),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::Efecty(_)),
            } => Ok(Self::Voucher),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::PagoEfectivo(_)),
            } => Ok(Self::Voucher),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::RedCompra(_)),
            } => Ok(Self::Voucher),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::RedPagos(_)),
            } => Ok(Self::Voucher),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::Alfamart(_)),
            } => Ok(Self::Voucher),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::Indomaret(_)),
            } => Ok(Self::Voucher),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::Oxxo(_)),
            } => Ok(Self::Voucher),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::SevenEleven(_)),
            } => Ok(Self::Voucher),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::Lawson(_)),
            } => Ok(Self::Voucher),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::MiniStop(_)),
            } => Ok(Self::Voucher),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::FamilyMart(_)),
            } => Ok(Self::Voucher),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::Seicomart(_)),
            } => Ok(Self::Voucher),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::PayEasy(_)),
            } => Ok(Self::Voucher),
            // DIRECT DEBIT
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::Ach(_)),
            } => Ok(Self::BankDebit),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::Sepa(_)),
            } => Ok(Self::BankDebit),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::Bacs(_)),
            } => Ok(Self::BankDebit),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::Becs(_)),
            } => Ok(Self::BankDebit),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::SepaGuaranteedDebit(_)),
            } => Ok(Self::BankDebit),
            grpc_api_types::payments::PaymentMethod {
                payment_method:
                    Some(grpc_api_types::payments::payment_method::PaymentMethod::Netbanking(_)),
            } => Ok(Self::BankRedirect),
            _ => Err(report!(IntegrationError::InvalidDataFormat { field_name: "unknown", context: IntegrationErrorContext { additional_context: Some("Unsupported payment method".to_string()), ..Default::default() } })),
        }
    }
}

// ForeignTryFrom for AuthenticationType gRPC enum to internal enum
impl ForeignTryFrom<grpc_api_types::payments::AuthenticationType>
    for common_enums::AuthenticationType
{
    type Error = IntegrationError;
    fn foreign_try_from(
        item: grpc_api_types::payments::AuthenticationType,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        match item {
            grpc_api_types::payments::AuthenticationType::Unspecified => Ok(Self::NoThreeDs), // Default to NoThreeDs for unspecified
            grpc_api_types::payments::AuthenticationType::ThreeDs => Ok(Self::ThreeDs),
            grpc_api_types::payments::AuthenticationType::NoThreeDs => Ok(Self::NoThreeDs),
        }
    }
}

impl ForeignTryFrom<grpc_api_types::payments::PaymentServiceGetRequest> for PaymentsSyncData {
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::payments::PaymentServiceGetRequest,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let capture_method = Some(CaptureMethod::foreign_try_from(value.capture_method())?);
        let amount = value.amount.ok_or(IntegrationError::MissingRequiredField {
            field_name: "amount",
            context: IntegrationErrorContext::default(),
        })?;
        let currency = common_enums::Currency::foreign_try_from(amount.currency())?;
        // Create ResponseId from resource_id
        let connector_transaction_id =
            ResponseId::ConnectorTransactionId(value.connector_transaction_id.clone());

        let setup_future_usage = match value.setup_future_usage() {
            grpc_payment_types::FutureUsage::Unspecified => None,
            _ => Some(FutureUsage::foreign_try_from(value.setup_future_usage())?),
        };

        let sync_type = match value.sync_type() {
            grpc_payment_types::SyncRequestType::MultipleCaptureSync => {
                router_request_types::SyncRequestType::MultipleCaptureSync
            }
            grpc_payment_types::SyncRequestType::SinglePaymentSync
            | grpc_payment_types::SyncRequestType::Unspecified => {
                router_request_types::SyncRequestType::SinglePaymentSync
            }
        };

        let payment_experience = match value.payment_experience() {
            grpc_payment_types::PaymentExperience::Unspecified => None,
            _ => Some(common_enums::PaymentExperience::foreign_try_from(
                value.payment_experience(),
            )?),
        };

        let connector_feature_data = value
            .connector_feature_data
            .map(|m| ForeignTryFrom::foreign_try_from((m, "connector metadata")))
            .transpose()?;

        Ok(Self {
            connector_transaction_id,
            encoded_data: value.encoded_data,
            capture_method,
            connector_feature_data,
            sync_type,
            mandate_id: None,
            payment_method_type: None,
            currency,
            payment_experience,
            amount: common_utils::types::MinorUnit::new(amount.minor_amount),
            integrity_object: None,
            all_keys_required: None, // Field not available in new proto structure
            split_payments: None,
            setup_future_usage,
        })
    }
}

impl ForeignFrom<common_enums::AttemptStatus> for grpc_api_types::payments::PaymentStatus {
    fn foreign_from(status: common_enums::AttemptStatus) -> Self {
        match status {
            common_enums::AttemptStatus::Charged => Self::Charged,
            common_enums::AttemptStatus::Pending => Self::Pending,
            common_enums::AttemptStatus::Failure => Self::Failure,
            common_enums::AttemptStatus::Authorized => Self::Authorized,
            common_enums::AttemptStatus::PartiallyAuthorized => Self::PartiallyAuthorized,
            common_enums::AttemptStatus::Started => Self::Started,
            common_enums::AttemptStatus::Expired => Self::Expired,
            common_enums::AttemptStatus::AuthenticationFailed => Self::AuthenticationFailed,
            common_enums::AttemptStatus::AuthenticationPending => Self::AuthenticationPending,
            common_enums::AttemptStatus::AuthenticationSuccessful => Self::AuthenticationSuccessful,
            common_enums::AttemptStatus::Authorizing => Self::Authorizing,
            common_enums::AttemptStatus::CaptureInitiated => Self::CaptureInitiated,
            common_enums::AttemptStatus::CaptureFailed => Self::CaptureFailed,
            common_enums::AttemptStatus::VoidInitiated => Self::VoidInitiated,
            common_enums::AttemptStatus::VoidPostCaptureInitiated => Self::VoidInitiated,
            common_enums::AttemptStatus::VoidFailed => Self::VoidFailed,
            common_enums::AttemptStatus::Voided => Self::Voided,
            common_enums::AttemptStatus::VoidedPostCapture => Self::VoidedPostCapture,
            common_enums::AttemptStatus::Unresolved => Self::Unresolved,
            common_enums::AttemptStatus::PaymentMethodAwaited => Self::PaymentMethodAwaited,
            common_enums::AttemptStatus::ConfirmationAwaited => Self::ConfirmationAwaited,
            common_enums::AttemptStatus::DeviceDataCollectionPending => {
                Self::DeviceDataCollectionPending
            }
            common_enums::AttemptStatus::RouterDeclined => Self::RouterDeclined,
            common_enums::AttemptStatus::AuthorizationFailed => Self::AuthorizationFailed,
            common_enums::AttemptStatus::CodInitiated => Self::CodInitiated,
            common_enums::AttemptStatus::AutoRefunded => Self::AutoRefunded,
            common_enums::AttemptStatus::PartialCharged => Self::PartialCharged,
            common_enums::AttemptStatus::PartialChargedAndChargeable => {
                Self::PartialChargedAndChargeable
            }
            common_enums::AttemptStatus::IntegrityFailure => Self::Failure,
            common_enums::AttemptStatus::Unspecified => Self::Unspecified,
            common_enums::AttemptStatus::Unknown => Self::Unspecified,
        }
    }
}

impl ForeignFrom<common_enums::AuthorizationStatus>
    for grpc_api_types::payments::AuthorizationStatus
{
    fn foreign_from(status: common_enums::AuthorizationStatus) -> Self {
        match status {
            common_enums::AuthorizationStatus::Success => Self::AuthorizationSuccess,
            common_enums::AuthorizationStatus::Unresolved => Self::AuthorizationUnresolved,
            common_enums::AuthorizationStatus::Processing => Self::AuthorizationProcessing,
            common_enums::AuthorizationStatus::Failure => Self::AuthorizationFailure,
        }
    }
}

impl ForeignTryFrom<grpc_api_types::payments::PaymentStatus> for common_enums::AttemptStatus {
    type Error = IntegrationError;

    fn foreign_try_from(
        status: grpc_api_types::payments::PaymentStatus,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        match status {
            grpc_api_types::payments::PaymentStatus::Charged => Ok(Self::Charged),
            grpc_api_types::payments::PaymentStatus::Pending => Ok(Self::Pending),
            grpc_api_types::payments::PaymentStatus::Failure => Ok(Self::Failure),
            grpc_api_types::payments::PaymentStatus::Authorized => Ok(Self::Authorized),
            grpc_api_types::payments::PaymentStatus::Started => Ok(Self::Started),
            grpc_api_types::payments::PaymentStatus::AuthenticationFailed => {
                Ok(Self::AuthenticationFailed)
            }
            grpc_api_types::payments::PaymentStatus::AuthenticationPending => {
                Ok(Self::AuthenticationPending)
            }
            grpc_api_types::payments::PaymentStatus::AuthenticationSuccessful => {
                Ok(Self::AuthenticationSuccessful)
            }
            grpc_api_types::payments::PaymentStatus::Authorizing => Ok(Self::Authorizing),
            grpc_api_types::payments::PaymentStatus::PartiallyAuthorized => {
                Ok(Self::PartiallyAuthorized)
            }
            grpc_api_types::payments::PaymentStatus::CaptureInitiated => Ok(Self::CaptureInitiated),
            grpc_api_types::payments::PaymentStatus::CaptureFailed => Ok(Self::CaptureFailed),
            grpc_api_types::payments::PaymentStatus::VoidInitiated => Ok(Self::VoidInitiated),
            grpc_api_types::payments::PaymentStatus::VoidFailed => Ok(Self::VoidFailed),
            grpc_api_types::payments::PaymentStatus::Voided => Ok(Self::Voided),
            grpc_api_types::payments::PaymentStatus::VoidedPostCapture => {
                Ok(Self::VoidedPostCapture)
            }
            grpc_api_types::payments::PaymentStatus::Expired => Ok(Self::Expired),
            grpc_api_types::payments::PaymentStatus::Unresolved => Ok(Self::Unresolved),
            grpc_api_types::payments::PaymentStatus::PaymentMethodAwaited => {
                Ok(Self::PaymentMethodAwaited)
            }
            grpc_api_types::payments::PaymentStatus::ConfirmationAwaited => {
                Ok(Self::ConfirmationAwaited)
            }
            grpc_api_types::payments::PaymentStatus::DeviceDataCollectionPending => {
                Ok(Self::DeviceDataCollectionPending)
            }
            grpc_api_types::payments::PaymentStatus::RouterDeclined => Ok(Self::RouterDeclined),
            grpc_api_types::payments::PaymentStatus::AuthorizationFailed => {
                Ok(Self::AuthorizationFailed)
            }
            grpc_api_types::payments::PaymentStatus::CodInitiated => Ok(Self::CodInitiated),
            grpc_api_types::payments::PaymentStatus::AutoRefunded => Ok(Self::AutoRefunded),
            grpc_api_types::payments::PaymentStatus::PartialCharged => Ok(Self::PartialCharged),
            grpc_api_types::payments::PaymentStatus::PartialChargedAndChargeable => {
                Ok(Self::PartialChargedAndChargeable)
            }
            grpc_api_types::payments::PaymentStatus::Unspecified => Ok(Self::Unknown),
        }
    }
}

impl ForeignFrom<common_enums::RefundStatus> for grpc_api_types::payments::RefundStatus {
    fn foreign_from(status: common_enums::RefundStatus) -> Self {
        match status {
            common_enums::RefundStatus::Failure => Self::RefundFailure,
            common_enums::RefundStatus::ManualReview => Self::RefundManualReview,
            common_enums::RefundStatus::Pending => Self::RefundPending,
            common_enums::RefundStatus::Success => Self::RefundSuccess,
            common_enums::RefundStatus::TransactionFailure => Self::RefundTransactionFailure,
        }
    }
}

pub fn generate_payment_void_response(
    router_data_v2: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
) -> Result<PaymentServiceVoidResponse, error_stack::Report<ConnectorError>> {
    let transaction_response = router_data_v2.response;

    // Create state if either access token or connector customer is available
    let state = if router_data_v2.resource_common_data.access_token.is_some()
        || router_data_v2
            .resource_common_data
            .connector_customer
            .is_some()
    {
        Some(ConnectorState {
            access_token: router_data_v2
                .resource_common_data
                .access_token
                .as_ref()
                .map(|token_data| grpc_api_types::payments::AccessToken {
                    token: Some(token_data.access_token.clone()),
                    expires_in_seconds: token_data.expires_in,
                    token_type: token_data.token_type.clone(),
                }),
            connector_customer_id: router_data_v2
                .resource_common_data
                .connector_customer
                .clone(),
        })
    } else {
        None
    };

    let raw_connector_request = router_data_v2
        .resource_common_data
        .get_raw_connector_request();

    match transaction_response {
        Ok(response) => match response {
            PaymentsResponseData::TransactionResponse {
                resource_id,
                redirection_data: _,
                connector_metadata,
                network_txn_id: _,
                connector_response_reference_id,
                incremental_authorization_allowed,
                mandate_reference,
                status_code,
            } => {
                let status = router_data_v2.resource_common_data.status;
                let grpc_status = grpc_api_types::payments::PaymentStatus::foreign_from(status);

                let grpc_resource_id = Option::foreign_try_from(resource_id)?;

                let mandate_reference_grpc =
                    mandate_reference.map(|m| grpc_api_types::payments::MandateReference {
                        mandate_id_type: Some(grpc_api_types::payments::mandate_reference::MandateIdType::ConnectorMandateId(
                            grpc_payment_types::ConnectorMandateReferenceId {
                                connector_mandate_id: m.connector_mandate_id,
                        payment_method_id: m.payment_method_id,
                        connector_mandate_request_reference_id: m
                            .connector_mandate_request_reference_id,
                            }))
                    });

                Ok(PaymentServiceVoidResponse {
                    connector_transaction_id: extract_connector_request_reference_id(
                        &grpc_resource_id,
                    ),
                    status: grpc_status.into(),
                    merchant_void_id: connector_response_reference_id,
                    error: None,
                    status_code: u32::from(status_code),
                    response_headers: router_data_v2
                        .resource_common_data
                        .get_connector_response_headers_as_map(),
                    raw_connector_request,
                    state,
                    mandate_reference: mandate_reference_grpc,
                    incremental_authorization_allowed,
                    connector_feature_data: convert_connector_metadata_to_secret_string(
                        connector_metadata,
                    ),
                })
            }
            _ => Err(report!(ConnectorError::UnexpectedResponseError {
                context: ResponseTransformationErrorContext {
                    http_status_code: None,
                    additional_context: Some(
                        "Invalid response type received from connector".to_owned()
                    ),
                },
            })),
        },
        Err(e) => {
            let status = match e.get_attempt_status_for_grpc(
                e.status_code,
                router_data_v2.resource_common_data.status,
            ) {
                Some(attempt_status) => {
                    grpc_api_types::payments::PaymentStatus::foreign_from(attempt_status)
                }
                None => grpc_api_types::payments::PaymentStatus::Unspecified,
            };
            Ok(PaymentServiceVoidResponse {
                connector_transaction_id: extract_connector_request_reference_id(
                    &e.connector_transaction_id,
                ),
                merchant_void_id: e.connector_transaction_id,
                status: status as i32,
                error: Some(grpc_api_types::payments::ErrorInfo {
                    unified_details: None,
                    connector_details: Some(grpc_api_types::payments::ConnectorErrorDetails {
                        code: Some(e.code.clone()),
                        message: Some(e.message.clone()),
                        reason: e.reason.clone(),
                    }),
                    issuer_details: None,
                }),
                status_code: e.status_code as u32,
                response_headers: router_data_v2
                    .resource_common_data
                    .get_connector_response_headers_as_map(),
                state: None,
                raw_connector_request,
                mandate_reference: None,
                incremental_authorization_allowed: None,
                connector_feature_data: None,
            })
        }
    }
}

pub fn generate_payment_void_post_capture_response(
    router_data_v2: RouterDataV2<
        VoidPC,
        PaymentFlowData,
        crate::connector_types::PaymentsCancelPostCaptureData,
        PaymentsResponseData,
    >,
) -> Result<PaymentServiceReverseResponse, error_stack::Report<ConnectorError>> {
    let transaction_response = router_data_v2.response;

    // If there's an access token in PaymentFlowData, it must be newly generated (needs caching)
    let _state = router_data_v2
        .resource_common_data
        .access_token
        .as_ref()
        .map(|token_data| ConnectorState {
            access_token: Some(grpc_api_types::payments::AccessToken {
                token: Some(token_data.access_token.clone()),
                expires_in_seconds: token_data.expires_in,
                token_type: token_data.token_type.clone(),
            }),
            connector_customer_id: router_data_v2
                .resource_common_data
                .connector_customer
                .clone(),
        });

    match transaction_response {
        Ok(response) => match response {
            PaymentsResponseData::TransactionResponse {
                resource_id,
                redirection_data: _,
                connector_metadata: _,
                network_txn_id: _,
                connector_response_reference_id,
                incremental_authorization_allowed: _,
                mandate_reference: _,
                status_code,
            } => {
                let status = router_data_v2.resource_common_data.status;
                let grpc_status = grpc_api_types::payments::PaymentStatus::foreign_from(status);

                let grpc_resource_id = Option::foreign_try_from(resource_id)?;

                Ok(PaymentServiceReverseResponse {
                    connector_transaction_id: extract_connector_request_reference_id(
                        &grpc_resource_id,
                    ),
                    status: grpc_status.into(),
                    merchant_reverse_id: connector_response_reference_id,
                    error: None,
                    status_code: u32::from(status_code),
                    response_headers: router_data_v2
                        .resource_common_data
                        .get_connector_response_headers_as_map(),
                })
            }
            _ => Err(report!(ConnectorError::UnexpectedResponseError {
                context: ResponseTransformationErrorContext {
                    http_status_code: None,
                    additional_context: Some(
                        "Invalid response type received from connector".to_owned()
                    ),
                },
            })),
        },
        Err(e) => {
            let status = match e.get_attempt_status_for_grpc(
                e.status_code,
                router_data_v2.resource_common_data.status,
            ) {
                Some(attempt_status) => {
                    grpc_api_types::payments::PaymentStatus::foreign_from(attempt_status)
                }
                None => grpc_api_types::payments::PaymentStatus::Unspecified,
            };
            Ok(PaymentServiceReverseResponse {
                connector_transaction_id: extract_connector_request_reference_id(
                    &e.connector_transaction_id,
                ),
                status: status.into(),
                merchant_reverse_id: e.connector_transaction_id,
                error: Some(grpc_api_types::payments::ErrorInfo {
                    unified_details: None,
                    connector_details: Some(grpc_api_types::payments::ConnectorErrorDetails {
                        code: Some(e.code.clone()),
                        message: Some(e.message.clone()),
                        reason: e.reason.clone(),
                    }),
                    issuer_details: None,
                }),
                status_code: u32::from(e.status_code),
                response_headers: router_data_v2
                    .resource_common_data
                    .get_connector_response_headers_as_map(),
            })
        }
    }
}

impl ForeignFrom<common_enums::DisputeStage> for grpc_api_types::payments::DisputeStage {
    fn foreign_from(status: common_enums::DisputeStage) -> Self {
        match status {
            common_enums::DisputeStage::PreDispute => Self::PreDispute,
            common_enums::DisputeStage::Dispute => Self::ActiveDispute,
            common_enums::DisputeStage::PreArbitration => Self::PreArbitration,
        }
    }
}

impl ForeignFrom<grpc_api_types::payments::ProductType> for common_enums::ProductType {
    fn foreign_from(value: grpc_api_types::payments::ProductType) -> Self {
        match value {
            grpc_api_types::payments::ProductType::Unspecified
            | grpc_api_types::payments::ProductType::Physical => Self::Physical,
            grpc_api_types::payments::ProductType::Digital => Self::Digital,
            grpc_api_types::payments::ProductType::Travel => Self::Travel,
            grpc_api_types::payments::ProductType::Ride => Self::Ride,
            grpc_api_types::payments::ProductType::Event => Self::Event,
            grpc_api_types::payments::ProductType::Accommodation => Self::Accommodation,
        }
    }
}

pub fn generate_access_token_response_data(
    router_data_v2: RouterDataV2<
        ServerAuthenticationToken,
        PaymentFlowData,
        ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData,
    >,
) -> Result<ServerAuthenticationTokenResponseData, router_data::ErrorResponse> {
    match router_data_v2.response {
        Ok(access_token_data) => {
            tracing::info!(
                "Access token created successfully with expiry: {:?}",
                access_token_data.expires_in
            );
            Ok(access_token_data)
        }
        Err(err) => Err(err),
    }
}

pub fn create_server_authentication_token_data(
    access_token_data: ServerAuthenticationTokenResponseData,
) -> MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse {
    MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse {
        access_token: Some(access_token_data.access_token),
        token_type: access_token_data.token_type,
        expires_in_seconds: access_token_data.expires_in,
        status: i32::from(grpc_api_types::payments::OperationStatus::Success),
        error: None,
        status_code: 200,
        merchant_access_token_id: None,
    }
}

pub fn generate_access_token_response(
    router_data_v2: RouterDataV2<
        ServerAuthenticationToken,
        PaymentFlowData,
        ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData,
    >,
) -> Result<
    MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse,
    error_stack::Report<ConnectorError>,
> {
    match generate_access_token_response_data(router_data_v2) {
        Ok(access_token_data) => Ok(create_server_authentication_token_data(access_token_data)),
        Err(error_response) => Ok(
            MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse {
                access_token: None,
                token_type: None,
                expires_in_seconds: None,
                status: i32::from(grpc_api_types::payments::OperationStatus::Failure),
                error: Some(grpc_api_types::payments::ErrorInfo {
                    unified_details: None,
                    connector_details: Some(grpc_api_types::payments::ConnectorErrorDetails {
                        message: Some(error_response.message),
                        code: Some(error_response.code),
                        reason: error_response.reason,
                    }),
                    issuer_details: None,
                }),
                status_code: error_response.status_code.into(),
                merchant_access_token_id: None,
            },
        ),
    }
}

pub fn generate_payment_sync_response(
    router_data_v2: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
) -> Result<PaymentServiceGetResponse, error_stack::Report<ConnectorError>> {
    let transaction_response = router_data_v2.response;
    let raw_connector_response = router_data_v2
        .resource_common_data
        .get_raw_connector_response();

    // Create state if either access token or connector customer is available
    let state = if router_data_v2.resource_common_data.access_token.is_some()
        || router_data_v2
            .resource_common_data
            .connector_customer
            .is_some()
    {
        Some(ConnectorState {
            access_token: router_data_v2
                .resource_common_data
                .access_token
                .as_ref()
                .map(|token_data| grpc_api_types::payments::AccessToken {
                    token: Some(token_data.access_token.clone()),
                    expires_in_seconds: token_data.expires_in,
                    token_type: token_data.token_type.clone(),
                }),
            connector_customer_id: router_data_v2
                .resource_common_data
                .connector_customer
                .clone(),
        })
    } else {
        None
    };

    let raw_connector_request = router_data_v2
        .resource_common_data
        .get_raw_connector_request();

    let connector_response = router_data_v2
        .resource_common_data
        .connector_response
        .as_ref()
        .map(|connector_response_data| {
            grpc_api_types::payments::ConnectorResponseData::foreign_try_from(
                connector_response_data.clone(),
            )
        })
        .transpose()?;

    match transaction_response {
        Ok(response) => match response {
            PaymentsResponseData::TransactionResponse {
                resource_id,
                redirection_data,
                connector_metadata: _,
                network_txn_id,
                connector_response_reference_id,
                incremental_authorization_allowed,
                mandate_reference,
                status_code,
            } => {
                let status = router_data_v2.resource_common_data.status;
                let grpc_status = grpc_api_types::payments::PaymentStatus::foreign_from(status);

                let grpc_resource_id = Option::foreign_try_from(resource_id)?;

                let mandate_reference_grpc =
                    mandate_reference.map(|m| grpc_api_types::payments::MandateReference {
                        mandate_id_type: Some(grpc_api_types::payments::mandate_reference::MandateIdType::ConnectorMandateId(
                            grpc_payment_types::ConnectorMandateReferenceId {
                                connector_mandate_id: m.connector_mandate_id,
                        payment_method_id: m.payment_method_id,
                        connector_mandate_request_reference_id: m
                            .connector_mandate_request_reference_id,
                            }))
                    });

                let amount = router_data_v2
                    .resource_common_data
                    .amount
                    .as_ref()
                    .map(|money| {
                        grpc_api_types::payments::Currency::foreign_try_from(money.currency).map(
                            |currency| grpc_api_types::payments::Money {
                                minor_amount: money.amount.get_amount_as_i64(),
                                currency: currency as i32,
                            },
                        )
                    })
                    .transpose()?;

                Ok(PaymentServiceGetResponse {
                    connector_transaction_id: extract_connector_request_reference_id(
                        &grpc_resource_id,
                    ),
                    merchant_transaction_id: connector_response_reference_id,
                    redirection_data: redirection_data
                        .map(|form| grpc_api_types::payments::RedirectForm::foreign_try_from(*form))
                        .transpose()?,
                    status: grpc_status as i32,
                    mandate_reference: mandate_reference_grpc,
                    error: None,
                    network_transaction_id: network_txn_id,
                    amount,
                    captured_amount: router_data_v2.resource_common_data.amount_captured,
                    payment_method_type: None,
                    capture_method: None,
                    auth_type: None,
                    created_at: None,
                    updated_at: None,
                    authorized_at: None,
                    captured_at: None,
                    customer_name: None,
                    email: None,
                    connector_customer_id: None,
                    merchant_order_id: None,
                    metadata: None,
                    status_code: status_code as u32,
                    raw_connector_response,
                    response_headers: router_data_v2
                        .resource_common_data
                        .get_connector_response_headers_as_map(),
                    state,
                    raw_connector_request,
                    connector_response,
                    incremental_authorization_allowed,
                    payment_method_update: None,
                })
            }
            _ => Err(report!(ConnectorError::UnexpectedResponseError {
                context: ResponseTransformationErrorContext {
                    http_status_code: None,
                    additional_context: Some(
                        "Invalid response type received from connector".to_owned()
                    ),
                },
            })),
        },
        Err(e) => {
            let status = match e.get_attempt_status_for_grpc(
                e.status_code,
                router_data_v2.resource_common_data.status,
            ) {
                Some(attempt_status) => {
                    grpc_api_types::payments::PaymentStatus::foreign_from(attempt_status)
                }
                None => grpc_api_types::payments::PaymentStatus::Unspecified,
            };
            Ok(PaymentServiceGetResponse {
                connector_transaction_id: extract_connector_request_reference_id(
                    &e.connector_transaction_id,
                ),
                merchant_transaction_id: None,
                mandate_reference: None,
                status: status as i32,
                error: Some(grpc_api_types::payments::ErrorInfo {
                    unified_details: None,
                    connector_details: Some(grpc_api_types::payments::ConnectorErrorDetails {
                        message: Some(e.message),
                        code: Some(e.code),
                        reason: e.reason,
                    }),
                    issuer_details: Some(grpc_payment_types::IssuerErrorDetails {
                        code: None,
                        message: e.network_error_message.clone(),
                        network_details: Some(grpc_api_types::payments::NetworkErrorDetails {
                            advice_code: e.network_advice_code,
                            decline_code: e.network_decline_code,
                            error_message: e.network_error_message,
                        }),
                    }),
                }),
                network_transaction_id: None,
                amount: None,
                captured_amount: None,
                payment_method_type: None,
                capture_method: None,
                auth_type: None,
                created_at: None,
                updated_at: None,
                authorized_at: None,
                captured_at: None,
                customer_name: None,
                email: None,
                connector_customer_id: None,
                merchant_order_id: None,
                metadata: None,
                raw_connector_response,
                status_code: e.status_code as u32,
                response_headers: router_data_v2
                    .resource_common_data
                    .get_connector_response_headers_as_map(),
                state,
                raw_connector_request,
                connector_response,
                redirection_data: None,
                incremental_authorization_allowed: None,
                payment_method_update: None,
            })
        }
    }
}

impl ForeignTryFrom<grpc_api_types::payments::RefundServiceGetRequest> for RefundSyncData {
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::payments::RefundServiceGetRequest,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        // Extract connector_transaction_id
        let connector_transaction_id = value.connector_transaction_id;

        Ok(Self {
            browser_info: value
                .browser_info
                .map(BrowserInformation::foreign_try_from)
                .transpose()?,
            connector_transaction_id,
            connector_refund_id: value.refund_id.clone(),
            reason: value.refund_reason.clone(),
            refund_status: common_enums::RefundStatus::Pending,
            refund_connector_metadata: value
                .refund_metadata
                .map(|m| ForeignTryFrom::foreign_try_from((m, "refund metadata")))
                .transpose()?,
            all_keys_required: None, // Field not available in new proto structure
            integrity_object: None,
            split_refunds: None,
            connector_feature_data: value
                .connector_feature_data
                .map(|m| ForeignTryFrom::foreign_try_from((m, "merchant account metadata")))
                .transpose()?,
        })
    }
}

impl
    ForeignTryFrom<(
        grpc_api_types::payments::RefundServiceGetRequest,
        Connectors,
        &MaskedMetadata,
    )> for RefundFlowData
{
    type Error = IntegrationError;

    fn foreign_try_from(
        (value, connectors, metadata): (
            grpc_api_types::payments::RefundServiceGetRequest,
            Connectors,
            &MaskedMetadata,
        ),
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let access_token = value
            .state
            .as_ref()
            .and_then(|state| state.access_token.as_ref())
            .map(ServerAuthenticationTokenResponseData::foreign_try_from)
            .transpose()?;

        let connector_feature_data = value
            .connector_feature_data
            .map(|m| ForeignTryFrom::foreign_try_from((m, "merchant account metadata")))
            .transpose()?;

        let payment_method = value
            .payment_method_type
            .map(|pm_type_i32| {
                // Convert i32 to gRPC PaymentMethodType enum
                let grpc_pm_type =
                    grpc_api_types::payments::PaymentMethodType::try_from(pm_type_i32)
                        .unwrap_or(grpc_api_types::payments::PaymentMethodType::Unspecified);

                // Convert from gRPC enum to internal PaymentMethod using ForeignTryFrom
                PaymentMethod::foreign_try_from(grpc_pm_type)
            })
            .transpose()?;

        let merchant_id_from_header = extract_merchant_id_from_metadata(metadata)?;
        Ok(Self {
            merchant_id: merchant_id_from_header,
            connector_request_reference_id: extract_connector_request_reference_id(
                &value.merchant_refund_id,
            ),

            status: common_enums::RefundStatus::Pending,
            refund_id: None,
            connectors,
            raw_connector_response: None,
            raw_connector_request: None,
            connector_response_headers: None,
            access_token,
            connector_feature_data,
            test_mode: value.test_mode,
            payment_method,
        })
    }
}

impl
    ForeignTryFrom<(
        grpc_api_types::payments::PaymentServiceRefundRequest,
        Connectors,
        &MaskedMetadata,
    )> for RefundFlowData
{
    type Error = IntegrationError;

    fn foreign_try_from(
        (value, connectors, metadata): (
            grpc_api_types::payments::PaymentServiceRefundRequest,
            Connectors,
            &MaskedMetadata,
        ),
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let access_token = value
            .state
            .as_ref()
            .and_then(|state| state.access_token.as_ref())
            .map(ServerAuthenticationTokenResponseData::foreign_try_from)
            .transpose()?;

        let connector_feature_data = value
            .connector_feature_data
            .map(|m| ForeignTryFrom::foreign_try_from((m, "merchant account metadata")))
            .transpose()?;

        let payment_method = value
            .payment_method_type
            .map(|pm_type_i32| {
                // Convert i32 to gRPC PaymentMethodType enum
                let grpc_pm_type =
                    grpc_api_types::payments::PaymentMethodType::try_from(pm_type_i32)
                        .unwrap_or(grpc_api_types::payments::PaymentMethodType::Unspecified);

                // Convert from gRPC enum to internal PaymentMethod using ForeignTryFrom
                PaymentMethod::foreign_try_from(grpc_pm_type)
            })
            .transpose()?;

        let merchant_id_from_header = extract_merchant_id_from_metadata(metadata)?;
        let refund_id = value.merchant_refund_id.clone();
        Ok(Self {
            merchant_id: merchant_id_from_header,
            connector_request_reference_id: extract_connector_request_reference_id(&refund_id),
            status: common_enums::RefundStatus::Pending,
            refund_id,
            connectors,
            raw_connector_response: None,
            raw_connector_request: None,
            connector_response_headers: None,
            access_token,
            connector_feature_data,
            test_mode: value.test_mode,
            payment_method,
        })
    }
}

impl ForeignTryFrom<grpc_api_types::payments::PaymentMethodType> for PaymentMethod {
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::payments::PaymentMethodType,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        match value {
            grpc_api_types::payments::PaymentMethodType::Credit => Ok(Self::Card),
            grpc_api_types::payments::PaymentMethodType::Debit => Ok(Self::Card),

            grpc_api_types::payments::PaymentMethodType::ApplePay => Ok(Self::Wallet),
            grpc_api_types::payments::PaymentMethodType::GooglePay => Ok(Self::Wallet),
            grpc_api_types::payments::PaymentMethodType::AmazonPay => Ok(Self::Wallet),
            grpc_api_types::payments::PaymentMethodType::PayPal => Ok(Self::Wallet),
            grpc_api_types::payments::PaymentMethodType::WeChatPay => Ok(Self::Wallet),
            grpc_api_types::payments::PaymentMethodType::AliPay => Ok(Self::Wallet),
            grpc_api_types::payments::PaymentMethodType::Cashapp => Ok(Self::Wallet),
            grpc_api_types::payments::PaymentMethodType::RevolutPay => Ok(Self::Wallet),
            grpc_api_types::payments::PaymentMethodType::MbWay => Ok(Self::Wallet),
            grpc_api_types::payments::PaymentMethodType::Satispay => Ok(Self::Wallet),
            grpc_api_types::payments::PaymentMethodType::Wero => Ok(Self::Wallet),

            grpc_api_types::payments::PaymentMethodType::UpiCollect => Ok(Self::Upi),
            grpc_api_types::payments::PaymentMethodType::UpiIntent => Ok(Self::Upi),

            grpc_api_types::payments::PaymentMethodType::Affirm => Ok(Self::PayLater),
            grpc_api_types::payments::PaymentMethodType::AfterpayClearpay => Ok(Self::PayLater),
            grpc_api_types::payments::PaymentMethodType::Alma => Ok(Self::PayLater),
            grpc_api_types::payments::PaymentMethodType::Atome => Ok(Self::PayLater),

            grpc_api_types::payments::PaymentMethodType::BancontactCard => Ok(Self::BankRedirect),
            grpc_api_types::payments::PaymentMethodType::Ideal => Ok(Self::BankRedirect),
            grpc_api_types::payments::PaymentMethodType::Sofort => Ok(Self::BankRedirect),
            grpc_api_types::payments::PaymentMethodType::TrustlyBankRedirect => {
                Ok(Self::BankRedirect)
            }
            grpc_api_types::payments::PaymentMethodType::Giropay => Ok(Self::BankRedirect),
            grpc_api_types::payments::PaymentMethodType::Eps => Ok(Self::BankRedirect),
            grpc_api_types::payments::PaymentMethodType::Przelewy24 => Ok(Self::BankRedirect),
            grpc_api_types::payments::PaymentMethodType::Blik => Ok(Self::BankRedirect),
            grpc_api_types::payments::PaymentMethodType::Bizum => Ok(Self::BankRedirect),
            grpc_api_types::payments::PaymentMethodType::OpenBankingUk => Ok(Self::BankRedirect),
            grpc_api_types::payments::PaymentMethodType::OnlineBankingFpx => Ok(Self::BankRedirect),

            grpc_api_types::payments::PaymentMethodType::Ach => Ok(Self::BankTransfer),
            grpc_api_types::payments::PaymentMethodType::Sepa => Ok(Self::BankTransfer),
            grpc_api_types::payments::PaymentMethodType::Bacs => Ok(Self::BankTransfer),

            grpc_api_types::payments::PaymentMethodType::ClassicReward => Ok(Self::Reward),
            grpc_api_types::payments::PaymentMethodType::Evoucher => Ok(Self::Reward),

            grpc_api_types::payments::PaymentMethodType::CryptoCurrency => Ok(Self::Crypto),

            grpc_api_types::payments::PaymentMethodType::DuitNow => Ok(Self::RealTimePayment),

            grpc_api_types::payments::PaymentMethodType::Boleto => Ok(Self::Voucher),
            grpc_api_types::payments::PaymentMethodType::Oxxo => Ok(Self::Voucher),
            grpc_api_types::payments::PaymentMethodType::CardRedirect => Ok(Self::CardRedirect),
            grpc_api_types::payments::PaymentMethodType::Knet => Ok(Self::CardRedirect),
            grpc_api_types::payments::PaymentMethodType::Benefit => Ok(Self::CardRedirect),
            grpc_api_types::payments::PaymentMethodType::MomoAtm => Ok(Self::CardRedirect),

            grpc_api_types::payments::PaymentMethodType::NetworkToken => Ok(Self::Card),

            grpc_api_types::payments::PaymentMethodType::Netbanking => Ok(Self::BankRedirect),

            _ => Err(IntegrationError::InvalidDataFormat {
                field_name: "payment_method_type",
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "This payment method type cannot be mapped to a high-level category"
                            .to_string(),
                    ),
                    ..Default::default()
                },
            }
            .into()),
        }
    }
}

impl ForeignFrom<common_enums::DisputeStatus> for grpc_api_types::payments::DisputeStatus {
    fn foreign_from(status: common_enums::DisputeStatus) -> Self {
        match status {
            common_enums::DisputeStatus::DisputeOpened => Self::DisputeOpened,
            common_enums::DisputeStatus::DisputeAccepted => Self::DisputeAccepted,
            common_enums::DisputeStatus::DisputeCancelled => Self::DisputeCancelled,
            common_enums::DisputeStatus::DisputeChallenged => Self::DisputeChallenged,
            common_enums::DisputeStatus::DisputeExpired => Self::DisputeExpired,
            common_enums::DisputeStatus::DisputeLost => Self::DisputeLost,
            common_enums::DisputeStatus::DisputeWon => Self::DisputeWon,
        }
    }
}

impl ForeignFrom<Method> for grpc_api_types::payments::HttpMethod {
    fn foreign_from(method: Method) -> Self {
        match method {
            Method::Post => Self::Post,
            Method::Get => Self::Get,
            Method::Put => Self::Put,
            Method::Delete => Self::Delete,
            Method::Patch => Self::Post, // Patch is not defined in gRPC, using Post
                                         // as a fallback
        }
    }
}

impl ForeignTryFrom<router_response_types::RedirectForm>
    for grpc_api_types::payments::RedirectForm
{
    type Error = ConnectorError;

    fn foreign_try_from(
        form: router_response_types::RedirectForm,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        match form {
            router_response_types::RedirectForm::Form {
                endpoint,
                method,
                form_fields,
            } => Ok(Self {
                form_type: Some(grpc_api_types::payments::redirect_form::FormType::Form(
                    grpc_api_types::payments::FormData {
                        endpoint,
                        method: grpc_api_types::payments::HttpMethod::foreign_from(method) as i32,
                        form_fields,
                    },
                )),
            }),
            router_response_types::RedirectForm::Html { html_data } => Ok(Self {
                form_type: Some(grpc_api_types::payments::redirect_form::FormType::Html(
                    grpc_api_types::payments::HtmlData { html_data },
                )),
            }),
            router_response_types::RedirectForm::Uri { uri } => Ok(Self {
                form_type: Some(grpc_api_types::payments::redirect_form::FormType::Uri(
                    grpc_api_types::payments::UriData { uri },
                )),
            }),
            router_response_types::RedirectForm::Mifinity {
                initialization_token,
            } => Ok(Self {
                form_type: Some(grpc_api_types::payments::redirect_form::FormType::Mifinity(
                    grpc_api_types::payments::MifinityData {
                        initialization_token,
                    },
                )),
            }),
            router_response_types::RedirectForm::Braintree {
                client_token,
                card_token,
                bin,
                acs_url,
            } => Ok(Self {
                form_type: Some(
                    grpc_api_types::payments::redirect_form::FormType::Braintree(
                        grpc_api_types::payments::BraintreeData {
                            client_token,
                            card_token,
                            bin,
                            acs_url,
                        },
                    ),
                ),
            }),
            router_response_types::RedirectForm::Nmi {
                amount,
                public_key,
                customer_vault_id,
                order_id,
                continue_redirection_url,
            } => Ok(Self {
                form_type: Some(grpc_api_types::payments::redirect_form::FormType::Nmi(
                    grpc_api_types::payments::NmiData {
                        amount: Some(amount),
                        public_key: Some(public_key),
                        customer_vault_id,
                        order_id,
                        continue_redirection_url,
                    },
                )),
            }),
            // Variants not supported in gRPC proto
            router_response_types::RedirectForm::BlueSnap { .. }
            | router_response_types::RedirectForm::CybersourceAuthSetup { .. }
            | router_response_types::RedirectForm::CybersourceConsumerAuth { .. }
            | router_response_types::RedirectForm::DeutschebankThreeDSChallengeFlow { .. }
            | router_response_types::RedirectForm::Payme
            | router_response_types::RedirectForm::WorldpayDDCForm { .. } => {
                Err(report!(ConnectorError::UnexpectedResponseError {
                    context: ResponseTransformationErrorContext {
                        http_status_code: None,
                        additional_context: Some(
                            "RedirectForm type not supported in gRPC API from connector response"
                                .to_string(),
                        ),
                    },
                }))
            }
        }
    }
}

pub fn generate_accept_dispute_response(
    router_data_v2: RouterDataV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>,
) -> Result<DisputeServiceAcceptResponse, error_stack::Report<ConnectorError>> {
    let dispute_response = router_data_v2.response;
    let response_headers = router_data_v2
        .resource_common_data
        .get_connector_response_headers_as_map();

    let raw_connector_request = router_data_v2
        .resource_common_data
        .get_raw_connector_request();

    match dispute_response {
        Ok(response) => {
            let grpc_status =
                grpc_api_types::payments::DisputeStatus::foreign_from(response.dispute_status);

            Ok(DisputeServiceAcceptResponse {
                dispute_status: grpc_status.into(),
                dispute_id: response.connector_dispute_id,
                connector_status_code: None,
                error: None,
                merchant_dispute_id: None,
                status_code: response.status_code as u32,
                response_headers,
                raw_connector_request,
            })
        }
        Err(e) => {
            let grpc_dispute_status = grpc_api_types::payments::DisputeStatus::default();

            Ok(DisputeServiceAcceptResponse {
                dispute_status: grpc_dispute_status as i32,
                dispute_id: e.connector_transaction_id.unwrap_or_default(),
                connector_status_code: None,
                error: Some(grpc_api_types::payments::ErrorInfo {
                    unified_details: None,
                    connector_details: Some(grpc_api_types::payments::ConnectorErrorDetails {
                        message: Some(e.message),
                        code: Some(e.code),
                        reason: e.reason,
                    }),
                    issuer_details: None,
                }),
                merchant_dispute_id: None,
                status_code: e.status_code as u32,
                response_headers,
                raw_connector_request,
            })
        }
    }
}

impl
    ForeignTryFrom<(
        grpc_api_types::payments::DisputeServiceAcceptRequest,
        Connectors,
    )> for DisputeFlowData
{
    type Error = IntegrationError;

    fn foreign_try_from(
        (value, connectors): (
            grpc_api_types::payments::DisputeServiceAcceptRequest,
            Connectors,
        ),
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        Ok(Self {
            dispute_id: None,
            connectors,
            connector_dispute_id: value.dispute_id,
            defense_reason_code: None,
            connector_request_reference_id: extract_connector_request_reference_id(
                &value.merchant_dispute_id.clone(),
            ),
            raw_connector_response: None,
            raw_connector_request: None,
            connector_response_headers: None,
        })
    }
}

impl
    ForeignTryFrom<(
        grpc_api_types::payments::DisputeServiceAcceptRequest,
        Connectors,
        &MaskedMetadata,
    )> for DisputeFlowData
{
    type Error = IntegrationError;

    fn foreign_try_from(
        (value, connectors, _metadata): (
            grpc_api_types::payments::DisputeServiceAcceptRequest,
            Connectors,
            &MaskedMetadata,
        ),
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        Ok(Self {
            connector_request_reference_id: extract_connector_request_reference_id(
                &value.merchant_dispute_id.clone(),
            ),
            dispute_id: None,
            connectors,
            connector_dispute_id: value.dispute_id,
            defense_reason_code: None,
            raw_connector_response: None,
            raw_connector_request: None,
            connector_response_headers: None,
        })
    }
}

pub fn generate_submit_evidence_response(
    router_data_v2: RouterDataV2<
        SubmitEvidence,
        DisputeFlowData,
        SubmitEvidenceData,
        DisputeResponseData,
    >,
) -> Result<DisputeServiceSubmitEvidenceResponse, error_stack::Report<ConnectorError>> {
    let dispute_response = router_data_v2.response;
    let response_headers = router_data_v2
        .resource_common_data
        .get_connector_response_headers_as_map();

    let raw_connector_request = router_data_v2
        .resource_common_data
        .get_raw_connector_request();

    match dispute_response {
        Ok(response) => {
            let grpc_status =
                grpc_api_types::payments::DisputeStatus::foreign_from(response.dispute_status);

            Ok(DisputeServiceSubmitEvidenceResponse {
                dispute_status: grpc_status.into(),
                dispute_id: Some(response.connector_dispute_id),
                submitted_evidence_ids: vec![],
                connector_status_code: None,
                error: None,
                merchant_dispute_id: None,
                status_code: response.status_code as u32,
                response_headers,
                raw_connector_request,
            })
        }
        Err(e) => {
            let grpc_attempt_status = e
                .attempt_status
                .map(grpc_api_types::payments::PaymentStatus::foreign_from)
                .unwrap_or_default();

            Ok(DisputeServiceSubmitEvidenceResponse {
                dispute_status: grpc_attempt_status.into(),
                dispute_id: e.connector_transaction_id,
                submitted_evidence_ids: vec![],
                connector_status_code: None,
                error: Some(grpc_api_types::payments::ErrorInfo {
                    unified_details: None,
                    connector_details: Some(grpc_api_types::payments::ConnectorErrorDetails {
                        message: Some(e.message),
                        code: Some(e.code),
                        reason: e.reason,
                    }),
                    issuer_details: None,
                }),
                merchant_dispute_id: None,
                status_code: e.status_code as u32,
                response_headers,
                raw_connector_request,
            })
        }
    }
}

impl
    ForeignTryFrom<(
        grpc_api_types::payments::DisputeServiceSubmitEvidenceRequest,
        Connectors,
    )> for DisputeFlowData
{
    type Error = IntegrationError;

    fn foreign_try_from(
        (value, connectors): (
            grpc_api_types::payments::DisputeServiceSubmitEvidenceRequest,
            Connectors,
        ),
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        Ok(Self {
            dispute_id: None,
            connectors,
            connector_dispute_id: value.dispute_id,
            defense_reason_code: None,
            connector_request_reference_id: value.merchant_dispute_id.unwrap_or_default(),
            raw_connector_response: None,
            raw_connector_request: None,
            connector_response_headers: None,
        })
    }
}

impl
    ForeignTryFrom<(
        grpc_api_types::payments::DisputeServiceSubmitEvidenceRequest,
        Connectors,
        &MaskedMetadata,
    )> for DisputeFlowData
{
    type Error = IntegrationError;

    fn foreign_try_from(
        (value, connectors, _metadata): (
            grpc_api_types::payments::DisputeServiceSubmitEvidenceRequest,
            Connectors,
            &MaskedMetadata,
        ),
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        Ok(Self {
            connector_request_reference_id: extract_connector_request_reference_id(
                &value.merchant_dispute_id,
            ),

            dispute_id: None,
            connectors,
            connector_dispute_id: value.dispute_id,
            defense_reason_code: None,
            raw_connector_response: None,
            raw_connector_request: None,
            connector_response_headers: None,
        })
    }
}

pub fn generate_refund_sync_response(
    router_data_v2: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
) -> Result<RefundResponse, error_stack::Report<ConnectorError>> {
    let refunds_response = router_data_v2.response;
    let raw_connector_response = router_data_v2
        .resource_common_data
        .get_raw_connector_response();

    let raw_connector_request = router_data_v2
        .resource_common_data
        .get_raw_connector_request();

    match refunds_response {
        Ok(response) => {
            let status = response.refund_status;
            let grpc_status = grpc_api_types::payments::RefundStatus::foreign_from(status);
            let response_headers = router_data_v2
                .resource_common_data
                .get_connector_response_headers_as_map();
            Ok(RefundResponse {
                connector_transaction_id: Some(
                    router_data_v2.request.connector_transaction_id.clone(),
                ),
                connector_refund_id: response.connector_refund_id.clone(),
                status: grpc_status as i32,
                merchant_refund_id: Some(response.connector_refund_id.clone()),
                error: None,
                refund_amount: None,
                payment_amount: None,
                refund_reason: None,
                created_at: None,
                updated_at: None,
                processed_at: None,
                customer_name: None,
                email: None,
                merchant_order_id: None,
                metadata: None,
                refund_metadata: None,
                raw_connector_response,
                status_code: response.status_code as u32,
                response_headers,
                state: None,
                raw_connector_request,
                acquirer_reference_number: None,
            })
        }
        Err(e) => {
            let status = e
                .attempt_status
                .map(grpc_api_types::payments::PaymentStatus::foreign_from)
                .unwrap_or_default();
            let response_headers = router_data_v2
                .resource_common_data
                .get_connector_response_headers_as_map();

            Ok(RefundResponse {
                connector_transaction_id: e.connector_transaction_id.clone(),
                connector_refund_id: String::new(),
                status: status as i32,
                merchant_refund_id: e.connector_transaction_id,
                error: Some(grpc_api_types::payments::ErrorInfo {
                    unified_details: None,
                    connector_details: Some(grpc_api_types::payments::ConnectorErrorDetails {
                        message: Some(e.message),
                        code: Some(e.code),
                        reason: e.reason,
                    }),
                    issuer_details: None,
                }),
                refund_amount: None,
                payment_amount: None,
                refund_reason: None,
                created_at: None,
                updated_at: None,
                processed_at: None,
                customer_name: None,
                email: None,
                raw_connector_response,
                merchant_order_id: None,
                metadata: None,
                refund_metadata: None,
                status_code: e.status_code as u32,
                response_headers,
                state: None,
                raw_connector_request,
                acquirer_reference_number: None,
            })
        }
    }
}
impl ForeignTryFrom<WebhookDetailsResponse> for PaymentServiceGetResponse {
    type Error = ConnectorError;

    fn foreign_try_from(
        value: WebhookDetailsResponse,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let status = grpc_api_types::payments::PaymentStatus::foreign_from(value.status);
        let response_headers = value
            .response_headers
            .map(|headers| {
                headers
                    .iter()
                    .filter_map(|(name, value)| {
                        value
                            .to_str()
                            .ok()
                            .map(|v| (name.to_string(), v.to_string()))
                    })
                    .collect()
            })
            .unwrap_or_default();
        let mandate_reference_grpc =
            value
                .mandate_reference
                .map(|m| {
                    grpc_api_types::payments::MandateReference {
                mandate_id_type: Some(
                    grpc_api_types::payments::mandate_reference::MandateIdType::ConnectorMandateId(
                        grpc_payment_types::ConnectorMandateReferenceId {
                            connector_mandate_id: m.connector_mandate_id,
                            payment_method_id: m.payment_method_id,
                            connector_mandate_request_reference_id: m
                                .connector_mandate_request_reference_id,
                        },
                    ),
                ),
            }
                });
        let payment_method_update_grpc = value.payment_method_update.map(|update| {
            grpc_api_types::payments::PaymentMethodUpdate {
                payment_method_update_data: Some(match update {
                    crate::connector_types::PaymentMethodUpdate::Card(card) => {
                        grpc_api_types::payments::payment_method_update::PaymentMethodUpdateData::Card(
                            grpc_api_types::payments::CardDetailUpdate {
                                card_exp_month: card.card_exp_month,
                                card_exp_year: card.card_exp_year,
                                last4_digits: card.last4_digits,
                                issuer_country: card.issuer_country,
                                card_issuer: card.card_issuer,
                                card_network: card.card_network,
                                card_holder_name: card.card_holder_name,
                            },
                        )
                    }
                }),
            }
        });
        Ok(Self {
            connector_transaction_id: extract_connector_request_reference_id(
                &value
                    .resource_id
                    .map(Option::foreign_try_from)
                    .transpose()?
                    .unwrap_or_default(),
            ),
            merchant_transaction_id: value.connector_response_reference_id,
            status: status as i32,
            mandate_reference: mandate_reference_grpc,
            error: Some(grpc_api_types::payments::ErrorInfo {
                unified_details: None,
                connector_details: Some(grpc_api_types::payments::ConnectorErrorDetails {
                    message: value.error_message.clone(),
                    code: value.error_code,
                    reason: None,
                }),
                issuer_details: None,
            }),
            network_transaction_id: value.network_txn_id,
            amount: None,
            captured_amount: value
                .minor_amount_captured
                .map(|amount_captured| amount_captured.get_amount_as_i64()),
            payment_method_type: None,
            capture_method: None,
            auth_type: None,
            created_at: None,
            updated_at: None,
            authorized_at: None,
            captured_at: None,
            customer_name: None,
            email: None,
            connector_customer_id: None,
            merchant_order_id: None,
            metadata: None,
            status_code: value.status_code as u32,
            raw_connector_response: None,
            response_headers,
            state: None,
            raw_connector_request: None,
            connector_response: None,
            redirection_data: None,
            incremental_authorization_allowed: None,
            payment_method_update: payment_method_update_grpc,
        })
    }
}

impl ForeignTryFrom<PaymentServiceVoidRequest> for PaymentVoidData {
    type Error = IntegrationError;

    fn foreign_try_from(
        value: PaymentServiceVoidRequest,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        // If currency is unspecified, send None, otherwise try to convert it
        let currency = if let Some(a) = value.amount {
            if a.currency() == grpc_api_types::payments::Currency::Unspecified {
                None
            } else {
                Some(common_enums::Currency::foreign_try_from(a.currency())?)
            }
        } else {
            None
        };
        let amount = value
            .amount
            .map(|a| common_utils::MinorUnit::new(a.minor_amount));
        Ok(Self {
            browser_info: value
                .browser_info
                .map(BrowserInformation::foreign_try_from)
                .transpose()?,
            connector_transaction_id: value.connector_transaction_id,
            metadata: value
                .metadata
                .map(|m| ForeignTryFrom::foreign_try_from((m, "metadata")))
                .transpose()?,
            cancellation_reason: value.cancellation_reason,
            raw_connector_response: None,
            integrity_object: None,
            amount,
            currency,
            connector_feature_data: value
                .connector_feature_data
                .map(|m| ForeignTryFrom::foreign_try_from((m, "connector metadata")))
                .transpose()?,
            merchant_order_id: value.merchant_order_id,
        })
    }
}

impl ForeignTryFrom<grpc_api_types::payments::PaymentServiceReverseRequest>
    for crate::connector_types::PaymentsCancelPostCaptureData
{
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::payments::PaymentServiceReverseRequest,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        Ok(Self {
            browser_info: value
                .browser_info
                .map(BrowserInformation::foreign_try_from)
                .transpose()?,
            connector_transaction_id: value.connector_transaction_id,
            cancellation_reason: value.cancellation_reason,
            raw_connector_response: None,
            integrity_object: None,
        })
    }
}

impl
    ForeignTryFrom<(
        grpc_api_types::payments::PaymentServiceReverseRequest,
        Connectors,
        &MaskedMetadata,
    )> for PaymentFlowData
{
    type Error = IntegrationError;

    fn foreign_try_from(
        (value, connectors, metadata): (
            grpc_api_types::payments::PaymentServiceReverseRequest,
            Connectors,
            &MaskedMetadata,
        ),
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        // For void post capture operations, address information is typically not available or required
        // Since this is a PaymentServiceReverseRequest, we use default address values
        let address: PaymentAddress = PaymentAddress::new(
            None,        // shipping
            None,        // billing
            None,        // payment_method_billing
            Some(false), // should_unify_address = false for void post capture operations
        );

        let merchant_id_from_header = extract_merchant_id_from_metadata(metadata)?;

        Ok(Self {
            merchant_id: merchant_id_from_header,
            payment_id: "IRRELEVANT_PAYMENT_ID".to_string(),
            attempt_id: "IRRELEVANT_ATTEMPT_ID".to_string(),
            status: common_enums::AttemptStatus::Pending,
            payment_method: PaymentMethod::Card, //TODO
            address,
            auth_type: common_enums::AuthenticationType::default(),
            connector_request_reference_id: extract_connector_request_reference_id(
                &value.merchant_reverse_id,
            ),
            customer_id: None,
            connector_customer: None,
            description: None,
            return_url: None,
            connector_feature_data: None,
            amount_captured: None,
            minor_amount_captured: None,
            access_token: None,
            session_token: None,
            reference_id: None,
            connector_order_id: None,
            preprocessing_id: None,
            connector_api_version: None,
            test_mode: None,
            connector_http_status_code: None,
            external_latency: None,
            connectors,
            raw_connector_response: None,
            raw_connector_request: None,
            connector_response_headers: None,
            vault_headers: None,
            minor_amount_capturable: None,
            amount: None,
            connector_response: None,
            recurring_mandate_payment_data: None,
            order_details: None,
            minor_amount_authorized: None,
            l2_l3_data: None,
        })
    }
}

impl ForeignTryFrom<PaymentServiceIncrementalAuthorizationRequest>
    for PaymentsIncrementalAuthorizationData
{
    type Error = IntegrationError;

    fn foreign_try_from(
        value: PaymentServiceIncrementalAuthorizationRequest,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let connector_transaction_id =
            ResponseId::ConnectorTransactionId(value.connector_transaction_id.clone());

        let connector_feature_data = value
            .connector_feature_data
            .map(|metadata| serde_json::from_str(&metadata.expose()))
            .transpose()
            .change_context(IntegrationError::InvalidDataFormat {
                field_name: "unknown",
                context: IntegrationErrorContext {
                    additional_context: Some("Failed to parse connector metadata".to_string()),
                    ..Default::default()
                },
            })?;

        let amount = value.amount.ok_or(IntegrationError::MissingRequiredField {
            field_name: "amount",
            context: IntegrationErrorContext::default(),
        })?;

        Ok(Self {
            minor_amount: common_utils::types::MinorUnit::new(amount.minor_amount),
            connector_transaction_id,
            connector_feature_data,
            currency: common_enums::Currency::foreign_try_from(amount.currency())?,
            reason: value.reason,
        })
    }
}

impl
    ForeignTryFrom<(
        PaymentServiceIncrementalAuthorizationRequest,
        Connectors,
        &MaskedMetadata,
    )> for PaymentFlowData
{
    type Error = IntegrationError;

    fn foreign_try_from(
        (value, connectors, metadata): (
            PaymentServiceIncrementalAuthorizationRequest,
            Connectors,
            &MaskedMetadata,
        ),
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        // For incremental authorization operations, address information is typically not available or required
        // Since this is a PaymentServiceIncrementalAuthorizationRequest, we use default address values
        let address: PaymentAddress = PaymentAddress::new(
            None,        // shipping
            None,        // billing
            None,        // payment_method_billing
            Some(false), // should_unify_address = false for void post capture operations
        );

        let merchant_id_from_header = extract_merchant_id_from_metadata(metadata)?;

        Ok(Self {
            merchant_id: merchant_id_from_header,
            payment_id: "IRRELEVANT_PAYMENT_ID".to_string(),
            attempt_id: "IRRELEVANT_ATTEMPT_ID".to_string(),
            status: common_enums::AttemptStatus::Pending,
            payment_method: PaymentMethod::Card, //TODO
            address,
            auth_type: common_enums::AuthenticationType::default(),
            connector_request_reference_id: extract_connector_request_reference_id(
                &value.merchant_authorization_id,
            ),
            customer_id: None,
            connector_customer: None,
            description: None,
            return_url: None,
            connector_feature_data: None,
            amount_captured: None,
            minor_amount_captured: None,
            access_token: None,
            session_token: None,
            reference_id: None,
            connector_order_id: None,
            preprocessing_id: None,
            connector_api_version: None,
            test_mode: None,
            connector_http_status_code: None,
            external_latency: None,
            connectors,
            raw_connector_response: None,
            raw_connector_request: None,
            connector_response_headers: None,
            vault_headers: None,
            minor_amount_capturable: None,
            amount: None,
            connector_response: None,
            recurring_mandate_payment_data: None,
            order_details: None,
            minor_amount_authorized: None,
            l2_l3_data: None,
        })
    }
}

impl ForeignTryFrom<RefundWebhookDetailsResponse> for RefundResponse {
    type Error = IntegrationError;

    fn foreign_try_from(
        value: RefundWebhookDetailsResponse,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let status = grpc_api_types::payments::RefundStatus::foreign_from(value.status);
        let response_headers = value
            .response_headers
            .map(|headers| {
                headers
                    .iter()
                    .filter_map(|(name, value)| {
                        value
                            .to_str()
                            .ok()
                            .map(|v| (name.to_string(), v.to_string()))
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(Self {
            connector_transaction_id: None,
            connector_refund_id: value.connector_refund_id.unwrap_or_default(),
            status: status.into(),
            merchant_refund_id: value.connector_response_reference_id,
            error: Some(grpc_api_types::payments::ErrorInfo {
                unified_details: None,
                connector_details: Some(grpc_api_types::payments::ConnectorErrorDetails {
                    message: value.error_message,
                    code: value.error_code,
                    reason: None,
                }),
                issuer_details: None,
            }),
            raw_connector_response: None,
            refund_amount: None,
            payment_amount: None,
            refund_reason: None,
            created_at: None,
            updated_at: None,
            processed_at: None,
            customer_name: None,
            email: None,
            merchant_order_id: None,
            metadata: None,
            refund_metadata: None,
            status_code: value.status_code as u32,
            response_headers,
            state: None,
            raw_connector_request: None,
            acquirer_reference_number: None,
        })
    }
}

impl ForeignTryFrom<DisputeWebhookDetailsResponse> for DisputeResponse {
    type Error = IntegrationError;

    fn foreign_try_from(
        value: DisputeWebhookDetailsResponse,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let grpc_status = grpc_api_types::payments::DisputeStatus::foreign_from(value.status);
        let grpc_stage = grpc_api_types::payments::DisputeStage::foreign_from(value.stage);
        let response_headers = value
            .response_headers
            .map(|headers| {
                headers
                    .iter()
                    .filter_map(|(name, value)| {
                        value
                            .to_str()
                            .ok()
                            .map(|v| (name.to_string(), v.to_string()))
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(Self {
            connector_dispute_id: Some(value.dispute_id),
            connector_transaction_id: None,
            dispute_status: grpc_status.into(),
            dispute_stage: grpc_stage.into(),
            connector_status_code: None,
            error: None,
            dispute_amount: None,
            dispute_date: None,
            service_date: None,
            shipping_date: None,
            due_date: None,
            evidence_documents: vec![],
            dispute_reason: None,
            dispute_message: value.dispute_message,
            merchant_dispute_id: value.connector_response_reference_id,
            status_code: value.status_code as u32,
            response_headers,
            raw_connector_request: None,
        })
    }
}

impl ForeignTryFrom<grpc_api_types::payments::PaymentServiceRefundRequest> for RefundsData {
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::payments::PaymentServiceRefundRequest,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let refund_amount = value
            .refund_amount
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "refund_amount",
                context: IntegrationErrorContext::default(),
            })?;

        let minor_refund_amount = common_utils::types::MinorUnit::new(refund_amount.minor_amount);

        let minor_payment_amount = common_utils::types::MinorUnit::new(value.payment_amount);

        // Extract transaction_id as connector_transaction_id
        let connector_transaction_id = value.connector_transaction_id;

        Ok(Self {
            refund_id: extract_connector_request_reference_id(&value.merchant_refund_id.clone()),
            connector_transaction_id,
            connector_refund_id: None, // refund_id field is used as refund_id, not connector_refund_id
            customer_id: value.customer_id.clone(),
            currency: common_enums::Currency::foreign_try_from(refund_amount.currency())?,
            payment_amount: value.payment_amount,
            reason: value.reason.clone(),
            webhook_url: value.webhook_url,
            refund_amount: refund_amount.minor_amount,
            connector_feature_data: value
                .connector_feature_data
                .clone()
                .map(|m| ForeignTryFrom::foreign_try_from((m, "connector metadata")))
                .transpose()?,
            refund_connector_metadata: value
                .refund_metadata
                .map(|m| ForeignTryFrom::foreign_try_from((m, "refund metadata")))
                .transpose()?,
            minor_payment_amount,
            minor_refund_amount,
            refund_status: common_enums::RefundStatus::Pending,
            merchant_account_id: value.merchant_account_id,
            capture_method: value
                .capture_method
                .map(|cm| {
                    CaptureMethod::foreign_try_from(
                        grpc_api_types::payments::CaptureMethod::try_from(cm).unwrap_or_default(),
                    )
                })
                .transpose()?,
            browser_info: value
                .browser_info
                .map(BrowserInformation::foreign_try_from)
                .transpose()?,
            integrity_object: None,
            split_refunds: None,
        })
    }
}

impl ForeignTryFrom<grpc_api_types::payments::DisputeServiceAcceptRequest> for AcceptDisputeData {
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::payments::DisputeServiceAcceptRequest,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        Ok(Self {
            connector_dispute_id: value.dispute_id,
            integrity_object: None,
        })
    }
}

impl ForeignTryFrom<grpc_api_types::payments::DisputeServiceSubmitEvidenceRequest>
    for SubmitEvidenceData
{
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::payments::DisputeServiceSubmitEvidenceRequest,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        // Initialize all fields to None
        let mut result = Self {
            dispute_id: Some(value.dispute_id.clone()),
            connector_dispute_id: value.dispute_id,
            integrity_object: None,
            access_activity_log: None,
            billing_address: None,
            cancellation_policy: None,
            cancellation_policy_file_type: None,
            cancellation_policy_provider_file_id: None,
            cancellation_policy_disclosure: None,
            cancellation_rebuttal: None,
            customer_communication: None,
            customer_communication_file_type: None,
            customer_communication_provider_file_id: None,
            customer_email_address: None,
            customer_name: None,
            customer_purchase_ip: None,
            customer_signature: None,
            customer_signature_file_type: None,
            customer_signature_provider_file_id: None,
            product_description: None,
            receipt: None,
            receipt_file_type: None,
            receipt_provider_file_id: None,
            refund_policy: None,
            refund_policy_file_type: None,
            refund_policy_provider_file_id: None,
            refund_policy_disclosure: None,
            refund_refusal_explanation: None,
            service_date: value.service_date.map(|date| date.to_string()),
            service_documentation: None,
            service_documentation_file_type: None,
            service_documentation_provider_file_id: None,
            shipping_address: None,
            shipping_carrier: None,
            shipping_date: value.shipping_date.map(|date| date.to_string()),
            shipping_documentation: None,
            shipping_documentation_file_type: None,
            shipping_documentation_provider_file_id: None,
            shipping_tracking_number: None,
            invoice_showing_distinct_transactions: None,
            invoice_showing_distinct_transactions_file_type: None,
            invoice_showing_distinct_transactions_provider_file_id: None,
            recurring_transaction_agreement: None,
            recurring_transaction_agreement_file_type: None,
            recurring_transaction_agreement_provider_file_id: None,
            uncategorized_file: None,
            uncategorized_file_type: None,
            uncategorized_file_provider_file_id: None,
            uncategorized_text: None,
        };

        // Extract evidence from evidence_documents array
        for document in value.evidence_documents {
            let evidence_type =
                grpc_api_types::payments::EvidenceType::try_from(document.evidence_type)
                    .unwrap_or(grpc_api_types::payments::EvidenceType::Unspecified);

            match evidence_type {
                grpc_api_types::payments::EvidenceType::CancellationPolicy => {
                    result.cancellation_policy = document.file_content;
                    result.cancellation_policy_file_type = document.file_mime_type;
                    result.cancellation_policy_provider_file_id = document.provider_file_id;
                }
                grpc_api_types::payments::EvidenceType::CustomerCommunication => {
                    result.customer_communication = document.file_content;
                    result.customer_communication_file_type = document.file_mime_type;
                    result.customer_communication_provider_file_id = document.provider_file_id;
                }
                grpc_api_types::payments::EvidenceType::CustomerSignature => {
                    result.customer_signature = document.file_content;
                    result.customer_signature_file_type = document.file_mime_type;
                    result.customer_signature_provider_file_id = document.provider_file_id;
                }
                grpc_api_types::payments::EvidenceType::Receipt => {
                    result.receipt = document.file_content;
                    result.receipt_file_type = document.file_mime_type;
                    result.receipt_provider_file_id = document.provider_file_id;
                }
                grpc_api_types::payments::EvidenceType::RefundPolicy => {
                    result.refund_policy = document.file_content;
                    result.refund_policy_file_type = document.file_mime_type;
                    result.refund_policy_provider_file_id = document.provider_file_id;
                }
                grpc_api_types::payments::EvidenceType::ServiceDocumentation => {
                    result.service_documentation = document.file_content;
                    result.service_documentation_file_type = document.file_mime_type;
                    result.service_documentation_provider_file_id = document.provider_file_id;
                }
                grpc_api_types::payments::EvidenceType::ShippingDocumentation => {
                    result.shipping_documentation = document.file_content;
                    result.shipping_documentation_file_type = document.file_mime_type;
                    result.shipping_documentation_provider_file_id = document.provider_file_id;
                }
                grpc_api_types::payments::EvidenceType::InvoiceShowingDistinctTransactions => {
                    result.invoice_showing_distinct_transactions = document.file_content;
                    result.invoice_showing_distinct_transactions_file_type =
                        document.file_mime_type;
                    result.invoice_showing_distinct_transactions_provider_file_id =
                        document.provider_file_id;
                }
                grpc_api_types::payments::EvidenceType::RecurringTransactionAgreement => {
                    result.recurring_transaction_agreement = document.file_content;
                    result.recurring_transaction_agreement_file_type = document.file_mime_type;
                    result.recurring_transaction_agreement_provider_file_id =
                        document.provider_file_id;
                }
                grpc_api_types::payments::EvidenceType::UncategorizedFile => {
                    result.uncategorized_file = document.file_content;
                    result.uncategorized_file_type = document.file_mime_type;
                    result.uncategorized_file_provider_file_id = document.provider_file_id;
                    result.uncategorized_text = document.text_content;
                }
                grpc_api_types::payments::EvidenceType::Unspecified => {
                    // Skip unspecified evidence types
                }
            }
        }

        Ok(result)
    }
}

pub fn generate_refund_response(
    router_data_v2: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
) -> Result<RefundResponse, error_stack::Report<ConnectorError>> {
    let refund_response = router_data_v2.response;
    let raw_connector_response = router_data_v2
        .resource_common_data
        .get_raw_connector_response();

    // RefundFlowData doesn't have access_token field, so no state to return
    let state = None;

    let raw_connector_request = router_data_v2
        .resource_common_data
        .get_raw_connector_request();

    match refund_response {
        Ok(response) => {
            let status = response.refund_status;
            let grpc_status = grpc_api_types::payments::RefundStatus::foreign_from(status);

            Ok(RefundResponse {
                connector_transaction_id: Some(
                    router_data_v2.request.connector_transaction_id.clone(),
                ),
                connector_refund_id: response.connector_refund_id,
                status: grpc_status as i32,
                merchant_refund_id: None,
                error: None,
                refund_amount: None,
                payment_amount: None,
                refund_reason: None,
                created_at: None,
                updated_at: None,
                processed_at: None,
                customer_name: None,
                email: None,
                merchant_order_id: None,
                raw_connector_response,
                metadata: None,
                refund_metadata: None,
                status_code: response.status_code as u32,
                response_headers: router_data_v2
                    .resource_common_data
                    .get_connector_response_headers_as_map(),
                state,
                raw_connector_request,
                acquirer_reference_number: None,
            })
        }
        Err(e) => {
            let status = e
                .attempt_status
                .map(grpc_api_types::payments::PaymentStatus::foreign_from)
                .unwrap_or_default();

            Ok(RefundResponse {
                connector_transaction_id: e.connector_transaction_id,
                connector_refund_id: String::new(),
                status: status as i32,
                merchant_refund_id: None,
                error: Some(grpc_api_types::payments::ErrorInfo {
                    unified_details: None,
                    connector_details: Some(grpc_api_types::payments::ConnectorErrorDetails {
                        message: Some(e.message.clone()),
                        code: Some(e.code.clone()),
                        reason: e.reason.clone(),
                    }),
                    issuer_details: None,
                }),
                refund_amount: None,
                payment_amount: None,
                refund_reason: None,
                created_at: None,
                updated_at: None,
                processed_at: None,
                customer_name: None,
                email: None,
                raw_connector_response,
                merchant_order_id: None,
                metadata: None,
                refund_metadata: None,
                status_code: e.status_code as u32,
                response_headers: router_data_v2
                    .resource_common_data
                    .get_connector_response_headers_as_map(),
                state,
                raw_connector_request,
                acquirer_reference_number: None,
            })
        }
    }
}

impl ForeignTryFrom<MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest>
    for ClientAuthenticationTokenRequestData
{
    type Error = IntegrationError;

    fn foreign_try_from(
        value: MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        // Extract domain-specific context from the oneof
        let payment_ctx = match value.domain_context {
            Some(grpc_api_types::payments::merchant_authentication_service_create_client_authentication_token_request::DomainContext::Payment(ctx)) => ctx,
            _ => return Err(report!(IntegrationError::InvalidDataFormat { field_name: "unknown", context: IntegrationErrorContext { additional_context: Some("Payment domain context is required for SDK session".to_string()), ..Default::default() } })),
        };

        let money = match payment_ctx.amount {
            Some(amount) => Ok(common_utils::types::Money {
                amount: common_utils::types::MinorUnit::new(amount.minor_amount),
                currency: common_enums::Currency::foreign_try_from(amount.currency())?,
            }),
            None => Err(report!(IntegrationError::MissingRequiredField {
                field_name: "amount",
                context: IntegrationErrorContext::default(),
            })),
        }?;

        let payment_method_type =
            <Option<PaymentMethodType>>::foreign_try_from(payment_ctx.payment_method_type())?;

        let email: Option<Email> = match payment_ctx
            .customer
            .clone()
            .and_then(|customer| customer.email)
        {
            Some(ref email_str) => {
                Some(Email::try_from(email_str.clone().expose()).map_err(|_| {
                    error_stack::Report::new(IntegrationError::InvalidDataFormat {
                        field_name: "unknown",
                        context: IntegrationErrorContext {
                            additional_context: Some("Invalid email".to_string()),
                            ..Default::default()
                        },
                    })
                })?)
            }
            None => None,
        };

        Ok(Self {
            amount: money.amount,
            currency: money.currency,
            country: {
                let country_code = payment_ctx.country_alpha2_code();
                if matches!(
                    country_code,
                    grpc_api_types::payments::CountryAlpha2::Unspecified
                ) {
                    None
                } else {
                    Some(CountryAlpha2::foreign_try_from(country_code)?)
                }
            },
            order_details: None,
            email,
            customer_name: payment_ctx
                .customer
                .and_then(|customer| customer.name)
                .map(Secret::new),
            order_tax_amount: payment_ctx
                .order_tax_amount
                .map(common_utils::types::MinorUnit::new),
            shipping_cost: payment_ctx
                .shipping_cost
                .map(common_utils::types::MinorUnit::new),
            payment_method_type,
        })
    }
}

impl ForeignTryFrom<grpc_api_types::payments::PaymentServiceCaptureRequest>
    for PaymentsCaptureData
{
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::payments::PaymentServiceCaptureRequest,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let capture_method = Some(CaptureMethod::foreign_try_from(value.capture_method())?);

        let connector_transaction_id =
            ResponseId::ConnectorTransactionId(value.connector_transaction_id);

        let multiple_capture_data =
            value
                .multiple_capture_data
                .clone()
                .map(|data| MultipleCaptureRequestData {
                    capture_sequence: data.capture_sequence,
                    capture_reference: data.capture_reference,
                });

        let amount = match value.amount_to_capture {
            Some(amount) => Ok(common_utils::types::Money {
                amount: common_utils::types::MinorUnit::new(amount.minor_amount),
                currency: common_enums::Currency::foreign_try_from(amount.currency())?,
            }),
            None => Err(report!(IntegrationError::MissingRequiredField {
                field_name: "amount_to_capture",
                context: IntegrationErrorContext::default(),
            })),
        }?;

        Ok(Self {
            amount_to_capture: amount.amount.get_amount_as_i64(),
            minor_amount_to_capture: amount.amount,
            currency: amount.currency,
            connector_transaction_id,
            multiple_capture_data,
            metadata: value
                .metadata
                .map(|m| ForeignTryFrom::foreign_try_from((m, "metadata")))
                .transpose()?,
            browser_info: value
                .browser_info
                .map(BrowserInformation::foreign_try_from)
                .transpose()?,
            integrity_object: None,
            capture_method,
            connector_feature_data: value
                .connector_feature_data
                .map(|m| ForeignTryFrom::foreign_try_from((m, "connector metadata")))
                .transpose()?,
            merchant_order_id: value.merchant_order_id,
        })
    }
}

impl
    ForeignTryFrom<(
        grpc_api_types::payments::PaymentServiceCaptureRequest,
        Connectors,
        &MaskedMetadata,
    )> for PaymentFlowData
{
    type Error = IntegrationError;

    fn foreign_try_from(
        (value, connectors, metadata): (
            grpc_api_types::payments::PaymentServiceCaptureRequest,
            Connectors,
            &MaskedMetadata,
        ),
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let merchant_id_from_header = extract_merchant_id_from_metadata(metadata)?;
        let access_token = value
            .state
            .as_ref()
            .and_then(|state| state.access_token.as_ref())
            .map(ServerAuthenticationTokenResponseData::foreign_try_from)
            .transpose()?;
        let connector_feature_data = value
            .connector_feature_data
            .map(|m| ForeignTryFrom::foreign_try_from((m, "feature data")))
            .transpose()?;
        Ok(Self {
            merchant_id: merchant_id_from_header,
            payment_id: "PAYMENT_ID".to_string(),
            attempt_id: "ATTEMPT_ID".to_string(),
            status: common_enums::AttemptStatus::Pending,
            payment_method: PaymentMethod::Card, // Default
            address: PaymentAddress::default(),
            auth_type: common_enums::AuthenticationType::default(),
            connector_request_reference_id: value.merchant_capture_id.unwrap_or_default(),
            customer_id: None,
            connector_customer: None,
            description: None,
            return_url: None,
            connector_feature_data,
            amount_captured: None,
            minor_amount_captured: None,
            minor_amount_capturable: None,
            amount: None,
            access_token,
            session_token: None,
            reference_id: None,
            connector_order_id: None,
            preprocessing_id: None,
            connector_api_version: None,
            test_mode: value.test_mode,
            connector_http_status_code: None,
            external_latency: None,
            connectors,
            raw_connector_response: None,
            raw_connector_request: None,
            connector_response_headers: None,
            vault_headers: None,
            connector_response: None,
            recurring_mandate_payment_data: None,
            order_details: None,
            minor_amount_authorized: None,
            l2_l3_data: None,
        })
    }
}

impl
    ForeignTryFrom<(
        MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest,
        Connectors,
        &MaskedMetadata,
    )> for PaymentFlowData
{
    type Error = IntegrationError;

    fn foreign_try_from(
        (value, connectors, metadata): (
            MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest,
            Connectors,
            &MaskedMetadata,
        ),
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let merchant_id_from_header = extract_merchant_id_from_metadata(metadata)?;

        let return_url = match &value.domain_context {
            Some(grpc_api_types::payments::merchant_authentication_service_create_client_authentication_token_request::DomainContext::Payment(ctx)) => ctx.return_url.clone(),
            _ => None,
        };

        Ok(Self {
            merchant_id: merchant_id_from_header,
            payment_id: "PAYMENT_ID".to_string(),
            attempt_id: "ATTEMPT_ID".to_string(),
            status: common_enums::AttemptStatus::Pending,
            payment_method: PaymentMethod::Wallet,
            address: PaymentAddress::default(),
            auth_type: common_enums::AuthenticationType::default(),
            connector_request_reference_id: value.merchant_client_session_id,
            customer_id: None,
            connector_customer: None,
            description: None,
            return_url,
            connector_feature_data: value
                .connector_feature_data
                .map(|metadata| serde_json::from_str(&metadata.expose()))
                .transpose()
                .change_context(IntegrationError::InvalidDataFormat {
                    field_name: "unknown",
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "Failed to parse merchant account metadata".to_string(),
                        ),
                        ..Default::default()
                    },
                })?,
            amount_captured: None,
            minor_amount_captured: None,
            minor_amount_capturable: None,
            amount: None,
            access_token: None,
            session_token: None,
            reference_id: None,
            connector_order_id: None,
            preprocessing_id: None,
            connector_api_version: None,
            test_mode: None,
            connector_http_status_code: None,
            external_latency: None,
            connectors,
            raw_connector_response: None,
            raw_connector_request: None,
            connector_response_headers: None,
            vault_headers: None,
            connector_response: None,
            recurring_mandate_payment_data: None,
            order_details: None,
            minor_amount_authorized: None,
            l2_l3_data: None,
        })
    }
}

pub fn generate_payment_incremental_authorization_response(
    router_data_v2: RouterDataV2<
        IncrementalAuthorization,
        PaymentFlowData,
        PaymentsIncrementalAuthorizationData,
        PaymentsResponseData,
    >,
) -> Result<PaymentServiceIncrementalAuthorizationResponse, error_stack::Report<ConnectorError>> {
    // Create state if either access token or connector customer is available
    let state = if router_data_v2.resource_common_data.access_token.is_some()
        || router_data_v2
            .resource_common_data
            .connector_customer
            .is_some()
    {
        Some(ConnectorState {
            access_token: router_data_v2
                .resource_common_data
                .access_token
                .as_ref()
                .map(|token_data| grpc_api_types::payments::AccessToken {
                    token: Some(token_data.access_token.clone()),
                    expires_in_seconds: token_data.expires_in,
                    token_type: token_data.token_type.clone(),
                }),
            connector_customer_id: router_data_v2
                .resource_common_data
                .connector_customer
                .clone(),
        })
    } else {
        None
    };

    match router_data_v2.response {
        Ok(response) => match response {
            PaymentsResponseData::IncrementalAuthorizationResponse {
                status,
                connector_authorization_id,
                status_code,
            } => {
                let grpc_status =
                    grpc_api_types::payments::AuthorizationStatus::foreign_from(status);

                Ok(PaymentServiceIncrementalAuthorizationResponse {
                    connector_authorization_id,
                    error: None,
                    status: grpc_status.into(),
                    status_code: status_code as u32,
                    response_headers: router_data_v2
                        .resource_common_data
                        .get_connector_response_headers_as_map(),
                    state,
                })
            }
            _ => Err(report!(ConnectorError::UnexpectedResponseError {
                context: ResponseTransformationErrorContext {
                    http_status_code: None,
                    additional_context: Some(
                        "Invalid response type received from connector".to_owned()
                    ),
                },
            })),
        },
        Err(e) => Ok(PaymentServiceIncrementalAuthorizationResponse {
            status: grpc_api_types::payments::AuthorizationStatus::AuthorizationFailure.into(),
            connector_authorization_id: None,
            error: Some(grpc_api_types::payments::ErrorInfo {
                unified_details: None,
                connector_details: Some(grpc_api_types::payments::ConnectorErrorDetails {
                    message: Some(e.message.clone()),
                    code: Some(e.code.clone()),
                    reason: e.reason.clone(),
                }),
                issuer_details: None,
            }),
            status_code: e.status_code as u32,
            response_headers: router_data_v2
                .resource_common_data
                .get_connector_response_headers_as_map(),
            state,
        }),
    }
}

pub fn generate_payment_capture_response(
    router_data_v2: RouterDataV2<
        Capture,
        PaymentFlowData,
        PaymentsCaptureData,
        PaymentsResponseData,
    >,
) -> Result<PaymentServiceCaptureResponse, error_stack::Report<ConnectorError>> {
    let transaction_response = router_data_v2.response;

    // Create state if either access token or connector customer is available
    let state = if router_data_v2.resource_common_data.access_token.is_some()
        || router_data_v2
            .resource_common_data
            .connector_customer
            .is_some()
    {
        Some(ConnectorState {
            access_token: router_data_v2
                .resource_common_data
                .access_token
                .as_ref()
                .map(|token_data| grpc_api_types::payments::AccessToken {
                    token: Some(token_data.access_token.clone()),
                    expires_in_seconds: token_data.expires_in,
                    token_type: token_data.token_type.clone(),
                }),
            connector_customer_id: router_data_v2
                .resource_common_data
                .connector_customer
                .clone(),
        })
    } else {
        None
    };

    let raw_connector_request = router_data_v2
        .resource_common_data
        .get_raw_connector_request();

    match transaction_response {
        Ok(response) => match response {
            PaymentsResponseData::TransactionResponse {
                resource_id,
                redirection_data: _,
                connector_metadata,
                network_txn_id: _,
                connector_response_reference_id,
                incremental_authorization_allowed,
                mandate_reference,
                status_code,
            } => {
                let status = router_data_v2.resource_common_data.status;
                let grpc_status = grpc_api_types::payments::PaymentStatus::foreign_from(status);
                let grpc_resource_id = Option::foreign_try_from(resource_id)?;

                let mandate_reference_grpc =
                    mandate_reference.map(|m| grpc_api_types::payments::MandateReference {
                        mandate_id_type: Some(grpc_api_types::payments::mandate_reference::MandateIdType::ConnectorMandateId(
                            grpc_payment_types::ConnectorMandateReferenceId {
                                connector_mandate_id: m.connector_mandate_id,
                        payment_method_id: m.payment_method_id,
                        connector_mandate_request_reference_id: m
                            .connector_mandate_request_reference_id,
                            })),
                    });

                Ok(PaymentServiceCaptureResponse {
                    connector_transaction_id: extract_connector_request_reference_id(
                        &grpc_resource_id,
                    ),
                    merchant_capture_id: connector_response_reference_id,
                    error: None,
                    status: grpc_status.into(),
                    status_code: status_code as u32,
                    response_headers: router_data_v2
                        .resource_common_data
                        .get_connector_response_headers_as_map(),
                    state,
                    raw_connector_request,
                    incremental_authorization_allowed,
                    mandate_reference: mandate_reference_grpc,
                    captured_amount: router_data_v2.resource_common_data.amount_captured,
                    connector_feature_data: convert_connector_metadata_to_secret_string(
                        connector_metadata,
                    ),
                })
            }
            _ => Err(report!(ConnectorError::UnexpectedResponseError {
                context: ResponseTransformationErrorContext {
                    http_status_code: None,
                    additional_context: Some(
                        "Invalid response type received from connector".to_owned()
                    ),
                },
            })),
        },
        Err(e) => {
            let status = match e.get_attempt_status_for_grpc(
                e.status_code,
                router_data_v2.resource_common_data.status,
            ) {
                Some(attempt_status) => {
                    grpc_api_types::payments::PaymentStatus::foreign_from(attempt_status)
                }
                None => grpc_api_types::payments::PaymentStatus::Unspecified,
            };
            Ok(PaymentServiceCaptureResponse {
                connector_transaction_id: extract_connector_request_reference_id(
                    &e.connector_transaction_id.clone(),
                ),
                merchant_capture_id: e.connector_transaction_id.clone(),
                status: status.into(),
                error: Some(grpc_api_types::payments::ErrorInfo {
                    unified_details: None,
                    connector_details: Some(grpc_api_types::payments::ConnectorErrorDetails {
                        message: Some(e.message.clone()),
                        code: Some(e.code.clone()),
                        reason: e.reason.clone(),
                    }),
                    issuer_details: None,
                }),
                status_code: e.status_code as u32,
                response_headers: router_data_v2
                    .resource_common_data
                    .get_connector_response_headers_as_map(),
                state,
                raw_connector_request,
                incremental_authorization_allowed: None,
                mandate_reference: None,
                captured_amount: None,
                connector_feature_data: None,
            })
        }
    }
}

impl
    ForeignTryFrom<(
        PaymentServiceSetupRecurringRequest,
        Connectors,
        consts::Env,
        &MaskedMetadata,
    )> for PaymentFlowData
{
    type Error = IntegrationError;

    fn foreign_try_from(
        (value, connectors, environment, metadata): (
            PaymentServiceSetupRecurringRequest,
            Connectors,
            consts::Env,
            &MaskedMetadata,
        ),
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let address = match value.address {
            Some(address) => PaymentAddress::foreign_try_from(address)?,
            None => {
                return Err(IntegrationError::InvalidDataFormat {
                    field_name: "unknown",
                    context: IntegrationErrorContext {
                        additional_context: Some("Address is required".to_string()),
                        ..Default::default()
                    },
                })?
            }
        };

        let l2_l3_data = value
            .l2_l3_data
            .as_ref()
            .map(|l2_l3| L2L3Data::foreign_try_from((l2_l3, &address, value.customer.as_ref())))
            .transpose()?;

        let test_mode = match environment {
            consts::Env::Development => Some(true),
            consts::Env::Production => Some(false),
            _ => Some(true),
        };

        let merchant_id_from_header = extract_merchant_id_from_metadata(metadata)?;
        let access_token = value
            .state
            .as_ref()
            .and_then(|state| state.access_token.as_ref())
            .map(ServerAuthenticationTokenResponseData::foreign_try_from)
            .transpose()?;
        let metadata = value
            .metadata
            .map(|m| SecretSerdeValue::foreign_try_from((m, "metadata")))
            .transpose()?;
        let description = metadata
            .as_ref()
            .and_then(|m| m.peek().get("description"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let connector_feature_data = value
            .connector_feature_data
            .map(|m| ForeignTryFrom::foreign_try_from((m, "merchant account metadata")))
            .transpose()?;

        Ok(Self {
            merchant_id: merchant_id_from_header,
            payment_id: "IRRELEVANT_PAYMENT_ID".to_string(),
            attempt_id: "IRRELEVANT_ATTEMPT_ID".to_string(),
            status: common_enums::AttemptStatus::Pending,
            payment_method: PaymentMethod::Card, //TODO
            address,
            auth_type: common_enums::AuthenticationType::default(),
            connector_request_reference_id: value.merchant_recurring_payment_id,
            customer_id: value
                .customer
                .clone()
                .and_then(|customer| customer.id)
                .clone()
                .map(|customer_id| CustomerId::try_from(Cow::from(customer_id)))
                .transpose()
                .change_context(IntegrationError::InvalidDataFormat {
                    field_name: "unknown",
                    context: IntegrationErrorContext {
                        additional_context: Some("Failed to parse Customer Id".to_string()),
                        ..Default::default()
                    },
                })?,
            connector_customer: value
                .customer
                .and_then(|customer| customer.connector_customer_id),
            description,
            return_url: None,
            connector_feature_data,
            amount_captured: None,
            minor_amount_captured: None,
            minor_amount_capturable: None,
            amount: None,
            access_token,
            session_token: value.session_token,
            reference_id: None,
            connector_order_id: None,
            preprocessing_id: None,
            connector_api_version: None,
            test_mode,
            connector_http_status_code: None,
            external_latency: None,
            connectors,
            raw_connector_response: None,
            raw_connector_request: None,
            connector_response_headers: None,
            vault_headers: None,
            connector_response: None,
            recurring_mandate_payment_data: None,
            order_details: None,
            minor_amount_authorized: None,
            l2_l3_data: l2_l3_data.map(Box::new),
        })
    }
}

impl
    ForeignTryFrom<(
        PaymentServiceSetupRecurringRequest,
        Connectors,
        &MaskedMetadata,
    )> for PaymentFlowData
{
    type Error = IntegrationError;

    fn foreign_try_from(
        (value, connectors, metadata): (
            PaymentServiceSetupRecurringRequest,
            Connectors,
            &MaskedMetadata,
        ),
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let address = match value.address {
            Some(address) => PaymentAddress::foreign_try_from(address)?,
            None => {
                return Err(IntegrationError::InvalidDataFormat {
                    field_name: "unknown",
                    context: IntegrationErrorContext {
                        additional_context: Some("Address is required".to_string()),
                        ..Default::default()
                    },
                })?
            }
        };

        let merchant_id_from_header = extract_merchant_id_from_metadata(metadata)?;
        let access_token = value
            .state
            .as_ref()
            .and_then(|state| state.access_token.as_ref())
            .map(ServerAuthenticationTokenResponseData::foreign_try_from)
            .transpose()?;
        let metadata_val = value
            .metadata
            .map(|m| SecretSerdeValue::foreign_try_from((m, "metadata")))
            .transpose()?;
        let description = metadata_val
            .as_ref()
            .and_then(|m| m.peek().get("description"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let connector_feature_data = value
            .connector_feature_data
            .map(|m| ForeignTryFrom::foreign_try_from((m, "merchant account metadata")))
            .transpose()?;

        Ok(Self {
            merchant_id: merchant_id_from_header,
            payment_id: "IRRELEVANT_PAYMENT_ID".to_string(),
            attempt_id: "IRRELEVANT_ATTEMPT_ID".to_string(),
            status: common_enums::AttemptStatus::Pending,
            payment_method: PaymentMethod::Card,
            address,
            auth_type: common_enums::AuthenticationType::default(),
            connector_request_reference_id: value.merchant_recurring_payment_id,
            customer_id: value
                .customer
                .clone()
                .and_then(|customer| customer.id)
                .map(|customer_id| CustomerId::try_from(Cow::from(customer_id)))
                .transpose()
                .change_context(IntegrationError::InvalidDataFormat {
                    field_name: "unknown",
                    context: IntegrationErrorContext {
                        additional_context: Some("Failed to parse Customer Id".to_string()),
                        ..Default::default()
                    },
                })?,
            connector_customer: value
                .customer
                .and_then(|customer| customer.connector_customer_id),
            description,
            return_url: None,
            connector_feature_data,
            amount_captured: None,
            minor_amount_captured: None,
            minor_amount_capturable: None,
            amount: None,
            access_token,
            session_token: value.session_token,
            reference_id: None,
            connector_order_id: None,
            preprocessing_id: None,
            connector_api_version: None,
            test_mode: None,
            connector_http_status_code: None,
            external_latency: None,
            connectors,
            raw_connector_response: None,
            raw_connector_request: None,
            connector_response_headers: None,
            vault_headers: None,
            connector_response: None,
            recurring_mandate_payment_data: None,
            order_details: None,
            minor_amount_authorized: None,
            l2_l3_data: None,
        })
    }
}

impl<
        T: PaymentMethodDataTypes
            + Default
            + Debug
            + Send
            + Eq
            + PartialEq
            + Serialize
            + serde::de::DeserializeOwned
            + Clone
            + CardConversionHelper<T>,
    > ForeignTryFrom<PaymentServiceSetupRecurringRequest> for SetupMandateRequestData<T>
{
    type Error = IntegrationError;

    fn foreign_try_from(
        value: PaymentServiceSetupRecurringRequest,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let email: Option<Email> = match value.customer.clone().and_then(|customer| customer.email)
        {
            Some(ref email_str) => {
                Some(Email::try_from(email_str.clone().expose()).map_err(|_| {
                    error_stack::Report::new(IntegrationError::InvalidDataFormat {
                        field_name: "unknown",
                        context: IntegrationErrorContext {
                            additional_context: Some("Invalid email".to_string()),
                            ..Default::default()
                        },
                    })
                })?)
            }
            None => None,
        };
        let customer_acceptance = value.customer_acceptance.clone().ok_or_else(|| {
            error_stack::Report::new(IntegrationError::InvalidDataFormat {
                field_name: "unknown",
                context: IntegrationErrorContext {
                    additional_context: Some("Customer acceptance is missing".to_string()),
                    ..Default::default()
                },
            })
        })?;

        let amount = match value.amount {
            Some(amount) => Ok(common_utils::types::Money {
                amount: common_utils::types::MinorUnit::new(amount.minor_amount),
                currency: common_enums::Currency::foreign_try_from(amount.currency())?,
            }),
            None => Err(report!(IntegrationError::MissingRequiredField {
                field_name: "amount",
                context: IntegrationErrorContext::default(),
            })),
        }?;

        let setup_future_usage = value.setup_future_usage();

        let setup_mandate_details = MandateData {
            update_mandate_id: None,
            customer_acceptance: Some(mandates::CustomerAcceptance::foreign_try_from(
                customer_acceptance.clone(),
            )?),
            mandate_type: None,
        };

        let billing_descriptor =
            value
                .billing_descriptor
                .as_ref()
                .map(|descriptor| BillingDescriptor {
                    name: descriptor.name.clone(),
                    city: descriptor.city.clone(),
                    phone: descriptor.phone.clone(),
                    statement_descriptor: descriptor.statement_descriptor.clone(),
                    statement_descriptor_suffix: descriptor.statement_descriptor_suffix.clone(),
                    reference: descriptor.reference.clone(),
                });

        let payment_channel = match value.payment_channel() {
            grpc_payment_types::PaymentChannel::Unspecified => None,
            _ => Some(common_enums::PaymentChannel::foreign_try_from(
                value.payment_channel(),
            )?),
        };

        Ok(Self {
            currency: amount.currency,
            payment_method_data: PaymentMethodData::<T>::foreign_try_from(
                value.payment_method.clone().ok_or_else(|| {
                    IntegrationError::InvalidDataFormat {
                        field_name: "unknown",
                        context: IntegrationErrorContext {
                            additional_context: Some("Payment method data is required".to_string()),
                            ..Default::default()
                        },
                    }
                })?,
            )?,
            amount: Some(amount.amount.get_amount_as_i64()),
            confirm: true,
            customer_acceptance: Some(mandates::CustomerAcceptance::foreign_try_from(
                customer_acceptance.clone(),
            )?),
            mandate_id: None,
            setup_future_usage: Some(common_enums::FutureUsage::foreign_try_from(
                setup_future_usage,
            )?),
            off_session: value.off_session,
            setup_mandate_details: Some(setup_mandate_details),
            router_return_url: value.return_url.clone(),
            webhook_url: value.webhook_url,
            browser_info: value
                .browser_info
                .map(BrowserInformation::foreign_try_from)
                .transpose()?,
            email,
            customer_name: value
                .customer
                .as_ref()
                .and_then(|customer| customer.name.clone()),
            return_url: value.return_url.clone(),
            payment_method_type: value
                .payment_method
                .clone()
                .map(<Option<common_enums::PaymentMethodType>>::foreign_try_from)
                .transpose()?
                .flatten(),
            request_incremental_authorization: false,
            metadata: value
                .metadata
                .map(|m| ForeignTryFrom::foreign_try_from((m, "metadata")))
                .transpose()?,
            complete_authorize_url: None,
            capture_method: None,
            integrity_object: None,
            minor_amount: Some(amount.amount),
            shipping_cost: None,
            customer_id: value
                .customer
                .and_then(|customer| customer.id)
                .clone()
                .map(|customer_id| CustomerId::try_from(Cow::from(customer_id)))
                .transpose()
                .change_context(IntegrationError::InvalidDataFormat {
                    field_name: "unknown",
                    context: IntegrationErrorContext {
                        additional_context: Some("Failed to parse Customer Id".to_string()),
                        ..Default::default()
                    },
                })?,
            billing_descriptor,
            merchant_order_id: value.merchant_order_id,
            payment_channel,
            enable_partial_authorization: value.enable_partial_authorization,
            locale: value.locale.clone(),
            connector_testing_data: value.connector_testing_data.and_then(|s| {
                serde_json::from_str(&s.expose())
                    .ok()
                    .map(common_utils::pii::SecretSerdeValue::new)
            }),
        })
    }
}

impl ForeignTryFrom<grpc_api_types::payments::PaymentChannel> for common_enums::PaymentChannel {
    type Error = IntegrationError;
    fn foreign_try_from(
        value: grpc_api_types::payments::PaymentChannel,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        match value {
            grpc_payment_types::PaymentChannel::Ecommerce => {
                Ok(common_enums::PaymentChannel::Ecommerce)
            }
            grpc_payment_types::PaymentChannel::MailOrder => {
                Ok(common_enums::PaymentChannel::MailOrder)
            }
            grpc_payment_types::PaymentChannel::TelephoneOrder => {
                Ok(common_enums::PaymentChannel::TelephoneOrder)
            }
            grpc_payment_types::PaymentChannel::Unspecified => {
                Err(IntegrationError::InvalidDataFormat {
                    field_name: "unknown",
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "Payment channel type must be specified".to_string(),
                        ),
                        ..Default::default()
                    },
                }
                .into())
            }
        }
    }
}

impl ForeignTryFrom<grpc_api_types::payments::CustomerAcceptance> for mandates::CustomerAcceptance {
    type Error = IntegrationError;
    fn foreign_try_from(
        value: grpc_api_types::payments::CustomerAcceptance,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        Ok(Self {
            acceptance_type: mandates::AcceptanceType::foreign_try_from(value.acceptance_type())?,
            accepted_at: time::OffsetDateTime::from_unix_timestamp(value.accepted_at)
                .ok()
                .map(|offset_dt| time::PrimitiveDateTime::new(offset_dt.date(), offset_dt.time())),
            online: value
                .online_mandate_details
                .map(mandates::OnlineMandate::foreign_try_from)
                .transpose()?,
        })
    }
}

impl
    From<(
        &grpc_api_types::payments::BillingDescriptor,
        Option<String>,
        Option<String>,
    )> for BillingDescriptor
{
    fn from(
        (descriptor, statement_descriptor_name, statement_descriptor_suffix): (
            &grpc_api_types::payments::BillingDescriptor,
            Option<String>,
            Option<String>,
        ),
    ) -> Self {
        BillingDescriptor {
            name: descriptor.name.clone(),
            city: descriptor.city.clone(),
            phone: descriptor.phone.clone(),
            statement_descriptor: descriptor
                .statement_descriptor
                .clone()
                .or(statement_descriptor_name),
            statement_descriptor_suffix: descriptor
                .statement_descriptor_suffix
                .clone()
                .or(statement_descriptor_suffix),
            reference: descriptor.reference.clone(),
        }
    }
}

impl
    ForeignTryFrom<(
        &grpc_api_types::payments::L2l3Data,
        &PaymentAddress,
        Option<&grpc_api_types::payments::Customer>,
    )> for L2L3Data
{
    type Error = IntegrationError;
    fn foreign_try_from(
        (l2l3_data, payment_address, customer): (
            &grpc_api_types::payments::L2l3Data,
            &PaymentAddress,
            Option<&grpc_api_types::payments::Customer>,
        ),
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let order_info = l2l3_data
            .order_info
            .as_ref()
            .map(OrderInfo::foreign_try_from)
            .transpose()?;

        let tax_info = l2l3_data
            .tax_info
            .as_ref()
            .map(TaxInfo::foreign_try_from)
            .transpose()?;

        let customer_info = customer.map(CustomerInfo::foreign_try_from).transpose()?;

        let shipping_address = payment_address.get_shipping();
        let billing_address = payment_address.get_payment_billing();

        Ok(Self {
            order_info,
            tax_info,
            customer_info,
            billing_details: billing_address
                .and_then(|address| address.address.as_ref())
                .cloned(),
            shipping_details: shipping_address
                .and_then(|address| address.address.as_ref())
                .cloned(),
        })
    }
}

impl ForeignTryFrom<&grpc_api_types::payments::OrderInfo> for OrderInfo {
    type Error = IntegrationError;
    fn foreign_try_from(
        value: &grpc_api_types::payments::OrderInfo,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let order_details = (!value.order_details.is_empty())
            .then(|| {
                value
                    .order_details
                    .clone()
                    .into_iter()
                    .map(OrderDetailsWithAmount::foreign_try_from)
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?;

        Ok(Self {
            order_date: value.order_date.and_then(|ts| {
                time::OffsetDateTime::from_unix_timestamp(ts)
                    .ok()
                    .map(|offset_dt| {
                        time::PrimitiveDateTime::new(offset_dt.date(), offset_dt.time())
                    })
            }),
            order_details,
            merchant_order_reference_id: value.merchant_order_reference_id.clone(),
            discount_amount: value
                .discount_amount
                .map(common_utils::types::MinorUnit::new),
            shipping_cost: value.shipping_cost.map(common_utils::types::MinorUnit::new),
            duty_amount: value.duty_amount.map(common_utils::types::MinorUnit::new),
        })
    }
}

impl ForeignTryFrom<&grpc_api_types::payments::TaxInfo> for TaxInfo {
    type Error = IntegrationError;
    fn foreign_try_from(
        value: &grpc_api_types::payments::TaxInfo,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let tax_status = match value.tax_status() {
            grpc_api_types::payments::TaxStatus::Unspecified => None,
            _ => Some(common_enums::TaxStatus::foreign_try_from(
                &value.tax_status(),
            )?),
        };

        Ok(Self {
            tax_status,
            customer_tax_registration_id: value.customer_tax_registration_id.clone(),
            merchant_tax_registration_id: value.merchant_tax_registration_id.clone(),
            shipping_amount_tax: value
                .shipping_amount_tax
                .map(common_utils::types::MinorUnit::new),
            order_tax_amount: value
                .order_tax_amount
                .map(common_utils::types::MinorUnit::new),
        })
    }
}

impl ForeignTryFrom<&grpc_api_types::payments::TaxStatus> for common_enums::TaxStatus {
    type Error = IntegrationError;
    fn foreign_try_from(
        value: &grpc_api_types::payments::TaxStatus,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        match value {
            grpc_api_types::payments::TaxStatus::Exempt => Ok(Self::Exempt),
            grpc_api_types::payments::TaxStatus::Taxable => Ok(Self::Taxable),
            grpc_api_types::payments::TaxStatus::Unspecified => {
                Err(IntegrationError::InvalidDataFormat {
                    field_name: "unknown",
                    context: IntegrationErrorContext {
                        additional_context: Some("Tax status must be specified".to_string()),
                        ..Default::default()
                    },
                }
                .into())
            }
        }
    }
}

impl ForeignTryFrom<&grpc_api_types::payments::Customer> for CustomerInfo {
    type Error = IntegrationError;
    fn foreign_try_from(
        value: &grpc_api_types::payments::Customer,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let customer_id = value
            .id
            .clone()
            .map(|customer_id| CustomerId::try_from(Cow::from(customer_id)))
            .transpose()
            .change_context(IntegrationError::InvalidDataFormat {
                field_name: "unknown",
                context: IntegrationErrorContext {
                    additional_context: Some("Failed to parse Customer Id".to_string()),
                    ..Default::default()
                },
            })?;

        let customer_email: Option<Email> = match value.email {
            Some(ref email_str) => {
                Some(Email::try_from(email_str.clone().expose()).map_err(|_| {
                    error_stack::Report::new(IntegrationError::InvalidDataFormat {
                        field_name: "unknown",
                        context: IntegrationErrorContext {
                            additional_context: Some("Invalid email".to_string()),
                            ..Default::default()
                        },
                    })
                })?)
            }
            None => None,
        };

        Ok(Self {
            customer_id,
            customer_email,
            customer_name: value.name.clone().map(Into::into),
            customer_phone_number: value.phone_number.clone().map(Into::into),
            customer_phone_country_code: value.phone_country_code.clone(),
        })
    }
}

impl ForeignTryFrom<grpc_api_types::payments::OnlineMandate> for mandates::OnlineMandate {
    type Error = IntegrationError;
    fn foreign_try_from(
        value: grpc_api_types::payments::OnlineMandate,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        Ok(Self {
            ip_address: value.ip_address.map(Secret::new),
            user_agent: value.user_agent,
        })
    }
}

impl ForeignTryFrom<grpc_api_types::payments::AcceptanceType> for mandates::AcceptanceType {
    type Error = IntegrationError;
    fn foreign_try_from(
        value: grpc_api_types::payments::AcceptanceType,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        match value {
            grpc_payment_types::AcceptanceType::Offline => Ok(Self::Offline),
            grpc_payment_types::AcceptanceType::Online => Ok(Self::Online),
            grpc_payment_types::AcceptanceType::Unspecified => {
                Err(IntegrationError::InvalidDataFormat {
                    field_name: "unknown",
                    context: IntegrationErrorContext {
                        additional_context: Some("Acceptance type must be specified".to_string()),
                        ..Default::default()
                    },
                }
                .into())
            }
        }
    }
}

impl ForeignTryFrom<grpc_api_types::payments::SetupMandateDetails> for MandateData {
    type Error = IntegrationError;
    fn foreign_try_from(
        value: grpc_api_types::payments::SetupMandateDetails,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        // Map the mandate_type from grpc type to domain type
        let mandate_type = value
            .mandate_type
            .and_then(|grpc_mandate_type| match grpc_mandate_type.mandate_type {
                Some(grpc_api_types::payments::mandate_type::MandateType::SingleUse(
                    amount_data,
                )) => Some(mandates::MandateDataType::SingleUse(
                    mandates::MandateAmountData {
                        amount: common_utils::types::MinorUnit::new(amount_data.amount),
                        currency: grpc_api_types::payments::Currency::try_from(
                            amount_data.currency,
                        )
                        .ok()
                        .and_then(|grpc_currency| {
                            common_enums::Currency::foreign_try_from(grpc_currency).ok()
                        })
                        .unwrap_or(common_enums::Currency::USD),
                        start_date: amount_data.start_date.and_then(|ts| {
                            time::OffsetDateTime::from_unix_timestamp(ts)
                                .ok()
                                .map(|offset_dt| {
                                    time::PrimitiveDateTime::new(offset_dt.date(), offset_dt.time())
                                })
                        }),
                        end_date: amount_data.end_date.and_then(|ts| {
                            time::OffsetDateTime::from_unix_timestamp(ts)
                                .ok()
                                .map(|offset_dt| {
                                    time::PrimitiveDateTime::new(offset_dt.date(), offset_dt.time())
                                })
                        }),
                        metadata: None,
                        amount_type: amount_data.amount_type,
                        frequency: amount_data.frequency,
                    },
                )),
                Some(grpc_api_types::payments::mandate_type::MandateType::MultiUse(
                    amount_data,
                )) => Some(mandates::MandateDataType::MultiUse(Some(
                    mandates::MandateAmountData {
                        amount: common_utils::types::MinorUnit::new(amount_data.amount),
                        currency: grpc_api_types::payments::Currency::try_from(
                            amount_data.currency,
                        )
                        .ok()
                        .and_then(|grpc_currency| {
                            common_enums::Currency::foreign_try_from(grpc_currency).ok()
                        })
                        .unwrap_or(common_enums::Currency::USD),
                        start_date: amount_data.start_date.and_then(|ts| {
                            time::OffsetDateTime::from_unix_timestamp(ts)
                                .ok()
                                .map(|offset_dt| {
                                    time::PrimitiveDateTime::new(offset_dt.date(), offset_dt.time())
                                })
                        }),
                        end_date: amount_data.end_date.and_then(|ts| {
                            time::OffsetDateTime::from_unix_timestamp(ts)
                                .ok()
                                .map(|offset_dt| {
                                    time::PrimitiveDateTime::new(offset_dt.date(), offset_dt.time())
                                })
                        }),
                        metadata: None,
                        amount_type: amount_data.amount_type,
                        frequency: amount_data.frequency,
                    },
                ))),
                None => None,
            });

        Ok(Self {
            update_mandate_id: value.update_mandate_id,
            customer_acceptance: value
                .customer_acceptance
                .map(mandates::CustomerAcceptance::foreign_try_from)
                .transpose()?,
            mandate_type,
        })
    }
}

impl ForeignTryFrom<grpc_api_types::payments::FutureUsage> for common_enums::FutureUsage {
    type Error = IntegrationError;
    fn foreign_try_from(
        value: grpc_api_types::payments::FutureUsage,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        match value {
            grpc_api_types::payments::FutureUsage::OffSession => Ok(Self::OffSession),
            grpc_api_types::payments::FutureUsage::OnSession => Ok(Self::OnSession),
            grpc_api_types::payments::FutureUsage::Unspecified => {
                Err(IntegrationError::InvalidDataFormat {
                    field_name: "unknown",
                    context: IntegrationErrorContext {
                        additional_context: Some("Future usage must be specified".to_string()),
                        ..Default::default()
                    },
                }
                .into())
            }
        }
    }
}

impl ForeignTryFrom<grpc_api_types::payments::MitCategory> for common_enums::MitCategory {
    type Error = IntegrationError;
    fn foreign_try_from(
        value: grpc_api_types::payments::MitCategory,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        match value {
            grpc_api_types::payments::MitCategory::RecurringMit => {
                Ok(common_enums::MitCategory::Recurring)
            }
            grpc_api_types::payments::MitCategory::InstallmentMit => {
                Ok(common_enums::MitCategory::Installment)
            }
            grpc_api_types::payments::MitCategory::UnscheduledMit => {
                Ok(common_enums::MitCategory::Unscheduled)
            }
            grpc_api_types::payments::MitCategory::ResubmissionMit => {
                Ok(common_enums::MitCategory::Resubmission)
            }
            grpc_api_types::payments::MitCategory::Unspecified => {
                Err(IntegrationError::InvalidDataFormat {
                    field_name: "unknown",
                    context: IntegrationErrorContext {
                        additional_context: Some("Mit category must be specified".to_string()),
                        ..Default::default()
                    },
                }
                .into())
            }
        }
    }
}

pub fn generate_setup_mandate_response<T: PaymentMethodDataTypes>(
    router_data_v2: RouterDataV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    >,
) -> Result<PaymentServiceSetupRecurringResponse, error_stack::Report<ConnectorError>> {
    let transaction_response = router_data_v2.response;
    let status = router_data_v2.resource_common_data.status;
    let grpc_status = grpc_api_types::payments::PaymentStatus::foreign_from(status);

    // Create state if either access token or connector customer is available
    let state = if router_data_v2.resource_common_data.access_token.is_some()
        || router_data_v2
            .resource_common_data
            .connector_customer
            .is_some()
    {
        Some(ConnectorState {
            access_token: router_data_v2
                .resource_common_data
                .access_token
                .as_ref()
                .map(|token_data| grpc_api_types::payments::AccessToken {
                    token: Some(token_data.access_token.clone()),
                    expires_in_seconds: token_data.expires_in,
                    token_type: token_data.token_type.clone(),
                }),
            connector_customer_id: router_data_v2
                .resource_common_data
                .connector_customer
                .clone(),
        })
    } else {
        None
    };

    let raw_connector_request = router_data_v2
        .resource_common_data
        .get_raw_connector_request();

    let connector_response = router_data_v2
        .resource_common_data
        .connector_response
        .as_ref()
        .map(|connector_response_data| {
            grpc_api_types::payments::ConnectorResponseData::foreign_try_from(
                connector_response_data.clone(),
            )
        })
        .transpose()?;

    // Set amount_captured based on status - only if Charged/PartialCharged
    let captured_amount = match status {
        common_enums::AttemptStatus::Charged
        | common_enums::AttemptStatus::PartialCharged
        | common_enums::AttemptStatus::PartialChargedAndChargeable => router_data_v2.request.amount,
        _ => None,
    };

    let minor_captured_amount = captured_amount;

    let response = match transaction_response {
        Ok(response) => match response {
            PaymentsResponseData::TransactionResponse {
                resource_id,
                redirection_data,
                connector_metadata,
                network_txn_id,
                connector_response_reference_id,
                incremental_authorization_allowed,
                mandate_reference,
                status_code,
            } => {
                let mandate_reference_grpc =
                    mandate_reference.map(|m| grpc_api_types::payments::MandateReference {
                        mandate_id_type: Some(grpc_api_types::payments::mandate_reference::MandateIdType::ConnectorMandateId(
                            grpc_payment_types::ConnectorMandateReferenceId { connector_mandate_id: m.connector_mandate_id,
                        payment_method_id: m.payment_method_id,
                        connector_mandate_request_reference_id: m
                            .connector_mandate_request_reference_id, }
                        )),
                    });

                PaymentServiceSetupRecurringResponse {
                    connector_recurring_payment_id: Option::foreign_try_from(resource_id)?,
                    redirection_data: redirection_data.map(|form| {
                            match *form {
                                router_response_types::RedirectForm::Form { endpoint, method, form_fields: _ } => {
                                    Ok::<grpc_api_types::payments::RedirectForm, error_stack::Report<ConnectorError>>(grpc_api_types::payments::RedirectForm {
                                        form_type: Some(grpc_api_types::payments::redirect_form::FormType::Form(
                                            grpc_api_types::payments::FormData {
                                                endpoint,
                                                method: match method {
                                                    Method::Get => 1,
                                                    Method::Post => 2,
                                                    Method::Put => 3,
                                                    Method::Delete => 4,
                                                    _ => 0,
                                                },
                                                form_fields: HashMap::default(), //TODO
                                            }
                                        ))
                                    })
                                },
                                router_response_types::RedirectForm::Html { html_data } => {
                                    Ok(grpc_api_types::payments::RedirectForm {
                                        form_type: Some(grpc_api_types::payments::redirect_form::FormType::Html(
                                            grpc_api_types::payments::HtmlData {
                                                html_data,
                                            }
                                        ))
                                    })
                                },
                                router_response_types::RedirectForm::Nmi {
                                    amount,
                                    public_key,
                                    customer_vault_id,
                                    order_id,
                                    continue_redirection_url,
                                } => Ok(grpc_api_types::payments::RedirectForm {
                                    form_type: Some(grpc_api_types::payments::redirect_form::FormType::Nmi(
                                        grpc_api_types::payments::NmiData {
                                            amount: Some(amount),
                                            public_key: Some(public_key),
                                            customer_vault_id,
                                            order_id,
                                            continue_redirection_url,
                                        }
                                    ))
                                }),
                                _ => Err(report!(
                                    ConnectorError::UnexpectedResponseError { context: ResponseTransformationErrorContext { http_status_code: None, additional_context: Some("Invalid redirect form type from connector response".to_owned()) } })),
                            }
                        }
                    ).transpose()?,
                    network_transaction_id: network_txn_id,
                    merchant_recurring_payment_id: extract_connector_request_reference_id(&connector_response_reference_id),
                    status: grpc_status as i32,
                    mandate_reference: mandate_reference_grpc,
                    incremental_authorization_allowed,
                    error: None,
                    status_code: status_code as u32,
                    response_headers: router_data_v2
                        .resource_common_data
                        .get_connector_response_headers_as_map(),
                    state,
                    raw_connector_request,
                    connector_response,
                    connector_feature_data: convert_connector_metadata_to_secret_string(connector_metadata),
                    captured_amount: minor_captured_amount,
                }
            }
            _ => {
                return Err(report!(ConnectorError::UnexpectedResponseError {
                    context: ResponseTransformationErrorContext {
                        http_status_code: None,
                        additional_context: Some(
                            "Invalid response type received from connector".to_owned()
                        ),
                    },
                }))
            }
        },
        Err(err) => {
            let status = match err.get_attempt_status_for_grpc(
                err.status_code,
                router_data_v2.resource_common_data.status,
            ) {
                Some(attempt_status) => {
                    grpc_api_types::payments::PaymentStatus::foreign_from(attempt_status)
                }
                None => grpc_api_types::payments::PaymentStatus::Unspecified,
            };
            PaymentServiceSetupRecurringResponse {
                connector_recurring_payment_id: None,
                redirection_data: None,
                network_transaction_id: None,
                merchant_recurring_payment_id: extract_connector_request_reference_id(
                    &err.connector_transaction_id,
                ),
                status: status as i32,
                mandate_reference: None,
                incremental_authorization_allowed: None,
                error: Some(grpc_api_types::payments::ErrorInfo {
                    unified_details: None,
                    connector_details: Some(grpc_api_types::payments::ConnectorErrorDetails {
                        message: Some(err.message.clone()),
                        code: Some(err.code.clone()),
                        reason: err.reason.clone(),
                    }),
                    issuer_details: None,
                }),
                status_code: err.status_code as u32,
                response_headers: router_data_v2
                    .resource_common_data
                    .get_connector_response_headers_as_map(),
                state,
                raw_connector_request,
                connector_response,
                connector_feature_data: None,
                captured_amount: None,
            }
        }
    };
    Ok(response)
}

impl ForeignTryFrom<(DisputeServiceDefendRequest, Connectors)> for DisputeFlowData {
    type Error = IntegrationError;

    fn foreign_try_from(
        (value, connectors): (DisputeServiceDefendRequest, Connectors),
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        Ok(Self {
            dispute_id: Some(value.dispute_id.clone()),
            connectors,
            connector_dispute_id: value.dispute_id,
            defense_reason_code: Some(value.reason_code.unwrap_or_default()),
            connector_request_reference_id: extract_connector_request_reference_id(
                &value.merchant_dispute_id,
            ),
            raw_connector_response: None,
            raw_connector_request: None,
            connector_response_headers: None,
        })
    }
}

impl ForeignTryFrom<(DisputeServiceDefendRequest, Connectors, &MaskedMetadata)>
    for DisputeFlowData
{
    type Error = IntegrationError;

    fn foreign_try_from(
        (value, connectors, _metadata): (DisputeServiceDefendRequest, Connectors, &MaskedMetadata),
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        Ok(Self {
            connector_request_reference_id: extract_connector_request_reference_id(
                &value.merchant_dispute_id,
            ),
            dispute_id: Some(value.dispute_id.clone()),
            connectors,
            connector_dispute_id: value.dispute_id,
            defense_reason_code: Some(value.reason_code.unwrap_or_default()),
            raw_connector_response: None,
            raw_connector_request: None,
            connector_response_headers: None,
        })
    }
}
impl ForeignTryFrom<DisputeServiceDefendRequest> for DisputeDefendData {
    type Error = IntegrationError;
    fn foreign_try_from(
        value: DisputeServiceDefendRequest,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let connector_dispute_id = value.dispute_id;
        Ok(Self {
            dispute_id: connector_dispute_id.clone(),
            connector_dispute_id,
            defense_reason_code: value.reason_code.unwrap_or_default(),
            integrity_object: None,
        })
    }
}

pub fn generate_defend_dispute_response(
    router_data_v2: RouterDataV2<
        DefendDispute,
        DisputeFlowData,
        DisputeDefendData,
        DisputeResponseData,
    >,
) -> Result<DisputeServiceDefendResponse, error_stack::Report<ConnectorError>> {
    let defend_dispute_response = router_data_v2.response;

    let raw_connector_request = router_data_v2
        .resource_common_data
        .get_raw_connector_request();

    match defend_dispute_response {
        Ok(response) => Ok(DisputeServiceDefendResponse {
            dispute_id: response.connector_dispute_id,
            dispute_status: response.dispute_status as i32,
            connector_status_code: None,
            error: None,
            merchant_dispute_id: None,
            status_code: response.status_code as u32,
            response_headers: router_data_v2
                .resource_common_data
                .get_connector_response_headers_as_map(),
            raw_connector_request,
        }),
        Err(e) => Ok(DisputeServiceDefendResponse {
            dispute_id: e
                .connector_transaction_id
                .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
            dispute_status: common_enums::DisputeStatus::DisputeLost as i32,
            connector_status_code: None,
            error: Some(grpc_api_types::payments::ErrorInfo {
                unified_details: None,
                connector_details: Some(grpc_api_types::payments::ConnectorErrorDetails {
                    message: Some(e.message.clone()),
                    code: Some(e.code.clone()),
                    reason: e.reason.clone(),
                }),
                issuer_details: None,
            }),
            merchant_dispute_id: None,
            status_code: e.status_code as u32,
            response_headers: router_data_v2
                .resource_common_data
                .get_connector_response_headers_as_map(),
            raw_connector_request,
        }),
    }
}

pub fn generate_session_token_response(
    router_data_v2: RouterDataV2<
        ServerSessionAuthenticationToken,
        PaymentFlowData,
        ServerSessionAuthenticationTokenRequestData,
        ServerSessionAuthenticationTokenResponseData,
    >,
) -> Result<
    MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenResponse,
    error_stack::Report<ConnectorError>,
> {
    let response_headers = router_data_v2
        .resource_common_data
        .get_connector_response_headers_as_map();
    let _ = response_headers; // headers not in proto type
    let session_token_response = router_data_v2.response;

    match session_token_response {
        Ok(response) => Ok(
            MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenResponse {
                session_token: response.session_token,
                status_code: 200,
                error: None,
            },
        ),
        Err(e) => Ok(
            MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenResponse {
                session_token: String::new(),
                status_code: e.status_code as u32,
                error: Some(grpc_api_types::payments::ErrorInfo {
                    unified_details: None,
                    connector_details: Some(grpc_api_types::payments::ConnectorErrorDetails {
                        code: Some(e.code.clone()),
                        message: Some(e.message.clone()),
                        reason: e.reason.clone(),
                    }),
                    issuer_details: None,
                }),
            },
        ),
    }
}

impl ForeignTryFrom<grpc_api_types::payments::PaymentServiceCreateOrderRequest>
    for PaymentCreateOrderData
{
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::payments::PaymentServiceCreateOrderRequest,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let amount = match value.amount {
            Some(amount) => Ok(common_utils::types::Money {
                amount: common_utils::types::MinorUnit::new(amount.minor_amount),
                currency: common_enums::Currency::foreign_try_from(amount.currency())?,
            }),
            None => Err(report!(IntegrationError::MissingRequiredField {
                field_name: "amount",
                context: IntegrationErrorContext::default(),
            })),
        }?;
        let webhook_url = value.webhook_url.clone();
        let payment_method_type = <Option<common_enums::PaymentMethodType>>::foreign_try_from(
            value.payment_method_type(),
        )?;

        Ok(Self {
            amount: amount.amount,
            currency: amount.currency,
            integrity_object: None,
            metadata: value
                .metadata
                .map(|m| ForeignTryFrom::foreign_try_from((m, "metadata")))
                .transpose()?,
            webhook_url,
            payment_method_type,
        })
    }
}

impl
    ForeignTryFrom<(
        grpc_api_types::payments::PaymentServiceCreateOrderRequest,
        Connectors,
        &MaskedMetadata,
    )> for PaymentFlowData
{
    type Error = IntegrationError;

    fn foreign_try_from(
        (value, connectors, metadata): (
            grpc_api_types::payments::PaymentServiceCreateOrderRequest,
            Connectors,
            &MaskedMetadata,
        ),
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let merchant_id_from_header = extract_merchant_id_from_metadata(metadata)?;
        let vault_headers = extract_headers_from_metadata(metadata);

        // For order creation, create a default address
        let address = PaymentAddress::new(
            None,        // shipping
            None,        // billing
            None,        // payment_method_billing
            Some(false), // should_unify_address
        );

        let connector_feature_data = value
            .connector_feature_data
            .map(|m| ForeignTryFrom::foreign_try_from((m, "merchant account metadata")))
            .transpose()?;

        // Extract access token from state if present
        let access_token = value
            .state
            .as_ref()
            .and_then(|state| state.access_token.as_ref())
            .map(ServerAuthenticationTokenResponseData::foreign_try_from)
            .transpose()?;

        Ok(Self {
            merchant_id: merchant_id_from_header,
            payment_id: "IRRELEVANT_PAYMENT_ID".to_string(),
            attempt_id: "IRRELEVANT_ATTEMPT_ID".to_string(),
            status: common_enums::AttemptStatus::Pending,
            payment_method: PaymentMethod::Card,
            address,
            auth_type: common_enums::AuthenticationType::default(),
            connector_request_reference_id: extract_connector_request_reference_id(
                &value.merchant_order_id,
            ),
            customer_id: None, // PaymentServiceCreateOrderRequest doesn't have customer_id field
            connector_customer: None,
            description: None,
            return_url: None,
            connector_feature_data,
            amount_captured: None,
            minor_amount_captured: None,
            minor_amount_capturable: None,
            amount: None,
            access_token,
            session_token: None,
            reference_id: None,
            connector_order_id: None,
            preprocessing_id: None,
            connector_api_version: None,
            test_mode: value.test_mode,
            connector_http_status_code: None,
            external_latency: None,
            connectors,
            raw_connector_response: None,
            raw_connector_request: None,
            connector_response_headers: None,
            vault_headers,
            connector_response: None,
            recurring_mandate_payment_data: None,
            order_details: None,
            minor_amount_authorized: None,
            l2_l3_data: None,
        })
    }
}
#[derive(Debug, Clone, ToSchema, Serialize)]
pub struct CardSpecificFeatures {
    /// Indicates whether three_ds card payments are supported
    // #[schema(value_type = FeatureStatus)]
    pub three_ds: FeatureStatus,
    /// Indicates whether non three_ds card payments are supported
    // #[schema(value_type = FeatureStatus)]
    pub no_three_ds: FeatureStatus,
    /// List of supported card networks
    // #[schema(value_type = Vec<CardNetwork>)]
    pub supported_card_networks: Vec<CardNetwork>,
}

#[derive(Debug, Clone, ToSchema, Serialize)]
#[serde(untagged)]
pub enum PaymentMethodSpecificFeatures {
    /// Card specific features
    Card(CardSpecificFeatures),
}
/// Represents details of a payment method.
#[derive(Debug, Clone)]
pub struct PaymentMethodDetails {
    /// Indicates whether mandates are supported by this payment method.
    pub mandates: FeatureStatus,
    /// Indicates whether refund is supported by this payment method.
    pub refunds: FeatureStatus,
    /// List of supported capture methods
    pub supported_capture_methods: Vec<CaptureMethod>,
    /// Payment method specific features
    pub specific_features: Option<PaymentMethodSpecificFeatures>,
}
/// The status of the feature
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    PartialEq,
    serde::Deserialize,
    serde::Serialize,
    strum::Display,
    ToSchema,
)]
#[strum(serialize_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum FeatureStatus {
    NotSupported,
    Supported,
}
pub type PaymentMethodTypeMetadata = HashMap<PaymentMethodType, PaymentMethodDetails>;
pub type SupportedPaymentMethods = HashMap<PaymentMethod, PaymentMethodTypeMetadata>;

#[derive(Debug, Clone)]
pub struct ConnectorInfo {
    /// Display name of the Connector
    pub display_name: &'static str,
    /// Description of the connector.
    pub description: &'static str,
    /// Connector Type
    pub connector_type: PaymentConnectorCategory,
}

/// Connector Access Method
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    PartialEq,
    serde::Deserialize,
    serde::Serialize,
    strum::Display,
    ToSchema,
)]
#[strum(serialize_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum PaymentConnectorCategory {
    PaymentGateway,
    AlternativePaymentMethod,
    BankAcquirer,
}

#[derive(Debug, strum::Display, Eq, PartialEq, Hash)]
pub enum PaymentMethodDataType {
    Card,
    Bluecode,
    Knet,
    Benefit,
    MomoAtm,
    CardRedirect,
    AliPayQr,
    AliPayRedirect,
    AliPayHkRedirect,
    AmazonPayRedirect,
    MomoRedirect,
    KakaoPayRedirect,
    GoPayRedirect,
    GcashRedirect,
    ApplePay,
    ApplePayRedirect,
    ApplePayThirdPartySdk,
    DanaRedirect,
    DuitNow,
    GooglePay,
    GooglePayRedirect,
    GooglePayThirdPartySdk,
    MbWayRedirect,
    MobilePayRedirect,
    PaypalRedirect,
    PaypalSdk,
    Paze,
    SamsungPay,
    TwintRedirect,
    VippsRedirect,
    TouchNGoRedirect,
    WeChatPayRedirect,
    WeChatPayQr,
    CashappQr,
    SwishQr,
    KlarnaRedirect,
    KlarnaSdk,
    AffirmRedirect,
    AfterpayClearpayRedirect,
    PayBrightRedirect,
    WalleyRedirect,
    AlmaRedirect,
    AtomeRedirect,
    BancontactCard,
    Bizum,
    Blik,
    Eft,
    Eps,
    Giropay,
    Ideal,
    Interac,
    LocalBankRedirect,
    OnlineBankingCzechRepublic,
    OnlineBankingFinland,
    OnlineBankingPoland,
    OnlineBankingSlovakia,
    OpenBankingUk,
    Przelewy24,
    Sofort,
    Trustly,
    OnlineBankingFpx,
    OnlineBankingThailand,
    AchBankDebit,
    SepaBankDebit,
    BecsBankDebit,
    BacsBankDebit,
    AchBankTransfer,
    SepaBankTransfer,
    BacsBankTransfer,
    MultibancoBankTransfer,
    PermataBankTransfer,
    BcaBankTransfer,
    BniVaBankTransfer,
    BriVaBankTransfer,
    CimbVaBankTransfer,
    DanamonVaBankTransfer,
    MandiriVaBankTransfer,
    Pix,
    Pse,
    Crypto,
    MandatePayment,
    Reward,
    Upi,
    Boleto,
    Efecty,
    PagoEfectivo,
    RedCompra,
    RedPagos,
    Alfamart,
    Indomaret,
    Oxxo,
    SevenEleven,
    Lawson,
    MiniStop,
    FamilyMart,
    Seicomart,
    PayEasy,
    Givex,
    PaySafeCar,
    PaymentMethodToken,
    LocalBankTransfer,
    Mifinity,
    Fps,
    PromptPay,
    VietQr,
    OpenBanking,
    NetworkToken,
    NetworkTransactionIdAndCardDetails,
    DirectCarrierBilling,
    InstantBankTransfer,
    InstantBankTransferPoland,
    InstantBankTransferFinland,
    CardDetailsForNetworkTransactionId,
    RevolutPay,
    MbWay,
    Satispay,
    Wero,
    SepaGuaranteedBankDebit,
    IndonesianBankTransfer,
    Netbanking,
}

impl ForeignTryFrom<String> for Secret<time::Date> {
    type Error = IntegrationError;

    fn foreign_try_from(date_string: String) -> Result<Self, error_stack::Report<Self::Error>> {
        let date = time::Date::parse(
            &date_string,
            &time::format_description::well_known::Iso8601::DATE,
        )
        .map_err(|err| {
            tracing::error!("Failed to parse date string: {}", err);
            IntegrationError::InvalidDataFormat {
                field_name: "unknown",
                context: IntegrationErrorContext {
                    additional_context: Some("Invalid date format".to_string()),
                    ..Default::default()
                },
            }
        })?;
        Ok(Self::new(date))
    }
}

impl ForeignTryFrom<grpc_api_types::payments::BrowserInformation> for BrowserInformation {
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::payments::BrowserInformation,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        Ok(Self {
            color_depth: value.color_depth.map(|cd| cd as u8),
            java_enabled: value.java_enabled,
            java_script_enabled: value.java_script_enabled,
            language: value.language,
            screen_height: value.screen_height,
            screen_width: value.screen_width,
            time_zone: value.time_zone_offset_minutes,
            ip_address: value.ip_address.and_then(|ip| ip.parse().ok()),
            accept_header: value.accept_header,
            user_agent: value.user_agent,
            os_type: value.os_type,
            os_version: value.os_version,
            device_model: value.device_model,
            accept_language: value.accept_language,
            referer: value.referer,
        })
    }
}

impl ForeignTryFrom<PaymentServiceAuthorizeRequest>
    for ServerSessionAuthenticationTokenRequestData
{
    type Error = IntegrationError;

    fn foreign_try_from(
        value: PaymentServiceAuthorizeRequest,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let amount = value.amount.ok_or_else(|| {
            report!(IntegrationError::MissingRequiredField {
                field_name: "amount",
                context: IntegrationErrorContext::default(),
            })
        })?;
        let amount = common_utils::types::Money {
            amount: common_utils::types::MinorUnit::new(amount.minor_amount),
            currency: common_enums::Currency::foreign_try_from(amount.currency())?,
        };
        Ok(Self {
            amount: amount.amount,
            currency: amount.currency,
            browser_info: value
                .browser_info
                .map(BrowserInformation::foreign_try_from)
                .transpose()?,
        })
    }
}

impl ForeignTryFrom<PaymentServiceAuthorizeRequest> for ServerAuthenticationTokenRequestData {
    type Error = IntegrationError;

    fn foreign_try_from(
        _value: PaymentServiceAuthorizeRequest,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        Ok(Self {
            grant_type: "client_credentials".to_string(),
        })
    }
}

impl ForeignTryFrom<grpc_api_types::payments::MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest>
    for ServerAuthenticationTokenRequestData
{
    type Error = IntegrationError;

    fn foreign_try_from(
        _value: grpc_api_types::payments::MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        Ok(Self {
            grant_type: "client_credentials".to_string(),
        })
    }
}

// Generic implementation for access token request from connector auth
impl ForeignTryFrom<&ConnectorSpecificConfig> for ServerAuthenticationTokenRequestData {
    type Error = IntegrationError;

    fn foreign_try_from(
        _auth_type: &ConnectorSpecificConfig,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        // Default to client_credentials grant type for OAuth
        // Connectors can override this with their own specific implementations
        Ok(Self {
            grant_type: "client_credentials".to_string(),
        })
    }
}

impl ForeignTryFrom<PaymentServiceAuthorizeRequest> for ConnectorCustomerData {
    type Error = IntegrationError;

    fn foreign_try_from(
        value: PaymentServiceAuthorizeRequest,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        // Try to get email from top level first, fallback to billing address
        let email_string = value
            .customer
            .clone()
            .and_then(|customer| customer.email)
            .or_else(|| {
                value
                    .address
                    .as_ref()
                    .and_then(|addr| addr.billing_address.as_ref())
                    .and_then(|billing| billing.email.clone())
            });

        let email = email_string.and_then(|email_str| Email::try_from(email_str.expose()).ok());

        // Try to get name from top level customer_name first, fallback to billing address first_name
        let name_string = value
            .customer
            .clone()
            .and_then(|customer| customer.name)
            .map(Secret::new)
            .or_else(|| {
                value
                    .address
                    .as_ref()
                    .and_then(|addr| addr.billing_address.as_ref())
                    .and_then(|billing| billing.first_name.clone())
            });

        Ok(Self {
            customer_id: value
                .customer
                .and_then(|customer| customer.id)
                .map(Secret::new),
            email: email.map(Secret::new),
            name: name_string,
            description: None,
            split_payments: None,
            phone: None,
            preprocessing_id: None,
        })
    }
}

impl<T: PaymentMethodDataTypes> From<&PaymentsAuthorizeData<T>>
    for PaymentMethodTokenizationData<T>
{
    fn from(data: &PaymentsAuthorizeData<T>) -> Self {
        Self {
            payment_method_data: data.payment_method_data.clone(),
            browser_info: data.browser_info.clone(),
            currency: data.currency,
            amount: data.amount,
            capture_method: data.capture_method,
            split_payments: data.split_payments.clone(),
            customer_acceptance: data.customer_acceptance.clone(),
            setup_future_usage: data.setup_future_usage,
            setup_mandate_details: data.setup_mandate_details.clone(),
            mandate_id: data.mandate_id.clone(),
            integrity_object: None,
            connector_feature_data: data.connector_feature_data.clone(),
        }
    }
}

impl
    ForeignTryFrom<grpc_api_types::payments::MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenRequest>
    for ServerSessionAuthenticationTokenRequestData
{
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::payments::MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenRequest,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        // Extract domain-specific context from the oneof
        let payment_ctx = match value.domain_context {
            Some(grpc_api_types::payments::merchant_authentication_service_create_server_session_authentication_token_request::DomainContext::Payment(ctx)) => ctx,
            _ => return Err(report!(IntegrationError::InvalidDataFormat { field_name: "unknown", context: IntegrationErrorContext { additional_context: Some("Payment domain context is required for connector session".to_string()), ..Default::default() } })),
        };

        let amount = match payment_ctx.amount {
            Some(amount) => Ok(common_utils::types::Money {
                amount: common_utils::types::MinorUnit::new(amount.minor_amount),
                currency: common_enums::Currency::foreign_try_from(amount.currency())?,
            }),
            None => Err(report!(IntegrationError::MissingRequiredField {
                field_name: "amount",
                context: IntegrationErrorContext::default(),
            })),
        }?;

        Ok(Self {
            amount: amount.amount,
            currency: amount.currency,
            browser_info: payment_ctx
                .browser_info
                .map(BrowserInformation::foreign_try_from)
                .transpose()?,
        })
    }
}

impl
    ForeignTryFrom<(
        grpc_api_types::payments::MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenRequest,
        Connectors,
        &MaskedMetadata,
    )> for PaymentFlowData
{
    type Error = IntegrationError;

    fn foreign_try_from(
        (value, connectors, metadata): (
            grpc_api_types::payments::MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenRequest,
            Connectors,
            &MaskedMetadata,
        ),
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        // For session token operations, address information is typically not available or required
        let address: PaymentAddress = PaymentAddress::new(
            None,        // shipping
            None,        // billing
            None,        // payment_method_billing
            Some(false), // should_unify_address = false for session token operations
        );

        let merchant_id_from_header = extract_merchant_id_from_metadata(metadata)?;

        let connector_feature_data = value
            .connector_feature_data
            .map(|m| ForeignTryFrom::foreign_try_from((m, "feature_data")))
            .transpose()?;

        Ok(Self {
            merchant_id: merchant_id_from_header,
            payment_id: "IRRELEVANT_PAYMENT_ID".to_string(),
            attempt_id: "IRRELEVANT_ATTEMPT_ID".to_string(),
            status: common_enums::AttemptStatus::Pending,
            payment_method: PaymentMethod::Card, // Default
            address,
            auth_type: common_enums::AuthenticationType::default(),
            connector_request_reference_id: extract_connector_request_reference_id(
                &value.merchant_server_session_id.clone(),
            ),
            customer_id: None,
            connector_customer: None,
            description: None,
            return_url: None,
            connector_feature_data,
            amount_captured: None,
            minor_amount_captured: None,
            minor_amount_capturable: None,
            amount: None,
            access_token: None,
            session_token: None,
            reference_id: None,
            connector_order_id: None,
            preprocessing_id: None,
            connector_api_version: None,
            test_mode: value.test_mode,
            connector_http_status_code: None,
            external_latency: None,
            connectors,
            raw_connector_response: None,
            raw_connector_request: None,
            connector_response_headers: None,
            vault_headers: None,
            connector_response: None,
            recurring_mandate_payment_data: None,
            order_details: None,
            minor_amount_authorized: None,
            l2_l3_data: None,
        })
    }
}

impl ForeignTryFrom<PaymentServiceSetupRecurringRequest> for ConnectorCustomerData {
    type Error = IntegrationError;

    fn foreign_try_from(
        value: PaymentServiceSetupRecurringRequest,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        match value.customer {
            Some(customer) => {
                let email = customer
                    .email
                    .and_then(|email_str| Email::try_from(email_str.expose()).ok());
                Ok(Self {
                    customer_id: customer.id.map(Secret::new),
                    email: email.map(Secret::new),
                    name: customer.name.map(Secret::new),
                    description: None,
                    split_payments: None,
                    phone: None,
                    preprocessing_id: None,
                })
            }
            None => Ok(Self {
                customer_id: None,
                email: None,
                name: None,
                description: None,
                split_payments: None,
                phone: None,
                preprocessing_id: None,
            }),
        }
    }
}

impl<
        T: PaymentMethodDataTypes
            + Default
            + Debug
            + Send
            + Eq
            + PartialEq
            + Serialize
            + serde::de::DeserializeOwned
            + Clone
            + CardConversionHelper<T>,
    > ForeignTryFrom<grpc_api_types::payments::PaymentMethodServiceTokenizeRequest>
    for PaymentMethodTokenizationData<T>
{
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::payments::PaymentMethodServiceTokenizeRequest,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let money = match value.amount {
            Some(amount) => Ok(common_utils::types::Money {
                amount: common_utils::types::MinorUnit::new(amount.minor_amount),
                currency: common_enums::Currency::foreign_try_from(amount.currency())?,
            }),
            None => Err(report!(IntegrationError::MissingRequiredField {
                field_name: "amount",
                context: IntegrationErrorContext::default(),
            })),
        }?;
        let currency = money.currency;

        Ok(Self {
            amount: money.amount,
            currency,
            payment_method_data: PaymentMethodData::<T>::foreign_try_from(
                value.payment_method.clone().ok_or_else(|| {
                    IntegrationError::InvalidDataFormat {
                        field_name: "unknown",
                        context: IntegrationErrorContext {
                            additional_context: Some("Payment method data is required".to_string()),
                            ..Default::default()
                        },
                    }
                })?,
            )?,
            browser_info: None,
            capture_method: None,
            customer_acceptance: None,
            setup_future_usage: None,
            mandate_id: None,
            setup_mandate_details: None,
            integrity_object: None,
            split_payments: None,
            connector_feature_data: value
                .connector_feature_data
                .map(|m| ForeignTryFrom::foreign_try_from((m, "feature data")))
                .transpose()?,
        })
    }
}

impl
    ForeignTryFrom<(
        grpc_api_types::payments::PaymentMethodServiceTokenizeRequest,
        Connectors,
        &MaskedMetadata,
    )> for PaymentFlowData
{
    type Error = IntegrationError;

    fn foreign_try_from(
        (value, connectors, metadata): (
            grpc_api_types::payments::PaymentMethodServiceTokenizeRequest,
            Connectors,
            &MaskedMetadata,
        ),
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let merchant_id_from_header = extract_merchant_id_from_metadata(metadata)?;
        // For payment method token creation, address is optional
        let address = value
            .address
            .map(|addr| {
                // Then create PaymentAddress
                PaymentAddress::foreign_try_from(addr)
            })
            .transpose()?
            .unwrap_or_else(PaymentAddress::default);

        Ok(Self {
            merchant_id: merchant_id_from_header,
            payment_id: "IRRELEVANT_PAYMENT_ID".to_string(),
            attempt_id: "IRRELEVANT_ATTEMPT_ID".to_string(),
            status: common_enums::AttemptStatus::Pending,
            payment_method: PaymentMethod::foreign_try_from(
                value.payment_method.unwrap_or_default(),
            )?,
            address,
            auth_type: common_enums::AuthenticationType::default(),
            connector_request_reference_id: extract_connector_request_reference_id(
                &value.merchant_payment_method_id.clone(),
            ),
            customer_id: value
                .customer
                .clone()
                .and_then(|customer| customer.id)
                .map(|customer_id| CustomerId::try_from(Cow::from(customer_id)))
                .transpose()
                .change_context(IntegrationError::InvalidDataFormat {
                    field_name: "unknown",
                    context: IntegrationErrorContext {
                        additional_context: Some("Failed to parse Customer Id".to_string()),
                        ..Default::default()
                    },
                })?,
            connector_customer: value.customer.and_then(|c| c.id),
            description: None,
            return_url: value.return_url,
            connector_feature_data: None,
            amount_captured: None,
            minor_amount_captured: None,
            minor_amount_capturable: None,
            amount: None,
            access_token: None,
            session_token: None,
            reference_id: None,
            connector_order_id: None,
            preprocessing_id: None,
            connector_api_version: None,
            test_mode: value.test_mode,
            connector_http_status_code: None,
            external_latency: None,
            connectors,
            raw_connector_response: None,
            raw_connector_request: None,
            connector_response_headers: None,
            vault_headers: None,
            connector_response: None,
            recurring_mandate_payment_data: None,
            order_details: None,
            minor_amount_authorized: None,
            l2_l3_data: None,
        })
    }
}

pub fn generate_create_payment_method_token_response<T: PaymentMethodDataTypes>(
    router_data_v2: RouterDataV2<
        PaymentMethodToken,
        PaymentFlowData,
        PaymentMethodTokenizationData<T>,
        PaymentMethodTokenResponse,
    >,
) -> Result<
    grpc_api_types::payments::PaymentMethodServiceTokenizeResponse,
    error_stack::Report<ConnectorError>,
> {
    let token_response = router_data_v2.response;

    match token_response {
        Ok(response) => {
            let token_clone = response.token.clone();
            Ok(
                grpc_api_types::payments::PaymentMethodServiceTokenizeResponse {
                    payment_method_token: response.token,
                    error: None,
                    status_code: 200,
                    response_headers: router_data_v2
                        .resource_common_data
                        .get_connector_response_headers_as_map(),
                    merchant_payment_method_id: Some(token_clone),
                    state: None,
                },
            )
        }
        Err(e) => Ok(
            grpc_api_types::payments::PaymentMethodServiceTokenizeResponse {
                payment_method_token: String::new(),
                error: Some(grpc_api_types::payments::ErrorInfo {
                    unified_details: None,
                    connector_details: Some(grpc_api_types::payments::ConnectorErrorDetails {
                        message: Some(e.message.clone()),
                        code: Some(e.code.clone()),
                        reason: e.reason.clone(),
                    }),
                    issuer_details: None,
                }),
                status_code: e.status_code as u32,
                response_headers: router_data_v2
                    .resource_common_data
                    .get_connector_response_headers_as_map(),
                merchant_payment_method_id: e.connector_transaction_id,
                state: None,
            },
        ),
    }
}

impl ForeignTryFrom<grpc_api_types::payments::CustomerServiceCreateRequest>
    for ConnectorCustomerData
{
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::payments::CustomerServiceCreateRequest,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let email = value
            .email
            .and_then(|email_str| Email::try_from(email_str.expose()).ok());

        Ok(Self {
            customer_id: value.merchant_customer_id.map(Secret::new),
            email: email.map(Secret::new),
            name: value.customer_name.map(Secret::new),
            description: None, // description field not available in this proto
            split_payments: None,
            phone: None,
            preprocessing_id: None,
        })
    }
}

impl
    ForeignTryFrom<(
        grpc_api_types::payments::CustomerServiceCreateRequest,
        Connectors,
        &MaskedMetadata,
    )> for PaymentFlowData
{
    type Error = IntegrationError;

    fn foreign_try_from(
        (value, connectors, metadata): (
            grpc_api_types::payments::CustomerServiceCreateRequest,
            Connectors,
            &MaskedMetadata,
        ),
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let merchant_id_from_header = extract_merchant_id_from_metadata(metadata)?;
        let address = value
            .address
            .map(|addr| {
                // Then create PaymentAddress
                PaymentAddress::foreign_try_from(addr)
            })
            .transpose()?
            .unwrap_or_else(PaymentAddress::default);

        let connector_feature_data = value
            .connector_feature_data
            .map(|m| ForeignTryFrom::foreign_try_from((m, "merchant account metadata")))
            .transpose()?;

        Ok(Self {
            merchant_id: merchant_id_from_header,
            payment_id: "IRRELEVANT_PAYMENT_ID".to_string(),
            attempt_id: "IRRELEVANT_ATTEMPT_ID".to_string(),
            status: common_enums::AttemptStatus::Pending,
            payment_method: PaymentMethod::Card, // Default for connector customer creation
            address,                             // Default address
            auth_type: common_enums::AuthenticationType::default(),
            connector_request_reference_id: value.merchant_customer_id.unwrap_or_default(), // request_ref_id field not available in this proto
            customer_id: None,
            connector_customer: None,
            description: None, // description field not available in this proto
            return_url: None,
            connector_feature_data,
            amount_captured: None,
            minor_amount_captured: None,
            minor_amount_capturable: None,
            amount: None,
            access_token: None,
            session_token: None,
            reference_id: None,
            connector_order_id: None,
            preprocessing_id: None,
            connector_api_version: None,
            test_mode: value.test_mode,
            connector_http_status_code: None,
            external_latency: None,
            connectors,
            raw_connector_response: None,
            raw_connector_request: None,
            connector_response_headers: None,
            vault_headers: None,
            connector_response: None,
            recurring_mandate_payment_data: None,
            order_details: None,
            minor_amount_authorized: None,
            l2_l3_data: None,
        })
    }
}

pub fn generate_create_connector_customer_response(
    router_data_v2: RouterDataV2<
        CreateConnectorCustomer,
        PaymentFlowData,
        ConnectorCustomerData,
        crate::connector_types::ConnectorCustomerResponse,
    >,
) -> Result<grpc_payment_types::CustomerServiceCreateResponse, error_stack::Report<ConnectorError>>
{
    let customer_response = router_data_v2.response;

    match customer_response {
        Ok(response) => Ok(grpc_payment_types::CustomerServiceCreateResponse {
            connector_customer_id: response.connector_customer_id.clone(),
            error: None,
            status_code: 200,
            response_headers: router_data_v2
                .resource_common_data
                .get_connector_response_headers_as_map(),
            merchant_customer_id: Some(response.connector_customer_id.clone()),
        }),
        Err(e) => Ok(grpc_payment_types::CustomerServiceCreateResponse {
            connector_customer_id: String::new(),
            status_code: e.status_code as u32,
            response_headers: router_data_v2
                .resource_common_data
                .get_connector_response_headers_as_map(),
            merchant_customer_id: e.connector_transaction_id,
            error: Some(grpc_api_types::payments::ErrorInfo {
                unified_details: None,
                connector_details: Some(grpc_api_types::payments::ConnectorErrorDetails {
                    message: Some(e.message.clone()),
                    code: Some(e.code.clone()),
                    reason: e.reason.clone(),
                }),
                issuer_details: None,
            }),
        }),
    }
}

impl<
        T: PaymentMethodDataTypes
            + Default
            + Debug
            + Send
            + Eq
            + PartialEq
            + Serialize
            + serde::de::DeserializeOwned
            + Clone
            + CardConversionHelper<T>,
    > ForeignTryFrom<grpc_api_types::payments::RecurringPaymentServiceChargeRequest>
    for RepeatPaymentData<T>
{
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::payments::RecurringPaymentServiceChargeRequest,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        // Extract values first to avoid partial move
        let merchant_configured_currency = match value.merchant_configured_currency {
            None => None,
            Some(_) => Some(common_enums::Currency::foreign_try_from(
                value.merchant_configured_currency(),
            )?),
        };
        let mit_category = match value.mit_category() {
            grpc_payment_types::MitCategory::Unspecified => None,
            _ => Some(common_enums::MitCategory::foreign_try_from(
                value.mit_category(),
            )?),
        };
        let payment_method_type =
            <Option<PaymentMethodType>>::foreign_try_from(value.payment_method_type())?;
        let capture_method = value.capture_method();
        let merchant_order_id = value.merchant_order_id;
        let webhook_url = value.webhook_url;

        let email: Option<Email> = match value.email {
            Some(ref email_str) => {
                Some(Email::try_from(email_str.clone().expose()).map_err(|_| {
                    error_stack::Report::new(IntegrationError::InvalidDataFormat {
                        field_name: "unknown",
                        context: IntegrationErrorContext {
                            additional_context: Some("Invalid email".to_string()),
                            ..Default::default()
                        },
                    })
                })?)
            }
            None => None,
        };

        // Extract mandate reference_id
        let mandate_ref = match value.connector_recurring_payment_id {
            Some(mandate_reference_id) => match mandate_reference_id.mandate_id_type {
                Some(grpc_payment_types::mandate_reference::MandateIdType::ConnectorMandateId(
                    cm,
                )) => MandateReferenceId::ConnectorMandateId(ConnectorMandateReferenceId::new(
                    cm.connector_mandate_id,
                    cm.payment_method_id,
                    None,
                    None,
                    cm.connector_mandate_request_reference_id,
                )),
                Some(grpc_payment_types::mandate_reference::MandateIdType::NetworkMandateId(
                    nmi,
                )) => MandateReferenceId::NetworkMandateId(nmi),
                Some(
                    grpc_payment_types::mandate_reference::MandateIdType::NetworkTokenWithNti(nti),
                ) => MandateReferenceId::NetworkTokenWithNTI(NetworkTokenWithNTIRef {
                    network_transaction_id: nti.network_transaction_id,
                    token_exp_month: nti.token_exp_month,
                    token_exp_year: nti.token_exp_year,
                }),
                None => Err(IntegrationError::InvalidDataFormat {
                    field_name: "unknown",
                    context: IntegrationErrorContext {
                        additional_context: Some("Mandate reference id is required".to_string()),
                        ..Default::default()
                    },
                })?,
            },
            None => Err(IntegrationError::InvalidDataFormat {
                field_name: "unknown",
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "Mandate reference is required for repeat payments".to_string(),
                    ),
                    ..Default::default()
                },
            })?,
        };

        let payment_method_data = value
            .payment_method
            .map(PaymentMethodData::<T>::foreign_try_from)
            .transpose()?
            .unwrap_or(PaymentMethodData::MandatePayment);

        let billing_descriptor =
            value
                .billing_descriptor
                .as_ref()
                .map(|descriptor| BillingDescriptor {
                    name: descriptor.name.clone(),
                    city: descriptor.city.clone(),
                    phone: descriptor.phone.clone(),
                    statement_descriptor: descriptor.statement_descriptor.clone(),
                    statement_descriptor_suffix: descriptor.statement_descriptor_suffix.clone(),
                    reference: descriptor.reference.clone(),
                });

        let authentication_data = value
            .authentication_data
            .clone()
            .map(router_request_types::AuthenticationData::try_from)
            .transpose()?;
        let amount = match value.amount {
            Some(amount) => Ok(common_utils::types::Money {
                amount: common_utils::types::MinorUnit::new(amount.minor_amount),
                currency: common_enums::Currency::foreign_try_from(amount.currency())?,
            }),
            None => Err(report!(IntegrationError::MissingRequiredField {
                field_name: "amount",
                context: IntegrationErrorContext::default(),
            })),
        }?;

        Ok(Self {
            mandate_reference: mandate_ref,
            amount: amount.amount.get_amount_as_i64(),
            minor_amount: amount.amount,
            currency: amount.currency,
            merchant_order_id,
            metadata: value
                .metadata
                .map(|m| ForeignTryFrom::foreign_try_from((m, "metadata")))
                .transpose()?,
            webhook_url,
            router_return_url: value.return_url,
            integrity_object: None,
            capture_method: Some(CaptureMethod::foreign_try_from(capture_method)?),
            email,
            browser_info: value
                .browser_info
                .map(BrowserInformation::foreign_try_from)
                .transpose()?,
            payment_method_type,
            connector_feature_data: value
                .connector_feature_data
                .map(|m| ForeignTryFrom::foreign_try_from((m, "feature data")))
                .transpose()?,
            off_session: value.off_session,
            split_payments: None,
            recurring_mandate_payment_data: match value.original_payment_authorized_amount {
                Some(money) => Some(RecurringMandatePaymentData {
                    payment_method_type: None,
                    original_payment_authorized_amount: Some(common_utils::types::Money {
                        amount: common_utils::types::MinorUnit::new(money.minor_amount),
                        currency: common_enums::Currency::foreign_try_from(money.currency())?,
                    }),
                    mandate_metadata: None,
                }),
                None => None,
            },
            shipping_cost: value.shipping_cost.map(common_utils::types::MinorUnit::new),
            mit_category,
            billing_descriptor,
            enable_partial_authorization: value.enable_partial_authorization,
            payment_method_data,
            authentication_data,
            locale: value.locale.clone(),
            connector_testing_data: value.connector_testing_data.and_then(|s| {
                serde_json::from_str(&s.expose())
                    .ok()
                    .map(common_utils::pii::SecretSerdeValue::new)
            }),
            merchant_account_id: value.merchant_account_id,
            merchant_configured_currency,
        })
    }
}

pub fn generate_repeat_payment_response<T: PaymentMethodDataTypes>(
    router_data_v2: RouterDataV2<
        RepeatPayment,
        PaymentFlowData,
        RepeatPaymentData<T>,
        PaymentsResponseData,
    >,
) -> Result<
    grpc_api_types::payments::RecurringPaymentServiceChargeResponse,
    error_stack::Report<ConnectorError>,
> {
    let transaction_response = router_data_v2.response;
    let status = router_data_v2.resource_common_data.status;
    let grpc_status = grpc_api_types::payments::PaymentStatus::foreign_from(status);

    // Create state if either access token or connector customer is available
    let state = if router_data_v2.resource_common_data.access_token.is_some()
        || router_data_v2
            .resource_common_data
            .connector_customer
            .is_some()
    {
        Some(ConnectorState {
            access_token: router_data_v2
                .resource_common_data
                .access_token
                .as_ref()
                .map(|token_data| grpc_api_types::payments::AccessToken {
                    token: Some(token_data.access_token.clone()),
                    expires_in_seconds: token_data.expires_in,
                    token_type: token_data.token_type.clone(),
                }),
            connector_customer_id: router_data_v2
                .resource_common_data
                .connector_customer
                .clone(),
        })
    } else {
        None
    };
    let raw_connector_response = router_data_v2
        .resource_common_data
        .get_raw_connector_response();

    let raw_connector_request = router_data_v2
        .resource_common_data
        .get_raw_connector_request();

    let connector_response = router_data_v2
        .resource_common_data
        .connector_response
        .clone()
        .and_then(|data| {
            match grpc_api_types::payments::ConnectorResponseData::foreign_try_from(data) {
                Ok(data) => Some(data),
                Err(err) => {
                    tracing::error!("Failed to convert ConnectorResponseData: {err:?}");
                    None
                }
            }
        });

    match transaction_response {
        Ok(response) => match response {
            PaymentsResponseData::TransactionResponse {
                resource_id,
                network_txn_id,
                connector_response_reference_id,
                connector_metadata,
                mandate_reference,
                status_code,
                incremental_authorization_allowed,
                ..
            } => Ok(
                grpc_api_types::payments::RecurringPaymentServiceChargeResponse {
                    connector_transaction_id: Option::foreign_try_from(resource_id)?,
                    status: grpc_status as i32,
                    error: None,
                    network_transaction_id: network_txn_id,
                    merchant_charge_id: connector_response_reference_id,
                    connector_feature_data: convert_connector_metadata_to_secret_string(
                        connector_metadata,
                    ),
                    mandate_reference: mandate_reference.map(|m| {
                        grpc_api_types::payments::MandateReference {
                            mandate_id_type: Some(grpc_api_types::payments::mandate_reference::MandateIdType::ConnectorMandateId(grpc_api_types::payments::ConnectorMandateReferenceId {
                            connector_mandate_id: m.connector_mandate_id,
                            payment_method_id: m.payment_method_id,
                            connector_mandate_request_reference_id: m
                                .connector_mandate_request_reference_id,
                        })),
                        }
                    }),
                    status_code: status_code as u32,
                    raw_connector_response,
                    response_headers: router_data_v2
                        .resource_common_data
                        .get_connector_response_headers_as_map(),
                    state,
                    raw_connector_request,
                    connector_response,
                    captured_amount: router_data_v2.resource_common_data.amount_captured,
                    incremental_authorization_allowed,
                },
            ),
            _ => Err(report!(ConnectorError::UnexpectedResponseError {
                context: ResponseTransformationErrorContext {
                    http_status_code: None,
                    additional_context: Some("Invalid response type received from connector".to_owned()),
                },
            })),
        },
        Err(err) => {
            let status = match err.get_attempt_status_for_grpc(
                err.status_code,
                router_data_v2.resource_common_data.status,
            ) {
                Some(attempt_status) => {
                    grpc_api_types::payments::PaymentStatus::foreign_from(attempt_status)
                }
                None => grpc_api_types::payments::PaymentStatus::Unspecified,
            };
            Ok(
                grpc_api_types::payments::RecurringPaymentServiceChargeResponse {
                    connector_transaction_id: err.connector_transaction_id.clone(),
                    status: status as i32,
                    error: Some(grpc_api_types::payments::ErrorInfo {
                        unified_details: None,
                        connector_details: Some(grpc_api_types::payments::ConnectorErrorDetails {
                            message: Some(err.message.clone()),
                            code: Some(err.code.clone()),
                            reason: err.reason.clone(),
                        }),
                        issuer_details: Some(grpc_payment_types::IssuerErrorDetails{
                            code: None,
                            message: err.network_error_message.clone(),
                            network_details: Some(grpc_payment_types::NetworkErrorDetails {
                                decline_code: err.network_decline_code.clone(),
                                advice_code: err.network_advice_code.clone(),
                                error_message: err.network_error_message.clone(),
                            }),
                        })
                    }),
                    network_transaction_id: None,
                    merchant_charge_id: err.connector_transaction_id,
                    connector_feature_data: None,
                    raw_connector_response: None,
                    status_code: err.status_code as u32,
                    response_headers: router_data_v2
                        .resource_common_data
                        .get_connector_response_headers_as_map(),
                    state,
                    mandate_reference: None,
                    raw_connector_request,
                    connector_response,
                    captured_amount: None,
                    incremental_authorization_allowed: None,
                },
            )
        }
    }
}

impl ForeignTryFrom<&grpc_api_types::payments::AccessToken>
    for ServerAuthenticationTokenResponseData
{
    type Error = IntegrationError;
    fn foreign_try_from(
        token: &grpc_api_types::payments::AccessToken,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let access_token = token
            .token
            .clone()
            .ok_or(IntegrationError::InvalidDataFormat {
                field_name: "unknown",
                context: IntegrationErrorContext {
                    additional_context: Some("Access Token is missing".to_string()),
                    ..Default::default()
                },
            })?;
        Ok(Self {
            access_token,
            token_type: token.token_type.clone(),
            expires_in: token.expires_in_seconds,
        })
    }
}

fn convert_connector_specific_to_grpc(
    data: ConnectorSpecificClientAuthenticationResponse,
) -> grpc_api_types::payments::ClientAuthenticationTokenData {
    let proto_response = match data {
        ConnectorSpecificClientAuthenticationResponse::Stripe(stripe_data) => {
            grpc_api_types::payments::ConnectorSpecificClientAuthenticationResponse {
                connector: Some(
                    grpc_api_types::payments::connector_specific_client_authentication_response::Connector::Stripe(
                        grpc_api_types::payments::StripeClientAuthenticationResponse {
                            client_secret: Some(stripe_data.client_secret),
                        },
                    ),
                ),
            }
        }
        ConnectorSpecificClientAuthenticationResponse::Adyen(adyen_data) => {
            grpc_api_types::payments::ConnectorSpecificClientAuthenticationResponse {
                connector: Some(
                    grpc_api_types::payments::connector_specific_client_authentication_response::Connector::Adyen(
                        grpc_api_types::payments::AdyenClientAuthenticationResponse {
                            session_id: adyen_data.session_id,
                            session_data: Some(adyen_data.session_data),
                        },
                    ),
                ),
            }
        }
        ConnectorSpecificClientAuthenticationResponse::Checkout(checkout_data) => {
            grpc_api_types::payments::ConnectorSpecificClientAuthenticationResponse {
                connector: Some(
                    grpc_api_types::payments::connector_specific_client_authentication_response::Connector::Checkout(
                        grpc_api_types::payments::CheckoutClientAuthenticationResponse {
                            payment_session_id: checkout_data.payment_session_id,
                            payment_session_token: Some(checkout_data.payment_session_token),
                            payment_session_secret: Some(checkout_data.payment_session_secret),
                        },
                    ),
                ),
            }
        }
        ConnectorSpecificClientAuthenticationResponse::Cybersource(cybersource_data) => {
            grpc_api_types::payments::ConnectorSpecificClientAuthenticationResponse {
                connector: Some(
                    grpc_api_types::payments::connector_specific_client_authentication_response::Connector::Cybersource(
                        grpc_api_types::payments::CybersourceClientAuthenticationResponse {
                            capture_context: Some(cybersource_data.capture_context),
                        },
                    ),
                ),
            }
        }
        ConnectorSpecificClientAuthenticationResponse::Nuvei(nuvei_data) => {
            grpc_api_types::payments::ConnectorSpecificClientAuthenticationResponse {
                connector: Some(
                    grpc_api_types::payments::connector_specific_client_authentication_response::Connector::Nuvei(
                        grpc_api_types::payments::NuveiClientAuthenticationResponse {
                            session_token: Some(nuvei_data.session_token),
                        },
                    ),
                ),
            }
        }
        ConnectorSpecificClientAuthenticationResponse::Mollie(mollie_data) => {
            grpc_api_types::payments::ConnectorSpecificClientAuthenticationResponse {
                connector: Some(
                    grpc_api_types::payments::connector_specific_client_authentication_response::Connector::Mollie(
                        grpc_api_types::payments::MollieClientAuthenticationResponse {
                            payment_id: mollie_data.payment_id,
                            checkout_url: Some(mollie_data.checkout_url),
                        },
                    ),
                ),
            }
        }
        ConnectorSpecificClientAuthenticationResponse::Globalpay(globalpay_data) => {
            grpc_api_types::payments::ConnectorSpecificClientAuthenticationResponse {
                connector: Some(
                    grpc_api_types::payments::connector_specific_client_authentication_response::Connector::Globalpay(
                        grpc_api_types::payments::GlobalpayClientAuthenticationResponse {
                            access_token: Some(globalpay_data.access_token),
                            token_type: globalpay_data.token_type,
                            expires_in: globalpay_data.expires_in,
                        },
                    ),
                ),
            }
        }
        ConnectorSpecificClientAuthenticationResponse::Bluesnap(bluesnap_data) => {
            grpc_api_types::payments::ConnectorSpecificClientAuthenticationResponse {
                connector: Some(
                    grpc_api_types::payments::connector_specific_client_authentication_response::Connector::Bluesnap(
                        grpc_api_types::payments::BluesnapClientAuthenticationResponse {
                            pf_token: Some(bluesnap_data.pf_token),
                        },
                    ),
                ),
            }
        }
        ConnectorSpecificClientAuthenticationResponse::Rapyd(rapyd_data) => {
            grpc_api_types::payments::ConnectorSpecificClientAuthenticationResponse {
                connector: Some(
                    grpc_api_types::payments::connector_specific_client_authentication_response::Connector::Rapyd(
                        grpc_api_types::payments::RapydClientAuthenticationResponse {
                            checkout_id: rapyd_data.checkout_id,
                            redirect_url: rapyd_data.redirect_url,
                        },
                    ),
                ),
            }
        }
        ConnectorSpecificClientAuthenticationResponse::Shift4(shift4_data) => {
            grpc_api_types::payments::ConnectorSpecificClientAuthenticationResponse {
                connector: Some(
                    grpc_api_types::payments::connector_specific_client_authentication_response::Connector::Shift4(
                        grpc_api_types::payments::Shift4ClientAuthenticationResponse {
                            client_secret: Some(shift4_data.client_secret),
                        },
                    ),
                ),
            }
        }
        ConnectorSpecificClientAuthenticationResponse::BankOfAmerica(boa_data) => {
            grpc_api_types::payments::ConnectorSpecificClientAuthenticationResponse {
                connector: Some(
                    grpc_api_types::payments::connector_specific_client_authentication_response::Connector::BankOfAmerica(
                        grpc_api_types::payments::BankOfAmericaClientAuthenticationResponse {
                            capture_context: Some(boa_data.capture_context),
                        },
                    ),
                ),
            }
        }
        ConnectorSpecificClientAuthenticationResponse::Wellsfargo(wf_data) => {
            grpc_api_types::payments::ConnectorSpecificClientAuthenticationResponse {
                connector: Some(
                    grpc_api_types::payments::connector_specific_client_authentication_response::Connector::Wellsfargo(
                        grpc_api_types::payments::WellsfargoClientAuthenticationResponse {
                            capture_context: Some(wf_data.capture_context),
                        },
                    ),
                ),
            }
        }
        ConnectorSpecificClientAuthenticationResponse::Fiserv(fiserv_data) => {
            grpc_api_types::payments::ConnectorSpecificClientAuthenticationResponse {
                connector: Some(
                    grpc_api_types::payments::connector_specific_client_authentication_response::Connector::Fiserv(
                        grpc_api_types::payments::FiservClientAuthenticationResponse {
                            session_id: Some(fiserv_data.session_id),
                        },
                    ),
                ),
            }
        }
        ConnectorSpecificClientAuthenticationResponse::Elavon(elavon_data) => {
            grpc_api_types::payments::ConnectorSpecificClientAuthenticationResponse {
                connector: Some(
                    grpc_api_types::payments::connector_specific_client_authentication_response::Connector::Elavon(
                        grpc_api_types::payments::ElavonClientAuthenticationResponse {
                            session_token: Some(elavon_data.session_token),
                        },
                    ),
                ),
            }
        }
        ConnectorSpecificClientAuthenticationResponse::Noon(noon_data) => {
            grpc_api_types::payments::ConnectorSpecificClientAuthenticationResponse {
                connector: Some(
                    grpc_api_types::payments::connector_specific_client_authentication_response::Connector::Noon(
                        grpc_api_types::payments::NoonClientAuthenticationResponse {
                            order_id: noon_data.order_id,
                            checkout_url: Some(noon_data.checkout_url),
                        },
                    ),
                ),
            }
        }
        ConnectorSpecificClientAuthenticationResponse::Paysafe(paysafe_data) => {
            grpc_api_types::payments::ConnectorSpecificClientAuthenticationResponse {
                connector: Some(
                    grpc_api_types::payments::connector_specific_client_authentication_response::Connector::Paysafe(
                        grpc_api_types::payments::PaysafeClientAuthenticationResponse {
                            payment_handle_token: Some(paysafe_data.payment_handle_token),
                        },
                    ),
                ),
            }
        }
        ConnectorSpecificClientAuthenticationResponse::Bamboraapac(bamboraapac_data) => {
            grpc_api_types::payments::ConnectorSpecificClientAuthenticationResponse {
                connector: Some(
                    grpc_api_types::payments::connector_specific_client_authentication_response::Connector::Bamboraapac(
                        grpc_api_types::payments::BamboraapacClientAuthenticationResponse {
                            token: Some(bamboraapac_data.token),
                        },
                    ),
                ),
            }
        }
        ConnectorSpecificClientAuthenticationResponse::Jpmorgan(jpmorgan_data) => {
            grpc_api_types::payments::ConnectorSpecificClientAuthenticationResponse {
                connector: Some(
                    grpc_api_types::payments::connector_specific_client_authentication_response::Connector::Jpmorgan(
                        grpc_api_types::payments::JpmorganClientAuthenticationResponse {
                            transaction_id: jpmorgan_data.transaction_id,
                            request_id: jpmorgan_data.request_id,
                        },
                    ),
                ),
            }
        }
        ConnectorSpecificClientAuthenticationResponse::Billwerk(billwerk_data) => {
            grpc_api_types::payments::ConnectorSpecificClientAuthenticationResponse {
                connector: Some(
                    grpc_api_types::payments::connector_specific_client_authentication_response::Connector::Billwerk(
                        grpc_api_types::payments::BillwerkClientAuthenticationResponse {
                            session_id: billwerk_data.session_id,
                        },
                    ),
                ),
            }
        }
        ConnectorSpecificClientAuthenticationResponse::Datatrans(datatrans_data) => {
            grpc_api_types::payments::ConnectorSpecificClientAuthenticationResponse {
                connector: Some(
                    grpc_api_types::payments::connector_specific_client_authentication_response::Connector::Datatrans(
                        grpc_api_types::payments::DatatransClientAuthenticationResponse {
                            transaction_id: Some(datatrans_data.transaction_id),
                        },
                    ),
                ),
            }
        }
        ConnectorSpecificClientAuthenticationResponse::Bambora(bambora_data) => {
            grpc_api_types::payments::ConnectorSpecificClientAuthenticationResponse {
                connector: Some(
                    grpc_api_types::payments::connector_specific_client_authentication_response::Connector::Bambora(
                        grpc_api_types::payments::BamboraClientAuthenticationResponse {
                            token: Some(bambora_data.token),
                        },
                    ),
                ),
            }
        }
        ConnectorSpecificClientAuthenticationResponse::Payload(payload_data) => {
            grpc_api_types::payments::ConnectorSpecificClientAuthenticationResponse {
                connector: Some(
                    grpc_api_types::payments::connector_specific_client_authentication_response::Connector::Payload(
                        grpc_api_types::payments::PayloadClientAuthenticationResponse {
                            client_token: Some(payload_data.client_token),
                        },
                    ),
                ),
            }
        }
        ConnectorSpecificClientAuthenticationResponse::Multisafepay(multisafepay_data) => {
            grpc_api_types::payments::ConnectorSpecificClientAuthenticationResponse {
                connector: Some(
                    grpc_api_types::payments::connector_specific_client_authentication_response::Connector::Multisafepay(
                        grpc_api_types::payments::MultisafepayClientAuthenticationResponse {
                            api_token: Some(multisafepay_data.api_token),
                        },
                    ),
                ),
            }
        }
        ConnectorSpecificClientAuthenticationResponse::Nexinets(nexinets_data) => {
            grpc_api_types::payments::ConnectorSpecificClientAuthenticationResponse {
                connector: Some(
                    grpc_api_types::payments::connector_specific_client_authentication_response::Connector::Nexinets(
                        grpc_api_types::payments::NexinetsClientAuthenticationResponse {
                            order_id: nexinets_data.order_id,
                        },
                    ),
                ),
            }
        }
        ConnectorSpecificClientAuthenticationResponse::Nexixpay(nexixpay_data) => {
            grpc_api_types::payments::ConnectorSpecificClientAuthenticationResponse {
                connector: Some(
                    grpc_api_types::payments::connector_specific_client_authentication_response::Connector::Nexixpay(
                        grpc_api_types::payments::NexixpayClientAuthenticationResponse {
                            security_token: Some(nexixpay_data.security_token),
                            hosted_page: nexixpay_data.hosted_page,
                        },
                    ),
                ),
            }
        }
    };
    grpc_api_types::payments::ClientAuthenticationTokenData {
        sdk_type: Some(
            grpc_api_types::payments::client_authentication_token_data::SdkType::ConnectorSpecific(
                proto_response,
            ),
        ),
    }
}

pub fn generate_payment_sdk_session_token_response(
    router_data_v2: RouterDataV2<
        ClientAuthenticationToken,
        PaymentFlowData,
        ClientAuthenticationTokenRequestData,
        PaymentsResponseData,
    >,
) -> Result<
    MerchantAuthenticationServiceCreateClientAuthenticationTokenResponse,
    error_stack::Report<ConnectorError>,
> {
    let transaction_response = router_data_v2.response;

    let raw_connector_request = router_data_v2
        .resource_common_data
        .get_raw_connector_request();

    let raw_connector_response = router_data_v2
        .resource_common_data
        .get_raw_connector_response();

    match transaction_response {
        Ok(response) => match response {
            PaymentsResponseData::ClientAuthenticationTokenResponse {
                session_data,
                status_code,
            } => {
                let grpc_session_data = match session_data {
                    ClientAuthenticationTokenData::GooglePay(gpay_token) => {
                        let gpay_response = grpc_api_types::payments::GpayClientAuthenticationResponse::foreign_try_from(*gpay_token)?;
                        Some(grpc_api_types::payments::ClientAuthenticationTokenData {
                                sdk_type: Some(
                                    grpc_api_types::payments::client_authentication_token_data::SdkType::GooglePay(
                                        gpay_response,
                                    ),
                                ),
                            })
                    }
                    ClientAuthenticationTokenData::Paypal(paypal_token) => {
                        let paypal_response =
                            grpc_api_types::payments::PaypalClientAuthenticationResponse {
                                connector: paypal_token.connector,
                                session_token: paypal_token.session_token,
                                sdk_next_action: grpc_api_types::payments::SdkNextAction::from(
                                    paypal_token.sdk_next_action.next_action,
                                )
                                .into(),
                                client_token: paypal_token.client_token,
                                transaction_info: paypal_token.transaction_info.map(grpc_api_types::payments::PaypalTransactionInfo::foreign_try_from).transpose()?,
                            };
                        Some(grpc_api_types::payments::ClientAuthenticationTokenData {
                                sdk_type: Some(
                                    grpc_api_types::payments::client_authentication_token_data::SdkType::Paypal(
                                        paypal_response,
                                    ),
                                ),
                            })
                    }
                    ClientAuthenticationTokenData::ApplePay(apple_pay_token) => {
                        let apple_pay_response = grpc_api_types::payments::ApplepayClientAuthenticationResponse {
                            session_response: apple_pay_token.session_response.map(grpc_api_types::payments::ApplePaySessionResponse::foreign_try_from).transpose()?,
                            payment_request_data: apple_pay_token.payment_request_data.map(grpc_api_types::payments::ApplePayPaymentRequest::foreign_try_from).transpose()?,
                            connector: apple_pay_token.connector,
                            delayed_session_token: apple_pay_token.delayed_session_token,
                            sdk_next_action: grpc_api_types::payments::SdkNextAction::from(apple_pay_token.sdk_next_action.next_action).into(),
                            connector_reference_id: apple_pay_token.connector_reference_id,
                            connector_sdk_public_key: apple_pay_token.connector_sdk_public_key,
                            connector_merchant_id: apple_pay_token.connector_merchant_id,
                        };
                        Some(grpc_api_types::payments::ClientAuthenticationTokenData {
                                sdk_type: Some(
                                    grpc_api_types::payments::client_authentication_token_data::SdkType::ApplePay(
                                        apple_pay_response,
                                    ),
                                ),
                            })
                    }
                    ClientAuthenticationTokenData::ConnectorSpecific(data) => {
                        Some(convert_connector_specific_to_grpc(*data))
                    }
                };

                Ok(
                    MerchantAuthenticationServiceCreateClientAuthenticationTokenResponse {
                        session_data: grpc_session_data,
                        error: None,
                        raw_connector_response,
                        status_code: status_code as u32,
                        raw_connector_request,
                    },
                )
            }
            _ => Err(report!(ConnectorError::UnexpectedResponseError {
                context: ResponseTransformationErrorContext {
                    http_status_code: None,
                    additional_context: Some(
                        "Invalid response type received from connector".to_owned()
                    ),
                },
            })),
        },
        Err(e) => Ok(
            MerchantAuthenticationServiceCreateClientAuthenticationTokenResponse {
                session_data: None,
                error: Some(grpc_api_types::payments::ErrorInfo {
                    unified_details: None,
                    connector_details: Some(grpc_api_types::payments::ConnectorErrorDetails {
                        message: Some(e.message.clone()),
                        code: Some(e.code.clone()),
                        reason: e.reason.clone(),
                    }),
                    issuer_details: None,
                }),
                raw_connector_response,
                status_code: e.status_code as u32,
                raw_connector_request,
            },
        ),
    }
}

impl From<NextActionCall> for grpc_api_types::payments::SdkNextAction {
    fn from(value: NextActionCall) -> Self {
        match value {
            NextActionCall::Confirm => Self::Confirm,
            NextActionCall::PostSessionTokens => Self::PostSessionTokens,
        }
    }
}

impl ForeignTryFrom<GpayClientAuthenticationResponse>
    for grpc_api_types::payments::GpayClientAuthenticationResponse
{
    type Error = ConnectorError;

    fn foreign_try_from(
        value: GpayClientAuthenticationResponse,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let gpay_session_token_response = match value {
            GpayClientAuthenticationResponse::GooglePaySession(session) => Self {
                google_pay_session: Some(grpc_api_types::payments::GooglePaySessionResponse {
                    merchant_info: Some(grpc_api_types::payments::GpayMerchantInfo {
                        merchant_id: session.merchant_info.merchant_id,
                        merchant_name: session.merchant_info.merchant_name,
                    }),
                    shipping_address_required: session.shipping_address_required,
                    email_required: session.email_required,
                    shipping_address_parameters: Some(
                        grpc_api_types::payments::GpayShippingAddressParameters {
                            phone_number_required: session
                                .shipping_address_parameters
                                .phone_number_required,
                        },
                    ),
                    allowed_payment_methods: session
                        .allowed_payment_methods
                        .into_iter()
                        .map(grpc_api_types::payments::GpayAllowedPaymentMethods::from)
                        .collect(),
                    transaction_info: Some(grpc_api_types::payments::GpayTransactionInfo {
                        country_code: grpc_api_types::payments::CountryAlpha2::foreign_try_from(
                            session.transaction_info.country_code,
                        )? as i32,
                        currency_code: grpc_api_types::payments::Currency::foreign_try_from(
                            session.transaction_info.currency_code,
                        )? as i32,
                        total_price_status: session.transaction_info.total_price_status,
                        total_price: session.transaction_info.total_price.get_amount_as_i64(),
                    }),
                    delayed_session_token: session.delayed_session_token,
                    connector: session.connector,
                    sdk_next_action: grpc_api_types::payments::SdkNextAction::from(
                        session.sdk_next_action.next_action,
                    )
                    .into(),
                    secrets: session.secrets.map(|s| {
                        grpc_api_types::payments::SecretInfoToInitiateSdk {
                            display: Some(s.display),
                            payment: s.payment,
                        }
                    }),
                }),
            },
        };
        Ok(gpay_session_token_response)
    }
}

impl From<GpayAllowedPaymentMethods> for grpc_api_types::payments::GpayAllowedPaymentMethods {
    fn from(value: GpayAllowedPaymentMethods) -> Self {
        Self {
            payment_method_type: value.payment_method_type,
            parameters: Some(grpc_api_types::payments::GpayAllowedMethodsParameters {
                allowed_auth_methods: value.parameters.allowed_auth_methods,
                allowed_card_networks: value.parameters.allowed_card_networks,
                billing_address: value.parameters.billing_address_required,
                billing_address_parameters: value.parameters.billing_address_parameters.map(|b| {
                    grpc_api_types::payments::GpayBillingAddressParameters {
                        phone_number: b.phone_number_required,
                        format: grpc_api_types::payments::GpayBillingAddressFormat::from(b.format)
                            as i32,
                    }
                }),
                assurance_details: value.parameters.assurance_details_required,
            }),
            tokenization_specification: Some(
                grpc_api_types::payments::GpayTokenizationSpecification {
                    token_specification_type: value
                        .tokenization_specification
                        .token_specification_type,
                    parameters: Some(grpc_api_types::payments::GpayTokenParameters {
                        gateway: value.tokenization_specification.parameters.gateway,
                        gateway_merchant_id: value
                            .tokenization_specification
                            .parameters
                            .gateway_merchant_id,
                        protocol_version: value
                            .tokenization_specification
                            .parameters
                            .protocol_version,
                        public_key: value.tokenization_specification.parameters.public_key,
                    }),
                },
            ),
        }
    }
}

impl From<GpayBillingAddressFormat> for grpc_api_types::payments::GpayBillingAddressFormat {
    fn from(value: GpayBillingAddressFormat) -> Self {
        match value {
            GpayBillingAddressFormat::MIN => Self::BillingAddressFormatMin,
            GpayBillingAddressFormat::FULL => Self::BillingAddressFormatFull,
        }
    }
}

impl ForeignTryFrom<ApplePaySessionResponse> for grpc_api_types::payments::ApplePaySessionResponse {
    type Error = ConnectorError;

    fn foreign_try_from(
        value: ApplePaySessionResponse,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let third_party_sdk = match value {
            ApplePaySessionResponse::ThirdPartySdk(third_party) => {
                grpc_api_types::payments::ThirdPartySdkSessionResponse {
                    secrets: Some(grpc_api_types::payments::SecretInfoToInitiateSdk {
                        display: Some(third_party.secrets.display),
                        payment: third_party.secrets.payment,
                    }),
                }
            }
        };
        Ok(Self {
            third_party_sdk: Some(third_party_sdk),
        })
    }
}

impl ForeignTryFrom<ApplePayPaymentRequest> for grpc_api_types::payments::ApplePayPaymentRequest {
    type Error = ConnectorError;

    fn foreign_try_from(
        value: ApplePayPaymentRequest,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let country_code =
            grpc_api_types::payments::CountryAlpha2::foreign_try_from(value.country_code)?;
        let currency_code =
            grpc_api_types::payments::Currency::foreign_try_from(value.currency_code)?;

        Ok(Self {
            country_code: country_code as i32,
            currency_code: currency_code as i32,
            total: Some(grpc_api_types::payments::AmountInfo {
                label: value.total.label,
                total_type: value.total.total_type,
                amount: value.total.amount.get_amount_as_i64(),
            }),
            merchant_capabilities: value.merchant_capabilities.unwrap_or_default(),
            supported_networks: value.supported_networks.unwrap_or_default(),
            merchant_identifier: value.merchant_identifier,
        })
    }
}

impl ForeignTryFrom<PaypalTransactionInfo> for grpc_api_types::payments::PaypalTransactionInfo {
    type Error = ConnectorError;

    fn foreign_try_from(
        value: PaypalTransactionInfo,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let currency_code =
            grpc_api_types::payments::Currency::foreign_try_from(value.currency_code)?;

        let flow = match value.flow {
            PaypalFlow::Checkout => grpc_api_types::payments::PaypalFlow::Checkout,
        };

        Ok(Self {
            flow: flow as i32,
            currency_code: currency_code as i32,
            total_price: value.total_price.get_amount_as_i64(),
        })
    }
}

impl ForeignTryFrom<ClientAuthenticationTokenData>
    for grpc_api_types::payments::ClientAuthenticationTokenData
{
    type Error = ConnectorError;

    fn foreign_try_from(
        value: ClientAuthenticationTokenData,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let session_token = match value {
            ClientAuthenticationTokenData::GooglePay(gpay_token) => {
                let gpay_response =
                    grpc_api_types::payments::GpayClientAuthenticationResponse::foreign_try_from(
                        *gpay_token,
                    )?;
                grpc_api_types::payments::ClientAuthenticationTokenData {
                    sdk_type: Some(
                        grpc_api_types::payments::client_authentication_token_data::SdkType::GooglePay(
                            gpay_response,
                        ),
                    ),
                }
            }
            ClientAuthenticationTokenData::Paypal(paypal_token) => {
                let paypal_response =
                    grpc_api_types::payments::PaypalClientAuthenticationResponse {
                        connector: paypal_token.connector,
                        session_token: paypal_token.session_token,
                        sdk_next_action: grpc_api_types::payments::SdkNextAction::from(
                            paypal_token.sdk_next_action.next_action,
                        )
                        .into(),
                        client_token: paypal_token.client_token,
                        transaction_info: paypal_token
                            .transaction_info
                            .map(grpc_api_types::payments::PaypalTransactionInfo::foreign_try_from)
                            .transpose()?,
                    };
                grpc_api_types::payments::ClientAuthenticationTokenData {
                    sdk_type: Some(
                        grpc_api_types::payments::client_authentication_token_data::SdkType::Paypal(
                            paypal_response,
                        ),
                    ),
                }
            }
            ClientAuthenticationTokenData::ApplePay(apple_pay_token) => {
                let apple_pay_response =
                    grpc_api_types::payments::ApplepayClientAuthenticationResponse {
                        session_response: apple_pay_token
                            .session_response
                            .map(
                                grpc_api_types::payments::ApplePaySessionResponse::foreign_try_from,
                            )
                            .transpose()?,
                        payment_request_data: apple_pay_token
                            .payment_request_data
                            .map(grpc_api_types::payments::ApplePayPaymentRequest::foreign_try_from)
                            .transpose()?,
                        connector: apple_pay_token.connector,
                        delayed_session_token: apple_pay_token.delayed_session_token,
                        sdk_next_action: grpc_api_types::payments::SdkNextAction::from(
                            apple_pay_token.sdk_next_action.next_action,
                        )
                        .into(),
                        connector_reference_id: apple_pay_token.connector_reference_id,
                        connector_sdk_public_key: apple_pay_token.connector_sdk_public_key,
                        connector_merchant_id: apple_pay_token.connector_merchant_id,
                    };
                grpc_api_types::payments::ClientAuthenticationTokenData {
                    sdk_type: Some(
                        grpc_api_types::payments::client_authentication_token_data::SdkType::ApplePay(
                            apple_pay_response,
                        ),
                    ),
                }
            }
            ClientAuthenticationTokenData::ConnectorSpecific(data) => {
                convert_connector_specific_to_grpc(*data)
            }
        };
        Ok(session_token)
    }
}

impl ForeignTryFrom<grpc_api_types::payments::BankNames> for common_enums::BankNames {
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::payments::BankNames,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        match value {
            grpc_api_types::payments::BankNames::Unspecified => {
                Err(report!(IntegrationError::InvalidDataFormat {
                    field_name: "unknown",
                    context: IntegrationErrorContext {
                        additional_context: Some("Bank name must be specified".to_string()),
                        ..Default::default()
                    }
                }))
            }
            grpc_api_types::payments::BankNames::AmericanExpress => Ok(Self::AmericanExpress),
            grpc_api_types::payments::BankNames::AffinBank => Ok(Self::AffinBank),
            grpc_api_types::payments::BankNames::AgroBank => Ok(Self::AgroBank),
            grpc_api_types::payments::BankNames::AllianceBank => Ok(Self::AllianceBank),
            grpc_api_types::payments::BankNames::AmBank => Ok(Self::AmBank),
            grpc_api_types::payments::BankNames::BankOfAmerica => Ok(Self::BankOfAmerica),
            grpc_api_types::payments::BankNames::BankOfChina => Ok(Self::BankOfChina),
            grpc_api_types::payments::BankNames::BankIslam => Ok(Self::BankIslam),
            grpc_api_types::payments::BankNames::BankMuamalat => Ok(Self::BankMuamalat),
            grpc_api_types::payments::BankNames::BankRakyat => Ok(Self::BankRakyat),
            grpc_api_types::payments::BankNames::BankSimpananNasional => {
                Ok(Self::BankSimpananNasional)
            }
            grpc_api_types::payments::BankNames::Barclays => Ok(Self::Barclays),
            grpc_api_types::payments::BankNames::BlikPsp => Ok(Self::BlikPSP),
            grpc_api_types::payments::BankNames::CapitalOne => Ok(Self::CapitalOne),
            grpc_api_types::payments::BankNames::Chase => Ok(Self::Chase),
            grpc_api_types::payments::BankNames::Citi => Ok(Self::Citi),
            grpc_api_types::payments::BankNames::CimbBank => Ok(Self::CimbBank),
            grpc_api_types::payments::BankNames::Discover => Ok(Self::Discover),
            grpc_api_types::payments::BankNames::NavyFederalCreditUnion => {
                Ok(Self::NavyFederalCreditUnion)
            }
            grpc_api_types::payments::BankNames::PentagonFederalCreditUnion => {
                Ok(Self::PentagonFederalCreditUnion)
            }
            grpc_api_types::payments::BankNames::SynchronyBank => Ok(Self::SynchronyBank),
            grpc_api_types::payments::BankNames::WellsFargo => Ok(Self::WellsFargo),
            grpc_api_types::payments::BankNames::AbnAmro => Ok(Self::AbnAmro),
            grpc_api_types::payments::BankNames::AsnBank => Ok(Self::AsnBank),
            grpc_api_types::payments::BankNames::Bunq => Ok(Self::Bunq),
            grpc_api_types::payments::BankNames::Handelsbanken => Ok(Self::Handelsbanken),
            grpc_api_types::payments::BankNames::HongLeongBank => Ok(Self::HongLeongBank),
            grpc_api_types::payments::BankNames::HsbcBank => Ok(Self::HsbcBank),
            grpc_api_types::payments::BankNames::Ing => Ok(Self::Ing),
            grpc_api_types::payments::BankNames::Knab => Ok(Self::Knab),
            grpc_api_types::payments::BankNames::KuwaitFinanceHouse => Ok(Self::KuwaitFinanceHouse),
            grpc_api_types::payments::BankNames::Moneyou => Ok(Self::Moneyou),
            grpc_api_types::payments::BankNames::Rabobank => Ok(Self::Rabobank),
            grpc_api_types::payments::BankNames::Regiobank => Ok(Self::Regiobank),
            grpc_api_types::payments::BankNames::Revolut => Ok(Self::Revolut),
            grpc_api_types::payments::BankNames::SnsBank => Ok(Self::SnsBank),
            grpc_api_types::payments::BankNames::TriodosBank => Ok(Self::TriodosBank),
            grpc_api_types::payments::BankNames::VanLanschot => Ok(Self::VanLanschot),
            grpc_api_types::payments::BankNames::ArzteUndApothekerBank => {
                Ok(Self::ArzteUndApothekerBank)
            }
            grpc_api_types::payments::BankNames::AustrianAnadiBankAg => {
                Ok(Self::AustrianAnadiBankAg)
            }
            grpc_api_types::payments::BankNames::BankAustria => Ok(Self::BankAustria),
            grpc_api_types::payments::BankNames::Bank99Ag => Ok(Self::Bank99Ag),
            grpc_api_types::payments::BankNames::BankhausCarlSpangler => {
                Ok(Self::BankhausCarlSpangler)
            }
            grpc_api_types::payments::BankNames::BankhausSchelhammerUndSchatteraAg => {
                Ok(Self::BankhausSchelhammerUndSchatteraAg)
            }
            grpc_api_types::payments::BankNames::BankMillennium => Ok(Self::BankMillennium),
            grpc_api_types::payments::BankNames::BawagPskAg => Ok(Self::BawagPskAg),
            grpc_api_types::payments::BankNames::BksBankAg => Ok(Self::BksBankAg),
            grpc_api_types::payments::BankNames::BrullKallmusBankAg => Ok(Self::BrullKallmusBankAg),
            grpc_api_types::payments::BankNames::BtvVierLanderBank => Ok(Self::BtvVierLanderBank),
            grpc_api_types::payments::BankNames::CapitalBankGraweGruppeAg => {
                Ok(Self::CapitalBankGraweGruppeAg)
            }
            grpc_api_types::payments::BankNames::CeskaSporitelna => Ok(Self::CeskaSporitelna),
            grpc_api_types::payments::BankNames::Dolomitenbank => Ok(Self::Dolomitenbank),
            grpc_api_types::payments::BankNames::EasybankAg => Ok(Self::EasybankAg),
            grpc_api_types::payments::BankNames::EPlatbyVub => Ok(Self::EPlatbyVUB),
            grpc_api_types::payments::BankNames::ErsteBankUndSparkassen => {
                Ok(Self::ErsteBankUndSparkassen)
            }
            grpc_api_types::payments::BankNames::FrieslandBank => Ok(Self::FrieslandBank),
            grpc_api_types::payments::BankNames::HypoAlpeadriabankInternationalAg => {
                Ok(Self::HypoAlpeadriabankInternationalAg)
            }
            grpc_api_types::payments::BankNames::HypoNoeLbFurNiederosterreichUWien => {
                Ok(Self::HypoNoeLbFurNiederosterreichUWien)
            }
            grpc_api_types::payments::BankNames::HypoOberosterreichSalzburgSteiermark => {
                Ok(Self::HypoOberosterreichSalzburgSteiermark)
            }
            grpc_api_types::payments::BankNames::HypoTirolBankAg => Ok(Self::HypoTirolBankAg),
            grpc_api_types::payments::BankNames::HypoVorarlbergBankAg => {
                Ok(Self::HypoVorarlbergBankAg)
            }
            grpc_api_types::payments::BankNames::HypoBankBurgenlandAktiengesellschaft => {
                Ok(Self::HypoBankBurgenlandAktiengesellschaft)
            }
            grpc_api_types::payments::BankNames::KomercniBanka => Ok(Self::KomercniBanka),
            grpc_api_types::payments::BankNames::MBank => Ok(Self::MBank),
            grpc_api_types::payments::BankNames::MarchfelderBank => Ok(Self::MarchfelderBank),
            grpc_api_types::payments::BankNames::Maybank => Ok(Self::Maybank),
            grpc_api_types::payments::BankNames::OberbankAg => Ok(Self::OberbankAg),
            grpc_api_types::payments::BankNames::OsterreichischeArzteUndApothekerbank => {
                Ok(Self::OsterreichischeArzteUndApothekerbank)
            }
            grpc_api_types::payments::BankNames::OcbcBank => Ok(Self::OcbcBank),
            grpc_api_types::payments::BankNames::PayWithIng => Ok(Self::PayWithING),
            grpc_api_types::payments::BankNames::PlaceZipko => Ok(Self::PlaceZIPKO),
            grpc_api_types::payments::BankNames::PlatnoscOnlineKartaPlatnicza => {
                Ok(Self::PlatnoscOnlineKartaPlatnicza)
            }
            grpc_api_types::payments::BankNames::PosojilnicaBankEGen => {
                Ok(Self::PosojilnicaBankEGen)
            }
            grpc_api_types::payments::BankNames::PostovaBanka => Ok(Self::PostovaBanka),
            grpc_api_types::payments::BankNames::PublicBank => Ok(Self::PublicBank),
            grpc_api_types::payments::BankNames::RaiffeisenBankengruppeOsterreich => {
                Ok(Self::RaiffeisenBankengruppeOsterreich)
            }
            grpc_api_types::payments::BankNames::RhbBank => Ok(Self::RhbBank),
            grpc_api_types::payments::BankNames::SchelhammerCapitalBankAg => {
                Ok(Self::SchelhammerCapitalBankAg)
            }
            grpc_api_types::payments::BankNames::StandardCharteredBank => {
                Ok(Self::StandardCharteredBank)
            }
            grpc_api_types::payments::BankNames::SchoellerbankAg => Ok(Self::SchoellerbankAg),
            grpc_api_types::payments::BankNames::SpardaBankWien => Ok(Self::SpardaBankWien),
            grpc_api_types::payments::BankNames::SporoPay => Ok(Self::SporoPay),
            grpc_api_types::payments::BankNames::SantanderPrzelew24 => Ok(Self::SantanderPrzelew24),
            grpc_api_types::payments::BankNames::TatraPay => Ok(Self::TatraPay),
            grpc_api_types::payments::BankNames::Viamo => Ok(Self::Viamo),
            grpc_api_types::payments::BankNames::VolksbankGruppe => Ok(Self::VolksbankGruppe),
            grpc_api_types::payments::BankNames::VolkskreditbankAg => Ok(Self::VolkskreditbankAg),
            grpc_api_types::payments::BankNames::VrBankBraunau => Ok(Self::VrBankBraunau),
            grpc_api_types::payments::BankNames::UobBank => Ok(Self::UobBank),
            grpc_api_types::payments::BankNames::PayWithAliorBank => Ok(Self::PayWithAliorBank),
            grpc_api_types::payments::BankNames::BankiSpoldzielcze => Ok(Self::BankiSpoldzielcze),
            grpc_api_types::payments::BankNames::PayWithInteligo => Ok(Self::PayWithInteligo),
            grpc_api_types::payments::BankNames::BnpParibasPoland => Ok(Self::BNPParibasPoland),
            grpc_api_types::payments::BankNames::BankNowySa => Ok(Self::BankNowySA),
            grpc_api_types::payments::BankNames::CreditAgricole => Ok(Self::CreditAgricole),
            grpc_api_types::payments::BankNames::PayWithBos => Ok(Self::PayWithBOS),
            grpc_api_types::payments::BankNames::PayWithCitiHandlowy => {
                Ok(Self::PayWithCitiHandlowy)
            }
            grpc_api_types::payments::BankNames::PayWithPlusBank => Ok(Self::PayWithPlusBank),
            grpc_api_types::payments::BankNames::ToyotaBank => Ok(Self::ToyotaBank),
            grpc_api_types::payments::BankNames::VeloBank => Ok(Self::VeloBank),
            grpc_api_types::payments::BankNames::ETransferPocztowy24 => {
                Ok(Self::ETransferPocztowy24)
            }
            grpc_api_types::payments::BankNames::PlusBank => Ok(Self::PlusBank),
            grpc_api_types::payments::BankNames::BankiSpbdzielcze => Ok(Self::BankiSpbdzielcze),
            grpc_api_types::payments::BankNames::BankNowyBfgSa => Ok(Self::BankNowyBfgSa),
            grpc_api_types::payments::BankNames::GetinBank => Ok(Self::GetinBank),
            grpc_api_types::payments::BankNames::BlikPoland => Ok(Self::Blik),
            grpc_api_types::payments::BankNames::NoblePay => Ok(Self::NoblePay),
            grpc_api_types::payments::BankNames::IdeaBank => Ok(Self::IdeaBank),
            grpc_api_types::payments::BankNames::EnveloBank => Ok(Self::EnveloBank),
            grpc_api_types::payments::BankNames::NestPrzelew => Ok(Self::NestPrzelew),
            grpc_api_types::payments::BankNames::MbankMtransfer => Ok(Self::MbankMtransfer),
            grpc_api_types::payments::BankNames::Inteligo => Ok(Self::Inteligo),
            grpc_api_types::payments::BankNames::PbacZIpko => Ok(Self::PbacZIpko),
            grpc_api_types::payments::BankNames::BnpParibas => Ok(Self::BnpParibas),
            grpc_api_types::payments::BankNames::BankPekaoSa => Ok(Self::BankPekaoSa),
            grpc_api_types::payments::BankNames::VolkswagenBank => Ok(Self::VolkswagenBank),
            grpc_api_types::payments::BankNames::AliorBank => Ok(Self::AliorBank),
            grpc_api_types::payments::BankNames::Boz => Ok(Self::Boz),
            grpc_api_types::payments::BankNames::BangkokBank => Ok(Self::BangkokBank),
            grpc_api_types::payments::BankNames::KrungsriBank => Ok(Self::KrungsriBank),
            grpc_api_types::payments::BankNames::KrungThaiBank => Ok(Self::KrungThaiBank),
            grpc_api_types::payments::BankNames::TheSiamCommercialBank => {
                Ok(Self::TheSiamCommercialBank)
            }
            grpc_api_types::payments::BankNames::KasikornBank => Ok(Self::KasikornBank),
            grpc_api_types::payments::BankNames::OpenBankSuccess => Ok(Self::OpenBankSuccess),
            grpc_api_types::payments::BankNames::OpenBankFailure => Ok(Self::OpenBankFailure),
            grpc_api_types::payments::BankNames::OpenBankCancelled => Ok(Self::OpenBankCancelled),
            grpc_api_types::payments::BankNames::Aib => Ok(Self::Aib),
            grpc_api_types::payments::BankNames::BankOfScotland => Ok(Self::BankOfScotland),
            grpc_api_types::payments::BankNames::DanskeBank => Ok(Self::DanskeBank),
            grpc_api_types::payments::BankNames::FirstDirect => Ok(Self::FirstDirect),
            grpc_api_types::payments::BankNames::FirstTrust => Ok(Self::FirstTrust),
            grpc_api_types::payments::BankNames::Halifax => Ok(Self::Halifax),
            grpc_api_types::payments::BankNames::Lloyds => Ok(Self::Lloyds),
            grpc_api_types::payments::BankNames::Monzo => Ok(Self::Monzo),
            grpc_api_types::payments::BankNames::NatWest => Ok(Self::NatWest),
            grpc_api_types::payments::BankNames::NationwideBank => Ok(Self::NationwideBank),
            grpc_api_types::payments::BankNames::RoyalBankOfScotland => {
                Ok(Self::RoyalBankOfScotland)
            }
            grpc_api_types::payments::BankNames::Starling => Ok(Self::Starling),
            grpc_api_types::payments::BankNames::TsbBank => Ok(Self::TsbBank),
            grpc_api_types::payments::BankNames::TescoBank => Ok(Self::TescoBank),
            grpc_api_types::payments::BankNames::UlsterBank => Ok(Self::UlsterBank),
            grpc_api_types::payments::BankNames::Yoursafe => Ok(Self::Yoursafe),
            grpc_api_types::payments::BankNames::N26 => Ok(Self::N26),
            grpc_api_types::payments::BankNames::NationaleNederlanden => {
                Ok(Self::NationaleNederlanden)
            }
            // Indian banks
            grpc_api_types::payments::BankNames::StateBank => Ok(Self::StateBank),
            grpc_api_types::payments::BankNames::HdfcBank => Ok(Self::HdfcBank),
            grpc_api_types::payments::BankNames::IciciBank => Ok(Self::IciciBank),
            grpc_api_types::payments::BankNames::AxisBank => Ok(Self::AxisBank),
            grpc_api_types::payments::BankNames::KotakMahindraBank => Ok(Self::KotakMahindraBank),
            grpc_api_types::payments::BankNames::PunjabNationalBank => Ok(Self::PunjabNationalBank),
            grpc_api_types::payments::BankNames::BankOfBaroda => Ok(Self::BankOfBaroda),
            grpc_api_types::payments::BankNames::UnionBankOfIndia => Ok(Self::UnionBankOfIndia),
            grpc_api_types::payments::BankNames::CanaraBank => Ok(Self::CanaraBank),
            grpc_api_types::payments::BankNames::IndusIndBank => Ok(Self::IndusIndBank),
            grpc_api_types::payments::BankNames::YesBank => Ok(Self::YesBank),
            grpc_api_types::payments::BankNames::IdbiBank => Ok(Self::IdbiBank),
            grpc_api_types::payments::BankNames::FederalBank => Ok(Self::FederalBank),
            grpc_api_types::payments::BankNames::IndianOverseasBank => Ok(Self::IndianOverseasBank),
            grpc_api_types::payments::BankNames::CentralBankOfIndia => Ok(Self::CentralBankOfIndia),
        }
    }
}

// New ForeignTryFrom implementations for individual 3DS authentication flow proto definitions

impl<
        T: PaymentMethodDataTypes
            + Default
            + Debug
            + Send
            + Eq
            + PartialEq
            + Serialize
            + serde::de::DeserializeOwned
            + Clone
            + CardConversionHelper<T>,
    >
    ForeignTryFrom<
        grpc_api_types::payments::PaymentMethodAuthenticationServicePreAuthenticateRequest,
    > for PaymentsPreAuthenticateData<T>
{
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::payments::PaymentMethodAuthenticationServicePreAuthenticateRequest,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let email: Option<Email> = match value.customer.and_then(|c| c.email) {
            Some(ref email_str) => {
                Some(Email::try_from(email_str.clone().expose()).map_err(|_| {
                    error_stack::Report::new(IntegrationError::InvalidDataFormat {
                        field_name: "unknown",
                        context: IntegrationErrorContext {
                            additional_context: Some("Invalid email".to_string()),
                            ..Default::default()
                        },
                    })
                })?)
            }
            None => None,
        };

        let amount = match value.amount {
            Some(amount) => Ok(common_utils::types::Money {
                amount: common_utils::types::MinorUnit::new(amount.minor_amount),
                currency: common_enums::Currency::foreign_try_from(amount.currency())?,
            }),
            None => Err(report!(IntegrationError::MissingRequiredField {
                field_name: "amount",
                context: IntegrationErrorContext::default(),
            })),
        }?;
        let return_url = value.return_url;
        let enrolled_for_3ds = value.enrolled_for_3ds;

        // Clone payment_method to avoid ownership issues
        let payment_method_clone = value.payment_method.clone();

        Ok(Self {
            payment_method_data: value
                .payment_method
                .map(PaymentMethodData::<T>::foreign_try_from)
                .transpose()?,
            amount: amount.amount,
            currency: Some(amount.currency),
            email,
            payment_method_type: <Option<PaymentMethodType>>::foreign_try_from(
                payment_method_clone.unwrap_or_default(),
            )?,
            continue_redirection_url: value
                .continue_redirection_url
                .map(|url_str| {
                    url::Url::parse(&url_str).change_context(IntegrationError::InvalidDataFormat {
                        field_name: "continue_redirection_url",
                        context: IntegrationErrorContext::default(),
                    })
                })
                .transpose()?,
            router_return_url: return_url
                .map(|url_str| {
                    url::Url::parse(&url_str).change_context(IntegrationError::InvalidDataFormat {
                        field_name: "router_return_url",
                        context: IntegrationErrorContext::default(),
                    })
                })
                .transpose()?,
            browser_info: value
                .browser_info
                .map(BrowserInformation::foreign_try_from)
                .transpose()?,
            enrolled_for_3ds,
            redirect_response: None,
            capture_method: value
                .capture_method
                .map(|cm| {
                    CaptureMethod::foreign_try_from(
                        grpc_api_types::payments::CaptureMethod::try_from(cm).unwrap_or_default(),
                    )
                })
                .transpose()?,
            mandate_reference: None,
        })
    }
}

impl<
        T: PaymentMethodDataTypes
            + Default
            + Debug
            + Send
            + Eq
            + PartialEq
            + Serialize
            + serde::de::DeserializeOwned
            + Clone
            + CardConversionHelper<T>,
    >
    ForeignTryFrom<grpc_api_types::payments::PaymentMethodAuthenticationServiceAuthenticateRequest>
    for PaymentsAuthenticateData<T>
{
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::payments::PaymentMethodAuthenticationServiceAuthenticateRequest,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let email: Option<Email> = match value.customer.and_then(|c| c.email) {
            Some(ref email_str) => {
                Some(Email::try_from(email_str.clone().expose()).map_err(|_| {
                    error_stack::Report::new(IntegrationError::InvalidDataFormat {
                        field_name: "unknown",
                        context: IntegrationErrorContext {
                            additional_context: Some("Invalid email".to_string()),
                            ..Default::default()
                        },
                    })
                })?)
            }
            None => None,
        };

        let amount = match value.amount {
            Some(amount) => Ok(common_utils::types::Money {
                amount: common_utils::types::MinorUnit::new(amount.minor_amount),
                currency: common_enums::Currency::foreign_try_from(amount.currency())?,
            }),
            None => Err(report!(IntegrationError::MissingRequiredField {
                field_name: "amount",
                context: IntegrationErrorContext::default(),
            })),
        }?;
        let return_url = value.return_url;

        // Clone payment_method to avoid ownership issues
        let payment_method_clone = value.payment_method.clone();

        let redirect_response =
            value
                .redirection_response
                .map(|redirection_response| ContinueRedirectionResponse {
                    params: redirection_response.params.map(Secret::new),
                    payload: Some(Secret::new(serde_json::Value::Object(
                        redirection_response
                            .payload
                            .into_iter()
                            .map(|(k, v)| (k, serde_json::Value::String(v)))
                            .collect(),
                    ))),
                });

        Ok(Self {
            payment_method_data: value
                .payment_method
                .map(PaymentMethodData::<T>::foreign_try_from)
                .transpose()?,
            amount: amount.amount,
            email,
            currency: Some(amount.currency),
            payment_method_type: <Option<PaymentMethodType>>::foreign_try_from(
                payment_method_clone.unwrap_or_default(),
            )?,
            router_return_url: return_url
                .map(|url_str| {
                    url::Url::parse(&url_str).change_context(IntegrationError::InvalidDataFormat {
                        field_name: "router_return_url",
                        context: IntegrationErrorContext::default(),
                    })
                })
                .transpose()?,
            continue_redirection_url: value
                .continue_redirection_url
                .map(|url_str| {
                    url::Url::parse(&url_str).change_context(IntegrationError::InvalidDataFormat {
                        field_name: "continue_redirection_url",
                        context: IntegrationErrorContext::default(),
                    })
                })
                .transpose()?,
            browser_info: value
                .browser_info
                .map(BrowserInformation::foreign_try_from)
                .transpose()?,
            enrolled_for_3ds: false,
            redirect_response,
            capture_method: value
                .capture_method
                .map(|cm| {
                    CaptureMethod::foreign_try_from(
                        grpc_api_types::payments::CaptureMethod::try_from(cm).unwrap_or_default(),
                    )
                })
                .transpose()?,
            authentication_data: value
                .authentication_data
                .map(router_request_types::AuthenticationData::try_from)
                .transpose()?,
        })
    }
}

impl<
        T: PaymentMethodDataTypes
            + Default
            + Debug
            + Send
            + Eq
            + PartialEq
            + Serialize
            + serde::de::DeserializeOwned
            + Clone
            + CardConversionHelper<T>,
    >
    ForeignTryFrom<
        grpc_api_types::payments::PaymentMethodAuthenticationServicePostAuthenticateRequest,
    > for PaymentsPostAuthenticateData<T>
{
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::payments::PaymentMethodAuthenticationServicePostAuthenticateRequest,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let email: Option<Email> = match value.customer.and_then(|c| c.email) {
            Some(ref email_str) => {
                Some(Email::try_from(email_str.clone().expose()).map_err(|_| {
                    error_stack::Report::new(IntegrationError::InvalidDataFormat {
                        field_name: "unknown",
                        context: IntegrationErrorContext {
                            additional_context: Some("Invalid email".to_string()),
                            ..Default::default()
                        },
                    })
                })?)
            }
            None => None,
        };

        let amount = match value.amount {
            Some(amount) => Ok(common_utils::types::Money {
                amount: common_utils::types::MinorUnit::new(amount.minor_amount),
                currency: common_enums::Currency::foreign_try_from(amount.currency())?,
            }),
            None => Err(report!(IntegrationError::MissingRequiredField {
                field_name: "amount",
                context: IntegrationErrorContext::default(),
            })),
        }?;
        let return_url = value.return_url;

        // Clone payment_method to avoid ownership issues
        let payment_method_clone = value.payment_method.clone();

        let redirect_response =
            value
                .redirection_response
                .map(|redirection_response| ContinueRedirectionResponse {
                    params: redirection_response.params.map(Secret::new),
                    payload: Some(Secret::new(serde_json::Value::Object(
                        redirection_response
                            .payload
                            .into_iter()
                            .map(|(k, v)| (k, serde_json::Value::String(v)))
                            .collect(),
                    ))),
                });
        Ok(Self {
            payment_method_data: value
                .payment_method
                .map(PaymentMethodData::<T>::foreign_try_from)
                .transpose()?,
            amount: amount.amount,
            currency: Some(amount.currency),
            email,
            payment_method_type: <Option<PaymentMethodType>>::foreign_try_from(
                payment_method_clone.unwrap_or_default(),
            )?,
            router_return_url: return_url
                .map(|url_str| {
                    url::Url::parse(&url_str).change_context(IntegrationError::InvalidDataFormat {
                        field_name: "router_return_url",
                        context: IntegrationErrorContext::default(),
                    })
                })
                .transpose()?,
            continue_redirection_url: value
                .continue_redirection_url
                .map(|url_str| {
                    url::Url::parse(&url_str).change_context(IntegrationError::InvalidDataFormat {
                        field_name: "continue_redirection_url",
                        context: IntegrationErrorContext::default(),
                    })
                })
                .transpose()?,
            browser_info: value
                .browser_info
                .map(BrowserInformation::foreign_try_from)
                .transpose()?,
            enrolled_for_3ds: false,
            redirect_response,
            capture_method: value
                .capture_method
                .map(|cm| {
                    CaptureMethod::foreign_try_from(
                        grpc_api_types::payments::CaptureMethod::try_from(cm).unwrap_or_default(),
                    )
                })
                .transpose()?,
        })
    }
}

// PaymentFlowData implementations for new proto definitions

impl
    ForeignTryFrom<(
        grpc_api_types::payments::PaymentMethodAuthenticationServicePreAuthenticateRequest,
        Connectors,
        &MaskedMetadata,
    )> for PaymentFlowData
{
    type Error = IntegrationError;

    fn foreign_try_from(
        (value, connectors, metadata): (
            grpc_api_types::payments::PaymentMethodAuthenticationServicePreAuthenticateRequest,
            Connectors,
            &MaskedMetadata,
        ),
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let address = match value.address {
            Some(address) => PaymentAddress::foreign_try_from(address)?,
            None => {
                return Err(IntegrationError::InvalidDataFormat {
                    field_name: "unknown",
                    context: IntegrationErrorContext {
                        additional_context: Some("Address is required".to_string()),
                        ..Default::default()
                    },
                })?
            }
        };

        let merchant_id_from_header = extract_merchant_id_from_metadata(metadata)?;
        let vault_headers = extract_headers_from_metadata(metadata);

        Ok(Self {
            merchant_id: merchant_id_from_header,
            payment_id: "IRRELEVANT_PAYMENT_ID".to_string(),
            attempt_id: "IRRELEVANT_ATTEMPT_ID".to_string(),
            status: common_enums::AttemptStatus::Pending,
            payment_method: PaymentMethod::foreign_try_from(
                value.payment_method.unwrap_or_default(),
            )?,
            address,
            auth_type: common_enums::AuthenticationType::ThreeDs, // Pre-auth typically uses 3DS
            connector_request_reference_id: extract_connector_request_reference_id(
                &value.merchant_order_id.clone(),
            ),
            customer_id: None,
            connector_customer: None,
            description: value.description,
            return_url: value.return_url.clone(),
            connector_feature_data: value
                .connector_feature_data
                .map(|m| ForeignTryFrom::foreign_try_from((m, "feature data")))
                .transpose()?,
            amount_captured: None,
            minor_amount_captured: None,
            minor_amount_capturable: None,
            amount: None,
            access_token: None,
            session_token: None,
            reference_id: None,
            connector_order_id: None,
            preprocessing_id: None,
            connector_api_version: None,
            test_mode: None,
            connector_http_status_code: None,
            external_latency: None,
            connectors,
            raw_connector_response: None,
            connector_response_headers: None,
            vault_headers,
            raw_connector_request: None,
            connector_response: None,
            recurring_mandate_payment_data: None,
            order_details: None,
            minor_amount_authorized: None,
            l2_l3_data: None,
        })
    }
}

impl
    ForeignTryFrom<(
        grpc_api_types::payments::PaymentMethodAuthenticationServiceAuthenticateRequest,
        Connectors,
        &MaskedMetadata,
    )> for PaymentFlowData
{
    type Error = IntegrationError;

    fn foreign_try_from(
        (value, connectors, metadata): (
            grpc_api_types::payments::PaymentMethodAuthenticationServiceAuthenticateRequest,
            Connectors,
            &MaskedMetadata,
        ),
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let address = match &value.address {
            Some(address_value) => PaymentAddress::foreign_try_from((*address_value).clone())?,
            None => {
                return Err(IntegrationError::InvalidDataFormat {
                    field_name: "unknown",
                    context: IntegrationErrorContext {
                        additional_context: Some("Address is required".to_string()),
                        ..Default::default()
                    },
                })?
            }
        };

        let merchant_id_from_header = extract_merchant_id_from_metadata(metadata)?;
        let vault_headers = extract_headers_from_metadata(metadata);

        let metadata = value
            .metadata
            .clone()
            .map(|m| SecretSerdeValue::foreign_try_from((m, "metadata")))
            .transpose()?;
        let description = metadata
            .as_ref()
            .and_then(|m| m.peek().get("description"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        Ok(Self {
            merchant_id: merchant_id_from_header,
            payment_id: "IRRELEVANT_PAYMENT_ID".to_string(),
            attempt_id: "IRRELEVANT_ATTEMPT_ID".to_string(),
            status: common_enums::AttemptStatus::Pending,
            payment_method: PaymentMethod::foreign_try_from(
                value.payment_method.unwrap_or_default(),
            )?,
            address,
            auth_type: common_enums::AuthenticationType::ThreeDs, // Auth step uses 3DS
            connector_request_reference_id: extract_connector_request_reference_id(
                &value.merchant_order_id.clone(),
            ),
            customer_id: None,
            connector_customer: None,
            description,
            return_url: value.return_url.clone(),
            connector_feature_data: value
                .connector_feature_data
                .map(|m| ForeignTryFrom::foreign_try_from((m, "feature data")))
                .transpose()?,
            amount_captured: None,
            minor_amount_captured: None,
            access_token: None,
            session_token: None,
            reference_id: None,
            connector_order_id: None,
            preprocessing_id: None,
            connector_api_version: None,
            test_mode: None,
            connector_http_status_code: None,
            external_latency: None,
            connectors,
            raw_connector_response: None,
            connector_response_headers: None,
            vault_headers,
            raw_connector_request: None,
            minor_amount_capturable: None,
            amount: None,
            connector_response: None,
            recurring_mandate_payment_data: None,
            order_details: None,
            minor_amount_authorized: None,
            l2_l3_data: None,
        })
    }
}

impl
    ForeignTryFrom<(
        grpc_api_types::payments::PaymentMethodAuthenticationServicePostAuthenticateRequest,
        Connectors,
        &MaskedMetadata,
    )> for PaymentFlowData
{
    type Error = IntegrationError;

    fn foreign_try_from(
        (value, connectors, metadata): (
            grpc_api_types::payments::PaymentMethodAuthenticationServicePostAuthenticateRequest,
            Connectors,
            &MaskedMetadata,
        ),
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let address = match &value.address {
            Some(address_value) => PaymentAddress::foreign_try_from((*address_value).clone())?,
            None => {
                return Err(IntegrationError::InvalidDataFormat {
                    field_name: "unknown",
                    context: IntegrationErrorContext {
                        additional_context: Some("Address is required".to_string()),
                        ..Default::default()
                    },
                })?
            }
        };

        let merchant_id_from_header = extract_merchant_id_from_metadata(metadata)?;
        let vault_headers = extract_headers_from_metadata(metadata);

        let access_token = value
            .state
            .as_ref()
            .and_then(|state| state.access_token.as_ref())
            .map(ServerAuthenticationTokenResponseData::foreign_try_from)
            .transpose()?;

        let metadata = value
            .metadata
            .clone()
            .map(|m| SecretSerdeValue::foreign_try_from((m, "metadata")))
            .transpose()?;
        let description = metadata
            .as_ref()
            .and_then(|m| m.peek().get("description"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        Ok(Self {
            merchant_id: merchant_id_from_header,
            payment_id: "IRRELEVANT_PAYMENT_ID".to_string(),
            attempt_id: "IRRELEVANT_ATTEMPT_ID".to_string(),
            status: common_enums::AttemptStatus::Pending,
            payment_method: PaymentMethod::foreign_try_from(
                value.payment_method.unwrap_or_default(),
            )?,
            address,
            auth_type: common_enums::AuthenticationType::ThreeDs, // Post-auth uses 3DS
            connector_request_reference_id: extract_connector_request_reference_id(
                &value.merchant_order_id.clone(),
            ),
            customer_id: None,
            connector_customer: None,
            description,
            return_url: value.return_url.clone(),
            connector_feature_data: value
                .connector_feature_data
                .map(|m| ForeignTryFrom::foreign_try_from((m, "feature data")))
                .transpose()?,
            amount_captured: None,
            minor_amount_captured: None,
            access_token,
            session_token: None,
            reference_id: value.connector_order_reference_id.clone(),
            connector_order_id: None,
            preprocessing_id: None,
            connector_api_version: None,
            test_mode: None,
            connector_http_status_code: None,
            external_latency: None,
            connectors,
            raw_connector_response: None,
            connector_response_headers: None,
            vault_headers,
            raw_connector_request: None,
            minor_amount_capturable: None,
            amount: None,
            connector_response: None,
            recurring_mandate_payment_data: None,
            order_details: None,
            minor_amount_authorized: None,
            l2_l3_data: None,
        })
    }
}

// Conversion implementations for MandateRevoke flow
impl ForeignTryFrom<RecurringPaymentServiceRevokeRequest> for MandateRevokeRequestData {
    type Error = IntegrationError;

    fn foreign_try_from(
        value: RecurringPaymentServiceRevokeRequest,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        Ok(Self {
            mandate_id: Secret::new(value.mandate_id),
            connector_mandate_id: value.connector_mandate_id.map(Secret::new),
            payment_method_type: None,
        })
    }
}

impl
    ForeignTryFrom<(
        RecurringPaymentServiceRevokeRequest,
        Connectors,
        &MaskedMetadata,
    )> for PaymentFlowData
{
    type Error = IntegrationError;

    fn foreign_try_from(
        (value, connectors, metadata): (
            RecurringPaymentServiceRevokeRequest,
            Connectors,
            &MaskedMetadata,
        ),
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let merchant_id_from_header = extract_merchant_id_from_metadata(metadata)?;

        Ok(Self {
            merchant_id: merchant_id_from_header,
            payment_id: "MANDATE_REVOKE_ID".to_string(),
            attempt_id: "MANDATE_REVOKE_ATTEMPT_ID".to_string(),
            status: common_enums::AttemptStatus::Pending,
            payment_method: common_enums::PaymentMethod::Card, // Default for mandate operations
            address: PaymentAddress::default(),
            auth_type: common_enums::AuthenticationType::default(),
            connector_request_reference_id: extract_connector_request_reference_id(
                &value.merchant_revoke_id.clone(),
            ),
            customer_id: None,
            connector_customer: None,
            description: Some("Mandate revoke operation".to_string()),
            return_url: None,
            connector_feature_data: None,
            amount_captured: None,
            minor_amount_captured: None,
            access_token: None,
            session_token: None,
            reference_id: None,
            connector_order_id: None,
            preprocessing_id: None,
            connector_api_version: None,
            test_mode: None,
            connector_http_status_code: None,
            external_latency: None,
            connectors,
            raw_connector_response: None,
            raw_connector_request: None,
            connector_response_headers: None,
            vault_headers: None,
            minor_amount_capturable: None,
            amount: None,
            connector_response: None,
            recurring_mandate_payment_data: None,
            order_details: None,
            minor_amount_authorized: None,
            l2_l3_data: None,
        })
    }
}

impl ForeignTryFrom<(bool, RedirectDetailsResponse)>
    for grpc_api_types::payments::PaymentServiceVerifyRedirectResponseResponse
{
    type Error = ConnectorError;

    fn foreign_try_from(
        (source_verified, redirect_details_response): (bool, RedirectDetailsResponse),
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        Ok(Self {
            source_verified,
            connector_transaction_id: redirect_details_response
                .resource_id
                .map(Option::foreign_try_from)
                .transpose()?
                .unwrap_or_default(),
            merchant_order_id: redirect_details_response.connector_response_reference_id,
            response_amount: match redirect_details_response.response_amount {
                Some(money) => Some(grpc_api_types::payments::Money {
                    minor_amount: money.amount.get_amount_as_i64(),
                    currency: grpc_api_types::payments::Currency::foreign_try_from(money.currency)?
                        .into(),
                }),
                None => None,
            },
            status: redirect_details_response
                .status
                .map(grpc_api_types::payments::PaymentStatus::foreign_from)
                .map(|status| status as i32),
            error: Some(grpc_api_types::payments::ErrorInfo {
                connector_details: Some(grpc_api_types::payments::ConnectorErrorDetails {
                    code: redirect_details_response.error_code.clone(),
                    reason: redirect_details_response.error_reason.clone(),
                    message: redirect_details_response.error_message.clone(),
                }),
                unified_details: None,
                issuer_details: None,
            }),
            raw_connector_response: redirect_details_response
                .raw_connector_response
                .map(|response| response.into()),
        })
    }
}

pub fn generate_payment_pre_authenticate_response<T: PaymentMethodDataTypes>(
    router_data_v2: RouterDataV2<
        PreAuthenticate,
        PaymentFlowData,
        PaymentsPreAuthenticateData<T>,
        PaymentsResponseData,
    >,
) -> Result<
    PaymentMethodAuthenticationServicePreAuthenticateResponse,
    error_stack::Report<ConnectorError>,
> {
    let transaction_response = router_data_v2.response;
    let status = router_data_v2.resource_common_data.status;
    let grpc_status = grpc_api_types::payments::PaymentStatus::foreign_from(status);
    let raw_connector_response = router_data_v2
        .resource_common_data
        .get_raw_connector_response();
    let response_headers = router_data_v2
        .resource_common_data
        .get_connector_response_headers_as_map();

    let response = match transaction_response {
        Ok(response) => match response {
            PaymentsResponseData::PreAuthenticateResponse {
                redirection_data,
                connector_response_reference_id,
                status_code,
                authentication_data,
            } => PaymentMethodAuthenticationServicePreAuthenticateResponse {
                connector_transaction_id: None,
                redirection_data: redirection_data
                    .map(|form| match *form {
                        router_response_types::RedirectForm::Form {
                            endpoint,
                            method,
                            form_fields,
                        } => Ok::<
                            grpc_api_types::payments::RedirectForm,
                            error_stack::Report<ConnectorError>,
                        >(grpc_api_types::payments::RedirectForm {
                            form_type: Some(
                                grpc_api_types::payments::redirect_form::FormType::Form(
                                    grpc_api_types::payments::FormData {
                                        endpoint,
                                        method: grpc_api_types::payments::HttpMethod::foreign_from(
                                            method,
                                        )
                                        .into(),
                                        form_fields,
                                    },
                                ),
                            ),
                        }),
                        router_response_types::RedirectForm::Html { html_data } => {
                            Ok(grpc_api_types::payments::RedirectForm {
                                form_type: Some(
                                    grpc_api_types::payments::redirect_form::FormType::Html(
                                        grpc_api_types::payments::HtmlData { html_data },
                                    ),
                                ),
                            })
                        }
                        router_response_types::RedirectForm::Uri { uri } => {
                            Ok(grpc_api_types::payments::RedirectForm {
                                form_type: Some(
                                    grpc_api_types::payments::redirect_form::FormType::Uri(
                                        grpc_api_types::payments::UriData { uri },
                                    ),
                                ),
                            })
                        }
                        router_response_types::RedirectForm::Mifinity {
                            initialization_token,
                        } => Ok(grpc_api_types::payments::RedirectForm {
                            form_type: Some(
                                grpc_api_types::payments::redirect_form::FormType::Uri(
                                    grpc_api_types::payments::UriData {
                                        uri: initialization_token,
                                    },
                                ),
                            ),
                        }),
                        router_response_types::RedirectForm::CybersourceAuthSetup {
                            access_token,
                            ddc_url,
                            reference_id,
                        } => {
                            let mut form_fields = HashMap::new();
                            form_fields.insert("access_token".to_string(), access_token);
                            form_fields.insert("ddc_url".to_string(), ddc_url.clone());
                            form_fields.insert("reference_id".to_string(), reference_id);

                            Ok(grpc_api_types::payments::RedirectForm {
                                form_type: Some(
                                    grpc_api_types::payments::redirect_form::FormType::Form(
                                        grpc_api_types::payments::FormData {
                                            endpoint: ddc_url,
                                            method: grpc_api_types::payments::HttpMethod::Post
                                                .into(),
                                            form_fields,
                                        },
                                    ),
                                ),
                            })
                        }
                        router_response_types::RedirectForm::Nmi {
                            amount,
                            public_key,
                            customer_vault_id,
                            order_id,
                            continue_redirection_url,
                        } => Ok(grpc_api_types::payments::RedirectForm {
                            form_type: Some(
                                grpc_api_types::payments::redirect_form::FormType::Nmi(
                                    grpc_api_types::payments::NmiData {
                                        amount: Some(amount),
                                        public_key: Some(public_key),
                                        customer_vault_id,
                                        order_id,
                                        continue_redirection_url,
                                    },
                                ),
                            ),
                        }),
                        _ => Err(report!(ConnectorError::UnexpectedResponseError {
                            context: ResponseTransformationErrorContext {
                                http_status_code: None,
                                additional_context: Some(
                                    "Invalid response type received from connector".to_owned()
                                ),
                            },
                        })),
                    })
                    .transpose()?,
                connector_feature_data: None,
                merchant_order_id: connector_response_reference_id,
                status: grpc_status.into(),
                error: None,
                raw_connector_response,
                status_code: status_code.into(),
                response_headers,
                network_transaction_id: None,
                state: None,
                authentication_data: authentication_data.map(ForeignFrom::foreign_from),
            },
            _ => {
                return Err(report!(ConnectorError::UnexpectedResponseError {
                    context: ResponseTransformationErrorContext {
                        http_status_code: None,
                        additional_context: Some(
                            "Invalid response type for pre authenticate from connector response"
                                .to_owned()
                        ),
                    },
                }))
            }
        },
        Err(err) => {
            let status = err
                .attempt_status
                .map(grpc_api_types::payments::PaymentStatus::foreign_from)
                .unwrap_or_default();
            PaymentMethodAuthenticationServicePreAuthenticateResponse {
                connector_transaction_id: None,
                redirection_data: None,
                network_transaction_id: None,
                merchant_order_id: None,
                status: status.into(),
                error: Some(grpc_api_types::payments::ErrorInfo {
                    unified_details: None,
                    connector_details: Some(grpc_api_types::payments::ConnectorErrorDetails {
                        code: Some(err.code),
                        message: Some(err.message.clone()),
                        reason: err.reason.clone(),
                    }),
                    issuer_details: Some(grpc_api_types::payments::IssuerErrorDetails {
                        code: None,
                        message: err.network_error_message.clone(),
                        network_details: Some(grpc_api_types::payments::NetworkErrorDetails {
                            advice_code: err.network_advice_code,
                            decline_code: err.network_decline_code,
                            error_message: err.network_error_message.clone(),
                        }),
                    }),
                }),
                status_code: err.status_code.into(),
                response_headers,
                raw_connector_response,
                connector_feature_data: None,
                state: None,
                authentication_data: None,
            }
        }
    };
    Ok(response)
}

pub fn generate_payment_authenticate_response<T: PaymentMethodDataTypes>(
    router_data_v2: RouterDataV2<
        Authenticate,
        PaymentFlowData,
        PaymentsAuthenticateData<T>,
        PaymentsResponseData,
    >,
) -> Result<
    PaymentMethodAuthenticationServiceAuthenticateResponse,
    error_stack::Report<ConnectorError>,
> {
    let transaction_response = router_data_v2.response;
    let status = router_data_v2.resource_common_data.status;
    let grpc_status = grpc_api_types::payments::PaymentStatus::foreign_from(status);
    let raw_connector_response = router_data_v2
        .resource_common_data
        .get_raw_connector_response();
    let response_headers = router_data_v2
        .resource_common_data
        .get_connector_response_headers_as_map();

    let response = match transaction_response {
        Ok(response) => match response {
            PaymentsResponseData::AuthenticateResponse {
                resource_id,
                redirection_data,
                authentication_data,
                connector_response_reference_id,
                status_code,
            } => PaymentMethodAuthenticationServiceAuthenticateResponse {
                merchant_order_id: connector_response_reference_id,
                connector_transaction_id: resource_id
                    .map(Option::foreign_try_from)
                    .transpose()?
                    .unwrap_or_default(),
                redirection_data: redirection_data
                    .map(|form| match *form {
                        router_response_types::RedirectForm::Form {
                            endpoint,
                            method,
                            form_fields,
                        } => Ok::<
                            grpc_api_types::payments::RedirectForm,
                            error_stack::Report<ConnectorError>,
                        >(grpc_api_types::payments::RedirectForm {
                            form_type: Some(
                                grpc_api_types::payments::redirect_form::FormType::Form(
                                    grpc_api_types::payments::FormData {
                                        endpoint,
                                        method: grpc_api_types::payments::HttpMethod::foreign_from(
                                            method,
                                        )
                                        .into(),
                                        form_fields,
                                    },
                                ),
                            ),
                        }),
                        router_response_types::RedirectForm::Html { html_data } => {
                            Ok(grpc_api_types::payments::RedirectForm {
                                form_type: Some(
                                    grpc_api_types::payments::redirect_form::FormType::Html(
                                        grpc_api_types::payments::HtmlData { html_data },
                                    ),
                                ),
                            })
                        }
                        router_response_types::RedirectForm::Uri { uri } => {
                            Ok(grpc_api_types::payments::RedirectForm {
                                form_type: Some(
                                    grpc_api_types::payments::redirect_form::FormType::Uri(
                                        grpc_api_types::payments::UriData { uri },
                                    ),
                                ),
                            })
                        }
                        router_response_types::RedirectForm::Mifinity {
                            initialization_token,
                        } => Ok(grpc_api_types::payments::RedirectForm {
                            form_type: Some(
                                grpc_api_types::payments::redirect_form::FormType::Uri(
                                    grpc_api_types::payments::UriData {
                                        uri: initialization_token,
                                    },
                                ),
                            ),
                        }),
                        router_response_types::RedirectForm::CybersourceConsumerAuth {
                            access_token,
                            step_up_url,
                        } => {
                            let mut form_fields = HashMap::new();
                            form_fields.insert("access_token".to_string(), access_token);
                            form_fields.insert("step_up_url".to_string(), step_up_url.clone());

                            Ok(grpc_api_types::payments::RedirectForm {
                                form_type: Some(
                                    grpc_api_types::payments::redirect_form::FormType::Form(
                                        grpc_api_types::payments::FormData {
                                            endpoint: step_up_url,
                                            method: grpc_api_types::payments::HttpMethod::Post
                                                .into(),
                                            form_fields,
                                        },
                                    ),
                                ),
                            })
                        }
                        _ => Err(report!(ConnectorError::UnexpectedResponseError {
                            context: ResponseTransformationErrorContext {
                                http_status_code: None,
                                additional_context: Some(
                                    "Invalid response type received from connector".to_owned()
                                ),
                            },
                        })),
                    })
                    .transpose()?,
                connector_feature_data: None,
                authentication_data: authentication_data.map(ForeignFrom::foreign_from),
                status: grpc_status.into(),
                error: None,
                raw_connector_response,
                status_code: status_code.into(),
                response_headers,
                network_transaction_id: None,
                state: None,
            },
            _ => {
                return Err(report!(ConnectorError::UnexpectedResponseError {
                    context: ResponseTransformationErrorContext {
                        http_status_code: None,
                        additional_context: Some(
                            "Invalid response type for authenticate from connector response"
                                .to_owned()
                        ),
                    },
                }))
            }
        },
        Err(err) => {
            let status = err
                .attempt_status
                .map(grpc_api_types::payments::PaymentStatus::foreign_from)
                .unwrap_or_default();
            PaymentMethodAuthenticationServiceAuthenticateResponse {
                connector_transaction_id: Some("session_created".to_string()),
                redirection_data: None,
                network_transaction_id: None,
                merchant_order_id: None,
                authentication_data: None,
                status: status.into(),
                error: Some(grpc_api_types::payments::ErrorInfo {
                    unified_details: None,
                    connector_details: Some(grpc_api_types::payments::ConnectorErrorDetails {
                        code: Some(err.code),
                        message: Some(err.message.clone()),
                        reason: err.reason.clone(),
                    }),
                    issuer_details: Some(grpc_api_types::payments::IssuerErrorDetails {
                        code: None,
                        message: err.network_error_message.clone(),
                        network_details: Some(grpc_api_types::payments::NetworkErrorDetails {
                            advice_code: err.network_advice_code,
                            decline_code: err.network_decline_code,
                            error_message: err.network_error_message.clone(),
                        }),
                    }),
                }),
                status_code: err.status_code.into(),
                raw_connector_response,
                response_headers,
                connector_feature_data: None,
                state: None,
            }
        }
    };
    Ok(response)
}

pub fn generate_payment_post_authenticate_response<T: PaymentMethodDataTypes>(
    router_data_v2: RouterDataV2<
        PostAuthenticate,
        PaymentFlowData,
        PaymentsPostAuthenticateData<T>,
        PaymentsResponseData,
    >,
) -> Result<
    PaymentMethodAuthenticationServicePostAuthenticateResponse,
    error_stack::Report<ConnectorError>,
> {
    let transaction_response = router_data_v2.response;
    let status = router_data_v2.resource_common_data.status;
    let grpc_status = grpc_api_types::payments::PaymentStatus::foreign_from(status);
    let raw_connector_response = router_data_v2
        .resource_common_data
        .get_raw_connector_response();
    let response_headers = router_data_v2
        .resource_common_data
        .get_connector_response_headers_as_map();

    let response = match transaction_response {
        Ok(response) => match response {
            PaymentsResponseData::PostAuthenticateResponse {
                authentication_data,
                connector_response_reference_id,
                status_code,
            } => PaymentMethodAuthenticationServicePostAuthenticateResponse {
                connector_transaction_id: None,
                redirection_data: None,
                connector_feature_data: None,
                network_transaction_id: None,
                merchant_order_id: connector_response_reference_id,
                authentication_data: authentication_data.map(ForeignFrom::foreign_from),
                incremental_authorization_allowed: None,
                status: grpc_status.into(),
                error: None,
                raw_connector_response,
                status_code: status_code.into(),
                response_headers,
                state: None,
            },
            _ => {
                return Err(report!(ConnectorError::UnexpectedResponseError {
                    context: ResponseTransformationErrorContext {
                        http_status_code: None,
                        additional_context: Some(
                            "Invalid response type for post authenticate from connector response"
                                .to_owned()
                        ),
                    },
                }))
            }
        },
        Err(err) => {
            let status = err
                .attempt_status
                .map(grpc_api_types::payments::PaymentStatus::foreign_from)
                .unwrap_or_default();
            PaymentMethodAuthenticationServicePostAuthenticateResponse {
                connector_transaction_id: None,
                redirection_data: None,
                network_transaction_id: None,
                merchant_order_id: None,
                authentication_data: None,
                incremental_authorization_allowed: None,
                status: status.into(),
                error: Some(grpc_api_types::payments::ErrorInfo {
                    unified_details: None,
                    connector_details: Some(grpc_api_types::payments::ConnectorErrorDetails {
                        code: Some(err.code),
                        message: Some(err.message.clone()),
                        reason: err.reason.clone(),
                    }),
                    issuer_details: Some(grpc_api_types::payments::IssuerErrorDetails {
                        code: None,
                        message: err.network_error_message.clone(),
                        network_details: Some(grpc_api_types::payments::NetworkErrorDetails {
                            advice_code: err.network_advice_code,
                            decline_code: err.network_decline_code,
                            error_message: err.network_error_message.clone(),
                        }),
                    }),
                }),
                status_code: err.status_code.into(),
                response_headers,
                raw_connector_response,
                connector_feature_data: None,
                state: None,
            }
        }
    };
    Ok(response)
}

// ============================================================================
// NON-PCI PAYMENT SERVICES — ForeignTryFrom impls for Tokenized/Proxy types
//
// Each "Tokenized" request type substitutes a connector token for raw card
// data; each "Proxy" request type uses CardDetails with card_proxy field
// instead of raw card data.  The conversions below normalise both non-PCI
// request types into the corresponding PCI base types so the existing
// ForeignTryFrom / flow-transformer machinery can be reused unchanged.
// ============================================================================

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Wrap `CardDetails` in a `PaymentMethodWrapper` using the `CardProxy` variant.
fn card_proxy_pm(card: grpc_payment_types::CardDetails) -> grpc_payment_types::PaymentMethod {
    grpc_payment_types::PaymentMethod {
        payment_method: Some(grpc_payment_types::payment_method::PaymentMethod::CardProxy(card)),
    }
}

// ---------------------------------------------------------------------------
// PaymentServiceTokenAuthorizeRequest
// ---------------------------------------------------------------------------

pub fn tokenized_authorize_to_base(
    v: grpc_payment_types::PaymentServiceTokenAuthorizeRequest,
) -> PaymentServiceAuthorizeRequest {
    PaymentServiceAuthorizeRequest {
        merchant_transaction_id: v.merchant_transaction_id,
        amount: v.amount,
        payment_method: Some(grpc_payment_types::PaymentMethod {
            payment_method: Some(grpc_payment_types::payment_method::PaymentMethod::Token(
                grpc_payment_types::TokenPaymentMethodType {
                    token: v.connector_token.clone(),
                },
            )),
        }),
        capture_method: v.capture_method,
        customer: v.customer,
        address: v.address,
        return_url: v.return_url,
        webhook_url: v.webhook_url,
        metadata: v.metadata,
        connector_feature_data: v.connector_feature_data,
        setup_future_usage: v.setup_future_usage,
        browser_info: v.browser_info,
        state: v.state,
        connector_order_id: v.connector_order_id,
        merchant_order_id: v.merchant_order_id,
        l2_l3_data: v.l2_l3_data,
        customer_acceptance: v.customer_acceptance,
        auth_type: grpc_payment_types::AuthenticationType::NoThreeDs as i32,
        // Fields present in TokenAuthorizeRequest
        billing_descriptor: v.billing_descriptor,
        payment_experience: v.payment_experience,
        description: v.description,
        payment_channel: v.payment_channel,
        test_mode: v.test_mode,
        // Fields not in TokenAuthorizeRequest - set to None/default
        authentication_data: None,
        complete_authorize_url: None,
        continue_redirection_url: None,
        enrolled_for_3ds: None,
        enable_partial_authorization: None,
        locale: None,
        off_session: None,
        order_category: None,
        order_details: Vec::new(),
        order_tax_amount: None,
        redirection_response: None,
        request_extended_authorization: None,
        request_incremental_authorization: None,
        session_token: None,
        setup_mandate_details: None,
        shipping_cost: None,
        statement_descriptor_name: None,
        statement_descriptor_suffix: None,
        threeds_completion_indicator: None,
        tokenization_strategy: None,
    }
}

impl
    ForeignTryFrom<(
        grpc_payment_types::PaymentServiceTokenAuthorizeRequest,
        Connectors,
        &MaskedMetadata,
    )> for PaymentFlowData
{
    type Error = IntegrationError;

    fn foreign_try_from(
        (v, connectors, meta): (
            grpc_payment_types::PaymentServiceTokenAuthorizeRequest,
            Connectors,
            &MaskedMetadata,
        ),
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        ForeignTryFrom::foreign_try_from((tokenized_authorize_to_base(v), connectors, meta))
    }
}

impl<
        T: PaymentMethodDataTypes
            + Default
            + Debug
            + Send
            + Eq
            + PartialEq
            + Serialize
            + serde::de::DeserializeOwned
            + Clone
            + CardConversionHelper<T>,
    > ForeignTryFrom<grpc_payment_types::PaymentServiceTokenAuthorizeRequest>
    for PaymentsAuthorizeData<T>
{
    type Error = IntegrationError;

    fn foreign_try_from(
        v: grpc_payment_types::PaymentServiceTokenAuthorizeRequest,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        ForeignTryFrom::foreign_try_from(tokenized_authorize_to_base(v))
    }
}

// ---------------------------------------------------------------------------
// PaymentServiceTokenSetupRecurringRequest
// ---------------------------------------------------------------------------

pub fn tokenized_setup_recurring_to_base(
    v: grpc_payment_types::PaymentServiceTokenSetupRecurringRequest,
) -> PaymentServiceSetupRecurringRequest {
    PaymentServiceSetupRecurringRequest {
        merchant_recurring_payment_id: v.merchant_recurring_payment_id,
        amount: v.amount,
        payment_method: Some(grpc_payment_types::PaymentMethod {
            payment_method: Some(grpc_payment_types::payment_method::PaymentMethod::Token(
                grpc_payment_types::TokenPaymentMethodType {
                    token: v.connector_token.clone(),
                },
            )),
        }),
        customer: v.customer,
        address: v.address,
        return_url: v.return_url,
        webhook_url: v.webhook_url,
        metadata: v.metadata,
        connector_feature_data: v.connector_feature_data,
        state: v.state,
        customer_acceptance: v.customer_acceptance,
        setup_mandate_details: v.setup_mandate_details,
        setup_future_usage: v.setup_future_usage,
        // Fields not in TokenSetupRecurringRequest - set to None/default
        auth_type: grpc_payment_types::AuthenticationType::NoThreeDs as i32,
        authentication_data: None,
        billing_descriptor: None,
        browser_info: None,
        complete_authorize_url: None,
        connector_testing_data: None,
        enable_partial_authorization: None,
        enrolled_for_3ds: false,
        locale: None,
        l2_l3_data: None,
        merchant_order_id: None,
        off_session: None,
        order_category: None,
        order_id: None,
        order_tax_amount: None,
        payment_channel: None,
        payment_experience: None,
        request_extended_authorization: None,
        request_incremental_authorization: false,
        session_token: None,
        shipping_cost: None,
    }
}

impl
    ForeignTryFrom<(
        grpc_payment_types::PaymentServiceTokenSetupRecurringRequest,
        Connectors,
        &MaskedMetadata,
    )> for PaymentFlowData
{
    type Error = IntegrationError;

    fn foreign_try_from(
        (v, connectors, meta): (
            grpc_payment_types::PaymentServiceTokenSetupRecurringRequest,
            Connectors,
            &MaskedMetadata,
        ),
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        ForeignTryFrom::foreign_try_from((tokenized_setup_recurring_to_base(v), connectors, meta))
    }
}

impl<
        T: PaymentMethodDataTypes
            + Default
            + Debug
            + Send
            + Eq
            + PartialEq
            + Serialize
            + serde::de::DeserializeOwned
            + Clone
            + CardConversionHelper<T>,
    > ForeignTryFrom<grpc_payment_types::PaymentServiceTokenSetupRecurringRequest>
    for SetupMandateRequestData<T>
{
    type Error = IntegrationError;

    fn foreign_try_from(
        v: grpc_payment_types::PaymentServiceTokenSetupRecurringRequest,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        ForeignTryFrom::foreign_try_from(tokenized_setup_recurring_to_base(v))
    }
}

// ---------------------------------------------------------------------------
// PaymentServiceProxyAuthorizeRequest
// ---------------------------------------------------------------------------

pub fn proxied_authorize_to_base(
    v: grpc_payment_types::PaymentServiceProxyAuthorizeRequest,
) -> Result<PaymentServiceAuthorizeRequest, error_stack::Report<IntegrationError>> {
    let card = v.card_proxy.ok_or_else(|| {
        report!(IntegrationError::InvalidDataFormat {
            field_name: "unknown",
            context: IntegrationErrorContext {
                additional_context: Some(
                    "Card proxy is required for proxy authorization".to_string()
                ),
                ..Default::default()
            }
        })
    })?;
    Ok(PaymentServiceAuthorizeRequest {
        merchant_transaction_id: v.merchant_transaction_id,
        amount: v.amount,
        payment_method: Some(card_proxy_pm(card)),
        capture_method: v.capture_method,
        customer: v.customer,
        address: v.address,
        return_url: v.return_url,
        webhook_url: v.webhook_url,
        metadata: v.metadata,
        connector_feature_data: v.connector_feature_data,
        setup_future_usage: v.setup_future_usage,
        browser_info: v.browser_info,
        state: v.state,
        connector_order_id: v.connector_order_id,
        merchant_order_id: v.merchant_order_id,
        l2_l3_data: v.l2_l3_data,
        customer_acceptance: v.customer_acceptance,
        auth_type: v.auth_type,
        authentication_data: v.authentication_data,
        threeds_completion_indicator: v.threeds_completion_indicator,
        redirection_response: v.redirection_response,
        billing_descriptor: v.billing_descriptor,
        complete_authorize_url: None,
        continue_redirection_url: None,
        description: v.description,
        // Fields not present in PaymentServiceProxyAuthorizeRequest - set to None/default
        enrolled_for_3ds: None,
        enable_partial_authorization: None,
        locale: None,
        off_session: None,
        request_incremental_authorization: None,
        request_extended_authorization: None,
        payment_channel: None,
        payment_experience: None,
        order_category: v.order_category,
        order_details: Vec::new(),
        session_token: None,
        shipping_cost: None,
        order_tax_amount: None,
        statement_descriptor_name: None,
        statement_descriptor_suffix: None,
        tokenization_strategy: None,
        setup_mandate_details: None,
        test_mode: None,
    })
}

impl
    ForeignTryFrom<(
        grpc_payment_types::PaymentServiceProxyAuthorizeRequest,
        Connectors,
        &MaskedMetadata,
    )> for PaymentFlowData
{
    type Error = IntegrationError;

    fn foreign_try_from(
        (v, connectors, meta): (
            grpc_payment_types::PaymentServiceProxyAuthorizeRequest,
            Connectors,
            &MaskedMetadata,
        ),
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        ForeignTryFrom::foreign_try_from((proxied_authorize_to_base(v)?, connectors, meta))
    }
}

impl<
        T: PaymentMethodDataTypes
            + Default
            + Debug
            + Send
            + Eq
            + PartialEq
            + Serialize
            + serde::de::DeserializeOwned
            + Clone
            + CardConversionHelper<T>,
    > ForeignTryFrom<grpc_payment_types::PaymentServiceProxyAuthorizeRequest>
    for PaymentsAuthorizeData<T>
{
    type Error = IntegrationError;

    fn foreign_try_from(
        v: grpc_payment_types::PaymentServiceProxyAuthorizeRequest,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        ForeignTryFrom::foreign_try_from(proxied_authorize_to_base(v)?)
    }
}

// ---------------------------------------------------------------------------
// PaymentServiceProxySetupRecurringRequest
// ---------------------------------------------------------------------------

pub fn proxied_setup_recurring_to_base(
    v: grpc_payment_types::PaymentServiceProxySetupRecurringRequest,
) -> Result<PaymentServiceSetupRecurringRequest, error_stack::Report<IntegrationError>> {
    let card = v.card_proxy.ok_or_else(|| {
        report!(IntegrationError::InvalidDataFormat {
            field_name: "unknown",
            context: IntegrationErrorContext {
                additional_context: Some(
                    "Card proxy is required for proxy setup recurring".to_string()
                ),
                ..Default::default()
            }
        })
    })?;
    Ok(PaymentServiceSetupRecurringRequest {
        merchant_recurring_payment_id: v.merchant_recurring_payment_id,
        amount: v.amount,
        payment_method: Some(card_proxy_pm(card)),
        customer: v.customer,
        address: v.address,
        return_url: v.return_url,
        webhook_url: v.webhook_url,
        metadata: v.metadata,
        state: v.state,
        customer_acceptance: v.customer_acceptance,
        setup_mandate_details: v.setup_mandate_details,
        setup_future_usage: v.setup_future_usage,
        browser_info: v.browser_info,
        // Fields not in ProxySetupRecurringRequest - set to None/default
        auth_type: grpc_payment_types::AuthenticationType::NoThreeDs as i32,
        authentication_data: None,
        billing_descriptor: None,
        connector_feature_data: None,
        connector_testing_data: None,
        complete_authorize_url: None,
        enable_partial_authorization: None,
        enrolled_for_3ds: false,
        locale: None,
        l2_l3_data: None,
        merchant_order_id: None,
        off_session: None,
        order_category: None,
        order_id: None,
        order_tax_amount: None,
        payment_channel: None,
        payment_experience: None,
        request_extended_authorization: None,
        request_incremental_authorization: false,
        session_token: None,
        shipping_cost: None,
    })
}

impl
    ForeignTryFrom<(
        grpc_payment_types::PaymentServiceProxySetupRecurringRequest,
        Connectors,
        &MaskedMetadata,
    )> for PaymentFlowData
{
    type Error = IntegrationError;

    fn foreign_try_from(
        (v, connectors, meta): (
            grpc_payment_types::PaymentServiceProxySetupRecurringRequest,
            Connectors,
            &MaskedMetadata,
        ),
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        ForeignTryFrom::foreign_try_from((proxied_setup_recurring_to_base(v)?, connectors, meta))
    }
}

impl<
        T: PaymentMethodDataTypes
            + Default
            + Debug
            + Send
            + Eq
            + PartialEq
            + Serialize
            + serde::de::DeserializeOwned
            + Clone
            + CardConversionHelper<T>,
    > ForeignTryFrom<grpc_payment_types::PaymentServiceProxySetupRecurringRequest>
    for SetupMandateRequestData<T>
{
    type Error = IntegrationError;

    fn foreign_try_from(
        v: grpc_payment_types::PaymentServiceProxySetupRecurringRequest,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        ForeignTryFrom::foreign_try_from(proxied_setup_recurring_to_base(v)?)
    }
}

pub fn generate_mandate_revoke_response(
    router_data_v2: RouterDataV2<
        MandateRevoke,
        PaymentFlowData,
        MandateRevokeRequestData,
        connector_types::MandateRevokeResponseData,
    >,
) -> Result<RecurringPaymentServiceRevokeResponse, error_stack::Report<ConnectorError>> {
    let mandate_revoke_response = router_data_v2.response;
    let raw_connector_response = router_data_v2
        .resource_common_data
        .get_raw_connector_response();
    let raw_connector_request = router_data_v2
        .resource_common_data
        .get_raw_connector_request();
    let response_headers = router_data_v2
        .resource_common_data
        .get_connector_response_headers_as_map();
    match mandate_revoke_response {
        Ok(response) => Ok(RecurringPaymentServiceRevokeResponse {
            status: match response.mandate_status {
                common_enums::MandateStatus::Active => {
                    grpc_api_types::payments::MandateStatus::Active
                }
                common_enums::MandateStatus::Inactive => {
                    grpc_api_types::payments::MandateStatus::MandateInactive
                }
                common_enums::MandateStatus::Pending => {
                    grpc_api_types::payments::MandateStatus::MandatePending
                }
                common_enums::MandateStatus::Revoked => {
                    grpc_api_types::payments::MandateStatus::Revoked
                }
            }
            .into(),
            error: None,
            status_code: response.status_code.into(),
            response_headers,
            network_transaction_id: None,
            merchant_revoke_id: None,
            raw_connector_response,
            raw_connector_request,
        }),
        Err(e) => Ok(RecurringPaymentServiceRevokeResponse {
            status: grpc_api_types::payments::MandateStatus::MandateRevokeFailed.into(), // Default status for failed revoke
            error: Some(grpc_api_types::payments::ErrorInfo {
                unified_details: None,
                connector_details: Some(grpc_api_types::payments::ConnectorErrorDetails {
                    code: Some(e.code),
                    message: Some(e.message.clone()),
                    reason: e.reason.clone(),
                }),
                issuer_details: None,
            }),
            status_code: e.status_code.into(),
            response_headers,
            network_transaction_id: None,
            merchant_revoke_id: e.connector_transaction_id,
            raw_connector_response,
            raw_connector_request,
        }),
    }
}
