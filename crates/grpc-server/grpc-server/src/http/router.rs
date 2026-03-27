use axum::{
    routing::{get, post},
    Router,
};

use super::{handlers, state::AppState};

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(handlers::health::health))
        .route(
            "/composite/payments/authorize",
            post(handlers::composite::payments::authorize),
        )
        .route(
            "/composite/payments/get",
            post(handlers::composite::payments::get),
        )
        .route(
            "/composite/refunds/refund",
            post(handlers::composite::payments::refund),
        )
        .route(
            "/composite/refunds/get",
            post(handlers::composite::refunds::refund_get),
        )
        .route(
            "/composite/payments/void",
            post(handlers::composite::payments::void),
        )
        .route(
            "/composite/payments/capture",
            post(handlers::composite::payments::capture),
        )
        .route("/payments/authorize", post(handlers::payments::authorize))
        // .route(
        //     "/payments/authorize_only",
        //     post(handlers::payments::authorize_only),
        // )
        .route("/payments/capture", post(handlers::payments::capture))
        .route("/payments/void", post(handlers::payments::void))
        .route(
            "/payments/void_post_capture",
            post(handlers::payments::void_post_capture),
        )
        .route("/payments/get", post(handlers::payments::get_payment))
        .route(
            "/payments/create_order",
            post(handlers::payments::create_order),
        )
        .route(
            "/payments/create_session_token",
            post(handlers::payments::create_session_token),
        )
        .route(
            "/payments/create_connector_customer",
            post(handlers::payments::create_connector_customer),
        )
        .route(
            "/payments/create_payment_method_token",
            post(handlers::payments::create_payment_method_token),
        )
        .route(
            "/payments/register",
            post(handlers::payments::setup_recurring),
        )
        // .route(
        //     "/payments/register_only",
        //     post(handlers::payments::register_only),
        // )
        .route(
            "/payments/repeat_everything",
            post(handlers::payments::repeat_everything),
        )
        .route("/payments/refund", post(handlers::payments::refund))
        // .route("/payments/dispute", post(handlers::payments::dispute))
        .route(
            "/payments/pre_authenticate",
            post(handlers::payments::pre_authenticate),
        )
        .route(
            "/payments/authenticate",
            post(handlers::payments::authenticate),
        )
        .route(
            "/payments/post_authenticate",
            post(handlers::payments::post_authenticate),
        )
        .route(
            "/payments/create_access_token",
            post(handlers::payments::create_access_token),
        )
        .route("/payments/transform", post(handlers::payments::transform))
        .route(
            "/payments/verify_redirect_response",
            post(handlers::payments::verify_redirect_response),
        )
        // RefundService routes
        .route("/refunds/get", post(handlers::refunds::get_refund))
        .route(
            "/refunds/transform",
            post(handlers::refunds::transform_refund),
        )
        // DisputeService routes
        .route(
            "/disputes/submit_evidence",
            post(handlers::disputes::submit_evidence),
        )
        .route("/disputes/get", post(handlers::disputes::get_dispute))
        .route("/disputes/defend", post(handlers::disputes::defend_dispute))
        .route("/disputes/accept", post(handlers::disputes::accept_dispute))
        .route(
            "/disputes/transform",
            post(handlers::disputes::transform_dispute),
        )
        .with_state(state)
}
