//! `TxProfile` — the single bag of dimensions every TSYS wire field can
//! ever depend on.
//!
//! Derived once at the top of a flow, then passed by reference to per-field
//! "rule" functions in `rules/`. Lets the cert spec's
//! `(profile-cell, field) -> value` table map 1:1 into Rust code instead
//! of being re-derived inline at every field.
//!
//! Axes:
//!   • `acceptance`       — channel + recurring classification
//!   • `card_family`      — Visa / MC / AMEX / Discover-like
//!   • `cof_phase`        — NoCof / CitSetup / CitUsingStored / Mit(kind)
//!   • `commercial_level` — None / L2 / L3
//!   • `three_ds`         — None / Present
//!   • `capture`          — Auto / Manual

pub mod acceptance;
pub mod card_family;
pub mod cof_phase;
pub mod commercial;

use std::fmt::Debug;

use common_enums::{CaptureMethod, PaymentChannel};
use domain_types::{
    connector_flow::{Authorize, SetupMandate},
    connector_types::{
        PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData, SetupMandateRequestData,
    },
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes},
    router_data_v2::RouterDataV2,
};
use hyperswitch_masking::PeekInterface;
use serde::Serialize;

pub use acceptance::{AcceptanceProfile, TerminalDataBlock};
pub use card_family::CardFamily;
pub use cof_phase::{CofPhase, MitIntent, MitKind};
pub use commercial::CommercialLevel;

use super::transformers::TsysTransitCardDataSource;

/// Map the acceptance channel to the `cardDataSource` wire value. This is an
/// axis ORTHOGONAL to the recurring/terminal semantics: a recurring MOTO
/// card-on-file transaction keeps its PHONE/MAIL channel here while still
/// emitting the recurring indicators elsewhere. `cardDataSource=RECURRING`
/// is never emitted (TSYS rejects it on MOTO).
pub fn channel_card_data_source(channel: Option<PaymentChannel>) -> TsysTransitCardDataSource {
    match channel {
        Some(PaymentChannel::TelephoneOrder) => TsysTransitCardDataSource::Phone,
        Some(PaymentChannel::MailOrder) => TsysTransitCardDataSource::Mail,
        Some(PaymentChannel::Ecommerce) | None => TsysTransitCardDataSource::Internet,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureKind {
    Auto,
    Manual,
}

impl CaptureKind {
    pub fn from_method(method: Option<CaptureMethod>) -> Self {
        match method {
            Some(CaptureMethod::Manual) | Some(CaptureMethod::ManualMultiple) => Self::Manual,
            _ => Self::Auto,
        }
    }

    pub fn is_manual(self) -> bool {
        matches!(self, Self::Manual)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreeDsKind {
    None,
    /// 3DS data present on the request (cavv / eci / etc.). Detailed values
    /// live on `RawInputs`; profile only tracks presence so rules can
    /// branch on "is this a 3DS-authenticated tx".
    Present,
}

#[derive(Debug, Clone)]
pub struct TxProfile {
    pub acceptance: AcceptanceProfile,
    pub card_family: CardFamily,
    pub cof_phase: CofPhase,
    pub commercial_level: CommercialLevel,
    pub three_ds: ThreeDsKind,
    pub capture: CaptureKind,
    /// `cardDataSource` derived purely from the acceptance channel
    /// (PHONE/MAIL/INTERNET) — independent of recurring classification.
    pub channel_source: TsysTransitCardDataSource,
}

impl TxProfile {
    /// Derive every profile axis for an `Authorize` flow. The single place
    /// that decides which acceptance profile a transaction lives in.
    /// Per-field rule functions take this output and never re-derive these
    /// values.
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
        let (card_network, card_number) = match &request.payment_method_data {
            PaymentMethodData::Card(card) => (
                card.card_network.clone(),
                Some(card.card_number.peek().to_string()),
            ),
            PaymentMethodData::CardDetailsForNetworkTransactionId(nti) => (
                nti.card_network.clone(),
                Some(nti.card_number.peek().to_string()),
            ),
            _ => (None, None),
        };
        let acceptance = AcceptanceProfile::derive(
            request.payment_channel.clone(),
            request.mit_category.clone(),
        );
        // A MIT card fetched from the locker can arrive without a network; fall
        // back to BIN detection so cert-critical Visa fields still fire.
        let card_family = match card_number.as_deref() {
            Some(number) => CardFamily::from_network_or_number(card_network.as_ref(), number),
            None => CardFamily::from_network(card_network.as_ref()),
        };
        let cof_phase = CofPhase::derive(
            request.mandate_id.as_ref(),
            request.mit_category.clone(),
            request.setup_future_usage,
            request.off_session,
        );

        // Commercial level — look at the same signals the original
        // `compute_commercial_card_context` used: L2L3 line items / tax /
        // shipping / duty either on the request or in resource_common_data.
        let l2_l3 = router_data.resource_common_data.l2_l3_data.as_deref();
        let has_order_details = l2_l3
            .and_then(|d| d.get_order_details())
            .map(|details| !details.is_empty())
            .unwrap_or_else(|| {
                router_data
                    .resource_common_data
                    .order_details
                    .as_ref()
                    .is_some_and(|d| !d.is_empty())
            });
        let has_tax_amount = l2_l3.and_then(|d| d.get_order_tax_amount()).is_some()
            || request.order_tax_amount.is_some();
        let has_shipping_charges =
            l2_l3.and_then(|d| d.get_shipping_cost()).is_some() || request.shipping_cost.is_some();
        let has_duty_charges = l2_l3.and_then(|d| d.get_duty_amount()).is_some();
        let commercial_level = CommercialLevel::derive(
            has_order_details,
            has_tax_amount,
            has_shipping_charges,
            has_duty_charges,
        );

        let three_ds = match request
            .authentication_data
            .as_ref()
            .and_then(|d| d.cavv.as_ref())
        {
            Some(cavv) if !cavv.peek().is_empty() => ThreeDsKind::Present,
            _ => ThreeDsKind::None,
        };
        let capture = CaptureKind::from_method(request.capture_method);
        let channel_source = channel_card_data_source(request.payment_channel.clone());

        Self {
            acceptance,
            card_family,
            cof_phase,
            commercial_level,
            three_ds,
            capture,
            channel_source,
        }
    }

    /// Derive profile axes for a `SetupMandate` / CardAuthentication flow.
    /// Card auth has no commercial level or 3DS; capture is always Auto
    /// (the auth is a self-contained 0.00 / nominal probe).
    pub fn derive_for_card_authentication<T>(
        router_data: &RouterDataV2<
            SetupMandate,
            PaymentFlowData,
            SetupMandateRequestData<T>,
            PaymentsResponseData,
        >,
    ) -> Self
    where
        T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize,
    {
        let request = &router_data.request;
        let card_network = match &request.payment_method_data {
            PaymentMethodData::Card(card) => card.card_network.clone(),
            _ => None,
        };
        let acceptance = AcceptanceProfile::derive(request.payment_channel.clone(), None);
        let card_family = CardFamily::from_network(card_network.as_ref());
        let cof_phase = CofPhase::derive(
            request.mandate_id.as_ref(),
            None,
            request.setup_future_usage,
            request.off_session,
        );
        Self {
            acceptance,
            card_family,
            cof_phase,
            commercial_level: CommercialLevel::None,
            three_ds: ThreeDsKind::None,
            capture: CaptureKind::Auto,
            channel_source: channel_card_data_source(request.payment_channel.clone()),
        }
    }
}
