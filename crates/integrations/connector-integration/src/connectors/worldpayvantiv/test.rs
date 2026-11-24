#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use domain_types::{
        payment_method_data::DefaultPCIHolder, router_data::ConnectorSpecificConfig,
        router_response_types::Response,
    };

    use crate::connectors::worldpayvantiv::{build_sync_error_response, Worldpayvantiv};

    fn response(body: &'static str, status_code: u16) -> Response {
        Response {
            headers: None,
            response: bytes::Bytes::from_static(body.as_bytes()),
            status_code,
        }
    }

    #[test]
    fn sync_json_error_body_keeps_the_connector_message() {
        let connector = Worldpayvantiv::<DefaultPCIHolder>::new();
        let error = build_sync_error_response(
            connector,
            response(
                r#"{"errorMessages":["Transaction not found","Check the id"]}"#,
                404,
            ),
            None,
            &ConnectorSpecificConfig::NoKey,
        )
        .unwrap();

        assert_eq!(error.status_code, 404);
        assert_eq!(error.message, "Transaction not found; Check the id");
        assert_eq!(
            error.reason.as_deref(),
            Some("Transaction not found; Check the id")
        );
    }

    #[test]
    fn sync_json_error_body_without_messages_uses_the_default_message() {
        let connector = Worldpayvantiv::<DefaultPCIHolder>::new();
        let error = build_sync_error_response(
            connector,
            response(r#"{"errorMessages":[]}"#, 500),
            None,
            &ConnectorSpecificConfig::NoKey,
        )
        .unwrap();

        assert_eq!(error.status_code, 500);
        assert_eq!(error.message, common_utils::consts::NO_ERROR_MESSAGE);
    }
}
