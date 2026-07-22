//! Card-family axis of `TxProfile`.
//!
//! Collapses the verbose `Option<CardNetwork>` into the small set of
//! families TSYS cert rules actually distinguish on. AMEX/JCB share a
//! CVV-input quirk; Discover/JCB/Diners/UnionPay share recurring-tag
//! treatment ("Discover-like").

use common_enums::CardNetwork;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardFamily {
    Visa,
    Mastercard,
    Amex,
    Discover,
    Jcb,
    Diners,
    UnionPay,
    Unknown,
}

impl CardFamily {
    pub fn from_network(network: Option<&CardNetwork>) -> Self {
        match network {
            Some(CardNetwork::Visa) => Self::Visa,
            Some(CardNetwork::Mastercard) => Self::Mastercard,
            Some(CardNetwork::AmericanExpress) => Self::Amex,
            Some(CardNetwork::Discover) => Self::Discover,
            Some(CardNetwork::JCB) => Self::Jcb,
            Some(CardNetwork::DinersClub) => Self::Diners,
            Some(CardNetwork::UnionPay) => Self::UnionPay,
            _ => Self::Unknown,
        }
    }

    /// AMEX and JCB share the "MANUALLY_ENTERED_WITH_KEYED_CID_AMEX_JCB"
    /// `cardDataInputMode` rule when a CVV is sent.
    pub fn is_amex_or_jcb(self) -> bool {
        matches!(self, Self::Amex | Self::Jcb)
    }

    /// Discover, JCB, Diners and UnionPay all flow through the same
    /// recurring-tag and `registeredUserIndicator` branches.
    pub fn is_discover_like(self) -> bool {
        matches!(
            self,
            Self::Discover | Self::Jcb | Self::Diners | Self::UnionPay
        )
    }

    pub fn is_visa(self) -> bool {
        matches!(self, Self::Visa)
    }

    pub fn is_mastercard(self) -> bool {
        matches!(self, Self::Mastercard)
    }

    pub fn is_amex(self) -> bool {
        matches!(self, Self::Amex)
    }

    pub fn is_visa_or_mastercard(self) -> bool {
        matches!(self, Self::Visa | Self::Mastercard)
    }
}
