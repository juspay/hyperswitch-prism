---
{
  "slug": "macros",
  "title": "Macros: the boilerplate generators",
  "tier": "machinery",
  "audience": "engineer",
  "one_liner": "Two macros -- create_all_prerequisites! and macro_connector_implementation! -- generate the repetitive trait code so a connector only writes the parts that differ per processor.",
  "analogy": "A fill-in-the-blanks contract template. The macro is the template with all the legal boilerplate; you only fill the blanks (this processor's specifics). It prints the full contract for you.",
  "depth": {
    "tldr": "Macros write the repetitive connector code for you. You declare which flows a connector supports, and the macros generate the trait implementations. Every flow must appear in BOTH macros.",
    "standard": "Implementing each flow's trait by hand would be hundreds of lines of identical boilerplate per connector. Instead, create_all_prerequisites! declares the flows and shared types a connector supports, and macro_connector_implementation! generates the ConnectorIntegrationV2 implementation for each flow. The rule that trips people up: a flow must be listed in both macros, or it will not be wired up.",
    "deep": "See stripe.rs for both macros in a real connector, and macro-reference.md for the full reference. The transformers you write are what the generated code calls into. See connector, transformer, and router-data-v2."
  },
  "prerequisites": ["connector"],
  "related": ["connector", "transformer", "router-data-v2"],
  "go_deeper": [
    {"path": ".skills/_shared/references/macro-reference.md", "why": "the full macro reference"},
    {"path": "crates/integrations/connector-integration/src/connectors/stripe.rs", "why": "both macros used in a real connector"}
  ],
  "verify_anchors": [
    {"path": ".skills/_shared/references/macro-reference.md", "must_contain": "create_all_prerequisites"},
    {"path": "crates/integrations/connector-integration/src/connectors/stripe.rs", "must_contain": "create_all_prerequisites"}
  ]
}
---
