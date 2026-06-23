/** Builds the end-to-end integration walkthrough, shared by the tool and the resource. */
import type { Framework, Flow } from "./constants.js";
import { REPO_URL, LLM_TXT_URL, CONFIG_ENV_VAR } from "./constants.js";
import { getConnector } from "./data/connectors.js";

export interface GuideStep {
  step: number;
  title: string;
  detail: string;
  tool?: string;
}

const FRAMEWORK_NOTES: Record<Framework, string> = {
  express: "Mount handlers as POST routes on an Express Router; parse JSON bodies with express.json().",
  next: "Implement each flow as a Next.js Route Handler (app/api/<flow>/route.ts) using the POST export.",
  nestjs: "Wrap the client in an injectable PaymentsService; expose flows via a controller with @Post() routes.",
  fastify: "Register handlers as Fastify routes (fastify.post('/<flow>', ...)); the client is a singleton plugin.",
  node: "Plain Node: export async functions per flow; call them from your own HTTP layer or a script.",
};

export function buildGuide(
  framework: Framework,
  flows: Flow[],
  connectorName?: string,
): { markdown: string; steps: GuideStep[]; framework: Framework; flows: Flow[]; connector: string | null } {
  const connector = connectorName ? getConnector(connectorName) : undefined;
  const connDisplay = connector ? `${connector.displayName} (\`${connector.connector}\`)` : "your chosen connector";
  const exampleConnector = connector?.connector ?? "stripe";

  const steps: GuideStep[] = [
    {
      step: 1,
      title: "Detect the stack & install the SDK",
      detail:
        `Confirm Node >= 18 and a ${framework} app. Install the SDK: \`npm install hyperswitch-prism\`. ` +
        `Run prism_doctor to confirm the native FFI binary supports this platform (linux-x64 / macOS only).`,
      tool: "prism_doctor",
    },
    {
      step: 2,
      title: "Pick the connector & fetch its exact credentials",
      detail:
        `Look up the precise credential fields for ${connDisplay} — names, which are secrets, the config template, ` +
        `and sandbox test cards. Never guess field names.`,
      tool: "prism_connector_requirements",
    },
    {
      step: 3,
      title: "Generate config & .env",
      detail:
        `Produce the ${CONFIG_ENV_VAR} JSON (env-driven) plus .env / .env.example. Keep secrets in env, never in code. ` +
        `Then sanity-check with prism_validate_config.`,
      tool: "prism_generate_config",
    },
    {
      step: 4,
      title: "Scaffold the client + flow handlers",
      detail:
        `Generate ${framework} code for the requested flows (${flows.join(", ")}). ${FRAMEWORK_NOTES[framework]} ` +
        `The scaffold uses real SDK types and numeric status checks (status is a NUMBER).`,
      tool: "prism_scaffold_integration",
    },
    {
      step: 5,
      title: "Handle statuses & errors correctly",
      detail:
        "Compare response.status against numeric enums (AUTHORIZED=6, CHARGED=8, VOIDED=11, FAILURE=21). " +
        "A soft decline is status FAILURE in the body (no throw). Hard failures throw IntegrationError / ConnectorError / " +
        "NetworkError — wrap calls in try/catch. Use prism_explain_error on anything unexpected.",
      tool: "prism_status_reference",
    },
    {
      step: 6,
      title: "Prove it with a sandbox test charge",
      detail:
        `Set ${CONFIG_ENV_VAR} to your sandbox creds and run a real sandbox authorize to reach AUTHORIZED/CHARGED. ` +
        `This confirms the whole path end-to-end before you wire up your UI.`,
      tool: "prism_test_charge",
    },
  ];

  const flowGuidance: string[] = [];
  if (flows.includes("authorize")) flowGuidance.push("**authorize** — charge a card; AUTOMATIC capture → CHARGED(8), MANUAL → AUTHORIZED(6).");
  if (flows.includes("capture")) flowGuidance.push("**capture** — capture a previously AUTHORIZED payment (needs connectorTransactionId) → CHARGED(8).");
  if (flows.includes("void")) flowGuidance.push("**void** — cancel an authorization before capture → VOIDED(11).");
  if (flows.includes("refund")) flowGuidance.push("**refund** — refund a captured payment; uses RefundStatus, success = REFUND_SUCCESS(4).");
  if (flows.includes("sync")) flowGuidance.push("**sync** — `client.get(...)` to poll a PENDING(20) payment to its terminal status.");
  if (flows.includes("webhook")) flowGuidance.push("**webhook** — verify & parse incoming connector events with EventClient to update payment state asynchronously.");

  const markdown =
    `# Integrate hyperswitch-prism into a ${framework} app\n\n` +
    `Connector: ${connDisplay}. Flows: ${flows.join(", ")}.\n\n` +
    `Follow these steps in order — each maps to an MCP tool you should call:\n\n` +
    steps.map((s) => `${s.step}. **${s.title}**${s.tool ? ` → \`${s.tool}\`` : ""}\n   ${s.detail}`).join("\n\n") +
    `\n\n## Flow notes\n${flowGuidance.map((f) => `- ${f}`).join("\n")}\n\n` +
    `## Reference\n- SDK quick-start example (connector \`${exampleConnector}\`): call \`prism_scaffold_integration\`.\n` +
    `- Official LLM reference: ${LLM_TXT_URL}\n- Repo: ${REPO_URL}\n`;

  return { markdown, steps, framework, flows, connector: connector?.connector ?? null };
}
