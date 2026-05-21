//! Default implementations for optional connector traits
//!
//! This file provides empty implementations for traits that are required by `ConnectorServiceTrait`
//! but not all connectors need to implement. Connectors that need specific implementations can
//! override these by implementing the trait in their own file (Rust will use the more specific impl).
//!
//! Pattern: When adding a new connector, add it to the macro invocation below in whichever bucket
//! reflects the gateway's reality:
//!   - `not_supported`   — gateway has no webhook-signing surface at all.
//!   - `not_implemented` — gateway DOES support webhook signing, but the URL-based dispatch path
//!     is not wired up here (verification, if any, lives in `IncomingWebhook`).
//!
//! If a connector needs a real implementation, add it in the connector's own file.

use crate::connectors::*;
use common_utils::{request::Request, CustomResult};
use domain_types::{
    connector_flow::VerifyWebhookSource, connector_types::VerifyWebhookSourceFlowData,
    errors::IntegrationError, payment_method_data::PaymentMethodDataTypes,
    router_data_v2::RouterDataV2, router_request_types::VerifyWebhookSourceRequestData,
    router_response_types::VerifyWebhookSourceResponseData,
};
use interfaces::connector_integration_v2::ConnectorIntegrationV2;
use interfaces::connector_types::VerifyWebhookSourceV2;

/// Inner helper: emit the `VerifyWebhookSourceV2` + `ConnectorIntegrationV2` default impls
/// for a single connector, routing `get_url` to the chosen `IntegrationError` constructor
/// (`connector_flow_not_supported` or `connector_flow_not_implemented`).
///
/// `build_request_v2` returns `Ok(None)` so the `get_url` `Err` is unreachable in normal
/// dispatch; it only fires if a future refactor bypasses `build_request_v2`.
#[macro_export]
macro_rules! default_impl_verify_webhook_source_v2_single {
    ($connector:ident, $err_helper:ident) => {
        impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + serde::Serialize>
            VerifyWebhookSourceV2 for $connector<T>
        {
        }

        impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + serde::Serialize>
            ConnectorIntegrationV2<
                VerifyWebhookSource,
                VerifyWebhookSourceFlowData,
                VerifyWebhookSourceRequestData,
                VerifyWebhookSourceResponseData,
            > for $connector<T>
        {
            fn get_url(
                &self,
                _req: &RouterDataV2<
                    VerifyWebhookSource,
                    VerifyWebhookSourceFlowData,
                    VerifyWebhookSourceRequestData,
                    VerifyWebhookSourceResponseData,
                >,
            ) -> CustomResult<String, IntegrationError> {
                Err(::domain_types::errors::IntegrationError::$err_helper(
                    ::interfaces::api::ConnectorCommon::id(self),
                    "verify_webhook_source",
                    ::domain_types::errors::IntegrationErrorContext::default(),
                )
                .into())
            }

            fn build_request_v2(
                &self,
                _req: &RouterDataV2<
                    VerifyWebhookSource,
                    VerifyWebhookSourceFlowData,
                    VerifyWebhookSourceRequestData,
                    VerifyWebhookSourceResponseData,
                >,
            ) -> CustomResult<Option<Request>, IntegrationError> {
                Ok(None)
            }
        }
    };
}

/// User-facing entry: accepts two optional buckets — `not_supported` and `not_implemented` —
/// and dispatches each connector to the corresponding error helper. Mirrors the bucketed-list
/// shape of `macro_connector_flow_status_impls!`.
///
/// Usage:
/// ```ignore
/// default_impl_verify_webhook_source_v2!(
///     not_supported: [ Loonio, Gigadat ],
///     not_implemented: [ Adyen, Stripe ],
/// );
/// ```
#[macro_export]
macro_rules! default_impl_verify_webhook_source_v2 {
    (
        $( not_supported: [ $($ns:ident),* $(,)? ] $(,)? )?
        $( not_implemented: [ $($ni:ident),* $(,)? ] $(,)? )?
    ) => {
        $( $( $crate::default_impl_verify_webhook_source_v2_single!(
            $ns, connector_flow_not_supported
        ); )* )?
        $( $( $crate::default_impl_verify_webhook_source_v2_single!(
            $ni, connector_flow_not_implemented
        ); )* )?
    };
}

// Generate default implementations for all connectors that don't have custom implementations.
// Connectors with real implementations (like PayPal) will override these.
//
// Partition rationale (evidence-based, not by-spec):
//
// - `not_supported`: connectors that have a real `IncomingWebhook::verify_webhook_source`
//   impl in their own .rs file. Their gateway DOES sign webhooks, but verification happens
//   inline via the `IncomingWebhook` trait rather than via this URL-based dispatch path —
//   so the URL-based default genuinely isn't a supported path for them. Surface as
//   `FLOW_NOT_SUPPORTED`.
//
// - `not_implemented`: connectors that have NO `verify_webhook_source` impl anywhere.
//   Either the gateway lacks webhook signing or no one has wired it up. Surface as
//   `NOT_IMPLEMENTED` so future work shows up in the right TODO bucket. A future audit
//   can move individual connectors from here into `not_supported` once it's confirmed
//   that their gateway has no signing surface at all.
default_impl_verify_webhook_source_v2!(
    not_supported: [
        Adyen,
        Authorizedotnet,
        Bluesnap,
        Calida,
        Cashtocode,
        Cryptopay,
        Fiuu,
        Imerchantsolutions,
        Noon,
        Novalnet,
        Payload,
        Phonepe,
        Ppro,
        Revolut,
        Sanlam,
        Trustly,
        Trustpay,
        Worldpayvantiv,
    ],
    not_implemented: [
        Aci,
        Airwallex,
        Authipay,
        Axisbank,
        Bambora,
        Bamboraapac,
        Bankofamerica,
        Barclaycard,
        Billwerk,
        Braintree,
        Cashfree,
        Celero,
        Checkout,
        Cybersource,
        Datatrans,
        Dlocal,
        Easebuzz,
        Elavon,
        Finix,
        Fiserv,
        Fiservcommercehub,
        Fiservemea,
        Forte,
        Getnet,
        Gigadat,
        Globalpay,
        Helcim,
        Hipay,
        Hyperpg,
        Iatapay,
        Itaubank,
        Jpmorgan,
        Loonio,
        Mifinity,
        Mollie,
        Multisafepay,
        Nexinets,
        Nexixpay,
        Nmi,
        Nuvei,
        Paybox,
        Payme,
        Paysafe,
        Paytm,
        Payu,
        Peachpayments,
        PinelabsOnline,
        Placetopay,
        Powertranz,
        Rapyd,
        Razorpay,
        RazorpayV2,
        Redsys,
        Revolv3,
        Shift4,
        Silverflow,
        Stax,
        Stripe,
        Trustpayments,
        Tsys,
        TwocTwopPaco,
        Volt,
        Wellsfargo,
        Worldpay,
        Worldpayxml,
        Xendit,
        Zift,
    ],
);
// PayPal has its own implementation in paypal.rs
