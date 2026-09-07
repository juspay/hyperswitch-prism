---
{
  "slug": "repo-tour",
  "title": "A tour of the repo's main folders",
  "tier": "orientation",
  "audience": "everyone",
  "one_liner": "Six folders matter most: crates/ (the Rust core + connectors), .skills/ (how-to guides), docs/ (concepts), grace/ (code generation), docs-generated/ (reference data), and sdk/ (language clients).",
  "analogy": "A building directory in a lobby. Before wandering the halls, you read the board: floor 3 is engineering (crates/), floor 2 is the handbook (.skills/), floor 1 is reception (docs/). This card is that board.",
  "depth": {
    "tldr": "crates/ = the real code and all connectors. .skills/ = step-by-step how-to guides. docs/ = explanations and concepts. grace/ = the connector code generator. docs-generated/ = auto-built reference (glossary, coverage). sdk/ = the language libraries apps use.",
    "standard": "crates/ holds the Rust workspace: integrations/connector-integration/src/connectors/ is where all ~95 connectors live, and types-traits/ holds the domain types and the proto contract. .skills/ holds task playbooks (new-connector, add-connector-flow, add-payment-method, pr-reviewer, ...). docs/ holds architecture concepts and getting-started guides. grace/ holds the AI-assisted codegen system. docs-generated/ holds machine-built references (glossary, the all_connector coverage matrix). sdk/ holds the Python/Node/Java/Rust clients.",
    "deep": "Use repo_map (the tool) to jump from a topic to an exact path. The connector registry is crates/integrations/connector-integration/src/connectors.rs; the proto is crates/types-traits/grpc-api-types/proto/payment.proto; shared how-to references are under .skills/_shared/references/."
  },
  "prerequisites": [],
  "related": ["ucs-vs-sdk", "connector", "grace"],
  "go_deeper": [
    {"path": "docs/SUMMARY.md", "why": "the documentation table of contents"},
    {"path": "README.md", "why": "the project overview"}
  ],
  "verify_anchors": []
}
---
