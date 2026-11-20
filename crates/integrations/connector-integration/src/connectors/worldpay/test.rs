#[cfg(test)]
mod tests {
    use common_enums::enums::AttemptStatus;

    use crate::connectors::worldpay::response::EventType;

    #[test]
    fn cancelled_last_event_is_voided() {
        assert_eq!(
            AttemptStatus::from(&EventType::Cancelled),
            AttemptStatus::Voided
        );
    }

    #[test]
    fn refused_and_expired_last_events_stay_failures() {
        assert_eq!(
            AttemptStatus::from(&EventType::Refused),
            AttemptStatus::Failure
        );
        assert_eq!(
            AttemptStatus::from(&EventType::Expired),
            AttemptStatus::Failure
        );
    }
}
