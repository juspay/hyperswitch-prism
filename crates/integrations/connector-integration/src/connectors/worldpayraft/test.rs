#[cfg(test)]
mod tests {
    use crate::connectors::worldpayraft::transformers::{
        build_connector_transaction_id, TXN_TYPE_CREDIT, TXN_TYPE_DEBIT,
    };

    #[test]
    fn complete_trace_numbers_build_the_prefixed_id() {
        assert_eq!(
            build_connector_transaction_id(
                TXN_TYPE_CREDIT,
                Some((Some("A1"), Some("R1"), Some("S1"))),
                true,
                "Authorize",
            ),
            "C|A1|R1|S1"
        );
        assert_eq!(
            build_connector_transaction_id(
                TXN_TYPE_DEBIT,
                Some((Some("A1"), Some("R1"), Some("S1"))),
                true,
                "Authorize",
            ),
            "D|A1|R1|S1"
        );
    }

    #[test]
    fn missing_trace_numbers_keep_the_existing_id_shape() {
        assert_eq!(
            build_connector_transaction_id(TXN_TYPE_CREDIT, None, true, "Authorize"),
            ""
        );
        assert_eq!(
            build_connector_transaction_id(
                TXN_TYPE_CREDIT,
                Some((None, Some("R1"), None)),
                true,
                "RepeatPayment",
            ),
            "C||R1|"
        );
    }
}
