#[cfg(test)]
mod tests {
    use common_enums::AttemptStatus;

    use crate::connectors::authipay::transformers::{
        map_status, AuthipayPaymentStatus, AuthipayTransactionState, AuthipayTransactionType,
    };

    #[test]
    fn captured_preauth_is_charged() {
        assert_eq!(
            map_status(
                Some(AuthipayPaymentStatus::Approved),
                None,
                Some(AuthipayTransactionState::Captured),
                AuthipayTransactionType::Preauth,
            ),
            AttemptStatus::Charged
        );
    }

    #[test]
    fn authorized_preauth_is_still_authorized() {
        assert_eq!(
            map_status(
                Some(AuthipayPaymentStatus::Approved),
                None,
                Some(AuthipayTransactionState::Authorized),
                AuthipayTransactionType::Preauth,
            ),
            AttemptStatus::Authorized
        );
    }
}
