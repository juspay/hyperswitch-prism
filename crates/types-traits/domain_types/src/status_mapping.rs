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
    ($connector_name:expr, CreateOrder, $($mapping:tt)*) => {
        $crate::__validate_flow_mapping_outcomes!(@allow_without_success $($mapping)*);
    };
    ($connector_name:expr, PreAuthenticate, $($mapping:tt)*) => {
        $crate::__validate_flow_mapping_outcomes!(@allow_without_success $($mapping)*);
    };
    ($connector_name:expr, Authenticate, $($mapping:tt)*) => {
        $crate::__validate_flow_mapping_outcomes!(@allow_without_success $($mapping)*);
    };
    ($connector_name:expr, PostAuthenticate, $($mapping:tt)*) => {
        $crate::__validate_flow_mapping_outcomes!(@allow_without_success $($mapping)*);
    };
    ($connector_name:expr, Authorize, $($mapping:tt)*) => {
        $crate::__validate_flow_mapping_outcomes!(@allow_without_success $($mapping)*);
    };
    ($connector_name:expr, $flow:ident, $($mapping:tt)*) => {
        $crate::__validate_flow_mapping_outcomes!(
            @require_success $connector_name, $($mapping)*
        );
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
    (@require_success $connector_name:expr, $($mapping:tt)*) => {
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
                    || $crate::flow_status::const_contains_str(
                        common_enums::ASYNC_ACK_STATUS_MAPPING_CONNECTORS,
                        $connector_name,
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
/// `connector_name` is listed in `ASYNC_ACK_STATUS_MAPPING_CONNECTORS`.
#[macro_export]
macro_rules! impl_flow_status_mapping {
    (
        $(generics: [$($generic:tt)*],)?
        connector: $connector:ty,
        connector_name: $connector_name:expr,
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
            $connector_name, $flow, $($mapping)*
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
        connector_name: $connector_name:expr,
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
            $connector_name, $flow, $($mapping)*
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

#[cfg(test)]
mod tests {
    use common_enums::{AttemptStatus, RefundStatus};

    use crate::{
        connector_flow::{Authorize, Capture, Refund, Void},
        flow_status::{ConnectorRuntimeStatusMapping, FlowStatusReader},
    };

    #[derive(Clone, Copy)]
    enum TestStatus {
        Succeeded,
        Failed,
        Processing,
    }

    struct PaymentCommonData(AttemptStatus);

    impl FlowStatusReader<AttemptStatus> for PaymentCommonData {
        fn current_mapped_flow_status(&self) -> AttemptStatus {
            self.0
        }
    }

    struct RefundCommonData(RefundStatus);

    impl FlowStatusReader<RefundStatus> for RefundCommonData {
        fn current_mapped_flow_status(&self) -> RefundStatus {
            self.0
        }
    }

    struct Request {
        auto_capture: bool,
    }

    struct Response {
        status: TestStatus,
    }

    struct SimpleConnector;

    crate::impl_flow_status_mapping! {
        connector: SimpleConnector,
        connector_name: "simple",
        flow: Capture,
        source: TestStatus,
        mapping: |status| {
            match status {
                TestStatus::Succeeded => success!(Charged),
                TestStatus::Failed => failure!(CaptureFailed),
                TestStatus::Processing => non_terminal!(CaptureInitiated),
            }
        },
        runtime: {
            request: Request,
            response: Response,
            source: |_common, _request, response, _http_status_code| Ok(response.status),
        }
    }

    struct ContextConnector;

    crate::impl_flow_status_mapping! {
        connector: ContextConnector,
        connector_name: "context",
        flow: Authorize,
        source: TestStatus,
        context: bool,
        mapping: |status, is_auto_capture| {
            match (status, is_auto_capture) {
                (TestStatus::Succeeded, true) => success!(Charged),
                (TestStatus::Succeeded, false) => success!(Authorized),
                (TestStatus::Failed, _) => failure!(AuthorizationFailed),
                (TestStatus::Processing, _) => non_terminal!(Authorizing),
            }
        },
        runtime: {
            request: Request,
            response: Response,
            source: |_common, _request, response, _http_status_code| Ok(response.status),
            context: |_common, request, _response, _http_status_code| Ok(request.auto_capture),
        }
    }

    struct AsyncConnector;

    crate::impl_flow_status_mapping! {
        connector: AsyncConnector,
        connector_name: "adyen",
        flow: Void,
        source: TestStatus,
        mapping: |status| {
            match status {
                TestStatus::Failed => failure!(VoidFailed),
                TestStatus::Succeeded | TestStatus::Processing => non_terminal!(Pending),
            }
        },
        runtime: {
            request: Request,
            response: Response,
            source: |_common, _request, response, _http_status_code| Ok(response.status),
        }
    }

    struct RefundConnector;

    crate::impl_refund_flow_status_mapping! {
        connector: RefundConnector,
        connector_name: "refund",
        flow: Refund,
        source: TestStatus,
        mapping: |status| {
            match status {
                TestStatus::Succeeded => success!(Success),
                TestStatus::Failed => failure!(Failure),
                TestStatus::Processing => non_terminal!(Pending),
            }
        },
        runtime: {
            request: Request,
            response: Response,
            source: |_common, _request, response, _http_status_code| Ok(response.status),
        }
    }

    struct TypedMappingConnector;

    crate::impl_flow_status_mapping! {
        connector: TypedMappingConnector,
        connector_name: "typed_mapping",
        flow: Capture,
        source: TestStatus,
        mapping: |status| {
            match status {
                TestStatus::Succeeded => success!(Charged),
                TestStatus::Failed => failure!(CaptureFailed),
                TestStatus::Processing => non_terminal!(CaptureInitiated),
            }
        },
        runtime: {
            request: Request,
            response: Response,
            source: |_common, _request, response, _http_status_code| Ok(response.status),
        }
    }

    struct TypedContextMappingConnector;

    crate::impl_flow_status_mapping! {
        connector: TypedContextMappingConnector,
        connector_name: "typed_context_mapping",
        flow: Authorize,
        source: TestStatus,
        context: bool,
        mapping: |status, is_auto_capture| {
            if is_auto_capture {
                success!(Charged)
            } else {
                match status {
                    TestStatus::Succeeded => success!(Authorized),
                    TestStatus::Failed => failure!(AuthorizationFailed),
                    TestStatus::Processing => non_terminal!(Authorizing),
                }
            }
        },
        runtime: {
            request: Request,
            response: Response,
            source: |_common, _request, response, _http_status_code| Ok(response.status),
            context: |_common, request, _response, _http_status_code| Ok(request.auto_capture),
        }
    }

    struct StatuslessMappingConnector;

    crate::impl_flow_status_mapping! {
        connector: StatuslessMappingConnector,
        connector_name: "statusless",
        flow: Authorize,
        source: (),
        mapping: |_status| {
            non_terminal!(AuthenticationPending)
        },
        runtime: {
            request: Request,
            response: Response,
            source: |_common, _request, _response, _http_status_code| Ok(()),
        }
    }

    #[test]
    fn maps_simple_payment_status() {
        let mapped = SimpleConnector::map_runtime_status(
            &PaymentCommonData(AttemptStatus::Started),
            &Request {
                auto_capture: false,
            },
            &Response {
                status: TestStatus::Succeeded,
            },
            200,
        )
        .unwrap();

        assert_eq!(mapped.status(), AttemptStatus::Charged);
    }

    #[test]
    fn maps_context_aware_payment_status() {
        let mapped = ContextConnector::map_runtime_status(
            &PaymentCommonData(AttemptStatus::Started),
            &Request {
                auto_capture: false,
            },
            &Response {
                status: TestStatus::Succeeded,
            },
            200,
        )
        .unwrap();

        assert_eq!(mapped.status(), AttemptStatus::Authorized);
    }

    #[test]
    fn permits_async_connector_without_success_mapping() {
        let mapped = AsyncConnector::map_runtime_status(
            &PaymentCommonData(AttemptStatus::Started),
            &Request {
                auto_capture: false,
            },
            &Response {
                status: TestStatus::Processing,
            },
            202,
        )
        .unwrap();

        assert_eq!(mapped.status(), AttemptStatus::Pending);
    }

    #[test]
    fn maps_refund_status_with_the_shared_macro_contract() {
        let mapped = RefundConnector::map_runtime_status(
            &RefundCommonData(RefundStatus::Pending),
            &Request {
                auto_capture: false,
            },
            &Response {
                status: TestStatus::Succeeded,
            },
            200,
        )
        .unwrap();

        assert_eq!(mapped.status(), RefundStatus::Success);
    }

    #[test]
    fn maps_status_with_typed_mapping_closure() {
        let mapped = TypedMappingConnector::map_runtime_status(
            &PaymentCommonData(AttemptStatus::Started),
            &Request {
                auto_capture: false,
            },
            &Response {
                status: TestStatus::Processing,
            },
            200,
        )
        .unwrap();

        assert_eq!(mapped.status(), AttemptStatus::CaptureInitiated);
    }

    #[test]
    fn maps_repeated_context_logic_with_typed_mapping_closure() {
        let mapped = TypedContextMappingConnector::map_runtime_status(
            &PaymentCommonData(AttemptStatus::Started),
            &Request { auto_capture: true },
            &Response {
                status: TestStatus::Failed,
            },
            200,
        )
        .unwrap();

        assert_eq!(mapped.status(), AttemptStatus::Charged);
    }

    #[test]
    fn maps_statusless_response_with_the_same_mapping_syntax() {
        let mapped = StatuslessMappingConnector::map_runtime_status(
            &PaymentCommonData(AttemptStatus::Started),
            &Request {
                auto_capture: false,
            },
            &Response {
                status: TestStatus::Processing,
            },
            202,
        )
        .unwrap();

        assert_eq!(mapped.status(), AttemptStatus::AuthenticationPending);
    }
}
