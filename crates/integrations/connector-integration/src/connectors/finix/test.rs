#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unwrap_used
)]
mod tests {
    use common_enums::AttemptStatus;
    use domain_types::connector_types::EventType;

    use crate::connectors::finix::transformers::{
        get_finix_attempt_status, get_finix_expiration_year, get_finix_webhook_payment_event_type,
        FinixFlow, FinixPaymentStatus,
    };

    const STATES: [(&str, FinixPaymentStatus); 6] = [
        ("SUCCEEDED", FinixPaymentStatus::Succeeded),
        ("PENDING", FinixPaymentStatus::Pending),
        ("FAILED", FinixPaymentStatus::Failed),
        ("CANCELED", FinixPaymentStatus::Canceled),
        ("RETURNED", FinixPaymentStatus::Returned),
        ("UNKNOWN", FinixPaymentStatus::Unknown),
    ];

    /// Pins the full `(flow, state, is_void) -> AttemptStatus` matrix.
    ///
    /// The load-bearing rows are the `CANCELED` ones: `CANCELED` may only become `Voided`
    /// when a void is actually in flight (`is_void == Some(true)`). Everywhere else it is a
    /// terminal non-success, because `is_payment_failure(Voided) == false` would otherwise
    /// turn a declined authorization into `Ok(TransactionResponse)` and discard the
    /// connector `failure_code` / `failure_message`.
    #[test]
    fn attempt_status_matrix_is_pinned() {
        // (flow, is_void, [status per STATES order])
        let expected: [(FinixFlow, Option<bool>, [AttemptStatus; 6]); 9] = [
            // ---- no void in flight (Authorize, Capture, PSync, RepeatPayment) ----
            (
                FinixFlow::Auth,
                None,
                [
                    AttemptStatus::Authorized,
                    AttemptStatus::AuthenticationPending,
                    AttemptStatus::AuthorizationFailed,
                    AttemptStatus::AuthorizationFailed,
                    AttemptStatus::AuthorizationFailed,
                    AttemptStatus::Pending,
                ],
            ),
            (
                FinixFlow::Transfer,
                None,
                [
                    AttemptStatus::Charged,
                    AttemptStatus::Pending,
                    AttemptStatus::Failure,
                    AttemptStatus::Failure,
                    AttemptStatus::Failure,
                    AttemptStatus::Pending,
                ],
            ),
            (
                FinixFlow::Capture,
                None,
                [
                    AttemptStatus::Pending,
                    AttemptStatus::Pending,
                    AttemptStatus::Failure,
                    AttemptStatus::Failure,
                    AttemptStatus::Failure,
                    AttemptStatus::Pending,
                ],
            ),
            // ---- `is_void: false` must behave exactly like an absent `is_void` ----
            (
                FinixFlow::Auth,
                Some(false),
                [
                    AttemptStatus::Authorized,
                    AttemptStatus::AuthenticationPending,
                    AttemptStatus::AuthorizationFailed,
                    AttemptStatus::AuthorizationFailed,
                    AttemptStatus::AuthorizationFailed,
                    AttemptStatus::Pending,
                ],
            ),
            (
                FinixFlow::Transfer,
                Some(false),
                [
                    AttemptStatus::Charged,
                    AttemptStatus::Pending,
                    AttemptStatus::Failure,
                    AttemptStatus::Failure,
                    AttemptStatus::Failure,
                    AttemptStatus::Pending,
                ],
            ),
            (
                FinixFlow::Capture,
                Some(false),
                [
                    AttemptStatus::Pending,
                    AttemptStatus::Pending,
                    AttemptStatus::Failure,
                    AttemptStatus::Failure,
                    AttemptStatus::Failure,
                    AttemptStatus::Pending,
                ],
            ),
            // ---- void in flight: flow-independent, matches Void at 45351c251 ----
            (
                FinixFlow::Auth,
                Some(true),
                [
                    AttemptStatus::Voided,
                    AttemptStatus::Pending,
                    AttemptStatus::VoidFailed,
                    AttemptStatus::Voided,
                    AttemptStatus::VoidFailed,
                    AttemptStatus::Pending,
                ],
            ),
            (
                FinixFlow::Transfer,
                Some(true),
                [
                    AttemptStatus::Voided,
                    AttemptStatus::Pending,
                    AttemptStatus::VoidFailed,
                    AttemptStatus::Voided,
                    AttemptStatus::VoidFailed,
                    AttemptStatus::Pending,
                ],
            ),
            (
                FinixFlow::Capture,
                Some(true),
                [
                    AttemptStatus::Voided,
                    AttemptStatus::Pending,
                    AttemptStatus::VoidFailed,
                    AttemptStatus::Voided,
                    AttemptStatus::VoidFailed,
                    AttemptStatus::Pending,
                ],
            ),
        ];

        for (flow, is_void, row) in expected {
            for (index, (state_name, state)) in STATES.iter().enumerate() {
                let actual = get_finix_attempt_status(state, flow, is_void);
                assert_eq!(
                    actual, row[index],
                    "flow={flow:?} state={state_name} is_void={is_void:?}: expected {:?}, got {actual:?}",
                    row[index]
                );
            }
        }
    }

    /// `CANCELED` outside a void must stay a failure so the transformer keeps returning
    /// `Err(ErrorResponse)` and surfaces `failure_code` / `failure_message`.
    #[test]
    fn canceled_without_void_is_a_payment_failure() {
        for flow in [FinixFlow::Auth, FinixFlow::Transfer, FinixFlow::Capture] {
            for is_void in [None, Some(false)] {
                let status = get_finix_attempt_status(&FinixPaymentStatus::Canceled, flow, is_void);
                assert!(
                    domain_types::utils::is_payment_failure(status),
                    "flow={flow:?} is_void={is_void:?}: CANCELED mapped to {status:?}, which is \
                     not a payment failure — failure_code/failure_message would be discarded"
                );
            }
        }
    }

    /// `UNKNOWN` is indeterminate and must never be terminal, on any flow.
    #[test]
    fn unknown_is_never_terminal() {
        for flow in [FinixFlow::Auth, FinixFlow::Transfer, FinixFlow::Capture] {
            for is_void in [None, Some(false), Some(true)] {
                assert_eq!(
                    get_finix_attempt_status(&FinixPaymentStatus::Unknown, flow, is_void),
                    AttemptStatus::Pending,
                    "flow={flow:?} is_void={is_void:?}"
                );
            }
        }
    }

    /// A `PENDING` void must not be reported as already `Voided`.
    #[test]
    fn pending_void_stays_pending() {
        for flow in [FinixFlow::Auth, FinixFlow::Transfer, FinixFlow::Capture] {
            assert_eq!(
                get_finix_attempt_status(&FinixPaymentStatus::Pending, flow, Some(true)),
                AttemptStatus::Pending,
                "flow={flow:?}"
            );
        }
    }

    /// The webhook event type and the webhook payload status are both derived from
    /// `get_finix_attempt_status`, so they must never contradict each other.
    #[test]
    fn webhook_event_type_agrees_with_webhook_status() {
        for flow in [FinixFlow::Auth, FinixFlow::Transfer] {
            for is_void in [None, Some(false), Some(true)] {
                for (state_name, state) in STATES.iter() {
                    let status = get_finix_attempt_status(state, flow, is_void);
                    let event = get_finix_webhook_payment_event_type(state, flow, is_void);

                    let compatible = match event {
                        // Indeterminate: asserts nothing, so it cannot contradict.
                        EventType::IncomingWebhookEventUnspecified => {
                            matches!(state, FinixPaymentStatus::Unknown)
                        }
                        EventType::PaymentIntentCancelled => status == AttemptStatus::Voided,
                        EventType::PaymentIntentCancelFailure => {
                            status == AttemptStatus::VoidFailed
                        }
                        EventType::PaymentIntentAuthorizationSuccess => {
                            status == AttemptStatus::Authorized
                        }
                        EventType::PaymentIntentAuthorizationFailure => {
                            status == AttemptStatus::AuthorizationFailed
                        }
                        EventType::PaymentIntentSuccess => status == AttemptStatus::Charged,
                        EventType::PaymentIntentFailure => status == AttemptStatus::Failure,
                        EventType::PaymentIntentProcessing => matches!(
                            status,
                            AttemptStatus::Pending | AttemptStatus::AuthenticationPending
                        ),
                        other => panic!("unexpected event type {other:?}"),
                    };

                    assert!(
                        compatible,
                        "flow={flow:?} state={state_name} is_void={is_void:?}: event {event:?} \
                         contradicts status {status:?}"
                    );
                }
            }
        }
    }

    /// Finix's `expiration_year` must be a four-digit year, and
    /// `get_expiry_year_4_digit()` only expands a *two*-digit input — every other length
    /// passes straight through. A three-digit `"203"` used to be forwarded to Finix as
    /// `"expiration_year": 203`; it must now be rejected locally.
    #[test]
    fn expiration_year_accepts_only_two_or_four_digits() {
        use domain_types::payment_method_data::{Card, DefaultPCIHolder, RawCardNumber};
        use hyperswitch_masking::{ExposeInterface, Secret};

        fn card(exp_year: &str) -> Card<DefaultPCIHolder> {
            Card {
                card_number: RawCardNumber::<DefaultPCIHolder>(
                    cards::CardNumber::try_from("4111111111111111".to_string())
                        .expect("valid test PAN"),
                ),
                card_exp_month: Secret::new("12".to_string()),
                card_exp_year: Secret::new(exp_year.to_string()),
                card_cvc: Secret::new("123".to_string()),
                card_issuer: None,
                card_network: None,
                card_type: None,
                card_issuing_country: None,
                bank_code: None,
                nick_name: None,
                card_holder_name: None,
                co_badged_card_data: None,
            }
        }

        let current_century = common_utils::date_time::now().year() / 100;

        for (input, expected) in [
            ("30", Some(current_century * 100 + 30)),
            ("2030", Some(2030)),
            ("203", None),
            ("20300", None),
            ("", None),
            ("abcd", None),
        ] {
            let actual = get_finix_expiration_year(&card(input))
                .map(|year| year.expose())
                .ok();
            assert_eq!(actual, expected, "card_exp_year={input:?}");
        }
    }

    /// A successful void of an authorization must report the void as *succeeded*, not as a
    /// cancel failure (the exact contradiction F-1 described).
    #[test]
    fn voided_authorization_emits_cancelled_not_cancel_failure() {
        assert_eq!(
            get_finix_webhook_payment_event_type(
                &FinixPaymentStatus::Canceled,
                FinixFlow::Auth,
                Some(true)
            ),
            EventType::PaymentIntentCancelled
        );
        assert_eq!(
            get_finix_attempt_status(&FinixPaymentStatus::Canceled, FinixFlow::Auth, Some(true)),
            AttemptStatus::Voided
        );
    }
}
