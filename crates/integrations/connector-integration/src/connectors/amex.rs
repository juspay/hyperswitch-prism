pub mod transformers;

use std::fmt::Debug;

use base64::Engine;
use common_enums::CurrencyUnit;
use common_utils::{
    consts::{BASE64_ENGINE, NO_ERROR_CODE, NO_ERROR_MESSAGE},
    errors::CustomResult,
    events,
    ext_traits::ByteSliceExt,
};
use domain_types::{
    errors,
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, FlowStatus},
    router_response_types::Response,
    types::Connectors,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, Mask, Maskable};
use interfaces::{api::ConnectorCommon, connector_types, decode::BodyDecoding};
use serde::Serialize;
use transformers as amex;

use crate::{connectors::macros, with_error_response_body};

pub(crate) mod headers {
    pub(crate) const AUTHORIZATION: &str = "Authorization";
}

#[derive(Debug, Clone)]
pub struct Amex<T: PaymentMethodDataTypes> {
    payment_method_type: std::marker::PhantomData<T>,
}

impl<T: PaymentMethodDataTypes> Amex<T> {
    pub const fn new() -> &'static Self {
        &Self {
            payment_method_type: std::marker::PhantomData,
        }
    }
}

// =============================================================================
// FLOW IMPLEMENTATIONS
// =============================================================================
// add_connector.sh appends the flow stubs to the bottom of this file via
// `macros::macro_connector_flow_status_impls!`. Replace a stub with a real
// `macros::macro_connector_implementation!(...)` block as you implement each
// flow - see connectors/xendit.rs or connectors/travelhub.rs.

// =============================================================================
// CONNECTOR COMMON IMPLEMENTATION
// =============================================================================
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Amex<T>
{
    fn id(&self) -> &'static str {
        "amex"
    }

    // Read the vendor spec and match its wire format. There is no safe default:
    // on HEAD the split across real connectors is StringMajorUnit 34,
    // FloatMajorUnit 26, MinorUnit 21, StringMinorUnit 19 - guessing
    // `StringMinorUnit` is wrong roughly four times out of five.
    //
    // The five amount types live in crates/common/common_utils/src/types.rs:
    //   MinorUnit             - integer minor units      (1000)
    //   StringMinorUnit       - string minor units       ("1000")
    //   StringMajorUnit       - string major units       ("10.00")
    //   FloatMajorUnit        - float major units        (10.0)
    //   StringTwoDecimalUnit  - string, always 2 dp      ("10.00")
    //
    // `get_currency_unit` only says Base vs Minor; the concrete conversion type
    // is chosen in the `amount_converter` on the connector struct. Set both to
    // whatever the vendor's request/response examples actually show.
    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Minor
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, _connectors: &'a Connectors) -> &'a str {
        "https://sandbox.dummy-amex.local"
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
        let auth = amex::AmexAuthType::try_from(auth_type).change_context(
            errors::IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            },
        )?;
        let credentials = format!(
            "merchant.{}:{}",
            auth.merchant_id.expose(),
            auth.shared_secret.expose()
        );
        Ok(vec![(
            headers::AUTHORIZATION.to_string(),
            format!("Basic {}", BASE64_ENGINE.encode(credentials)).into_masked(),
        )])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<domain_types::router_data::ErrorResponse, errors::ConnectorError> {
        let response: amex::AmexErrorResponse = res
            .response
            .parse_struct("AmexErrorResponse")
            .change_context(crate::utils::response_deserialization_fail(
                res.status_code,
                "amex: error body did not match the documented error shape",
            ))?;

        with_error_response_body!(event_builder, response);

        // `attempt_status` is `Option<FlowStatus>`, and this method is shared by
        // EVERY flow of this connector - Authorize, PSync, Capture, Void, Refund,
        // RSync. Two ways to get this wrong:
        //
        //   1. Hardcoding a terminal payment failure
        //      (`Some(FlowStatus::Payment(AttemptStatus::Failure))`). A 4xx on
        //      PSync for an already-charged payment then reports a CHARGED
        //      payment as FAILURE.
        //   2. Hardcoding `None` for everything. A hard-declined refund then
        //      stays Pending and the framework retries it forever.
        //
        // Correct shape: branch on the connector's OWN error code. Set a terminal
        // status only for codes the vendor documents as final, and pick the
        // FlowStatus arm that matches the flow the code came from
        // (`FlowStatus::Refund(RefundStatus::Failure)` for refund-side codes).
        //
        // Exemplars to copy:
        //   connectors/flywire.rs  - full form, refund-aware classification
        //   connectors/noon.rs     - minimal form, one terminal code
        let attempt_status: Option<FlowStatus> = None;

        Ok(domain_types::router_data::ErrorResponse {
            status_code: res.status_code,
            // Never `.unwrap_or_default()` here: an empty `code`/`message` is
            // indistinguishable from a connector that really returned "". The
            // NO_ERROR_CODE / NO_ERROR_MESSAGE sentinels are what the rest of
            // the platform greps for.
            code: response
                .code
                .clone()
                .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
            message: response
                .message
                .clone()
                .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
            reason: response.message.clone(),
            attempt_status,
            typed_connector_response: macros::serialize_typed_connector_payload(
                &response,
                "typed_connector_response",
            ),
            // `ErrorResponse` has 13 fields and `impl Default`
            // (crates/types-traits/domain_types/src/router_data.rs). Use the
            // functional-update tail so a new field upstream does not break this.
            ..Default::default()
        })
    }
}

// =============================================================================
// BODY DECODING IMPLEMENTATION
// =============================================================================
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Amex<T>
{
}
// =============================================================================
// DYNAMICALLY GENERATED IMPLEMENTATIONS
// =============================================================================
// Auto-generated by add_connector.sh using the macro-based pattern. All flow
// traits are stubbed via `macros::macro_connector_flow_status_impls!` with
// `not_implemented` status, which emits both the marker-trait impl and a stub
// `ConnectorIntegrationV2` impl per flow.
//
// To implement a flow:
//   1. Remove that flow's name from the `not_implemented` list below.
//   2. Add a manual marker-trait impl, e.g.
//        impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
//            connector_types::PaymentAuthorizeV2<T> for Amex<T> {}
//   3. Add a `macros::macro_connector_implementation!(...)` block with the
//      flow's request/response types, HTTP method, and `get_url`/`get_headers`.
//      For a flow with NO outbound HTTP call (everything decided locally), use
//      `macros::macro_connector_local_flow_implementation!` instead - its keys
//      are connector / flow_name / resource_common_data / flow_request /
//      flow_response / handle_response / generic_type / [bounds], and it wires
//      `CallConnectorAction::HandleResponseWithoutBuildRequest` for you.
//
// See crates/integrations/connector-integration/src/connectors/xendit.rs for a
// reference implementation that follows this pattern.
// =============================================================================

// ===== CONNECTOR SERVICE TRAIT IMPLEMENTATION =====
// Aggregate trait - composes all other connector traits.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Amex<T>
{
}

// ===== BASE (NON-FLOW) TRAIT IMPLEMENTATIONS =====
// These are simple marker traits that are NOT flows and therefore have no arm
// in expand_flow_status_impl!. They must be impl'd manually.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Amex<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    interfaces::webhooks::IncomingWebhook for Amex<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Amex<T>
{
}

// ===== SOURCE VERIFICATION IMPLEMENTATION =====
// `VerifyRedirectResponse: SourceVerification + BodyDecoding`
// (crates/types-traits/interfaces/src/connector_types.rs), so both supertraits
// must be impl'd or the VerifyRedirectResponse impl above does not compile.
// SourceVerification is emitted here; BodyDecoding already comes from
// template-generation/connector.rs.template - do NOT emit a second one here,
// two impls of the same trait for the same type do not compile.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    interfaces::verification::SourceVerification for Amex<T>
{
}

// ===== PAYOUT TRAIT IMPLEMENTATIONS =====
// Emits payout marker-trait impls and default no-op ConnectorIntegrationV2
// impls for all PayoutXxxV2 flows.
crate::connectors::macros::macro_connector_payout_implementation!(
    connector: Amex,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

// ===== FLOW STATUS IMPLEMENTATIONS =====
// Emits marker-trait impls AND stub ConnectorIntegrationV2 impls for every
// flow listed. Each stub's get_url returns
// IntegrationError::connector_flow_not_implemented(...).
crate::connectors::macros::macro_connector_flow_status_impls!(
    connector: Amex,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        Accept,
        ClientAuthenticationToken,
        CreateConnectorCustomer,
        DefendDispute,
        GetConnectorCustomer,
        MandateRevoke,
        Authenticate,
        Authorize,
        Capture,
        IncrementalAuthorization,
        CreateOrder,
        PostAuthenticate,
        PreAuthenticate,
        PSync,
        PaymentMethodToken,
        VoidPC,
        Void,
        RSync,
        Refund,
        VoidPostRefund,
        RepeatPayment,
        ServerAuthenticationToken,
        ServerSessionAuthenticationToken,
        SetupMandate,
        SubmitEvidence
    ],
);
