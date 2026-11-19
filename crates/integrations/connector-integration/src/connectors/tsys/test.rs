#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
mod tests {
    use common_enums::enums::RefundStatus;
    use domain_types::connector_types::PaymentsResponseData;

    use crate::connectors::tsys::transformers::{
        get_setup_mandate_response, TsysPaymentStatus, TsysResponse, TsysTransactionDetails,
    };

    fn refund_sync_status(transaction_status: &str) -> RefundStatus {
        let details: TsysTransactionDetails = serde_json::from_value(serde_json::json!({
            "transactionID": "T1",
            "transactionType": "Return",
            "transactionStatus": transaction_status,
        }))
        .unwrap();
        RefundStatus::from(details)
    }

    #[test]
    fn approved_return_is_a_successful_refund() {
        assert_eq!(refund_sync_status("APPROVED"), RefundStatus::Success);
    }

    #[test]
    fn void_and_declined_returns_keep_their_mapping() {
        assert_eq!(refund_sync_status("VOID"), RefundStatus::Success);
        assert_eq!(refund_sync_status("DECLINED"), RefundStatus::Failure);
    }

    #[test]
    fn setup_mandate_does_not_report_the_tsys_id_as_network_txn_id() {
        let response = get_setup_mandate_response(
            TsysResponse {
                status: TsysPaymentStatus::Pass,
                response_code: "A0000".to_string(),
                response_message: "Success".to_string(),
                transaction_id: "TSYS123".to_string(),
            },
            200,
        );
        match response {
            PaymentsResponseData::TransactionResponse {
                network_txn_id,
                mandate_reference,
                ..
            } => {
                assert_eq!(network_txn_id, None);
                assert_eq!(
                    mandate_reference.and_then(|m| m.connector_mandate_id),
                    Some("TSYS123".to_string())
                );
            }
            other => panic!("expected TransactionResponse, got {other:?}"),
        }
    }
}
