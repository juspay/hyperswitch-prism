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

    /// Fallback when the card network is not supplied — infer the family from
    /// the card number's BIN. A MIT card fetched from the locker
    /// (`CardDetailsForNetworkTransactionId`) can arrive without a network, and
    /// cert-critical fields (cardOnFile / cardOnFileTransactionIdentifier) are
    /// gated on the family, so the network must still be recognised from the PAN.
    pub fn from_card_number(card_number: &str) -> Self {
        use domain_types::utils::CardIssuer;
        match domain_types::utils::get_card_issuer(card_number) {
            Ok(CardIssuer::Visa) => Self::Visa,
            Ok(CardIssuer::Master | CardIssuer::Maestro) => Self::Mastercard,
            Ok(CardIssuer::AmericanExpress) => Self::Amex,
            Ok(CardIssuer::Discover) => Self::Discover,
            Ok(CardIssuer::JCB) => Self::Jcb,
            Ok(CardIssuer::DinersClub | CardIssuer::CarteBlanche | CardIssuer::CartesBancaires) => {
                Self::Diners
            }
            Ok(CardIssuer::UnionPay) => Self::UnionPay,
            // The shared `get_card_issuer` DinersClub regex only matches the
            // legacy 14-digit format; modern Diners cards (which TSYS routes via
            // the Discover network) are 16 digits and fall through. Recognise
            // the Diners Club BIN ranges (300–305, 3095, 36, 38–39) length-
            // agnostically so the Discover-family recurring/installment tags
            // still fire. JCB (35xx) is already matched above, so it can't be
            // incorrectly classified here.
            Err(_) if is_diners_bin(card_number) => Self::Diners,
            Err(_) => Self::Unknown,
        }
    }

    /// Prefer the explicit network; fall back to BIN detection from the PAN.
    pub fn from_network_or_number(network: Option<&CardNetwork>, card_number: &str) -> Self {
        match Self::from_network(network) {
            Self::Unknown => Self::from_card_number(card_number),
            family => family,
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

/// Diners Club BIN ranges (300–305, 3095, 36, 38–39), matched length-
/// agnostically. Used only as a fallback for the modern 16-digit Diners cards
/// the shared `get_card_issuer` (14-digit-only) does not recognise. JCB (35xx)
/// is intentionally excluded so it is never incorrectly classified as Diners.
fn is_diners_bin(card_number: &str) -> bool {
    let digits: String = card_number.chars().filter(|c| c.is_ascii_digit()).collect();
    let starts = |p: &str| digits.starts_with(p);
    starts("36")
        || starts("38")
        || starts("39")
        || starts("3095")
        || (0..=5).any(|n| starts(&format!("30{n}")))
}
