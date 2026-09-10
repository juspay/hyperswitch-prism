use std::collections::{BTreeMap, HashMap};

use domain_types::connector_types::ConnectorEnum;
use grpc_api_types::payments::PaymentMethod;

use crate::auth::{dummy_auth, load_config, make_masked_metadata};
use crate::config::get_config;
use crate::flow_registry::{probe_flow_by_definition, FLOW_DEFINITIONS};
use crate::types::*;

pub(crate) fn probe_connector(connector: &ConnectorEnum) -> ConnectorResult {
    let name = format!("{connector:?}").to_lowercase();
    let config = load_config();
    let metadata = make_masked_metadata();
    let pm_variants: HashMap<String, PaymentMethod> = get_config()
        .get_enabled_payment_methods()
        .into_iter()
        .map(|(k, build)| {
            let mut pm = build();
            crate::registry::apply_wallet_token_override(&mut pm, connector, k);
            (k.to_string(), pm)
        })
        .collect();

    let mut flows: BTreeMap<String, BTreeMap<String, FlowResult>> = BTreeMap::new();

    // Probe all flows defined in FLOW_DEFINITIONS
    for def in FLOW_DEFINITIONS {
        let auth = dummy_auth(connector);

        if let Some(results) =
            probe_flow_by_definition(def, connector, &config, auth, &metadata, &pm_variants)
        {
            flows.insert(def.key.to_string(), results);
        }
    }

    ConnectorResult {
        connector: name,
        flows,
    }
}
