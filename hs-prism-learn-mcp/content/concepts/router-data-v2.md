---
{
  "slug": "router-data-v2",
  "title": "RouterDataV2: the data a flow carries",
  "tier": "machinery",
  "audience": "engineer",
  "one_liner": "RouterDataV2 is the struct that carries everything a flow needs -- the request, the place for the response, the connector config, and flow metadata -- through the connector code.",
  "analogy": "A job folder passed down an assembly line. It holds the order, a slot for the result, and the worker's credentials. Each station reads and updates the same folder. RouterDataV2 is that folder.",
  "depth": {
    "tldr": "RouterDataV2 is the bundle of data a connector works with for one flow: the incoming request, somewhere to put the response, the credentials/config, and which flow it is. Transformers read from it and write to it.",
    "standard": "RouterDataV2 is the generic container the connector traits operate on. It carries the flow type, the request data, the response slot, and the connector configuration. Transformers implement TryFrom<RouterDataV2<...>> to build a processor request, and produce a RouterDataV2 with the response filled in. This repo uses the V2 type exclusively -- the older RouterData does not exist here, which is a common newcomer mistake.",
    "deep": "Defined in crates/types-traits/domain_types/src/router_data_v2.rs. Always import V2 types from domain_types (not hyperswitch_domain_models) and use ConnectorIntegrationV2. See transformer, domain-types, and the troubleshoot entry for 'RouterData not found'."
  },
  "prerequisites": ["transformer"],
  "related": ["domain-types", "transformer", "macros"],
  "go_deeper": [
    {"path": "crates/types-traits/domain_types/src/router_data_v2.rs", "why": "the RouterDataV2 definition"},
    {"path": ".skills/_shared/references/type-system.md", "why": "how RouterDataV2 fits the type system"}
  ],
  "verify_anchors": [
    {"path": ".skills/_shared/references/type-system.md", "must_contain": "RouterDataV2"}
  ]
}
---
