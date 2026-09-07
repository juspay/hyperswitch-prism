---
{
  "slug": "grace",
  "title": "Grace: AI-assisted connector code generation",
  "tier": "machinery",
  "audience": "engineer",
  "one_liner": "Grace is the system that generates most of a new connector for you: you write a technical spec from the processor's API docs, run Grace, and it produces roughly 70% of the Rust code.",
  "analogy": "A prefab house kit. Instead of cutting every board by hand, you provide the blueprint (the tech spec) and the kit produces the framed structure. You finish the details.",
  "depth": {
    "tldr": "Grace turns a processor's API documentation into most of a connector's code. The flow: write a tech spec, run Grace, get generated Rust, then refine and test. It makes adding connectors fast.",
    "standard": "Grace is the AI-assisted codegen system for UCS connectors. You first produce a technical specification (the generate-tech-spec skill discovers the processor's API docs and structures them). Grace then generates connector foundation and flow code from that spec following the codegen rulesbook. The result is reviewed against a quality checklist and tested via gRPC. Grace supports the whole lifecycle: new connectors, adding flows, and adding payment methods.",
    "deep": "The rulesbook lives at grace/rulesbook/codegen/ (README is the 600-line reference; .gracerules controls full connector generation, with variants for adding flows and payment methods). Tech specs are saved under grace/rulesbook/codegen/references/. See generate-tech-spec and new-connector skills, and the connector card."
  },
  "prerequisites": ["connector"],
  "related": ["connector", "transformer", "payment-proto"],
  "go_deeper": [
    {"path": "grace/rulesbook/codegen/README.md", "why": "the GRACE-UCS codegen reference"},
    {"path": ".skills/generate-tech-spec/SKILL.md", "why": "producing the tech spec Grace consumes"}
  ],
  "verify_anchors": []
}
---
