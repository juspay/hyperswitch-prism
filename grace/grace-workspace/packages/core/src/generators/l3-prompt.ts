import type { PipelineContext } from "../types.js";

export const L3_SYSTEM = `You are a senior payment systems engineer producing a BROAD architectural breakdown for implementing a payment method across connectors in the hyperswitch-prism system.

Your input is the L2 spec (summary, scope, technical constraints) for implementing task.paymentMethod across task.targetConnectors.

=== OUTPUT RULES ===
Stay HIGH-LEVEL. Think in terms of connector implementations, not individual file edits.

Rules:
- Produce tasks for EACH target connector (e.g., "Implement ApplePay for Stripe", "Implement ApplePay for Adyen")
- Each task represents implementing the payment method for one connector
- task.description is ONE sentence describing the intent
- Use "backend" array for connector implementation tasks (payment connector work is backend)
- "frontend" can be empty or minimal for connector-only work
- "shared" for any common types/utilities across connectors
- Capture: architectureNotes (how connectors integrate), dataFlow (RouterDataV2 → ConnectorRequest → Response)
- dependsOn only when genuinely needed (usually independent per connector)

You MUST return ONLY valid JSON:
{
  "architectureNotes": "string — how the payment method integrates with existing connectors",
  "dataFlow": "string — RouterDataV2 → ConnectorRequest → ConnectorResponse → RouterDataV2",
  "affectedModules": ["crates/.../connectors/stripe/", "crates/.../connectors/adyen/"],
  "frontend": [],
  "backend": [
    { "id": "stripe-applepay", "title": "Implement ApplePay for Stripe", "description": "Add ApplePay transformers and types for Stripe connector", "dependsOn": [] },
    { "id": "adyen-applepay", "title": "Implement ApplePay for Adyen", "description": "Add ApplePay transformers and types for Adyen connector", "dependsOn": [] }
  ],
  "shared": []
}`;

export function buildL3User(ctx: PipelineContext): string {
  const base: Record<string, unknown> = {
    l2_spec: ctx.artifacts.l2,
    task: ctx.artifacts.task,
  };
  const regen = ctx.artifacts.l3RegeneratePrompt;
  if (regen) {
    base.regenerationNote =
      "The previous L3 spec was rejected by the human reviewer. Incorporate this feedback.";
    base.reviewerGuidance = regen;
    base.previousRejectedSpec = ctx.artifacts.previousL3;
  }
  return JSON.stringify(base, null, 2);
}
