use axum::{
    middleware,
    routing::{get, post},
    Router,
};

use crate::{auth, handlers, state::AppState};

pub fn build_router(state: AppState) -> Router {
    // Routes that require Bearer auth (everything API-facing).
    let authed = Router::new()
        .route("/v1/payment_intents", post(handlers::authorize::create))
        .route(
            "/v1/payment_intents/:id",
            get(handlers::sync::payment_intent),
        )
        .route(
            "/v1/payment_intents/:id/capture",
            post(handlers::capture::capture),
        )
        .route(
            "/v1/payment_intents/:id/cancel",
            post(handlers::void::cancel),
        )
        .route(
            "/v1/payment_intents/:id/increment_authorization",
            post(handlers::capture::increment_authorization),
        )
        .route("/v1/setup_intents", post(handlers::authorize::create_setup))
        .route("/v1/setup_intents/:id", get(handlers::sync::setup_intent))
        .route(
            "/v1/payment_methods",
            post(handlers::authorize::create_payment_method),
        )
        .route("/v1/customers", post(handlers::authorize::create_customer))
        .route("/v1/refunds", post(handlers::refund::create))
        .route("/v1/refunds/:id", get(handlers::refund_sync::get))
        .route(
            "/admin/trigger-webhook",
            post(handlers::webhook_trigger::trigger),
        )
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth::bearer_auth,
        ));

    // Public — browser-facing redirect-completion page.
    let public = Router::new().route("/redirect/:attempt_id", get(handlers::redirect::page));

    Router::new()
        .nest("/dummy", authed.merge(public))
        .with_state(state)
}
