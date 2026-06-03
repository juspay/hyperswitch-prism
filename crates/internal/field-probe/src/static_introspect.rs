//! Static introspection of connector capabilities.
//!
//! Because `ConnectorSpecifications` is a supertrait of `ConnectorServiceTrait`,
//! every connector is *guaranteed* to expose `get_supported_payment_methods()`
//! by the type system — we can dispatch through the existing
//! `ConnectorData::get_connector_by_name(&ConnectorEnum)` plumbing without a
//! hand-maintained match table.
//!
//! This is the canonical path that feeds the matrix at
//! `docs-generated/all_connector.md`. The runtime probe (see `orchestrator.rs`)
//! still exists as an opt-in cross-check.

#![allow(clippy::redundant_field_names)]

use std::collections::BTreeMap;

use common_enums::EventClass;
use connector_integration::types::ConnectorData;
use domain_types::{
    connector_types::ConnectorEnum,
    payment_method_data::DefaultPCIHolder,
    types::SupportedPaymentMethods,
};
use serde::Serialize;

/// One per-connector static capability snapshot.
///
/// The shape mirrors what `generate.py` consumes from
/// `data/connector_capabilities/<connector>.json`.
#[derive(Debug, Serialize)]
pub(crate) struct CapabilityReport {
    /// Lowercase connector name (matches the runtime probe naming convention).
    pub connector: String,

    /// `{ PaymentMethod: { PaymentMethodType: PaymentMethodDetails } }`.
    /// Serialised via the existing serde derives on the domain types.
    /// Empty `{}` is allowed (matches `EMPTY_SUPPORTED_PAYMENT_METHODS`).
    pub supported_payment_methods: BTreeMap<String, BTreeMap<String, serde_json::Value>>,

    /// Webhook event classes the connector self-reports as supported.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub supported_webhook_flows: Vec<EventClass>,

    /// Optional human-facing connector metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub about: Option<serde_json::Value>,
}

/// Builds a `CapabilityReport` for a single connector.
///
/// `DefaultPCIHolder` is used as the concrete `PaymentMethodDataTypes` —
/// `ConnectorSpecifications` does not depend on this generic parameter, so
/// the choice does not affect the declared support map.
pub(crate) fn introspect_connector(connector: &ConnectorEnum) -> CapabilityReport {
    let data: ConnectorData<DefaultPCIHolder> = ConnectorData::get_connector_by_name(connector);
    let conn = &data.connector;

    let pms = normalize_supported(conn.get_supported_payment_methods());
    let webhooks = conn
        .get_supported_webhook_flows()
        .map(<[_]>::to_vec)
        .unwrap_or_default();
    let about = conn
        .get_connector_about()
        .and_then(|info| serde_json::to_value(info).ok());

    CapabilityReport {
        connector: format!("{connector:?}").to_lowercase(),
        supported_payment_methods: pms,
        supported_webhook_flows: webhooks,
        about,
    }
}

/// Convert the `HashMap<PaymentMethod, HashMap<PaymentMethodType, _>>` shape
/// into a `BTreeMap<String, BTreeMap<String, Value>>` so the JSON output is
/// deterministically ordered and the inner `PaymentMethodDetails` keeps its
/// full structure via serde.
fn normalize_supported(
    src: &'static SupportedPaymentMethods,
) -> BTreeMap<String, BTreeMap<String, serde_json::Value>> {
    let mut out: BTreeMap<String, BTreeMap<String, serde_json::Value>> = BTreeMap::new();
    for (pm, types) in src {
        let mut inner: BTreeMap<String, serde_json::Value> = BTreeMap::new();
        for (pmt, details) in types {
            let Ok(detail_json) = serde_json::to_value(details) else {
                continue;
            };
            inner.insert(format!("{pmt:?}"), detail_json);
        }
        out.insert(format!("{pm:?}"), inner);
    }
    out
}
