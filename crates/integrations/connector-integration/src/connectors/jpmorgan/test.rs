#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use crate::connectors::jpmorgan::{responses::JpmorganRefundResponse, transformers};

    #[test]
    fn refund_status_code_is_the_http_status_not_the_textual_response_code() {
        let response: JpmorganRefundResponse = serde_json::from_value(serde_json::json!({
            "transactionId": "TXN1",
            "requestId": "REQ1",
            "transactionState": "CLOSED",
            "responseStatus": "SUCCESS",
            "responseCode": "APPROVED",
            "responseMessage": "Approved"
        }))
        .unwrap();
        let data = transformers::refunds_response_data(&response, 201).unwrap();
        assert_eq!(data.status_code, 201);
    }
}
