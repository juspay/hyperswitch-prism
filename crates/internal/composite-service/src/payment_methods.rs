use common_enums;
use connector_integration::types::{AuthenticatorConnectorData, ConnectorData};
use domain_types::{connector_types::ConnectorVariant, utils::ForeignTryFrom as _};
use grpc_api_types::payments::{
    composite_payment_method_service_server::CompositePaymentMethodService,
    merchant_authentication_service_server::MerchantAuthenticationService,
    payment_method_service_server::PaymentMethodService, CompositePaymentMethodCreateRequest,
    CompositePaymentMethodCreateResponse, CompositePaymentMethodGetRequest,
    CompositePaymentMethodGetResponse, CompositePaymentMethodRechargeRequest,
    CompositePaymentMethodRechargeResponse,
    MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse,
    PaymentMethodServiceCreateRequest, PaymentMethodServiceGetRequest,
    PaymentMethodServiceRechargeRequest, PaymentMethodServiceTokenizeRequest,
};
use ucs_env::error::ResultExtGrpc;

use crate::payments::CompositeAccessTokenRequest;
use crate::transformers::ForeignFrom;
use crate::utils::{
    connector_from_composite_authorize_metadata, connector_variant_from_composite_metadata,
};

/// Reports whether a wallet payload carries a decrypted *network token* — a PAN accompanied by
/// a network cryptogram — instead of the wallet provider's own encrypted token.
///
/// Must stay in lockstep with `stripe::transformers::is_decrypted_network_token`, which asks the
/// same question of the converted domain type — the composite has not chosen the PCI-holder
/// generic yet, so the two cannot share one function. This decides whether Tokenize runs at all;
/// the connector-side one decides which endpoint it runs against. Change both.
///
/// The rule is deliberately narrower than "the payload is decrypted": **the cryptogram has to be
/// there**. A Google Pay `PAN_ONLY` credential decrypts to a bare PAN with no cryptogram and no
/// ECI; once decrypted there is nothing wallet-specific left about it, so connectors charge it as
/// an ordinary card on their ordinary payment endpoint and it must not be reported here. Only the
/// cryptogram-bearing shape needs the separate handling this flag exists to trigger. Apple Pay's
/// decrypted payload always carries a cryptogram (`ApplePayCryptogramData.online_payment_cryptogram`
/// is a required field, and the domain conversion rejects the payload without it), so a decrypted
/// Apple Pay payload always reports `true`.
///
/// `common_enums::PaymentMethod` / `PaymentMethodType` collapse every wallet shape onto the same
/// `Wallet` + `GooglePay` pair, but connectors can need opposite routing for them — see
/// [`interfaces::connector_types::ValidationTrait::should_do_payment_method_token`]. Anything that
/// is not a cryptogram-bearing decrypted Apple Pay / Google Pay payload reports `false`.
fn is_wallet_payload_decrypted_network_token(
    payment_method: Option<&grpc_api_types::payments::PaymentMethod>,
) -> bool {
    use grpc_api_types::payments::{apple_wallet, google_wallet, payment_method::PaymentMethod};

    match payment_method.and_then(|pm| pm.payment_method.as_ref()) {
        Some(PaymentMethod::GooglePaySdk(google_wallet)) => matches!(
            google_wallet
                .tokenization_data
                .as_ref()
                .and_then(|data| data.tokenization_data.as_ref()),
            Some(google_wallet::tokenization_data::TokenizationData::DecryptedData(decrypted))
                if decrypted.cryptogram.is_some()
        ),
        Some(PaymentMethod::ApplePaySdk(apple_wallet)) => matches!(
            apple_wallet
                .payment_data
                .as_ref()
                .and_then(|data| data.payment_data.as_ref()),
            Some(apple_wallet::payment_data::PaymentData::DecryptedData(_))
        ),
        _ => false,
    }
}

/// Implementation of CompositeAccessTokenRequest for payment method requests.
/// These requests don't have a specific payment_method field since payment-method-management
/// flows aren't gated on a specific payment method.
impl CompositeAccessTokenRequest for CompositePaymentMethodRechargeRequest {
    fn payment_method(&self) -> Option<grpc_api_types::payments::PaymentMethod> {
        None
    }

    fn state(&self) -> Option<&grpc_api_types::payments::ConnectorState> {
        self.state.as_ref()
    }

    fn build_access_token_request(
        &self,
        connector: &ConnectorVariant,
    ) -> grpc_api_types::payments::MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest
    {
        grpc_api_types::payments::MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest::foreign_from((
            self, connector,
        ))
    }
}

impl CompositeAccessTokenRequest for CompositePaymentMethodCreateRequest {
    fn payment_method(&self) -> Option<grpc_api_types::payments::PaymentMethod> {
        None
    }

    fn state(&self) -> Option<&grpc_api_types::payments::ConnectorState> {
        self.state.as_ref()
    }

    fn build_access_token_request(
        &self,
        connector: &ConnectorVariant,
    ) -> grpc_api_types::payments::MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest
    {
        grpc_api_types::payments::MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest::foreign_from((
            self, connector,
        ))
    }
}

impl CompositeAccessTokenRequest for CompositePaymentMethodGetRequest {
    fn payment_method(&self) -> Option<grpc_api_types::payments::PaymentMethod> {
        None
    }

    fn state(&self) -> Option<&grpc_api_types::payments::ConnectorState> {
        self.state.as_ref()
    }

    fn build_access_token_request(
        &self,
        connector: &ConnectorVariant,
    ) -> grpc_api_types::payments::MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest
    {
        grpc_api_types::payments::MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest::foreign_from((
            self, connector,
        ))
    }
}

/// Composite Payment Method Service that combines payment-method operations
/// with the access-token bootstrap.
///
/// Each composite RPC (Create, Get, Recharge) auto-bootstraps the connector's
/// session token (only when the connector requires one AND the caller didn't
/// already supply one), splices the minted token into the inner request, and
/// forwards to the underlying `PaymentMethodService`.
#[derive(Clone)]
pub struct PaymentMethods<PM, MA>
where
    PM: PaymentMethodService + Clone + Send + Sync + 'static,
    MA: MerchantAuthenticationService + Clone + Send + Sync + 'static,
{
    payment_method_service: PM,
    merchant_authentication_service: MA,
}

impl<PM, MA> PaymentMethods<PM, MA>
where
    PM: PaymentMethodService + Clone + Send + Sync + 'static,
    MA: MerchantAuthenticationService + Clone + Send + Sync + 'static,
{
    pub fn new(payment_method_service: PM, merchant_authentication_service: MA) -> Self {
        Self {
            payment_method_service,
            merchant_authentication_service,
        }
    }

    /// Bootstrap the connector's session token if (a) the connector requires
    /// one and (b) the caller didn't already pass one via
    /// `state.access_token`. Returns `None` otherwise so the response's
    /// `access_token_response` slot stays unset.
    async fn create_server_authentication_token<R: CompositeAccessTokenRequest>(
        &self,
        connector: &ConnectorVariant,
        payload: &R,
        metadata: &tonic::metadata::MetadataMap,
        extensions: &tonic::Extensions,
    ) -> Result<
        Option<MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse>,
        tonic::Status,
    > {
        let should_do_access_token = {
            let payment_method = payload
                .payment_method()
                .map(common_enums::PaymentMethod::foreign_try_from)
                .transpose()
                .into_grpc_status()?;
            match connector {
                ConnectorVariant::Payment(c) => ConnectorData::<
                    domain_types::payment_method_data::DefaultPCIHolder,
                >::get_connector_by_name(c)
                .connector
                .should_do_access_token(payment_method),
                ConnectorVariant::Authenticator(c) => {
                    AuthenticatorConnectorData::get_connector_by_name(c)
                        .connector
                        .should_do_access_token(payment_method)
                }
                _ => false,
            }
        };
        let payload_access_token = payload
            .state()
            .and_then(|state| state.access_token.as_ref())
            .and_then(|token| {
                domain_types::connector_types::ServerAuthenticationTokenResponseData::foreign_try_from(
                    token,
                )
                .ok()
            });
        let should_create_access_token = should_do_access_token && payload_access_token.is_none();

        let access_token_response = match should_create_access_token {
            true => {
                let access_token_payload = payload.build_access_token_request(connector);
                let mut access_token_request = tonic::Request::new(access_token_payload);
                *access_token_request.metadata_mut() = metadata.clone();
                *access_token_request.extensions_mut() = extensions.clone();

                let access_token_response = self
                    .merchant_authentication_service
                    .create_server_authentication_token(access_token_request)
                    .await?
                    .into_inner();

                Some(access_token_response)
            }
            false => None,
        };

        Ok(access_token_response)
    }

    async fn create_payment_method_token(
        &self,
        connector: &ConnectorVariant,
        payload: &CompositePaymentMethodGetRequest,
        metadata: &tonic::metadata::MetadataMap,
        extensions: &tonic::Extensions,
    ) -> Result<Option<grpc_api_types::payments::PaymentMethodServiceTokenizeResponse>, tonic::Status>
    {
        // Skip if the caller already has a token (e.g. a previously obtained access_token).
        if payload.payment_method_token.is_none() {
            let should_do_payment_method_token = {
                let payment_method = payload
                    .payment_method
                    .as_ref()
                    .map(|pm| common_enums::PaymentMethod::foreign_try_from(pm.clone()))
                    .transpose()
                    .into_grpc_status()?
                    .unwrap_or_default();
                let payment_method_type = common_enums::PaymentMethodType::foreign_try_from(
                    payload.payment_method_type(),
                )
                .ok();
                let is_wallet_decrypted_network_token =
                    is_wallet_payload_decrypted_network_token(payload.payment_method.as_ref());
                match connector {
                    ConnectorVariant::Payment(c) => ConnectorData::<
                        domain_types::payment_method_data::DefaultPCIHolder,
                    >::get_connector_by_name(c)
                    .connector
                    .should_do_payment_method_token(
                        payment_method,
                        payment_method_type,
                        is_wallet_decrypted_network_token,
                    ),
                    ConnectorVariant::Authenticator(c) => {
                        AuthenticatorConnectorData::get_connector_by_name(c)
                            .connector
                            .should_do_payment_method_token(
                                payment_method,
                                payment_method_type,
                                is_wallet_decrypted_network_token,
                            )
                    }
                    _ => false,
                }
            };

            match should_do_payment_method_token {
                true => {
                    let tokenize_inner = PaymentMethodServiceTokenizeRequest::foreign_from(payload);
                    let mut tokenize_request = tonic::Request::new(tokenize_inner);
                    *tokenize_request.metadata_mut() = metadata.clone();
                    *tokenize_request.extensions_mut() = extensions.clone();
                    Ok(Some(
                        self.payment_method_service
                            .tokenize(tokenize_request)
                            .await?
                            .into_inner(),
                    ))
                }
                false => Ok(None),
            }
        } else {
            Ok(None)
        }
    }

    async fn process_recharge(
        &self,
        request: tonic::Request<CompositePaymentMethodRechargeRequest>,
    ) -> Result<tonic::Response<CompositePaymentMethodRechargeResponse>, tonic::Status> {
        let (metadata, extensions, payload) = request.into_parts();
        let connector = ConnectorVariant::Payment(
            connector_from_composite_authorize_metadata(&metadata).map_err(|err| *err)?,
        );
        let access_token_response = self
            .create_server_authentication_token(&connector, &payload, &metadata, &extensions)
            .await?;

        let inner = PaymentMethodServiceRechargeRequest::foreign_from((
            &payload,
            access_token_response.as_ref(),
        ));
        let mut inner_request = tonic::Request::new(inner);
        *inner_request.metadata_mut() = metadata;
        *inner_request.extensions_mut() = extensions;

        let recharge_response = self
            .payment_method_service
            .recharge(inner_request)
            .await?
            .into_inner();

        Ok(tonic::Response::new(
            CompositePaymentMethodRechargeResponse {
                access_token_response,
                recharge_response: Some(recharge_response),
            },
        ))
    }

    async fn process_create(
        &self,
        request: tonic::Request<CompositePaymentMethodCreateRequest>,
    ) -> Result<tonic::Response<CompositePaymentMethodCreateResponse>, tonic::Status> {
        let (metadata, extensions, payload) = request.into_parts();
        let connector = ConnectorVariant::Payment(
            connector_from_composite_authorize_metadata(&metadata).map_err(|err| *err)?,
        );
        let access_token_response = self
            .create_server_authentication_token(&connector, &payload, &metadata, &extensions)
            .await?;

        let inner = PaymentMethodServiceCreateRequest::foreign_from((
            &payload,
            access_token_response.as_ref(),
        ));
        let mut inner_request = tonic::Request::new(inner);
        *inner_request.metadata_mut() = metadata;
        *inner_request.extensions_mut() = extensions;

        let create_response = self
            .payment_method_service
            .create(inner_request)
            .await?
            .into_inner();

        Ok(tonic::Response::new(CompositePaymentMethodCreateResponse {
            access_token_response,
            create_response: Some(create_response),
        }))
    }

    async fn process_get(
        &self,
        request: tonic::Request<CompositePaymentMethodGetRequest>,
    ) -> Result<tonic::Response<CompositePaymentMethodGetResponse>, tonic::Status> {
        let (metadata, extensions, payload) = request.into_parts();
        let connector = connector_variant_from_composite_metadata(&metadata).map_err(|err| *err)?;
        let access_token_response = self
            .create_server_authentication_token(&connector, &payload, &metadata, &extensions)
            .await?;
        let tokenize_response = self
            .create_payment_method_token(&connector, &payload, &metadata, &extensions)
            .await?;

        let inner = PaymentMethodServiceGetRequest::foreign_from((
            &payload,
            access_token_response.as_ref(),
            tokenize_response.as_ref(),
        ));
        let mut inner_request = tonic::Request::new(inner);
        *inner_request.metadata_mut() = metadata;
        *inner_request.extensions_mut() = extensions;

        let get_response = self
            .payment_method_service
            .get(inner_request)
            .await?
            .into_inner();

        Ok(tonic::Response::new(CompositePaymentMethodGetResponse {
            access_token_response,
            get_response: Some(get_response),
            tokenize_response,
        }))
    }
}

#[tonic::async_trait]
impl<PM, MA> CompositePaymentMethodService for PaymentMethods<PM, MA>
where
    PM: PaymentMethodService + Clone + Send + Sync + 'static,
    MA: MerchantAuthenticationService + Clone + Send + Sync + 'static,
{
    /// Create a payment method (e.g. provision a wallet). Bootstraps the
    /// connector session token when needed, then forwards to the underlying
    /// `PaymentMethodService.Create`.
    async fn create(
        &self,
        request: tonic::Request<CompositePaymentMethodCreateRequest>,
    ) -> Result<tonic::Response<CompositePaymentMethodCreateResponse>, tonic::Status> {
        self.process_create(request).await
    }

    /// Look up a payment method (e.g. fetch wallet details). Same bootstrap +
    /// forward pattern.
    async fn get(
        &self,
        request: tonic::Request<CompositePaymentMethodGetRequest>,
    ) -> Result<tonic::Response<CompositePaymentMethodGetResponse>, tonic::Status> {
        self.process_get(request).await
    }

    /// Recharge a payment method (e.g. credit value to a wallet).
    async fn recharge(
        &self,
        request: tonic::Request<CompositePaymentMethodRechargeRequest>,
    ) -> Result<tonic::Response<CompositePaymentMethodRechargeResponse>, tonic::Status> {
        self.process_recharge(request).await
    }
}
