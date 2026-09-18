---
{
  "slug": "domain-types",
  "title": "domain_types: the crate to import from",
  "tier": "machinery",
  "audience": "engineer",
  "one_liner": "domain_types is the Rust crate that defines Prism's core types -- the one connectors import from. A common mistake is importing the old hyperswitch_domain_models instead.",
  "analogy": "The official parts catalogue. There is one approved catalogue (domain_types) every build must order parts from. Ordering from an old catalogue (hyperswitch_domain_models) gets your build rejected.",
  "depth": {
    "tldr": "domain_types is where Prism's shared types live (RouterDataV2, flow types, payment-method data, request/response types). Connectors import from it. Do not import from hyperswitch_domain_models -- that is the old name and it is wrong here.",
    "standard": "domain_types is the canonical crate for the UCS type system: connector flow types, RouterDataV2, PaymentMethodData, request and response types, and errors. A firm convention in this codebase: import from domain_types, use RouterDataV2 and ConnectorIntegrationV2, and read auth from the request config. Using the legacy hyperswitch_domain_models names breaks the build and fails review.",
    "deep": "Located at crates/types-traits/domain_types/src/ (see lib.rs for the module map: connector_flow.rs, router_data_v2.rs, payment_method_data.rs, router_request_types.rs, router_response_types.rs). The new-connector skill lists these conventions under Critical Conventions. See router-data-v2 and transformer."
  },
  "prerequisites": ["transformer"],
  "related": ["router-data-v2", "macros", "transformer"],
  "go_deeper": [
    {"path": "crates/types-traits/domain_types/src/lib.rs", "why": "the module map of the core type crate"},
    {"path": ".skills/new-connector/SKILL.md", "why": "the import conventions (domain_types, V2 types)"}
  ],
  "verify_anchors": [
    {"path": ".skills/new-connector/SKILL.md", "must_contain": "domain_types"}
  ]
}
---
