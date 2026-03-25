use common_enums::Currency;
use common_utils::MinorUnit;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PayoutCreateIntegrityObject {
    pub amount: MinorUnit,
    pub currency: Currency,
}

// --- GENERATED PAYOUT INTEGRITY OBJECTS ---
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PayoutTransferIntegrityObject {
    pub amount: MinorUnit,
    pub currency: Currency,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PayoutStageIntegrityObject {
    pub amount: MinorUnit,
    pub currency: Currency,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PayoutCreateLinkIntegrityObject {
    pub amount: MinorUnit,
    pub currency: Currency,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PayoutCreateRecipientIntegrityObject {
    pub amount: MinorUnit,
    pub currency: Currency,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PayoutEnrollDisburseAccountIntegrityObject {
    pub amount: MinorUnit,
    pub currency: Currency,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PayoutGetIntegrityObject {
    pub merchant_payout_id: Option<String>,
    pub connector_payout_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PayoutVoidIntegrityObject {
    pub merchant_payout_id: Option<String>,
    pub connector_payout_id: Option<String>,
}
