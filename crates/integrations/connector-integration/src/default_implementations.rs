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
    connector_flow::{
        CreatePaymentMethod, GetPaymentMethod, PaymentMethodEligibility, Recharge,
        VerifyWebhookSource,
    },
    connector_types::{
        CreatePaymentMethodData, CreatePaymentMethodResponseData, GetPaymentMethodData,
        GetPaymentMethodResponseData, PaymentFlowData, PaymentMethodEligibilityData,
        PaymentMethodEligibilityResponse, RechargeRequestData, RechargeResponseData,
        VerifyWebhookSourceFlowData,
    },
    errors::IntegrationError,
    payment_method_data::PaymentMethodDataTypes,
    router_data_v2::RouterDataV2,
    router_request_types::VerifyWebhookSourceRequestData,
    router_response_types::VerifyWebhookSourceResponseData,
};
use interfaces::connector_integration_v2::ConnectorIntegrationV2;
use interfaces::connector_types::{
    CreatePaymentMethodV2, GetPaymentMethodV2, PaymentMethodEligibilityV2, RechargeV2,
    VerifyWebhookSourceV2,
};

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
macro_rules! default_impl_payment_method_eligibility_v2_single {
    ($connector:ident, $err_helper:ident) => {
        impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + serde::Serialize>
            PaymentMethodEligibilityV2 for $connector<T>
        {
        }

        impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + serde::Serialize>
            ConnectorIntegrationV2<
                PaymentMethodEligibility,
                PaymentFlowData,
                PaymentMethodEligibilityData,
                PaymentMethodEligibilityResponse,
            > for $connector<T>
        {
            fn get_url(
                &self,
                _req: &RouterDataV2<
                    PaymentMethodEligibility,
                    PaymentFlowData,
                    PaymentMethodEligibilityData,
                    PaymentMethodEligibilityResponse,
                >,
            ) -> CustomResult<String, IntegrationError> {
                Err(::domain_types::errors::IntegrationError::$err_helper(
                    ::interfaces::api::ConnectorCommon::id(self),
                    "eligibility",
                    ::domain_types::errors::IntegrationErrorContext::default(),
                )
                .into())
            }
        }
    };
}

#[macro_export]
macro_rules! default_impl_payment_method_eligibility_v2 {
    (
        $( not_supported: [ $($ns:ident),* $(,)? ] $(,)? )?
        $( not_implemented: [ $($ni:ident),* $(,)? ] $(,)? )?
    ) => {
        $( $( $crate::default_impl_payment_method_eligibility_v2_single!(
            $ns, connector_flow_not_supported
        ); )* )?
        $( $( $crate::default_impl_payment_method_eligibility_v2_single!(
            $ni, connector_flow_not_implemented
        ); )* )?
    };
}

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
        Dlocal,
        Fiuu,
        Imerchantsolutions,
        Noon,
        Novalnet,
        Payload,
        Phonepe,
        Ppro,
        Revolut,
        Tamara,
        AbsaSanlam,
        Trustly,
        Trustpay,
        Worldpayvantiv,
        Qwikcilver,
    ],
    not_implemented: [
        Aci,
        Airwallex,
        Authipay,
        Axisbank,
        Deutschebank,
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
        Easebuzz,
        Elavon,
        Finix,
        Fiserv,
        Fiservcommercehub,
        Fiservemea,
        Flywire,
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
        TsysTransit,
        TwocTwopPaco,
        Volt,
        Wellsfargo,
        Worldpay,
        Worldpayxml,
        Xendit,
        Zift,
        Juspay,
        Payconex,
        Kount,
        Hyperswitch,
        Affirm,
    ],
);
// PayPal has its own implementation in paypal.rs

// ============================================================================
// RechargeV2 default impls
//
// The `core_changes_fro_recharge` work made `RechargeV2` a supertrait of
// `ConnectorServiceTrait`. Until each connector wires Recharge for real, we
// give every connector an empty `RechargeV2` impl plus a stub
// `ConnectorIntegrationV2<Recharge, ...>` whose `get_url` returns
// `connector_flow_not_implemented`. This mirrors `VerifyWebhookSourceV2` and
// keeps the workspace buildable while individual connectors opt in.
//
// Connectors that DO implement Recharge in their own file (none yet) should
// be removed from this list — Rust's coherence rules forbid two impls of the
// same trait for the same type.
// ============================================================================

#[macro_export]
macro_rules! default_impl_recharge_v2_single {
    ($connector:ident) => {
        impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + serde::Serialize>
            RechargeV2 for $connector<T>
        {
        }

        impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + serde::Serialize>
            ConnectorIntegrationV2<
                Recharge,
                PaymentFlowData,
                RechargeRequestData,
                RechargeResponseData,
            > for $connector<T>
        {
            fn get_url(
                &self,
                _req: &RouterDataV2<
                    Recharge,
                    PaymentFlowData,
                    RechargeRequestData,
                    RechargeResponseData,
                >,
            ) -> CustomResult<String, IntegrationError> {
                Err(::domain_types::errors::IntegrationError::connector_flow_not_implemented(
                    ::interfaces::api::ConnectorCommon::id(self),
                    "recharge",
                    ::domain_types::errors::IntegrationErrorContext::default(),
                )
                .into())
            }

            fn build_request_v2(
                &self,
                _req: &RouterDataV2<
                    Recharge,
                    PaymentFlowData,
                    RechargeRequestData,
                    RechargeResponseData,
                >,
            ) -> CustomResult<Option<Request>, IntegrationError> {
                Ok(None)
            }
        }
    };
}

#[macro_export]
macro_rules! default_impl_recharge_v2 {
    ( $( $connector:ident ),* $(,)? ) => {
        $( $crate::default_impl_recharge_v2_single!($connector); )*
    };
}

default_impl_recharge_v2!(
    AbsaSanlam,
    Aci,
    Kount,
    Adyen,
    Airwallex,
    Authipay,
    Authorizedotnet,
    Axisbank,
    Bambora,
    Bamboraapac,
    Bankofamerica,
    Barclaycard,
    Billwerk,
    Bluesnap,
    Braintree,
    Calida,
    Cashfree,
    Cashtocode,
    Celero,
    Checkout,
    Cryptopay,
    Cybersource,
    Datatrans,
    Deutschebank,
    Dlocal,
    Easebuzz,
    Elavon,
    Finix,
    Fiserv,
    Fiservcommercehub,
    Fiservemea,
    Fiuu,
    Flywire,
    Forte,
    Getnet,
    Gigadat,
    Globalpay,
    Helcim,
    Hipay,
    Hyperpg,
    Iatapay,
    Imerchantsolutions,
    Itaubank,
    Jpmorgan,
    Juspay,
    Loonio,
    Mifinity,
    Mollie,
    Multisafepay,
    Nexinets,
    Nexixpay,
    Nmi,
    Noon,
    Novalnet,
    Nuvei,
    Paybox,
    Payconex,
    Payload,
    Payme,
    Paypal,
    Paysafe,
    Paytm,
    Payu,
    Peachpayments,
    Phonepe,
    PinelabsOnline,
    Placetopay,
    Powertranz,
    Ppro,
    Rapyd,
    Razorpay,
    RazorpayV2,
    Redsys,
    Revolut,
    Revolv3,
    Shift4,
    Silverflow,
    Stax,
    Stripe,
    Tamara,
    Hyperswitch,
    Affirm,
    Truelayer,
    Trustly,
    Trustpay,
    Trustpayments,
    Tsys,
    TsysTransit,
    TwocTwopPaco,
    Volt,
    Wellsfargo,
    Worldpay,
    Worldpayvantiv,
    Worldpayxml,
    Xendit,
    Zift,
);

// ============================================================================
// CreatePaymentMethod / GetPaymentMethod default impls
//
// Same pattern as `default_impl_recharge_v2!` — the two traits are now
// supertraits of `ConnectorServiceTrait`, so every connector needs an impl.
// Connectors that wire a real implementation in their own file opt out by
// being removed from the lists below.
// ============================================================================

#[macro_export]
macro_rules! default_impl_create_payment_method_v2_single {
    ($connector:ident) => {
        impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + serde::Serialize>
            CreatePaymentMethodV2 for $connector<T>
        {
        }

        impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + serde::Serialize>
            ConnectorIntegrationV2<
                CreatePaymentMethod,
                PaymentFlowData,
                CreatePaymentMethodData,
                CreatePaymentMethodResponseData,
            > for $connector<T>
        {
            fn get_url(
                &self,
                _req: &RouterDataV2<
                    CreatePaymentMethod,
                    PaymentFlowData,
                    CreatePaymentMethodData,
                    CreatePaymentMethodResponseData,
                >,
            ) -> CustomResult<String, IntegrationError> {
                Err(::domain_types::errors::IntegrationError::connector_flow_not_implemented(
                    ::interfaces::api::ConnectorCommon::id(self),
                    "create_payment_method",
                    ::domain_types::errors::IntegrationErrorContext::default(),
                )
                .into())
            }

            fn build_request_v2(
                &self,
                _req: &RouterDataV2<
                    CreatePaymentMethod,
                    PaymentFlowData,
                    CreatePaymentMethodData,
                    CreatePaymentMethodResponseData,
                >,
            ) -> CustomResult<Option<Request>, IntegrationError> {
                Ok(None)
            }
        }
    };
}

#[macro_export]
macro_rules! default_impl_create_payment_method_v2 {
    ( $( $connector:ident ),* $(,)? ) => {
        $( $crate::default_impl_create_payment_method_v2_single!($connector); )*
    };
}

#[macro_export]
macro_rules! default_impl_get_payment_method_v2_single {
    ($connector:ident) => {
        impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + serde::Serialize>
            GetPaymentMethodV2 for $connector<T>
        {
        }

        impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + serde::Serialize>
            ConnectorIntegrationV2<
                GetPaymentMethod,
                PaymentFlowData,
                GetPaymentMethodData,
                GetPaymentMethodResponseData,
            > for $connector<T>
        {
            fn get_url(
                &self,
                _req: &RouterDataV2<
                    GetPaymentMethod,
                    PaymentFlowData,
                    GetPaymentMethodData,
                    GetPaymentMethodResponseData,
                >,
            ) -> CustomResult<String, IntegrationError> {
                Err(::domain_types::errors::IntegrationError::connector_flow_not_implemented(
                    ::interfaces::api::ConnectorCommon::id(self),
                    "get_payment_method",
                    ::domain_types::errors::IntegrationErrorContext::default(),
                )
                .into())
            }

            fn build_request_v2(
                &self,
                _req: &RouterDataV2<
                    GetPaymentMethod,
                    PaymentFlowData,
                    GetPaymentMethodData,
                    GetPaymentMethodResponseData,
                >,
            ) -> CustomResult<Option<Request>, IntegrationError> {
                Ok(None)
            }
        }
    };
}

#[macro_export]
macro_rules! default_impl_get_payment_method_v2 {
    ( $( $connector:ident ),* $(,)? ) => {
        $( $crate::default_impl_get_payment_method_v2_single!($connector); )*
    };
}

// Same connector universe as default_impl_recharge_v2! above.
default_impl_create_payment_method_v2!(
    AbsaSanlam,
    Aci,
    Kount,
    Adyen,
    Airwallex,
    Authipay,
    Authorizedotnet,
    Axisbank,
    Bambora,
    Bamboraapac,
    Bankofamerica,
    Barclaycard,
    Billwerk,
    Bluesnap,
    Braintree,
    Calida,
    Cashfree,
    Cashtocode,
    Celero,
    Checkout,
    Cryptopay,
    Cybersource,
    Datatrans,
    Deutschebank,
    Dlocal,
    Easebuzz,
    Elavon,
    Finix,
    Fiserv,
    Fiservcommercehub,
    Fiservemea,
    Fiuu,
    Flywire,
    Forte,
    Getnet,
    Gigadat,
    Globalpay,
    Helcim,
    Hipay,
    Hyperpg,
    Iatapay,
    Imerchantsolutions,
    Itaubank,
    Jpmorgan,
    Juspay,
    Loonio,
    Mifinity,
    Mollie,
    Multisafepay,
    Nexinets,
    Nexixpay,
    Nmi,
    Noon,
    Novalnet,
    Nuvei,
    Paybox,
    Payconex,
    Payload,
    Payme,
    Paypal,
    Paysafe,
    Paytm,
    Payu,
    Peachpayments,
    Phonepe,
    PinelabsOnline,
    Placetopay,
    Powertranz,
    Ppro,
    Rapyd,
    Razorpay,
    RazorpayV2,
    Redsys,
    Revolut,
    Revolv3,
    Shift4,
    Silverflow,
    Stax,
    Stripe,
    Tamara,
    Hyperswitch,
    Affirm,
    Truelayer,
    Trustly,
    Trustpay,
    Trustpayments,
    Tsys,
    TsysTransit,
    TwocTwopPaco,
    Volt,
    Wellsfargo,
    Worldpay,
    Worldpayvantiv,
    Worldpayxml,
    Xendit,
    Zift,
);

default_impl_get_payment_method_v2!(
    AbsaSanlam,
    Aci,
    Kount,
    Adyen,
    Airwallex,
    Authipay,
    Authorizedotnet,
    Axisbank,
    Bambora,
    Bamboraapac,
    Bankofamerica,
    Barclaycard,
    Billwerk,
    Bluesnap,
    Braintree,
    Calida,
    Cashfree,
    Cashtocode,
    Celero,
    Checkout,
    Cryptopay,
    Cybersource,
    Datatrans,
    Deutschebank,
    Dlocal,
    Easebuzz,
    Elavon,
    Finix,
    Fiserv,
    Fiservcommercehub,
    Fiservemea,
    Fiuu,
    Flywire,
    Forte,
    Getnet,
    Gigadat,
    Globalpay,
    Helcim,
    Hipay,
    Hyperpg,
    Iatapay,
    Imerchantsolutions,
    Itaubank,
    Jpmorgan,
    Juspay,
    Loonio,
    Mifinity,
    Mollie,
    Multisafepay,
    Nexinets,
    Nexixpay,
    Nmi,
    Noon,
    Novalnet,
    Nuvei,
    Paybox,
    Payconex,
    Payload,
    Payme,
    Paypal,
    Paysafe,
    Paytm,
    Payu,
    Peachpayments,
    Phonepe,
    PinelabsOnline,
    Placetopay,
    Powertranz,
    Ppro,
    Rapyd,
    Razorpay,
    RazorpayV2,
    Redsys,
    Revolut,
    Revolv3,
    Shift4,
    Silverflow,
    Stax,
    Stripe,
    Tamara,
    Hyperswitch,
    Affirm,
    Truelayer,
    Trustly,
    Trustpay,
    Trustpayments,
    Tsys,
    TsysTransit,
    TwocTwopPaco,
    Volt,
    Wellsfargo,
    Worldpay,
    Worldpayvantiv,
    Worldpayxml,
    Xendit,
    Zift,
);

default_impl_payment_method_eligibility_v2!(
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
        AbsaSanlam,
        Trustly,
        Trustpay,
        Worldpayvantiv,
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
        Payconex,
        Payme,
        Paysafe,
        Paytm,
        Payu,
        Peachpayments,
        PinelabsOnline,
        Placetopay,
        Powertranz,
        Qwikcilver,
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
        TsysTransit,
        TwocTwopPaco,
        Volt,
        Wellsfargo,
        Worldpay,
        Worldpayxml,
        Xendit,
        Zift,
        Juspay,
        Paypal,
        Truelayer,
        Hyperswitch,
        Affirm,
    ],
);
