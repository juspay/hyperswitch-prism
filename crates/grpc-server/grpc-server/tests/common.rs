use std::{future::Future, sync::Arc};

use grpc_api_types::{
    health_check::health_client::HealthClient,
    payments::{
        customer_service_client::CustomerServiceClient,
        direct_payment_service_client::DirectPaymentServiceClient,
        dispute_service_client::DisputeServiceClient,
        merchant_authentication_service_client::MerchantAuthenticationServiceClient,
        payment_method_authentication_service_client::PaymentMethodAuthenticationServiceClient,
        payment_method_service_client::PaymentMethodServiceClient,
        recurring_payment_service_client::RecurringPaymentServiceClient,
        refund_service_client::RefundServiceClient,
    },
    payouts::{
        payout_service_client::PayoutServiceClient, payout_service_server::PayoutServiceServer,
    },
};
use http::Uri;
use hyper_util::rt::TokioIo; // Add this import
use tempfile::NamedTempFile;
use tokio::net::UnixListener;
use tokio_stream::wrappers::UnixListenerStream;
use tonic::transport::{Channel, Endpoint, Server};
use tower::service_fn;
use ucs_env::configs::Config;

/// Interceptor that adds config to request extensions.
///
/// Note: Tests use interceptors instead of layers because:
/// - Interceptors work seamlessly with `serve_with_incoming()` in tests
/// - Layers have type constraints (Error = Status vs Infallible) incompatible with test setup
/// - Production uses RequestExtensionsLayer with `serve_with_shutdown()`
/// - This achieves the same goal (config in extensions) for testing
#[derive(Clone)]
struct ConfigInterceptor {
    config: Arc<Config>,
}

impl tonic::service::Interceptor for ConfigInterceptor {
    fn call(&mut self, mut req: tonic::Request<()>) -> Result<tonic::Request<()>, tonic::Status> {
        req.extensions_mut().insert(self.config.clone());
        Ok(req)
    }
}

pub trait AutoClient {
    fn new(channel: Channel) -> Self;
}
impl AutoClient for DirectPaymentServiceClient<Channel> {
    fn new(channel: Channel) -> Self {
        Self::new(channel)
    }
}
impl AutoClient for HealthClient<Channel> {
    fn new(channel: Channel) -> Self {
        Self::new(channel)
    }
}

impl AutoClient for RefundServiceClient<Channel> {
    fn new(channel: Channel) -> Self {
        Self::new(channel)
    }
}

impl AutoClient for RecurringPaymentServiceClient<Channel> {
    fn new(channel: Channel) -> Self {
        Self::new(channel)
    }
}

impl AutoClient for DisputeServiceClient<Channel> {
    fn new(channel: Channel) -> Self {
        Self::new(channel)
    }
}

impl AutoClient for PaymentMethodServiceClient<Channel> {
    fn new(channel: Channel) -> Self {
        Self::new(channel)
    }
}

impl AutoClient for CustomerServiceClient<Channel> {
    fn new(channel: Channel) -> Self {
        Self::new(channel)
    }
}

impl AutoClient for MerchantAuthenticationServiceClient<Channel> {
    fn new(channel: Channel) -> Self {
        Self::new(channel)
    }
}

impl AutoClient for PaymentMethodAuthenticationServiceClient<Channel> {
    fn new(channel: Channel) -> Self {
        Self::new(channel)
    }
}

impl AutoClient for PayoutServiceClient<Channel> {
    fn new(channel: Channel) -> Self {
        Self::new(channel)
    }
}

/// Builds a gRPC server with all services registered.
fn build_server(
    service: grpc_server::app::Service,
    interceptor: ConfigInterceptor,
) -> tonic::transport::server::Router {
    Server::builder()
        .add_service(grpc_api_types::health_check::health_server::HealthServer::new(
            service.health_check_service,
        ))
        .add_service(
            grpc_api_types::payments::direct_payment_service_server::DirectPaymentServiceServer::with_interceptor(
                service.payments_service,
                interceptor.clone(),
            ),
        )
        .add_service(
            grpc_api_types::payments::refund_service_server::RefundServiceServer::with_interceptor(
                service.refunds_service,
                interceptor.clone(),
            ),
        )
        .add_service(
            grpc_api_types::payments::recurring_payment_service_server::RecurringPaymentServiceServer::with_interceptor(
                service.recurring_payment_service,
                interceptor.clone(),
            ),
        )
        .add_service(
            grpc_api_types::payments::dispute_service_server::DisputeServiceServer::with_interceptor(
                service.disputes_service,
                interceptor.clone(),
            ),
        )
        .add_service(
            grpc_api_types::payments::payment_method_service_server::PaymentMethodServiceServer::with_interceptor(
                service.payment_method_service,
                interceptor.clone(),
            ),
        )
        .add_service(
            grpc_api_types::payments::customer_service_server::CustomerServiceServer::with_interceptor(
                service.customer_service,
                interceptor.clone(),
            ),
        )
        .add_service(
            grpc_api_types::payments::merchant_authentication_service_server::MerchantAuthenticationServiceServer::with_interceptor(
                service.merchant_authentication_service,
                interceptor.clone(),
            ),
        )
        .add_service(
            grpc_api_types::payments::payment_method_authentication_service_server::PaymentMethodAuthenticationServiceServer::with_interceptor(
                service.payment_method_authentication_service,
                interceptor.clone(),
            ),
        )
        .add_service(
            PayoutServiceServer::with_interceptor(
                service.payouts_service,
                interceptor,
            ),
        )
}

/// # Panics
///
/// Will panic if the socket file cannot be created or removed
#[allow(dead_code)]
pub async fn server_and_client_stub<T>(
    service: grpc_server::app::Service,
    base_config: Arc<Config>,
) -> Result<(impl Future<Output = ()>, T), Box<dyn std::error::Error>>
where
    T: AutoClient,
{
    let socket = NamedTempFile::new()?;
    let socket = Arc::new(socket.into_temp_path());
    std::fs::remove_file(&*socket)?;

    let uds = UnixListener::bind(&*socket)?;
    let stream = UnixListenerStream::new(uds);

    let interceptor = ConfigInterceptor {
        config: base_config,
    };

    let router = build_server(service, interceptor);

    let serve_future = async move {
        let result = router.serve_with_incoming(stream).await;
        // Server must be running fine...
        assert!(result.is_ok());
    };

    let socket = Arc::clone(&socket);
    // Connect to the server over a Unix socket
    // The URL will be ignored.
    let channel = Endpoint::try_from("http://any.url")?
        .connect_with_connector(service_fn(move |_: Uri| {
            let socket = Arc::clone(&socket);
            async move {
                // Wrap the UnixStream with TokioIo to make it compatible with hyper
                let unix_stream = tokio::net::UnixStream::connect(&*socket).await?;
                Ok::<_, std::io::Error>(TokioIo::new(unix_stream))
            }
        }))
        .await?;

    let client = T::new(channel);

    Ok((serve_future, client))
}

/// # Panics
///
/// Will panic if the socket file cannot be created or removed
#[allow(dead_code)]
pub async fn server_and_channel_stub(
    service: grpc_server::app::Service,
    base_config: Arc<Config>,
) -> Result<(impl Future<Output = ()>, Channel), Box<dyn std::error::Error>> {
    let socket = NamedTempFile::new()?;
    let socket = Arc::new(socket.into_temp_path());
    std::fs::remove_file(&*socket)?;

    let uds = UnixListener::bind(&*socket)?;
    let stream = UnixListenerStream::new(uds);

    let interceptor = ConfigInterceptor {
        config: base_config,
    };

    let router = build_server(service, interceptor);

    let serve_future = async move {
        let result = router.serve_with_incoming(stream).await;
        assert!(result.is_ok());
    };

    let socket = Arc::clone(&socket);
    let channel = Endpoint::try_from("http://any.url")?
        .connect_with_connector(service_fn(move |_: Uri| {
            let socket = Arc::clone(&socket);
            async move {
                let unix_stream = tokio::net::UnixStream::connect(&*socket).await?;
                Ok::<_, std::io::Error>(TokioIo::new(unix_stream))
            }
        }))
        .await?;

    Ok((serve_future, channel))
}

#[macro_export]
macro_rules! grpc_test {
    ($client:ident, $c_type:ty, $body:block) => {
        let config = configs::Config::new().expect("Failed while parsing config");
        let base_config = std::sync::Arc::new(config);
        let server = app::Service::new(base_config.clone()).await;
        let (server_fut, mut $client) =
            common::server_and_client_stub::<$c_type>(server, base_config)
                .await
                .expect("Failed to create the server client pair");
        let response = async { $body };

        tokio::select! {
            _ = server_fut => panic!("Server failed"),
            _ = response => {}
        }
    };
    ([$($client:ident: $c_type:ty),+], $body:block) => {
        let config = configs::Config::new().expect("Failed while parsing config");
        let base_config = std::sync::Arc::new(config);
        let server = app::Service::new(base_config.clone()).await;
        let (server_fut, channel) =
            common::server_and_channel_stub(server, base_config)
                .await
                .expect("Failed to create the server channel");
        $(let mut $client = <$c_type as common::AutoClient>::new(channel.clone());)+
        let response = async { $body };

        tokio::select! {
            _ = server_fut => panic!("Server failed"),
            _ = response => {}
        }
    };
}
