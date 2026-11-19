#[cfg(test)]
mod tests {
    use common_enums::{AttemptStatus, RefundStatus};

    use crate::connectors::tamara::transformers::{TamaraPaymentStatus, TamaraRefundStatus};

    #[test]
    fn updated_order_is_not_voided() {
        assert_eq!(
            AttemptStatus::from(TamaraPaymentStatus::Updated),
            AttemptStatus::Pending
        );
        assert_eq!(
            AttemptStatus::from(TamaraPaymentStatus::Canceled),
            AttemptStatus::Voided
        );
    }

    #[test]
    fn updated_order_is_not_a_failed_refund() {
        assert_eq!(
            RefundStatus::from(TamaraRefundStatus::Updated),
            RefundStatus::Pending
        );
        assert_eq!(
            RefundStatus::from(TamaraRefundStatus::Canceled),
            RefundStatus::Failure
        );
    }
}
