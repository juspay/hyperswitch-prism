#![allow(clippy::expect_used, clippy::panic, clippy::as_conversions)]

use composite_service::payments::Payments;
use grpc_api_types::payments::{
    self as p, composite_payment_service_server::CompositePaymentService,
};
use std::{
    future::Future,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    task::{Context, Poll, Wake, Waker},
};

#[derive(Clone)]
struct StubServices {
    redirect: bool,
    authorize_calls: Arc<AtomicUsize>,
    pre_authenticate_calls: Arc<AtomicUsize>,
}

#[tonic::async_trait]
impl p::payment_service_server::PaymentService for StubServices {
    async fn authorize(
        &self,
        _request: tonic::Request<p::PaymentServiceAuthorizeRequest>,
    ) -> Result<tonic::Response<p::PaymentServiceAuthorizeResponse>, tonic::Status> {
        self.authorize_calls.fetch_add(1, Ordering::SeqCst);
        Ok(tonic::Response::new(p::PaymentServiceAuthorizeResponse {
            merchant_transaction_id: Some("composite-redirect-test".to_owned()),
            connector_transaction_id: Some("provider-test-id".to_owned()),
            status: if self.redirect {
                p::PaymentStatus::AuthenticationPending as i32
            } else {
                p::PaymentStatus::Charged as i32
            },
            redirection_data: self.redirect.then(|| p::RedirectForm {
                form_type: Some(p::redirect_form::FormType::Uri(p::UriData {
                    uri: "https://example.test/challenge".to_owned(),
                })),
            }),
            ..Default::default()
        }))
    }
    async fn get(
        &self,
        _request: tonic::Request<p::PaymentServiceGetRequest>,
    ) -> Result<tonic::Response<p::PaymentServiceGetResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented(
            "unexpected service call in authorize redirect test",
        ))
    }
    async fn void(
        &self,
        _request: tonic::Request<p::PaymentServiceVoidRequest>,
    ) -> Result<tonic::Response<p::PaymentServiceVoidResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented(
            "unexpected service call in authorize redirect test",
        ))
    }
    async fn reverse(
        &self,
        _request: tonic::Request<p::PaymentServiceReverseRequest>,
    ) -> Result<tonic::Response<p::PaymentServiceReverseResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented(
            "unexpected service call in authorize redirect test",
        ))
    }
    async fn capture(
        &self,
        _request: tonic::Request<p::PaymentServiceCaptureRequest>,
    ) -> Result<tonic::Response<p::PaymentServiceCaptureResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented(
            "unexpected service call in authorize redirect test",
        ))
    }
    async fn create_order(
        &self,
        _request: tonic::Request<p::PaymentServiceCreateOrderRequest>,
    ) -> Result<tonic::Response<p::PaymentServiceCreateOrderResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented(
            "unexpected service call in authorize redirect test",
        ))
    }
    async fn refund(
        &self,
        _request: tonic::Request<p::PaymentServiceRefundRequest>,
    ) -> Result<tonic::Response<p::RefundResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented(
            "unexpected service call in authorize redirect test",
        ))
    }
    async fn incremental_authorization(
        &self,
        _request: tonic::Request<p::PaymentServiceIncrementalAuthorizationRequest>,
    ) -> Result<tonic::Response<p::PaymentServiceIncrementalAuthorizationResponse>, tonic::Status>
    {
        Err(tonic::Status::unimplemented(
            "unexpected service call in authorize redirect test",
        ))
    }
    async fn verify_redirect_response(
        &self,
        _request: tonic::Request<p::PaymentServiceVerifyRedirectResponseRequest>,
    ) -> Result<tonic::Response<p::PaymentServiceVerifyRedirectResponseResponse>, tonic::Status>
    {
        Err(tonic::Status::unimplemented(
            "unexpected service call in authorize redirect test",
        ))
    }
    async fn setup_recurring(
        &self,
        _request: tonic::Request<p::PaymentServiceSetupRecurringRequest>,
    ) -> Result<tonic::Response<p::PaymentServiceSetupRecurringResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented(
            "unexpected service call in authorize redirect test",
        ))
    }
    async fn token_authorize(
        &self,
        _request: tonic::Request<p::PaymentServiceTokenAuthorizeRequest>,
    ) -> Result<tonic::Response<p::PaymentServiceAuthorizeResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented(
            "unexpected service call in authorize redirect test",
        ))
    }
    async fn token_setup_recurring(
        &self,
        _request: tonic::Request<p::PaymentServiceTokenSetupRecurringRequest>,
    ) -> Result<tonic::Response<p::PaymentServiceSetupRecurringResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented(
            "unexpected service call in authorize redirect test",
        ))
    }
    async fn proxy_authorize(
        &self,
        _request: tonic::Request<p::PaymentServiceProxyAuthorizeRequest>,
    ) -> Result<tonic::Response<p::PaymentServiceAuthorizeResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented(
            "unexpected service call in authorize redirect test",
        ))
    }
    async fn proxy_setup_recurring(
        &self,
        _request: tonic::Request<p::PaymentServiceProxySetupRecurringRequest>,
    ) -> Result<tonic::Response<p::PaymentServiceSetupRecurringResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented(
            "unexpected service call in authorize redirect test",
        ))
    }
}

#[tonic::async_trait]
impl p::merchant_authentication_service_server::MerchantAuthenticationService for StubServices {
    async fn create_server_authentication_token(
        &self,
        _request: tonic::Request<
            p::MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest,
        >,
    ) -> Result<
        tonic::Response<p::MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse>,
        tonic::Status,
    > {
        Ok(tonic::Response::new(Default::default()))
    }
    async fn create_server_session_authentication_token(
        &self,
        _request: tonic::Request<
            p::MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenRequest,
        >,
    ) -> Result<
        tonic::Response<
            p::MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenResponse,
        >,
        tonic::Status,
    > {
        Ok(tonic::Response::new(Default::default()))
    }
    async fn create_client_authentication_token(
        &self,
        _request: tonic::Request<
            p::MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest,
        >,
    ) -> Result<
        tonic::Response<p::MerchantAuthenticationServiceCreateClientAuthenticationTokenResponse>,
        tonic::Status,
    > {
        Ok(tonic::Response::new(Default::default()))
    }
}

#[tonic::async_trait]
impl p::customer_service_server::CustomerService for StubServices {
    async fn create(
        &self,
        _request: tonic::Request<p::CustomerServiceCreateRequest>,
    ) -> Result<tonic::Response<p::CustomerServiceCreateResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented(
            "unexpected service call in authorize redirect test",
        ))
    }
    async fn get(
        &self,
        _request: tonic::Request<p::CustomerServiceGetRequest>,
    ) -> Result<tonic::Response<p::CustomerServiceGetResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented(
            "unexpected service call in authorize redirect test",
        ))
    }
}

#[tonic::async_trait]
impl p::refund_service_server::RefundService for StubServices {
    async fn get(
        &self,
        _request: tonic::Request<p::RefundServiceGetRequest>,
    ) -> Result<tonic::Response<p::RefundResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented(
            "unexpected service call in authorize redirect test",
        ))
    }
    async fn void_post_refund(
        &self,
        _request: tonic::Request<p::RefundServiceVoidPostRefundRequest>,
    ) -> Result<tonic::Response<p::RefundResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented(
            "unexpected service call in authorize redirect test",
        ))
    }
}

#[tonic::async_trait]
impl p::payment_method_authentication_service_server::PaymentMethodAuthenticationService
    for StubServices
{
    async fn pre_authenticate(
        &self,
        _request: tonic::Request<p::PaymentMethodAuthenticationServicePreAuthenticateRequest>,
    ) -> Result<
        tonic::Response<p::PaymentMethodAuthenticationServicePreAuthenticateResponse>,
        tonic::Status,
    > {
        self.pre_authenticate_calls.fetch_add(1, Ordering::SeqCst);
        Ok(tonic::Response::new(Default::default()))
    }
    async fn authenticate(
        &self,
        _request: tonic::Request<p::PaymentMethodAuthenticationServiceAuthenticateRequest>,
    ) -> Result<
        tonic::Response<p::PaymentMethodAuthenticationServiceAuthenticateResponse>,
        tonic::Status,
    > {
        Err(tonic::Status::unimplemented(
            "unexpected service call in authorize redirect test",
        ))
    }
    async fn post_authenticate(
        &self,
        _request: tonic::Request<p::PaymentMethodAuthenticationServicePostAuthenticateRequest>,
    ) -> Result<
        tonic::Response<p::PaymentMethodAuthenticationServicePostAuthenticateResponse>,
        tonic::Status,
    > {
        Err(tonic::Status::unimplemented(
            "unexpected service call in authorize redirect test",
        ))
    }
}

struct NoopWake;
impl Wake for NoopWake {
    fn wake(self: Arc<Self>) {}
}

// Stub calls resolve immediately; no network or async runtime is needed.
fn complete<F: Future>(future: F) -> F::Output {
    let waker = Waker::from(Arc::new(NoopWake));
    let mut context = Context::from_waker(&waker);
    match std::pin::pin!(future).as_mut().poll(&mut context) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("unexpected pending operation in composite test"),
    }
}

fn run(connector: &'static str, redirect: bool) -> p::CompositeAuthorizeResponse {
    let count = Arc::new(AtomicUsize::new(0));
    let services = StubServices {
        redirect,
        authorize_calls: count.clone(),
        pre_authenticate_calls: Arc::new(AtomicUsize::new(0)),
    };
    let composite = Payments::new(
        services.clone(),
        services.clone(),
        services.clone(),
        services.clone(),
        services,
    );
    let mut request = tonic::Request::new(p::CompositeAuthorizeRequest {
        merchant_transaction_id: Some("composite-redirect-test".to_owned()),
        auth_type: p::AuthenticationType::NoThreeDs as i32,
        state: Some(p::ConnectorState {
            connector_customer_id: Some("existing-test-customer".to_owned()),
            ..Default::default()
        }),
        amount: Some(p::Money {
            minor_amount: 1000,
            currency: p::Currency::Eur as i32,
        }),
        payment_method: Some(p::PaymentMethod {
            payment_method: Some(p::payment_method::PaymentMethod::Card(Default::default())),
        }),
        ..Default::default()
    });
    request.metadata_mut().insert(
        "x-connector",
        connector.parse().expect("connector metadata"),
    );
    let response = complete(composite.authorize(request))
        .expect("composite authorize succeeds")
        .into_inner();
    assert_eq!(
        count.load(Ordering::SeqCst),
        1,
        "{connector}: authorize must execute exactly once"
    );
    assert!(
        response.pre_authenticate_response.is_none(),
        "redirect comes from authorize"
    );
    assert!(
        response.authenticate_response.is_none(),
        "redirect comes from authorize"
    );
    response
}

#[test]
fn authorize_redirect_sets_redirect_required_for_nuvei_and_other_connectors() {
    for connector in ["nuvei", "stripe", "adyen"] {
        let response = run(connector, true);
        assert_eq!(
            response.composite_status,
            p::CompositeStatus::RedirectRequired as i32,
            "{connector}"
        );
        let authorize = response
            .authorize_response
            .expect("authorize response preserved");
        assert_eq!(
            authorize.status,
            p::PaymentStatus::AuthenticationPending as i32
        );
        assert_eq!(
            authorize.redirection_data,
            Some(p::RedirectForm {
                form_type: Some(p::redirect_form::FormType::Uri(p::UriData {
                    uri: "https://example.test/challenge".to_owned()
                }))
            })
        );
    }
}

#[test]
fn authorize_without_redirect_remains_completed_for_nuvei_and_other_connectors() {
    for connector in ["nuvei", "stripe", "adyen"] {
        let response = run(connector, false);
        assert_eq!(
            response.composite_status,
            p::CompositeStatus::Completed as i32,
            "{connector}"
        );
        let authorize = response
            .authorize_response
            .expect("authorize response preserved");
        assert_eq!(authorize.status, p::PaymentStatus::Charged as i32);
        assert!(authorize.redirection_data.is_none());
    }
}

fn gpay(cryptogram: Option<&str>, authenticated: bool) -> p::PaymentMethod {
    serde_json::from_value(serde_json::json!({"payment_method":{"google_pay_sdk":{
        "type":"CARD","description":"Local routing regression",
        "info":{"card_network":"VISA","card_details":"1111","assurance_details":{
            "card_holder_authenticated":authenticated,"account_verified":true}},
        "tokenization_data":{"tokenization_data":{"decrypted_data":{
            "card_exp_month":"12","card_exp_year":"2030",
            "application_primary_account_number":"4111111111111111",
            "cryptogram":cryptogram,"eci_indicator":"05"}}}
    }}}))
    .expect("Google Pay fixture")
}
fn apple(cryptogram: &str) -> p::PaymentMethod {
    serde_json::from_value(serde_json::json!({"payment_method":{"apple_pay_sdk":{
        "transaction_identifier":"local-test","payment_method":{"display_name":"Visa","network":"Visa","type":"credit"},
        "payment_data":{"payment_data":{"decrypted_data":{
            "application_primary_account_number":"4111111111111111",
            "application_expiration_month":"12","application_expiration_year":"2030",
            "payment_data":{"online_payment_cryptogram":cryptogram,"eci_indicator":"05"}
        }}}
    }}})).expect("Apple Pay fixture")
}

#[test]
fn composite_wallet_domain_conversion_preserves_nuvei_routing() {
    for (name, method, auth_type, authentication_data, expected_pre_auth) in [
        (
            "pan_only",
            gpay(None, false),
            p::AuthenticationType::ThreeDs,
            None,
            1,
        ),
        (
            "gpay_authenticated",
            gpay(Some("synthetic"), true),
            p::AuthenticationType::ThreeDs,
            None,
            0,
        ),
        (
            "apple_authenticated",
            apple("synthetic"),
            p::AuthenticationType::ThreeDs,
            None,
            0,
        ),
        (
            "pan_no_three_ds",
            gpay(None, false),
            p::AuthenticationType::NoThreeDs,
            None,
            0,
        ),
        (
            "pan_external_mpi",
            gpay(None, false),
            p::AuthenticationType::ThreeDs,
            Some(p::AuthenticationData {
                eci: Some("05".to_owned()),
                ..Default::default()
            }),
            0,
        ),
    ] {
        let authorize_calls = Arc::new(AtomicUsize::new(0));
        let pre_authenticate_calls = Arc::new(AtomicUsize::new(0));
        let services = StubServices {
            redirect: false,
            authorize_calls: authorize_calls.clone(),
            pre_authenticate_calls: pre_authenticate_calls.clone(),
        };
        let composite = Payments::new(
            services.clone(),
            services.clone(),
            services.clone(),
            services.clone(),
            services,
        );
        let mut request = tonic::Request::new(p::CompositeAuthorizeRequest {
            merchant_transaction_id: Some("composite-wallet-test".to_owned()),
            auth_type: auth_type as i32,
            payment_method: Some(method),
            authentication_data,
            state: Some(p::ConnectorState {
                connector_customer_id: Some("existing-test-customer".to_owned()),
                ..Default::default()
            }),
            amount: Some(p::Money {
                minor_amount: 1000,
                currency: p::Currency::Eur as i32,
            }),
            ..Default::default()
        });
        request
            .metadata_mut()
            .insert("x-connector", "nuvei".parse().expect("metadata"));
        let response = complete(composite.authorize(request))
            .expect(name)
            .into_inner();
        assert_eq!(authorize_calls.load(Ordering::SeqCst), 1, "{name}");
        assert_eq!(
            pre_authenticate_calls.load(Ordering::SeqCst),
            expected_pre_auth,
            "{name}"
        );
        assert_eq!(
            response.pre_authenticate_response.is_some(),
            expected_pre_auth == 1,
            "{name}"
        );
        assert!(response.authenticate_response.is_none(), "{name}");
        assert_eq!(
            response.composite_status,
            p::CompositeStatus::Completed as i32,
            "{name}"
        );
    }
}
