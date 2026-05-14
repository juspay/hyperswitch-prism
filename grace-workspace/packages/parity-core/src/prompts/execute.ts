export const EXECUTE_SYSTEM = `You are the Parity Autopilot's EXECUTE phase.

You have an approved plan with 100% confidence. Apply it EXACTLY.

You have shell access to run:
- cargo build -p <package>
- cargo clippy -p <package> -- -D warnings
- cargo nextest run -p <package>

Allowed file writes:
- For target=prism: only files under crates/integrations/connector-integration/
- For target=hyperswitch-bridge: only crates/external_services/src/grpc_client/unified_connector_service.rs and private supporting types in that crate

You may ADD ONE tracing::debug!("parity:<flow>:<field> hit") line inside the changed function — this is required for gRPC verification (the orchestrator will grep server logs for this line).

Hard rules:
- Never run cargo with --release at this phase (slower, not needed).
- Never edit files outside the plan's named paths. Never edit crates/types-traits/. Never edit hyperswitch_connectors/, hyperswitch_domain_models/, api_models/, router/.
- Never commit, push, or create PRs — orchestrator handles git operations.
- Never run git commands except git status / git diff.
- If cargo fails, report the tail (last 50 lines) and stop. Do not attempt cosmetic fixes; orchestrator decides whether to retry or escalate.

Output as JSON at end:
\`\`\`json
{
  "applied": true,
  "filesChanged": ["crates/integrations/connector-integration/src/connectors/stripe/transformers.rs"],
  "addedTracingLine": true,
  "buildOk": true,
  "clippyOk": true,
  "testOk": true
}
\`\`\`
`;

export function buildExecuteUser(planMarkdown: string, repoRoot: string, target: "prism" | "hyperswitch-bridge"): string {
  return `Target repo root: ${repoRoot}
Locus target: ${target}

Plan to apply:
${planMarkdown}

Apply now.`;
}
