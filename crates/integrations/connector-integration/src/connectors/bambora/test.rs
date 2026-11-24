#[cfg(test)]
mod tests {
    use common_enums::RefundStatus;

    use crate::connectors::bambora::transformers::{bambora_refund_status, BamboraPaymentType};

    #[test]
    fn approved_return_is_a_successful_refund() {
        assert_eq!(
            bambora_refund_status("1", &BamboraPaymentType::Return),
            RefundStatus::Success
        );
    }

    #[test]
    fn approved_void_refund_is_not_a_successful_refund() {
        assert_eq!(
            bambora_refund_status("1", &BamboraPaymentType::VoidRefund),
            RefundStatus::Failure
        );
    }

    #[test]
    fn declined_refund_is_a_failure() {
        assert_eq!(
            bambora_refund_status("0", &BamboraPaymentType::Return),
            RefundStatus::Failure
        );
    }
}
