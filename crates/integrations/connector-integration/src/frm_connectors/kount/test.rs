#[cfg(test)]
mod tests {
    use common_enums::AttemptStatus;

    use crate::frm_connectors::kount::transformers::{KountAuthResult, KountPaymentStatus};

    #[test]
    fn failed_capture_is_not_reported_as_a_declined_payment() {
        assert_eq!(
            KountPaymentStatus::from_attempt_status(AttemptStatus::CaptureFailed),
            None
        );
        assert_eq!(
            KountPaymentStatus::from_attempt_status(AttemptStatus::AuthorizationFailed),
            Some(KountPaymentStatus::Declined)
        );
    }

    #[test]
    fn failed_capture_or_void_keeps_an_approved_authorization() {
        assert_eq!(
            KountAuthResult::from_attempt_status(AttemptStatus::CaptureFailed),
            KountAuthResult::Approved
        );
        assert_eq!(
            KountAuthResult::from_attempt_status(AttemptStatus::VoidFailed),
            KountAuthResult::Approved
        );
        assert_eq!(
            KountAuthResult::from_attempt_status(AttemptStatus::AuthorizationFailed),
            KountAuthResult::Declined
        );
    }
}
