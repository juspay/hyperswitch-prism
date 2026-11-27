#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use domain_types::{router_data::ErrorResponse, router_response_types::Response};

    use crate::connectors::cybersource::transformers::ForeignTryFrom;

    #[test]
    fn system_error_5xx_leaves_the_attempt_status_unknown() {
        let response = Response {
            headers: None,
            response: bytes::Bytes::from_static(
                br#"{"status":"SERVER_ERROR","reason":"SYSTEM_ERROR","message":"General system failure."}"#,
            ),
            status_code: 502,
        };
        let error = ErrorResponse::foreign_try_from(&response).unwrap();
        assert_eq!(error.status_code, 502);
        assert!(error.attempt_status.is_none());
    }
}
