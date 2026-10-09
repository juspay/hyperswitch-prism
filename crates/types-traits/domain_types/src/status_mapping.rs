//! Typed connector-status mapping declarations.

#[doc(hidden)]
#[macro_export]
macro_rules! __flow_mapping_success_count {
    () => { 0usize };
    (success ! $arguments:tt $($remaining:tt)*) => {
        1usize + $crate::__flow_mapping_success_count!($($remaining)*)
    };
    (($($nested:tt)*) $($remaining:tt)*) => {
        $crate::__flow_mapping_success_count!($($nested)*)
            + $crate::__flow_mapping_success_count!($($remaining)*)
    };
    ([$($nested:tt)*] $($remaining:tt)*) => {
        $crate::__flow_mapping_success_count!($($nested)*)
            + $crate::__flow_mapping_success_count!($($remaining)*)
    };
    ({$($nested:tt)*} $($remaining:tt)*) => {
        $crate::__flow_mapping_success_count!($($nested)*)
            + $crate::__flow_mapping_success_count!($($remaining)*)
    };
    ($_token:tt $($remaining:tt)*) => {
        $crate::__flow_mapping_success_count!($($remaining)*)
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __flow_mapping_failure_count {
    () => { 0usize };
    (failure ! $arguments:tt $($remaining:tt)*) => {
        1usize + $crate::__flow_mapping_failure_count!($($remaining)*)
    };
    (($($nested:tt)*) $($remaining:tt)*) => {
        $crate::__flow_mapping_failure_count!($($nested)*)
            + $crate::__flow_mapping_failure_count!($($remaining)*)
    };
    ([$($nested:tt)*] $($remaining:tt)*) => {
        $crate::__flow_mapping_failure_count!($($nested)*)
            + $crate::__flow_mapping_failure_count!($($remaining)*)
    };
    ({$($nested:tt)*} $($remaining:tt)*) => {
        $crate::__flow_mapping_failure_count!($($nested)*)
            + $crate::__flow_mapping_failure_count!($($remaining)*)
    };
    ($_token:tt $($remaining:tt)*) => {
        $crate::__flow_mapping_failure_count!($($remaining)*)
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __flow_mapping_non_terminal_count {
    () => { 0usize };
    (non_terminal ! $arguments:tt $($remaining:tt)*) => {
        1usize + $crate::__flow_mapping_non_terminal_count!($($remaining)*)
    };
    (($($nested:tt)*) $($remaining:tt)*) => {
        $crate::__flow_mapping_non_terminal_count!($($nested)*)
            + $crate::__flow_mapping_non_terminal_count!($($remaining)*)
    };
    ([$($nested:tt)*] $($remaining:tt)*) => {
        $crate::__flow_mapping_non_terminal_count!($($nested)*)
            + $crate::__flow_mapping_non_terminal_count!($($remaining)*)
    };
    ({$($nested:tt)*} $($remaining:tt)*) => {
        $crate::__flow_mapping_non_terminal_count!($($nested)*)
            + $crate::__flow_mapping_non_terminal_count!($($remaining)*)
    };
    ($_token:tt $($remaining:tt)*) => {
        $crate::__flow_mapping_non_terminal_count!($($remaining)*)
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __validate_flow_mapping_outcomes {
    ($connector:ty, CreateOrder, $($mapping:tt)*) => {
        $crate::__validate_flow_mapping_outcomes!(@allow_without_success $($mapping)*);
    };
    ($connector:ty, PreAuthenticate, $($mapping:tt)*) => {
        $crate::__validate_flow_mapping_outcomes!(@allow_without_success $($mapping)*);
    };
    ($connector:ty, Authenticate, $($mapping:tt)*) => {
        $crate::__validate_flow_mapping_outcomes!(@allow_without_success $($mapping)*);
    };
    ($connector:ty, PostAuthenticate, $($mapping:tt)*) => {
        $crate::__validate_flow_mapping_outcomes!(@allow_without_success $($mapping)*);
    };
    ($connector:ty, Authorize, $($mapping:tt)*) => {
        $crate::__validate_flow_mapping_outcomes!(@allow_without_success $($mapping)*);
    };
    ($connector:ty, Capture, $($mapping:tt)*) => {
        $crate::__validate_flow_mapping_outcomes!(
            @require_success_or_async_ack $connector, $($mapping)*
        );
    };
    ($connector:ty, Void, $($mapping:tt)*) => {
        $crate::__validate_flow_mapping_outcomes!(
            @require_success_or_async_ack $connector, $($mapping)*
        );
    };
    ($connector:ty, Refund, $($mapping:tt)*) => {
        $crate::__validate_flow_mapping_outcomes!(
            @require_success_or_async_ack $connector, $($mapping)*
        );
    };
    ($connector:ty, $flow:ident, $($mapping:tt)*) => {
        $crate::__validate_flow_mapping_outcomes!(@require_success $($mapping)*);
    };
    (@allow_without_success $($mapping:tt)*) => {
        const _: () = {
            const SUCCESS_COUNT: usize = $crate::__flow_mapping_success_count!($($mapping)*);
            const FAILURE_COUNT: usize = $crate::__flow_mapping_failure_count!($($mapping)*);
            const NON_TERMINAL_COUNT: usize =
                $crate::__flow_mapping_non_terminal_count!($($mapping)*);

            assert!(
                SUCCESS_COUNT + FAILURE_COUNT + NON_TERMINAL_COUNT > 0,
                "flow status mapping must declare at least one outcome"
            );
        };
    };
    (@require_success $($mapping:tt)*) => {
        const _: () = {
            const SUCCESS_COUNT: usize = $crate::__flow_mapping_success_count!($($mapping)*);
            const FAILURE_COUNT: usize = $crate::__flow_mapping_failure_count!($($mapping)*);
            const NON_TERMINAL_COUNT: usize =
                $crate::__flow_mapping_non_terminal_count!($($mapping)*);

            assert!(
                SUCCESS_COUNT + FAILURE_COUNT + NON_TERMINAL_COUNT > 0,
                "flow status mapping must declare at least one outcome"
            );
            assert!(
                SUCCESS_COUNT > 0,
                "success mapping is mandatory for this flow"
            );
        };
    };
    (@require_success_or_async_ack $connector:ty, $($mapping:tt)*) => {
        const _: () = {
            const SUCCESS_COUNT: usize = $crate::__flow_mapping_success_count!($($mapping)*);
            const FAILURE_COUNT: usize = $crate::__flow_mapping_failure_count!($($mapping)*);
            const NON_TERMINAL_COUNT: usize =
                $crate::__flow_mapping_non_terminal_count!($($mapping)*);

            assert!(
                SUCCESS_COUNT + FAILURE_COUNT + NON_TERMINAL_COUNT > 0,
                "flow status mapping must declare at least one outcome"
            );
            assert!(
                SUCCESS_COUNT > 0
                    || $crate::flow_status::const_contains_connector_type(
                        common_enums::ASYNC_ACK_STATUS_MAPPING_CONNECTORS,
                        stringify!($connector),
                    ),
                "success mapping is mandatory for terminal flows unless the connector is listed in ASYNC_ACK_STATUS_MAPPING_CONNECTORS"
            );
        };
    };
}

/// Maps connector-native statuses directly into [`ConnectorFlowStatus`].
///
/// Mapping bodies construct typed outcomes with `success!`, `failure!`, and
/// `non_terminal!`. At least one `success!` invocation is required unless
/// the connector's Rust type is listed in `ASYNC_ACK_STATUS_MAPPING_CONNECTORS`.
#[macro_export]
macro_rules! impl_flow_status_mapping {
    (
        $(generics: [$($generic:tt)*],)?
        connector: $connector:ty,
        flow: $flow:ident,
        source: $source:ty,
        context: $context:ty,
        mapping: |$source_value:ident, $context_value:ident| { $($mapping:tt)* },
        runtime: {
            request: $request:ty,
            response: $response:ty,
            source: $source_from:expr,
            context: $context_from:expr $(,)?
        } $(,)?
    ) => {
        $crate::__validate_flow_mapping_outcomes!(
            $connector, $flow, $($mapping)*
        );

        $crate::paste::paste! {
            impl $(<$($generic)*>)?
                $crate::flow_status::ConnectorRuntimeStatusMapping<$flow, $request, $response>
                for $connector
            {
                fn map_runtime_status<CommonData>(
                    common_data: &CommonData,
                    request: &$request,
                    response: &$response,
                    http_status_code: u16,
                ) -> Result<
                    $crate::flow_status::ConnectorFlowStatus<$flow>,
                    $crate::ConnectorError,
                >
                where
                    CommonData: $crate::flow_status::FlowStatusReader<
                        <$flow as $crate::flow_status::FlowSpec>::Status,
                    >,
                {
                    #[allow(unused_macros)]
                    macro_rules! success {
                        ($target:ident) => {
                            Ok($crate::flow_status::ConnectorFlowStatus::Success(
                                $crate::flow_status::__status_mapping::[<$flow SuccessStatus>]::$target,
                            ))
                        };
                    }
                    #[allow(unused_macros)]
                    macro_rules! failure {
                        ($target:ident) => {
                            Ok($crate::flow_status::ConnectorFlowStatus::Failure(
                                $crate::flow_status::__status_mapping::[<$flow FailureStatus>]::$target,
                            ))
                        };
                    }
                    #[allow(unused_macros)]
                    macro_rules! non_terminal {
                        ($target:ident) => {
                            Ok($crate::flow_status::ConnectorFlowStatus::NonTerminal(
                                $crate::flow_status::__status_mapping::[<$flow NonTerminalStatus>]::$target,
                            ))
                        };
                    }

                    let source_from: fn(
                        &CommonData,
                        &$request,
                        &$response,
                        u16,
                    ) -> Result<$source, $crate::ConnectorError> = $source_from;
                    let context_from: fn(
                        &CommonData,
                        &$request,
                        &$response,
                        u16,
                    ) -> Result<$context, $crate::ConnectorError> = $context_from;
                    let mapping: fn($source, $context) -> Result<
                        $crate::flow_status::ConnectorFlowStatus<$flow>,
                        $crate::ConnectorError,
                    > = |$source_value, $context_value| { $($mapping)* };

                    let source = source_from(common_data, request, response, http_status_code)?;
                    let context = context_from(common_data, request, response, http_status_code)?;
                    mapping(source, context)
                }
            }
        }
    };

    (
        $(generics: [$($generic:tt)*],)?
        connector: $connector:ty,
        flow: $flow:ident,
        source: $source:ty,
        mapping: |$source_value:ident| { $($mapping:tt)* },
        runtime: {
            request: $request:ty,
            response: $response:ty,
            source: $source_from:expr $(,)?
        } $(,)?
    ) => {
        $crate::__validate_flow_mapping_outcomes!(
            $connector, $flow, $($mapping)*
        );

        $crate::paste::paste! {
            impl $(<$($generic)*>)?
                $crate::flow_status::ConnectorRuntimeStatusMapping<$flow, $request, $response>
                for $connector
            {
                fn map_runtime_status<CommonData>(
                    common_data: &CommonData,
                    request: &$request,
                    response: &$response,
                    http_status_code: u16,
                ) -> Result<
                    $crate::flow_status::ConnectorFlowStatus<$flow>,
                    $crate::ConnectorError,
                >
                where
                    CommonData: $crate::flow_status::FlowStatusReader<
                        <$flow as $crate::flow_status::FlowSpec>::Status,
                    >,
                {
                    #[allow(unused_macros)]
                    macro_rules! success {
                        ($target:ident) => {
                            Ok($crate::flow_status::ConnectorFlowStatus::Success(
                                $crate::flow_status::__status_mapping::[<$flow SuccessStatus>]::$target,
                            ))
                        };
                    }
                    #[allow(unused_macros)]
                    macro_rules! failure {
                        ($target:ident) => {
                            Ok($crate::flow_status::ConnectorFlowStatus::Failure(
                                $crate::flow_status::__status_mapping::[<$flow FailureStatus>]::$target,
                            ))
                        };
                    }
                    #[allow(unused_macros)]
                    macro_rules! non_terminal {
                        ($target:ident) => {
                            Ok($crate::flow_status::ConnectorFlowStatus::NonTerminal(
                                $crate::flow_status::__status_mapping::[<$flow NonTerminalStatus>]::$target,
                            ))
                        };
                    }

                    let source_from: fn(
                        &CommonData,
                        &$request,
                        &$response,
                        u16,
                    ) -> Result<$source, $crate::ConnectorError> = $source_from;
                    let mapping: fn($source) -> Result<
                        $crate::flow_status::ConnectorFlowStatus<$flow>,
                        $crate::ConnectorError,
                    > = |$source_value| { $($mapping)* };

                    let source = source_from(common_data, request, response, http_status_code)?;
                    mapping(source)
                }
            }
        }
    };

}
/// Refund connectors use the same typed mapping contract as payment connectors.
#[macro_export]
macro_rules! impl_refund_flow_status_mapping {
    ($($tokens:tt)*) => {
        $crate::impl_flow_status_mapping! { $($tokens)* }
    };
}
