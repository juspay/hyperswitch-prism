#[cfg(test)]
mod tests {
    use common_enums::PayoutStatus;

    use crate::payout_connectors::gigadat::transformers::{
        in_flight_payout_status, GigadatPayoutStatus,
    };

    #[test]
    fn pending_in_flight_payout_is_pending_not_requires_fulfillment() {
        assert_eq!(
            in_flight_payout_status(GigadatPayoutStatus::StatusPending),
            PayoutStatus::Pending
        );
        // Create still asks for fulfilment.
        assert_eq!(
            PayoutStatus::from(GigadatPayoutStatus::StatusPending),
            PayoutStatus::RequiresFulfillment
        );
    }

    #[test]
    fn other_statuses_are_unchanged() {
        assert_eq!(
            in_flight_payout_status(GigadatPayoutStatus::StatusSuccess),
            PayoutStatus::Success
        );
        assert_eq!(
            in_flight_payout_status(GigadatPayoutStatus::StatusFailed),
            PayoutStatus::Failure
        );
    }
}
