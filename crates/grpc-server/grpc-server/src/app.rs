use axum::{extract::Request, http};
use common_utils::consts;
use external_services::shared_metrics as metrics;
use grpc_api_types::{
    health_check::health_server,
    payments::{
        composite_payment_service_server, composite_refund_service_server, customer_service_server,
        dispute_service_server, event_service_server, merchant_authentication_service_server,
        payment_method_authentication_service_server, payment_method_service_server,
        payment_service_server, recurring_payment_service_server, refund_service_server,
    },
    payouts::payout_service_server,
};
use std::{future::Future, net, sync::Arc};
use tokio::{
    signal::unix::{signal, SignalKind},
    sync::oneshot,
};
use tonic::transport::Server;
use tower_http::{request_id::MakeRequestUuid, trace as tower_trace};

use ucs_env::{configs, error::ConfigurationError, logger};

use crate::{
    config_overrides::RequestExtensionsLayer, http::config_middleware::HttpRequestExtensionsLayer,
    utils,
};

/// # Panics
///
/// Will panic if redis connection establishment fails or signal handling fails
pub async fn server_builder(config: configs::Config) -> Result<(), ConfigurationError> {
    let server_config = config.server.clone();
    let socket_addr = net::SocketAddr::new(server_config.host.parse()?, server_config.port);

    // Signal handler
    let (tx, rx) = oneshot::channel();

    #[allow(clippy::expect_used)]
    tokio::spawn(async move {
        let mut sig_int =
            signal(SignalKind::interrupt()).expect("Failed to initialize SIGINT signal handler");
        let mut sig_term =
            signal(SignalKind::terminate()).expect("Failed to initialize SIGTERM signal handler");
        let mut sig_quit =
            signal(SignalKind::quit()).expect("Failed to initialize QUIT signal handler");
        let mut sig_hup =
            signal(SignalKind::hangup()).expect("Failed to initialize SIGHUP signal handler");

        tokio::select! {
            _ = sig_int.recv() => {
                logger::info!("Received SIGINT");
                tx.send(()).expect("Failed to send SIGINT signal");
            }
            _ = sig_term.recv() => {
                logger::info!("Received SIGTERM");
                tx.send(()).expect("Failed to send SIGTERM signal");
            }
            _ = sig_quit.recv() => {
                logger::info!("Received QUIT");
                tx.send(()).expect("Failed to send QUIT signal");
            }
            _ = sig_hup.recv() => {
                logger::info!("Received SIGHUP");
                tx.send(()).expect("Failed to send SIGHUP signal");
            }
        }
    });

    #[allow(clippy::expect_used)]
    let shutdown_signal = async {
        rx.await.expect("Failed to receive shutdown signal");
        logger::info!("Shutdown signal received");
    };

    let base_config = Arc::new(config);
    let service = Service::new(Arc::clone(&base_config));

    logger::info!(host = %server_config.host, port = %server_config.port, r#type = ?server_config.type_, "starting connector service");

    match server_config.type_ {
        configs::ServiceType::Grpc => {
            Box::pin(
                service
                    .await
                    .grpc_server(base_config, socket_addr, shutdown_signal),
            )
            .await?
        }
        configs::ServiceType::Http => {
            service
                .await
                .http_server(base_config, socket_addr, shutdown_signal)
                .await?
        }
    }

    Ok(())
}

pub struct Service {
    pub health_check_service: crate::server::health_check::HealthCheck,
    pub composite_payments_service: composite_service::payments::Payments<
        crate::server::payments::Payments,
        crate::server::payments::MerchantAuthentication,
        crate::server::payments::Customer,
        crate::server::refunds::Refunds,
    >,
    pub payments_service: crate::server::payments::Payments,
    pub refunds_service: crate::server::refunds::Refunds,
    pub disputes_service: crate::server::disputes::Disputes,
    pub recurring_payment_service: crate::server::payments::RecurringPayments,
    pub event_service: crate::server::events::EventServiceImpl,
    pub payment_method_service: crate::server::payments::PaymentMethod,
    pub merchant_authentication_service: crate::server::payments::MerchantAuthentication,
    pub customer_service: crate::server::payments::Customer,
    pub payment_method_authentication_service: crate::server::payments::PaymentMethodAuthentication,
    pub payouts_service: crate::server::payouts::Payouts,
}

impl Service {
    /// # Panics
    ///
    /// Will panic if database password, hash key isn't present in configs or unable to
    /// deserialize any of the above keys
    #[allow(clippy::expect_used)]
    pub async fn new(config: Arc<configs::Config>) -> Self {
        // Initialize the global EventPublisher - logs a warning if Kafka is unavailable
        if config.events.enabled {
            common_utils::init_event_publisher(&config.events);
        } else {
            logger::info!("EventPublisher disabled in configuration");
        }

        #[cfg(feature = "connector-request-kafka")]
        if config.connector_request_kafka.enabled {
            connector_request_kafka::init_kafka_producer(&config.connector_request_kafka)
                .expect("Failed to initialize Kafka producer for publishing connector requests during startup");
        } else {
            logger::info!("Connector request Kafka disabled in configuration");
        }

        let customer_service = crate::server::payments::Customer;
        let merchant_authentication_service = crate::server::payments::MerchantAuthentication;
        let refunds_service = crate::server::refunds::Refunds;

        let payments_service = crate::server::payments::Payments {
            customer_service: customer_service.clone(),
            merchant_authentication_service: merchant_authentication_service.clone(),
        };

        let composite_payments_service = composite_service::payments::Payments::new(
            payments_service.clone(),
            merchant_authentication_service.clone(),
            customer_service.clone(),
            refunds_service.clone(),
        );

        Self {
            health_check_service: crate::server::health_check::HealthCheck,
            composite_payments_service,
            payments_service,
            refunds_service,
            disputes_service: crate::server::disputes::Disputes,
            recurring_payment_service: crate::server::payments::RecurringPayments,
            event_service: crate::server::events::EventServiceImpl,
            payment_method_service: crate::server::payments::PaymentMethod,
            merchant_authentication_service,
            customer_service,
            payment_method_authentication_service:
                crate::server::payments::PaymentMethodAuthentication,
            payouts_service: crate::server::payouts::Payouts,
        }
    }

    pub async fn http_server(
        self,
        base_config: Arc<configs::Config>,
        socket: net::SocketAddr,
        shutdown_signal: impl Future<Output = ()> + Send + 'static,
    ) -> Result<(), ConfigurationError> {
        let logging_layer = tower_trace::TraceLayer::new_for_http()
            .make_span_with(|request: &Request<_>| utils::record_fields_from_header(request))
            .on_request(tower_trace::DefaultOnRequest::new().level(tracing::Level::INFO))
            .on_response(
                tower_trace::DefaultOnResponse::new()
                    .level(tracing::Level::INFO)
                    .latency_unit(tower_http::LatencyUnit::Micros),
            )
            .on_failure(
                tower_trace::DefaultOnFailure::new()
                    .latency_unit(tower_http::LatencyUnit::Micros)
                    .level(tracing::Level::ERROR),
            );

        let request_id_layer = tower_http::request_id::SetRequestIdLayer::new(
            http::HeaderName::from_static(consts::X_REQUEST_ID),
            MakeRequestUuid,
        );

        let propagate_request_id_layer = tower_http::request_id::PropagateRequestIdLayer::new(
            http::HeaderName::from_static(consts::X_REQUEST_ID),
        );

        let config_override_layer = HttpRequestExtensionsLayer::new(base_config.clone());
        let app_state = crate::http::AppState::new(
            self.composite_payments_service,
            self.payments_service,
            self.refunds_service,
            self.disputes_service,
            self.recurring_payment_service,
            self.event_service,
            self.payment_method_service,
            self.merchant_authentication_service,
            self.customer_service,
            self.payment_method_authentication_service,
        );
        let router = crate::http::create_router(app_state)
            .layer(logging_layer)
            .layer(request_id_layer)
            .layer(propagate_request_id_layer)
            .layer(config_override_layer);

        let listener = tokio::net::TcpListener::bind(socket).await?;

        axum::serve(listener, router.into_make_service())
            .with_graceful_shutdown(shutdown_signal)
            .await?;

        Ok(())
    }

    pub async fn grpc_server(
        self,
        base_config: Arc<configs::Config>,
        socket: net::SocketAddr,
        shutdown_signal: impl Future<Output = ()>,
    ) -> Result<(), ConfigurationError> {
        let reflection_service = tonic_reflection::server::Builder::configure()
            .register_encoded_file_descriptor_set(grpc_api_types::FILE_DESCRIPTOR_SET)
            .build_v1()?;

        let logging_layer = tower_trace::TraceLayer::new_for_http()
            .make_span_with(|request: &http::request::Request<_>| {
                utils::record_fields_from_header(request)
            })
            .on_request(tower_trace::DefaultOnRequest::new().level(tracing::Level::INFO))
            .on_response(
                tower_trace::DefaultOnResponse::new()
                    .level(tracing::Level::INFO)
                    .latency_unit(tower_http::LatencyUnit::Micros),
            )
            .on_failure(
                tower_trace::DefaultOnFailure::new()
                    .latency_unit(tower_http::LatencyUnit::Micros)
                    .level(tracing::Level::ERROR),
            );

        let metrics_layer = metrics::GrpcMetricsLayer::new();

        let request_id_layer = tower_http::request_id::SetRequestIdLayer::new(
            http::HeaderName::from_static(consts::X_REQUEST_ID),
            MakeRequestUuid,
        );
        let propagate_request_id_layer = tower_http::request_id::PropagateRequestIdLayer::new(
            http::HeaderName::from_static(consts::X_REQUEST_ID),
        );
        let config_override_layer = RequestExtensionsLayer::new(base_config.clone());

        Server::builder()
            .layer(logging_layer)
            .layer(request_id_layer)
            .layer(propagate_request_id_layer)
            .layer(config_override_layer)
            .layer(metrics_layer)
            .add_service(reflection_service)
            .add_service(health_server::HealthServer::new(self.health_check_service))
            .add_service(payment_service_server::PaymentServiceServer::new(
                self.payments_service.clone(),
            ))
            .add_service(refund_service_server::RefundServiceServer::new(
                self.refunds_service,
            ))
            .add_service(dispute_service_server::DisputeServiceServer::new(
                self.disputes_service,
            ))
            .add_service(event_service_server::EventServiceServer::new(
                self.event_service,
            ))
            .add_service(
                composite_payment_service_server::CompositePaymentServiceServer::new(
                    self.composite_payments_service.clone(),
                ),
            )
            .add_service(
                composite_refund_service_server::CompositeRefundServiceServer::new(
                    self.composite_payments_service,
                ),
            )
            .add_service(customer_service_server::CustomerServiceServer::new(
                self.customer_service,
            ))
            .add_service(
                merchant_authentication_service_server::MerchantAuthenticationServiceServer::new(
                    self.merchant_authentication_service,
                ),
            )
            .add_service(payment_method_service_server::PaymentMethodServiceServer::new(
                self.payment_method_service,
            ))
            .add_service(
                recurring_payment_service_server::RecurringPaymentServiceServer::new(
                    self.recurring_payment_service,
                ),
            )
            .add_service(
                payment_method_authentication_service_server::PaymentMethodAuthenticationServiceServer::new(
                    self.payment_method_authentication_service,
                ),
            )
            .add_service(payout_service_server::PayoutServiceServer::new(
                self.payouts_service,
            ))
            .serve_with_shutdown(socket, shutdown_signal)
            .await?;

        Ok(())
    }
}

pub async fn metrics_server_builder(config: configs::Config) -> Result<(), ConfigurationError> {
    let listener = config.metrics.tcp_listener().await?;

    let router = axum::Router::new().route(
        "/metrics",
        axum::routing::get(|| async {
            let output = metrics::metrics_handler().await;
            match output {
                Ok(metrics) => Ok(metrics),
                Err(error) => {
                    tracing::error!(?error, "Error fetching metrics");

                    Err((
                        http::StatusCode::INTERNAL_SERVER_ERROR,
                        "Error fetching metrics".to_string(),
                    ))
                }
            }
        }),
    );

    axum::serve(listener, router.into_make_service())
        .with_graceful_shutdown(async {
            let output = tokio::signal::ctrl_c().await;
            tracing::error!(?output, "shutting down");
        })
        .await?;

    Ok(())
}
