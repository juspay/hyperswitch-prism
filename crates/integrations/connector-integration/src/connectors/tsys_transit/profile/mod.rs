//! `TxProfile` — the single bag of dimensions every TSYS wire field can
//! ever depend on.
//!
//! Derived once at the top of a flow, then passed by reference to per-field
//! "rule" functions. Lets the cert spec's `(profile-cell, field) -> value`
//! table map 1:1 into Rust code, instead of the rules being re-derived
//! inline at every field.

pub mod acceptance;
pub mod card_family;
pub mod cof_phase;
pub mod commercial;

use std::fmt::Debug;

use common_enums::CaptureMethod;
use domain_types::{
    connector_flow::Authorize,
    connector_types::{PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData},
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes},
};
use domain_types::router_data_v2::RouterDataV2;
use hyperswitch_masking::PeekInterface;
use serde::Serialize;

pub use acceptance::{AcceptanceProfile, TerminalDataBlock};
pub use card_family::CardFamily;
pub use cof_phase::{CofPhase, MitIntent, MitKind};
pub use commercial::CommercialLevel;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureKind {
    Auto,
    Manual,
}

#[allow(dead_code)]
impl CaptureKind {
    pub fn from(method: Option<CaptureMethod>) -> Self {
        match method {
            Some(CaptureMethod::Manual) | Some(CaptureMethod::ManualMultiple) => Self::Manual,
            _ => Self::Auto,
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreeDsKind {
    None,
    /// 3DS data present on the request (cavv / eci / etc.). The detailed
    /// values stay on `RawInputs`; profile only tracks presence so rules
    /// can branch on "is this a 3DS-authenticated tx".
    Present,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct TxProfile {
    pub acceptance: AcceptanceProfile,
    pub card_family: CardFamily,
    pub cof_phase: CofPhase,
    pub commercial_level: CommercialLevel,
    pub three_ds: ThreeDsKind,
    pub capture: CaptureKind,
}

#[allow(dead_code)]
impl TxProfile {
    /// Derive every profile axis for an `Authorize` flow. The single place
    /// that decides which acceptance profile a transaction lives in. Per-field
    /// rule functions take this output and never re-derive these values.
    ///
    /// Commercial level stays at `None` for now — the rules-extraction PR
    /// will fold the existing `compute_commercial_card_context` decision in.
    pub fn derive_for_authorize<T>(
        router_data: &RouterDataV2<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<T>,
            PaymentsResponseData,
        >,
    ) -> Self
    where
        T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize,
    {
        let request = &router_data.request;
        let card_network = match &request.payment_method_data {
            PaymentMethodData::Card(card) => card.card_network.clone(),
            PaymentMethodData::CardDetailsForNetworkTransactionId(nti) => nti.card_network.clone(),
            _ => None,
        };

        let acceptance =
            AcceptanceProfile::derive(request.payment_channel.clone(), request.mit_category.clone());
        let card_family = CardFamily::from_network(card_network.as_ref());
        let cof_phase = CofPhase::derive(
            request.mandate_id.as_ref(),
            request.mit_category.clone(),
            request.setup_future_usage,
            request.off_session,
        );
        let three_ds = match request
            .authentication_data
            .as_ref()
            .and_then(|d| d.cavv.as_ref())
        {
            Some(cavv) if !cavv.peek().is_empty() => ThreeDsKind::Present,
            _ => ThreeDsKind::None,
        };
        let capture = CaptureKind::from(request.capture_method);

        Self {
            acceptance,
            card_family,
            cof_phase,
            commercial_level: CommercialLevel::None,
            three_ds,
            capture,
        }
    }
}
