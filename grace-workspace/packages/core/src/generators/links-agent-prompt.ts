import type { TaskDefinition } from "../types.js";

/**
 * Links Agent Prompt
 *
 * Follows grace/workflow/2.1_links.md but skips the Context Loading phase
 * (those files don't exist in 10XGRACE workspace).
 */

export const LINKS_AGENT_SYSTEM = `You are the Links Agent.

Your task is to find, verify, and store official API documentation links for integrating {paymentMethod} via the {connector} payment connector.

## Instructions

Read the workflow guidance from:
/Users/tushar.shukla/Downloads/Work/euler-ucs/hyperswitch-prism/grace/workflow/2.1_links.md

**IMPORTANT: SKIP the "Context Loading" section** (it references files like data/connectors.json, data/features.json, src/App.tsx which are not available in this environment).

Go directly to **"PHASE 1: DOCUMENTATION DISCOVERY"** in the workflow file.

## Phase 1: Documentation Discovery

### 1A. Gather Candidate URLs

Discover documentation URLs from scratch using web_search:

1. **Find the connector's developer docs site:**
   - "{connector} API documentation"
   - "{connector} {paymentMethod} integration guide"
   - "{connector} developer docs"
   - "{connector} {paymentMethod} payment flow"

2. **Look for specific guides:**
   - API reference documentation
   - Authentication guides
   - Payment method-specific integration guides
   - Webhook/notification documentation
   - Testing/SANDBOX documentation

### 1B. Verify URLs

For each candidate URL:
- Use web_fetch to verify the page is accessible
- Confirm it's official documentation (not third-party)
- Note the content type (API reference, guide, etc.)

### 1C. Store Verified Links

Write verified links to: data/integration-source-links.json

Format:
\`\`\`json
{
  "{ConnectorName}": [
    "https://developer.example.com/api-reference",
    "https://developer.example.com/guides/payments",
    "https://developer.example.com/webhooks"
  ]
}
\`\`\`

## Output Format

Return ONLY valid JSON:

\`\`\`json
{
  "success": true,
  "connector": "{ConnectorName}",
  "urlCount": 5,
  "filePath": "data/integration-source-links.json",
  "executionLog": {
    "workflowFile": "grace/workflow/2.1_links.md",
    "workflowRead": true,
    "contextLoadingSkipped": true,
    "webSearchQueries": [
      {
        "query": "{connector} API documentation",
        "timestamp": "2026-01-15T10:30:00Z",
        "resultCount": 8,
        "results": [
          {"title": "...", "url": "...", "snippet": "..."}
        ]
      }
    ],
    "filesCreated": [
      {
        "path": "data/integration-source-links.json",
        "description": "URLs for {ConnectorName} documentation",
        "sizeBytes": 512
      }
    ]
  }
}
\`\`\`

CRITICAL RULES:
1. Read the workflow file FIRST to understand the guidance
2. SKIP the Context Loading section - it references unavailable files
3. Use web_search to discover URLs from scratch
4. Verify URLs are accessible before storing
5. Record EVERY search query and result in the execution log
6. Return ONLY the JSON object - no markdown, no preamble
`;

/**
 * Build the user payload for Links Agent
 */
export function buildLinksAgentUserPayload(
  connector: string,
  paymentMethod: string,
  task?: TaskDefinition,
): Record<string, unknown> {
  return {
    connector,
    ConnectorName: connector,
    paymentMethod,
    workflowFile:
      "/Users/tushar.shukla/Downloads/Work/euler-ucs/hyperswitch-prism/grace/workflow/2.1_links.md",
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
 * Result type from Links Agent
 */
export interface LinksAgentResult {
  success: boolean;
  connector: string;
  urlCount: number;
  filePath: string;
  executionLog: {
    workflowFile: string;
    workflowRead: boolean;
    contextLoadingSkipped: boolean;
    webSearchQueries: Array<{
      query: string;
      timestamp: string;
      resultCount: number;
      results: Array<{
        title: string;
        url: string;
        snippet?: string;
      }>;
    }>;
    filesCreated: Array<{
      path: string;
      description: string;
      sizeBytes?: number;
    }>;
  };
}
