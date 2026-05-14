import type { TaskDefinition } from "../types.js";

/**
 * Tech Spec Agent Prompt
 *
 * Instructs the model to read and follow grace/workflow/2.2_techspec.md exactly,
 * with one exception: skip Phase 1c (grace techspec CLI).
 */

export const TECHSPEC_AGENT_SYSTEM = `You are the Tech Spec Agent.

Your task is to generate a technical specification by following the official Grace workflow.

## Instructions

1. Use read_file to read:
   /Users/tushar.shukla/Downloads/Work/euler-ucs/hyperswitch-prism/grace/workflow/2.2_techspec.md

2. Follow the instructions in that file EXACTLY as written, with ONE EXCEPTION:

   **EXCEPTION - Skip Phase 1c:**
   - Do NOT execute the command: \`grace techspec {Connector_Name} -e\`
   - Do NOT activate virtualenv or switch to grace/ directory for CLI execution
   - Instead, generate the technical specification CONTENT directly using the URLs

3. Execute all other phases exactly as written:
   - Phase 1a: Extract URLs from data/integration-source-links.json
   - Phase 1b: Create the URL file ({connector}.txt)
   - Phase 1d: Verify tech spec output (REQUIRED)

4. Generate the technical specification with these 8 sections:
   1. **Connector Profile** - Name, primary flow scope, API family, hosts (production/sandbox/regional)
   2. **Authentication** - Scheme (e.g., HTTP Signature HMAC-SHA256), credentials required, implementation notes
   3. **Supported Flows** - Markdown table with columns: Flow | HTTP | Path | Notes
      (Include: Authorize, Capture, Refund, Credit, Void, PSync, RSync, Webhooks if applicable)
   4. **Request Schema Highlights** - Key fields, enums, required vs optional, idempotency keys
   5. **Response Schema Highlights** - Transaction IDs, status values, timestamps, links
   6. **Error Handling** - Markdown table with columns: HTTP | \`status\` | Cause
   7. **Webhooks / Async Notifications** - Subscription, delivery format, verification, retry policy, documented gaps
   8. **References** - All documentation URLs used

## Output Format

Return ONLY valid JSON. Include TWO parts:

1. The exact format specified in 2.2_techspec.md:
\`\`\`
CONNECTOR: {CONNECTOR}
FLOW: {FLOW}
STATUS: SUCCESS | FAILED
TECHSPEC_PATH: <path>
REASON: <details, if failed>
\`\`\`

2. Plus a JSON object with this EXACT structure:
\`\`\`json
{
  "success": true,
  "connector": "Cybersource",
  "flow": "BankDebit",
  "specPath": "techspecs/Cybersource_BankDebit_spec.md",
  "specContent": "# Technical Specification\\n\\n## Connector Profile\\n...",
  "executionLog": {
    "workflowFile": "grace/workflow/2.2_techspec.md",
    "workflowRead": true,
    "phasesCompleted": ["1a", "1b", "1d"],
    "phase1cSkipped": true,
    "commandsExecuted": [
      {
        "command": "cat data/integration-source-links.json",
        "workingDir": "/workspace",
        "output": "file contents...",
        "durationMs": 100,
        "status": "success"
      }
    ],
    "filesCreated": [
      {
        "path": "techspecs/Cybersource_BankDebit_spec.md",
        "description": "Technical specification content",
        "sizeBytes": 8500
      }
    ]
  }
}
\`\`\`

CRITICAL RULES:
1. Read the workflow file FIRST before taking any action
2. Follow the workflow instructions EXACTLY (except Phase 1c)
3. Phase 1d (Verify output) is REQUIRED - do not skip
4. Generate ALL 8 sections of the tech spec
5. Use markdown tables for Supported Flows and Error Handling
6. **specPath is REQUIRED** - must be "techspecs/{Connector}_{Flow}_spec.md"
7. **commandsExecuted MUST be objects** with: command, workingDir, output, durationMs, status
8. Return ONLY the JSON object - no markdown, no preamble
`;

/**
 * Build the user payload for Tech Spec Agent
 */
export function buildTechspecAgentUserPayload(
  connector: string,
  paymentMethod: string,
  task?: TaskDefinition,
): Record<string, unknown> {
  return {
    connector,
    ConnectorName: connector,
    connector_lc: connector.toLowerCase(),
    paymentMethod,
    flow: paymentMethod,
    workflowFile:
      "/Users/tushar.shukla/Downloads/Work/euler-ucs/hyperswitch-prism/grace/workflow/2.2_techspec.md",
    skipPhase: "1c",
    generateInsteadOfCLI: true,
    // Full task context - all fields from task creation
    task: task
      ? {
          title: task.title,
          description: task.description,
          acceptanceCriteria: task.acceptanceCriteria,
          connectorDocUrls: task.connectorDocUrls,
          targetFiles: task.targetFiles,
          projectRoot: task.projectRoot,
          // Grace/10XGRACE workflow fields
          paymentMethod: task.paymentMethod,
          targetConnectors: task.targetConnectors,
          paymentMethodCategory: task.paymentMethodCategory,
          priority: task.priority,
          prerequisites: task.prerequisites,
          estimatedComplexity: task.estimatedComplexity,
        }
      : undefined,
  };
}

/**
 * Result type from Tech Spec Agent
 */
export interface TechspecAgentResult {
  success: boolean;
  connector: string;
  flow: string;
  specPath: string;
  specContent: string;
  executionLog: {
    workflowFile: string;
    workflowRead: boolean;
    phasesCompleted: string[];
    phase1cSkipped: boolean;
    commandsExecuted: Array<{
      command: string;
      workingDir: string;
      output?: string;
      durationMs?: number;
      status: "success" | "failed";
    }>;
    filesCreated: Array<{
      path: string;
      description: string;
      sizeBytes?: number;
    }>;
  };
}
