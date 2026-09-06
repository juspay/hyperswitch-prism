/// Enforce correct terminal status mapping for a connector flow.
///
/// ## What it does
///
/// 1. Makes `success:` and `failure:` **syntactically mandatory** — omitting either
///    is a compile error (macro parse failure).
/// 2. Emits `const` assertions that the declared success and failure targets are
///    members of `Flow::TERMINAL_SUCCESS_SET` and `Flow::TERMINAL_FAILURE_SET`.
/// 3. Implements `ConnectorTerminalMapping<Flow>` for the connector type with
///    `type MappingContext = ()` (no context).
///
/// For connectors whose status mapping depends on extra context (e.g. a field from
/// the response that disambiguates the terminal), use `impl_flow_status_mapping_ctx!`
/// instead.
///
/// ## Syntax
///
/// For simple (non-generic) connectors:
///
/// ```rust,ignore
/// impl_flow_status_mapping! {
///     connector: Adyen,
///     flow:      connector_flow::Capture,
///     source:    AdyenStatus,
///     success:   Authorised   => Charged,
///     failure:   Refused      => CaptureFailed,
///     { Received => Pending, Error => CaptureFailed }
/// }
/// ```
///
/// For generic connectors (e.g. `Stripe<T>`), add a `generics:` key:
///
/// ```rust,ignore
/// impl_flow_status_mapping! {
///     generics:  [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
///     connector: Stripe<T>,
///     flow:      connector_flow::Void,
///     source:    StripePaymentStatus,
///     success:   Canceled              => Voided,
///     failure:   Failed                => VoidFailed,
///     { ... }
/// }
/// ```
///
/// Note: each match arm maps exactly one source variant to one target.  If the original
/// code uses `A | B => X`, split it into two entries (`A => X, B => X`).
///
/// ## Multi-terminal flows (Authorize)
///
/// Authorize has multiple valid success terminals.  Each connector picks the one it
/// produces.  `Authorized` and `Charged` are both accepted because both are in
/// `Authorize::TERMINAL_SUCCESS_SET`.
#[macro_export]
macro_rules! impl_flow_status_mapping {
    // ── with explicit generics ────────────────────────────────────────────
    (
        generics:  [ $($generic:tt)* ],
        connector: $connector:ty,
        flow:      $flow:ty,
        source:    $source:ty,

        success: $success_variant:ident => $success_target:ident,
        failure: $failure_variant:ident => $failure_target:ident,

        {
            $( $variant:ident => $target:ident ),* $(,)?
        }
    ) => {
        $crate::impl_flow_status_mapping!(
            @emit
            [$($generic)*],
            $connector, $flow, $source,
            $success_variant, $success_target,
            $failure_variant, $failure_target,
            [$( $variant => $target ),*]
        );
    };

    // ── without generics (simple connector types) ─────────────────────────
    (
        connector: $connector:ty,
        flow:      $flow:ty,
        source:    $source:ty,

        success: $success_variant:ident => $success_target:ident,
        failure: $failure_variant:ident => $failure_target:ident,

        {
            $( $variant:ident => $target:ident ),* $(,)?
        }
    ) => {
        $crate::impl_flow_status_mapping!(
            @emit
            [],
            $connector, $flow, $source,
            $success_variant, $success_target,
            $failure_variant, $failure_target,
            [$( $variant => $target ),*]
        );
    };

    // ── internal emitter ─────────────────────────────────────────────────
    (
        @emit
        [$($generic:tt)*],
        $connector:ty, $flow:ty, $source:ty,
        $success_variant:ident, $success_target:ident,
        $failure_variant:ident, $failure_target:ident,
        [$( $variant:ident => $target:ident ),*]
    ) => {
        // ── const assertions ─────────────────────────────────────────────
        const _: () = assert!(
            $crate::flow_status::const_contains(
                <$flow as $crate::flow_status::FlowStatusRules>::TERMINAL_SUCCESS_SET,
                common_enums::AttemptStatus::$success_target,
            ),
            concat!(
                "impl_flow_status_mapping: success target `AttemptStatus::",
                stringify!($success_target),
                "` is not in the flow's TERMINAL_SUCCESS_SET"
            )
        );

        const _: () = assert!(
            $crate::flow_status::const_contains(
                <$flow as $crate::flow_status::FlowStatusRules>::TERMINAL_FAILURE_SET,
                common_enums::AttemptStatus::$failure_target,
            ),
            concat!(
                "impl_flow_status_mapping: failure target `AttemptStatus::",
                stringify!($failure_target),
                "` is not in the flow's TERMINAL_FAILURE_SET"
            )
        );

        // ── ConnectorTerminalMapping impl ─────────────────────────────────

        impl<$($generic)*> $crate::flow_status::ConnectorTerminalMapping<$flow> for $connector {
            type ConnectorStatus = $source;
            type MappingContext = ();

            fn success_connector_status() -> $source {
                <$source>::$success_variant
            }

            fn failure_connector_status() -> $source {
                <$source>::$failure_variant
            }

            fn map_attempt_status(status: $source, _: ()) -> common_enums::AttemptStatus {
                match status {
                    <$source>::$success_variant => common_enums::AttemptStatus::$success_target,
                    <$source>::$failure_variant => common_enums::AttemptStatus::$failure_target,
                    $(
                        <$source>::$variant => common_enums::AttemptStatus::$target,
                    )*
                }
            }
        }
    };
}

/// Context-aware variant of `impl_flow_status_mapping!`.
///
/// Use when the `AttemptStatus` cannot be determined from the connector status alone —
/// e.g. Nexinets returns `NexinetsPaymentStatus::Success` for both manual-capture
/// (→ `Authorized`) and auto-capture (→ `Charged`) Authorize responses.  The
/// disambiguating field (`NexinetsTransactionType`) is the context.
///
/// ## Syntax
///
/// The `params:` key names the two function parameters (`status` and `ctx` by
/// convention) so that the body block sees them without hygiene issues.
///
/// ```rust,ignore
/// impl_flow_status_mapping_ctx! {
///     generics:       [T: PaymentMethodDataTypes + Debug + ...],
///     connector:      Nexinets<T>,
///     flow:           domain_types::connector_flow::Authorize,
///     source:         nexinets::NexinetsPaymentStatus,
///     context:        nexinets::NexinetsTransactionType,
///
///     // Names for the two function parameters — must match what `body` references.
///     params: [status, ctx],
///
///     // The representative success connector status (used by assert_terminal_mapping!).
///     success_status:  Success,
///     // ALL AttemptStatus values the body can produce for success — each gets a
///     // const assertion against TERMINAL_SUCCESS_SET.
///     success_targets: [Authorized, Charged],
///
///     // The representative failure connector status.
///     failure_status:  Declined,
///     // The AttemptStatus the canonical failure path produces.
///     failure_target:  AuthorizationFailed,
///
///     // The full map_attempt_status body.
///     {
///         match (status, ctx) {
///             (nexinets::NexinetsPaymentStatus::Success,
///              nexinets::NexinetsTransactionType::Preauth) => AttemptStatus::Authorized,
///             ...
///         }
///     }
/// }
/// ```
///
/// `MappingContext` must implement `Default`.  The default value represents the
/// canonical context used by `assert_terminal_mapping!` when testing the success path.
#[macro_export]
macro_rules! impl_flow_status_mapping_ctx {
    // ── with generics ────────────────────────────────────────────────────
    (
        generics:        [ $($generic:tt)* ],
        connector:       $connector:ty,
        flow:            $flow:ty,
        source:          $source:ty,
        context:         $ctx:ty,

        params:          [$status_name:ident, $ctx_name:ident],

        success_status:  $success_variant:ident,
        success_targets: [ $($success_target:ident),+ $(,)? ],

        failure_status:  $failure_variant:ident,
        failure_target:  $failure_target:ident,

        $body:block
    ) => {
        $crate::impl_flow_status_mapping_ctx!(
            @emit
            [$($generic)*],
            $connector, $flow, $source, $ctx,
            $status_name, $ctx_name,
            $success_variant, [$($success_target),+],
            $failure_variant, $failure_target,
            $body
        );
    };

    // ── without generics ─────────────────────────────────────────────────
    (
        connector:       $connector:ty,
        flow:            $flow:ty,
        source:          $source:ty,
        context:         $ctx:ty,

        params:          [$status_name:ident, $ctx_name:ident],

        success_status:  $success_variant:ident,
        success_targets: [ $($success_target:ident),+ $(,)? ],

        failure_status:  $failure_variant:ident,
        failure_target:  $failure_target:ident,

        $body:block
    ) => {
        $crate::impl_flow_status_mapping_ctx!(
            @emit
            [],
            $connector, $flow, $source, $ctx,
            $status_name, $ctx_name,
            $success_variant, [$($success_target),+],
            $failure_variant, $failure_target,
            $body
        );
    };

    // ── internal emitter ─────────────────────────────────────────────────
    (
        @emit
        [$($generic:tt)*],
        $connector:ty, $flow:ty, $source:ty, $ctx:ty,
        $status_name:ident, $ctx_name:ident,
        $success_variant:ident, [$($success_target:ident),+],
        $failure_variant:ident, $failure_target:ident,
        $body:block
    ) => {
        // Verify every declared success target is in the flow's TERMINAL_SUCCESS_SET.
        $(
            const _: () = assert!(
                $crate::flow_status::const_contains(
                    <$flow as $crate::flow_status::FlowStatusRules>::TERMINAL_SUCCESS_SET,
                    common_enums::AttemptStatus::$success_target,
                ),
                concat!(
                    "impl_flow_status_mapping_ctx: success target `AttemptStatus::",
                    stringify!($success_target),
                    "` is not in the flow's TERMINAL_SUCCESS_SET"
                )
            );
        )+

        const _: () = assert!(
            $crate::flow_status::const_contains(
                <$flow as $crate::flow_status::FlowStatusRules>::TERMINAL_FAILURE_SET,
                common_enums::AttemptStatus::$failure_target,
            ),
            concat!(
                "impl_flow_status_mapping_ctx: failure target `AttemptStatus::",
                stringify!($failure_target),
                "` is not in the flow's TERMINAL_FAILURE_SET"
            )
        );

        impl<$($generic)*> $crate::flow_status::ConnectorTerminalMapping<$flow> for $connector {
            type ConnectorStatus = $source;
            type MappingContext = $ctx;

            fn success_connector_status() -> $source {
                <$source>::$success_variant
            }

            fn failure_connector_status() -> $source {
                <$source>::$failure_variant
            }

            // $status_name and $ctx_name are captured identifiers from the call site,
            // sharing hygiene context with $body — no hygiene mismatch.
            fn map_attempt_status(
                $status_name: $source,
                $ctx_name: $ctx,
            ) -> common_enums::AttemptStatus {
                $body
            }
        }
    };
}

/// Test-time verification that the declared terminal mappings are correct.
///
/// Generates a `#[test]` function that calls `success_connector_status()` and
/// `failure_connector_status()`, maps them through `map_attempt_status()` with an
/// explicit context value, and asserts they land in `TERMINAL_SUCCESS_SET` /
/// `TERMINAL_FAILURE_SET`.
///
/// Pass `()` as `$ctx` for connectors with `MappingContext = ()`, and a concrete
/// value for context-dependent connectors.  Pass a unique `$test_name` per invocation:
///
/// ```rust,ignore
/// // Simple connector — no context
/// assert_terminal_mapping!(Stripe<Card>, connector_flow::Capture, (), capture_test);
/// assert_terminal_mapping!(Stripe<Card>, connector_flow::Void,    (), void_test);
///
/// // Context-dependent connector — explicit canonical context for each path
/// assert_terminal_mapping!(
///     Nexinets<Card>,
///     connector_flow::Authorize,
///     NexinetsTransactionType::Preauth,   // canonical context (manual-capture path)
///     nexinets_authorize_manual_test,
/// );
/// assert_terminal_mapping!(
///     Nexinets<Card>,
///     connector_flow::Authorize,
///     NexinetsTransactionType::Debit,     // auto-capture path — success maps to Charged
///     nexinets_authorize_auto_test,
/// );
/// ```
#[macro_export]
macro_rules! assert_terminal_mapping {
    ($connector:ty, $flow:ty, $ctx:expr, $test_name:ident) => {
        #[test]
        fn $test_name() {
            use $crate::flow_status::{ConnectorTerminalMapping, FlowStatusRules};

            let success_input =
                <$connector as ConnectorTerminalMapping<$flow>>::success_connector_status();
            let mapped = <$connector as ConnectorTerminalMapping<$flow>>::map_attempt_status(
                success_input,
                $ctx,
            );
            assert!(
                <$flow as FlowStatusRules>::TERMINAL_SUCCESS_SET.contains(&mapped),
                "{}: success_connector_status() maps to {:?}, \
                 which is not in TERMINAL_SUCCESS_SET {:?}",
                stringify!($connector),
                mapped,
                <$flow as FlowStatusRules>::TERMINAL_SUCCESS_SET,
            );

            let failure_input =
                <$connector as ConnectorTerminalMapping<$flow>>::failure_connector_status();
            let mapped = <$connector as ConnectorTerminalMapping<$flow>>::map_attempt_status(
                failure_input,
                $ctx,
            );
            assert!(
                <$flow as FlowStatusRules>::TERMINAL_FAILURE_SET.contains(&mapped),
                "{}: failure_connector_status() maps to {:?}, \
                 which is not in TERMINAL_FAILURE_SET {:?}",
                stringify!($connector),
                mapped,
                <$flow as FlowStatusRules>::TERMINAL_FAILURE_SET,
            );
        }
    };
}
