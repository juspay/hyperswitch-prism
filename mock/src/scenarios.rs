#[derive(Clone, Debug)]
pub enum Outcome {
    Succeed,
    Decline { message: String, code: String },
    RequiresAction,
}

pub fn classify_card(pan: &str) -> Outcome {
    match pan {
        "4000000000000002" => Outcome::Decline {
            message: "Your card was declined.".into(),
            code: "card_declined".into(),
        },
        "4000003800000446" => Outcome::RequiresAction,
        "4242424242424242"
        | "4111111111111111"
        | "5555555555554444"
        | "378282246310005"
        | "5200828282828210" => Outcome::Succeed,
        _ => Outcome::Decline {
            message: "Card not supported by mock.".into(),
            code: "card_not_supported".into(),
        },
    }
}

pub fn classify_upi(vpa: &str) -> Outcome {
    match vpa {
        "success@upi" => Outcome::Succeed,
        "failure@upi" => Outcome::Decline {
            message: "UPI collect declined.".into(),
            code: "upi_declined".into(),
        },
        _ => Outcome::Decline {
            message: "Invalid VPA.".into(),
            code: "invalid_vpa".into(),
        },
    }
}

pub const REDIRECT_PM_TYPES: &[&str] = &[
    "bancontact", "ideal", "trustly", "blik", "mb_way", "satispay", "wero",
    "alipay", "wechat_pay", "revolut_pay",
];

pub fn is_redirect_pm(pm_type: &str) -> bool {
    REDIRECT_PM_TYPES.iter().any(|p| p.eq_ignore_ascii_case(pm_type))
}
