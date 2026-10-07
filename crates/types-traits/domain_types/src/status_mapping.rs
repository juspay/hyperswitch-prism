//! Typed connector-status mapping declarations.

/// Maps connector-native statuses directly into [`ConnectorFlowStatus`].
///
/// `success` is required unless `connector_name` is listed in
/// `ASYNC_ACK_STATUS_MAPPING_CONNECTORS`. `failure` and `non_terminal` are
/// optional, but every declaration must contain at least one outcome.
#[macro_export]
macro_rules! impl_flow_status_mapping {
    (
        $(generics: [$($generic:tt)*],)?
        connector: $connector:ty,
        connector_name: $connector_name:expr,
        flow: $flow:ident,
        source: $source:ty,
        context: $context:ty,

        $(success: { $(($success_variant:ident, $success_context:pat_param) => $success_target:ident),+ $(,)? },)?
        $(failure: { $(($failure_variant:ident, $failure_context:pat_param) => $failure_target:ident),+ $(,)? },)?
        $(non_terminal: { $(($non_terminal_variant:ident, $non_terminal_context:pat_param) => $non_terminal_target:ident),+ $(,)? },)?

        runtime: {
            request: $request:ty,
            response: $response:ty,
            source: $source_from:expr,
            context: $context_from:expr $(,)?
        } $(,)?
    ) => {
        const _: () = {
            const SUCCESS_COUNT: usize = 0usize $(+ [$(stringify!($success_variant)),+].len())?;
            const FAILURE_COUNT: usize = 0usize $(+ [$(stringify!($failure_variant)),+].len())?;
            const NON_TERMINAL_COUNT: usize =
                0usize $(+ [$(stringify!($non_terminal_variant)),+].len())?;

            assert!( // first assertion rejects an empty declaration.
                SUCCESS_COUNT + FAILURE_COUNT + NON_TERMINAL_COUNT > 0,
                "flow status mapping must declare at least one outcome"
            );
            assert!( // second assertion requires at least one success mapping unless ASYNC_ACK_STATUS_MAPPING_CONNECTORS
                SUCCESS_COUNT > 0
                    || $crate::flow_status::const_contains_str(
                        common_enums::ASYNC_ACK_STATUS_MAPPING_CONNECTORS,
                        $connector_name,
                    ),
                "success mapping is mandatory unless the connector is listed in ASYNC_ACK_STATUS_MAPPING_CONNECTORS"
            );
        };

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

                    let source = source_from(common_data, request, response, http_status_code)?;
                    let context = context_from(common_data, request, response, http_status_code)?;

                    Ok(match (source, context) {
                        $(
                            $(
                            (<$source>::$success_variant, $success_context) =>
                                $crate::flow_status::ConnectorFlowStatus::Success(
                                    $crate::flow_status::[<$flow SuccessStatus>]::$success_target,
                                ),
                            )+
                        )?
                        $(
                            $(
                            (<$source>::$failure_variant, $failure_context) =>
                                $crate::flow_status::ConnectorFlowStatus::Failure(
                                    $crate::flow_status::[<$flow FailureStatus>]::$failure_target,
                                ),
                            )+
                        )?
                        $(
                            $(
                            (<$source>::$non_terminal_variant, $non_terminal_context) =>
                                $crate::flow_status::ConnectorFlowStatus::NonTerminal(
                                    $crate::flow_status::[<$flow NonTerminalStatus>]::$non_terminal_target,
                                ),
                            )+
                        )?
                    })
                }
            }
        }
    };
    // handles the case where only the connector status matters
    (
        $(generics: [$($generic:tt)*],)?
        connector: $connector:ty,
        connector_name: $connector_name:expr,
        flow: $flow:ident,
        source: $source:ty,

        $(success: { $($success_variant:ident => $success_target:ident),+ $(,)? },)?
        $(failure: { $($failure_variant:ident => $failure_target:ident),+ $(,)? },)?
        $(non_terminal: { $($non_terminal_variant:ident => $non_terminal_target:ident),+ $(,)? },)?

        runtime: {
            request: $request:ty,
            response: $response:ty,
            source: $source_from:expr $(,)?
        } $(,)?
    ) => {
        const _: () = {
            const SUCCESS_COUNT: usize = 0usize $(+ [$(stringify!($success_variant)),+].len())?;
            const FAILURE_COUNT: usize = 0usize $(+ [$(stringify!($failure_variant)),+].len())?;
            const NON_TERMINAL_COUNT: usize =
                0usize $(+ [$(stringify!($non_terminal_variant)),+].len())?;

            assert!( // first assertion rejects an empty declaration.
                SUCCESS_COUNT + FAILURE_COUNT + NON_TERMINAL_COUNT > 0,
                "flow status mapping must declare at least one outcome"
            );
            assert!( // second assertion requires at least one success mapping unless ASYNC_ACK_STATUS_MAPPING_CONNECTORS
                SUCCESS_COUNT > 0
                    || $crate::flow_status::const_contains_str(
                        common_enums::ASYNC_ACK_STATUS_MAPPING_CONNECTORS,
                        $connector_name,
                    ),
                "success mapping is mandatory unless the connector is listed in ASYNC_ACK_STATUS_MAPPING_CONNECTORS"
            );
        };

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
                    let source_from: fn(
                        &CommonData,
                        &$request,
                        &$response,
                        u16,
                    ) -> Result<$source, $crate::ConnectorError> = $source_from;
                    let source = source_from(common_data, request, response, http_status_code)?;

                    Ok(match source {
                        $(
                            $(
                            <$source>::$success_variant =>
                                $crate::flow_status::ConnectorFlowStatus::Success(
                                    $crate::flow_status::[<$flow SuccessStatus>]::$success_target,
                                ),
                            )+
                        )?
                        $(
                            $(
                            <$source>::$failure_variant =>
                                $crate::flow_status::ConnectorFlowStatus::Failure(
                                    $crate::flow_status::[<$flow FailureStatus>]::$failure_target,
                                ),
                            )+
                        )?
                        $(
                            $(
                            <$source>::$non_terminal_variant =>
                                $crate::flow_status::ConnectorFlowStatus::NonTerminal(
                                    $crate::flow_status::[<$flow NonTerminalStatus>]::$non_terminal_target,
                                ),
                            )+
                        )?
                    })
                }
            }
        }
    };

    (
        $(generics: [$($generic:tt)*],)?
        connector: $connector:ty,
        connector_name: $connector_name:expr,
        flow: $flow:ident,
        source: (),
        success: { _ => $target:ident $(,)? },
        runtime: {
            request: $request:ty,
            response: $response:ty,
            source: $source_from:expr $(,)?
        } $(,)?
    ) => {
        $crate::__impl_fixed_flow_status_mapping! {
            $(generics: [$($generic)*],)?
            connector: $connector,
            flow: $flow,
            request: $request,
            response: $response,
            source: $source_from,
            category: Success,
            status_type: SuccessStatus,
            target: $target
        }
    };
    // support responses without a connector status field
    (
        $(generics: [$($generic:tt)*],)?
        connector: $connector:ty,
        connector_name: $connector_name:expr,
        flow: $flow:ident,
        source: (),
        non_terminal: { _ => $target:ident $(,)? },
        runtime: {
            request: $request:ty,
            response: $response:ty,
            source: $source_from:expr $(,)?
        } $(,)?
    ) => {
        
        const _: () = assert!( // second assertion requires at least one success mapping unless ASYNC_ACK_STATUS_MAPPING_CONNECTORS
            $crate::flow_status::const_contains_str(
                common_enums::ASYNC_ACK_STATUS_MAPPING_CONNECTORS,
                $connector_name,
            ),
            "success mapping is mandatory unless the connector is listed in ASYNC_ACK_STATUS_MAPPING_CONNECTORS"
        );
        $crate::__impl_fixed_flow_status_mapping! {
            $(generics: [$($generic)*],)?
            connector: $connector,
            flow: $flow,
            request: $request,
            response: $response,
            source: $source_from,
            category: NonTerminal,
            status_type: NonTerminalStatus,
            target: $target
        }
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __impl_fixed_flow_status_mapping { // support async acknowledgements without a response status
    (
        $(generics: [$($generic:tt)*],)?
        connector: $connector:ty,
        flow: $flow:ident,
        request: $request:ty,
        response: $response:ty,
        source: $source_from:expr,
        category: $category:ident,
        status_type: $status_type:ident,
        target: $target:ident
    ) => {
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
                    let source_from: fn(
                        &CommonData,
                        &$request,
                        &$response,
                        u16,
                    ) -> Result<(), $crate::ConnectorError> = $source_from;
                    source_from(common_data, request, response, http_status_code)?;

                    Ok($crate::flow_status::ConnectorFlowStatus::$category(
                        $crate::flow_status::[<$flow $status_type>]::$target,
                    ))
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
        success: { Succeeded => Charged },
        failure: { Failed => CaptureFailed },
        non_terminal: { Processing => CaptureInitiated },
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
        success: {
            (Succeeded, true) => Charged,
            (Succeeded, false) => Authorized,
        },
        failure: {
            (Failed, _) => AuthorizationFailed,
        },
        non_terminal: {
            (Processing, _) => Authorizing,
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
        failure: { Failed => VoidFailed },
        non_terminal: { Processing => Pending },
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
        success: { Succeeded => Success },
        failure: { Failed => Failure },
        non_terminal: { Processing => Pending },
        runtime: {
            request: Request,
            response: Response,
            source: |_common, _request, response, _http_status_code| Ok(response.status),
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
}
