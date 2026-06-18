use super::frm_types::{PostRiskCheckResponse, PreRiskCheckResponse};
use crate::connector_types::{ConnectorResponseHeaders, RawConnectorRequestResponse};
use crate::utils::ForeignFrom;

pub fn generate_pre_risk_check_response(
    router_data_v2: crate::router_data_v2::RouterDataV2<
        crate::connector_flow::PreRiskCheck,
        super::frm_types::FrmFlowData,
        super::frm_types::PreRiskCheckRequest,
        super::frm_types::PreRiskCheckResponse,
    >,
) -> Result<
    grpc_api_types::frm::FrmServicePreRiskCheckResponse,
    error_stack::Report<crate::errors::ConnectorError>,
> {
    let raw_connector_response = router_data_v2
        .resource_common_data
        .get_raw_connector_response();
    let raw_connector_request = router_data_v2
        .resource_common_data
        .get_raw_connector_request();
    let response_headers = router_data_v2
        .resource_common_data
        .get_connector_response_headers_as_map();

    let response = match router_data_v2.response {
        Ok(PreRiskCheckResponse {
            frm_decision,
            risk_score,
            reason,
            frm_transaction_id,
            status_code,
        }) => {
            let grpc_frm_decision = frm_decision
                .map(grpc_api_types::frm::FrmDecision::foreign_from)
                .unwrap_or(grpc_api_types::frm::FrmDecision::Unspecified);

            grpc_api_types::frm::FrmServicePreRiskCheckResponse {
                frm_decision: Some(grpc_frm_decision as i32),
                risk_score,
                reason,
                frm_transaction_id,
                status_code: status_code.into(),
                error: None,
                raw_connector_request,
                raw_connector_response,
                response_headers,
            }
        }
        Err(err) => grpc_api_types::frm::FrmServicePreRiskCheckResponse {
            frm_decision: Some(grpc_api_types::frm::FrmDecision::Unspecified as i32),
            risk_score: None,
            reason: None,
            frm_transaction_id: None,
            status_code: err.status_code.into(),
            error: Some(grpc_api_types::frm::ErrorInfo {
                unified_details: None,
                connector_details: Some(grpc_api_types::frm::ConnectorErrorDetails {
                    code: Some(err.code),
                    message: Some(err.message.clone()),
                    reason: None,
                    connector_transaction_id: err.connector_transaction_id.clone(),
                    status: None,
                }),
                issuer_details: None,
            }),
            raw_connector_request,
            raw_connector_response,
            response_headers,
        },
    };
    Ok(response)
}

pub fn generate_post_risk_check_response(
    router_data_v2: crate::router_data_v2::RouterDataV2<
        crate::connector_flow::PostRiskCheck,
        super::frm_types::FrmFlowData,
        super::frm_types::PostRiskCheckRequest,
        super::frm_types::PostRiskCheckResponse,
    >,
) -> Result<
    grpc_api_types::frm::FrmServicePostRiskCheckResponse,
    error_stack::Report<crate::errors::ConnectorError>,
> {
    let raw_connector_response = router_data_v2
        .resource_common_data
        .get_raw_connector_response();
    let raw_connector_request = router_data_v2
        .resource_common_data
        .get_raw_connector_request();
    let response_headers = router_data_v2
        .resource_common_data
        .get_connector_response_headers_as_map();

    let response = match router_data_v2.response {
        Ok(PostRiskCheckResponse {
            frm_decision,
            risk_score,
            reason,
            frm_transaction_id,
            status_code,
        }) => {
            let grpc_frm_decision = frm_decision
                .map(grpc_api_types::frm::FrmDecision::foreign_from)
                .unwrap_or(grpc_api_types::frm::FrmDecision::Unspecified);

            grpc_api_types::frm::FrmServicePostRiskCheckResponse {
                frm_decision: Some(grpc_frm_decision as i32),
                risk_score,
                reason,
                frm_transaction_id,
                status_code: status_code.into(),
                error: None,
                raw_connector_request,
                raw_connector_response,
                response_headers,
            }
        }
        Err(err) => grpc_api_types::frm::FrmServicePostRiskCheckResponse {
            frm_decision: Some(grpc_api_types::frm::FrmDecision::Unspecified as i32),
            risk_score: None,
            reason: None,
            frm_transaction_id: None,
            status_code: err.status_code.into(),
            error: Some(grpc_api_types::frm::ErrorInfo {
                unified_details: None,
                connector_details: Some(grpc_api_types::frm::ConnectorErrorDetails {
                    code: Some(err.code),
                    message: Some(err.message.clone()),
                    reason: None,
                    connector_transaction_id: err.connector_transaction_id.clone(),
                    status: None,
                }),
                issuer_details: None,
            }),
            raw_connector_request,
            raw_connector_response,
            response_headers,
        },
    };
    Ok(response)
}

pub fn generate_frm_payment_outcome_response(
    router_data_v2: crate::router_data_v2::RouterDataV2<
        crate::connector_flow::FrmPaymentOutcome,
        super::frm_types::FrmFlowData,
        super::frm_types::FrmPaymentOutcomeRequest,
        super::frm_types::FrmPaymentOutcomeResponse,
    >,
) -> Result<
    grpc_api_types::payments::NotifyConnectorResponse,
    error_stack::Report<crate::errors::ConnectorError>,
> {
    match router_data_v2.response {
        Ok(response) => Ok(grpc_api_types::payments::NotifyConnectorResponse {
            status_code: response.status_code.into(),
            error: None,
        }),
        Err(e) => Ok(grpc_api_types::payments::NotifyConnectorResponse {
            status_code: e.status_code.into(),
            error: Some(grpc_api_types::payments::ErrorInfo {
                unified_details: None,
                connector_details: Some(grpc_api_types::payments::ConnectorErrorDetails {
                    code: Some(e.code),
                    message: Some(e.message.clone()),
                    reason: e.reason.clone(),
                    connector_transaction_id: e.connector_transaction_id.clone(),
                    status: None,
                }),
                issuer_details: None,
            }),
        }),
    }
}

pub fn generate_frm_refund_processed_response(
    router_data_v2: crate::router_data_v2::RouterDataV2<
        crate::connector_flow::FrmRefundProcessed,
        super::frm_types::FrmFlowData,
        super::frm_types::FrmRefundProcessedRequest,
        super::frm_types::FrmRefundProcessedResponse,
    >,
) -> Result<
    grpc_api_types::payments::NotifyConnectorResponse,
    error_stack::Report<crate::errors::ConnectorError>,
> {
    match router_data_v2.response {
        Ok(response) => Ok(grpc_api_types::payments::NotifyConnectorResponse {
            status_code: response.status_code.into(),
            error: None,
        }),
        Err(e) => Ok(grpc_api_types::payments::NotifyConnectorResponse {
            status_code: e.status_code.into(),
            error: Some(grpc_api_types::payments::ErrorInfo {
                unified_details: None,
                connector_details: Some(grpc_api_types::payments::ConnectorErrorDetails {
                    code: Some(e.code),
                    message: Some(e.message.clone()),
                    reason: e.reason.clone(),
                    connector_transaction_id: e.connector_transaction_id.clone(),
                    status: None,
                }),
                issuer_details: None,
            }),
        }),
    }
}

pub fn generate_frm_chargeback_received_response(
    router_data_v2: crate::router_data_v2::RouterDataV2<
        crate::connector_flow::FrmChargebackReceived,
        super::frm_types::FrmFlowData,
        super::frm_types::FrmChargebackReceivedRequest,
        super::frm_types::FrmChargebackReceivedResponse,
    >,
) -> Result<
    grpc_api_types::payments::NotifyConnectorResponse,
    error_stack::Report<crate::errors::ConnectorError>,
> {
    match router_data_v2.response {
        Ok(response) => Ok(grpc_api_types::payments::NotifyConnectorResponse {
            status_code: response.status_code.into(),
            error: None,
        }),
        Err(e) => Ok(grpc_api_types::payments::NotifyConnectorResponse {
            status_code: e.status_code.into(),
            error: Some(grpc_api_types::payments::ErrorInfo {
                unified_details: None,
                connector_details: Some(grpc_api_types::payments::ConnectorErrorDetails {
                    code: Some(e.code),
                    message: Some(e.message.clone()),
                    reason: e.reason.clone(),
                    connector_transaction_id: e.connector_transaction_id.clone(),
                    status: None,
                }),
                issuer_details: None,
            }),
        }),
    }
}
