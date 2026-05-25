import { promises as fs } from "node:fs";
import path from "node:path";
import type { Checkpoint, TaskDefinition } from "../types.js";
import { callLlm } from "../llm.js";
import { safeParseJson } from "../utils.js";

interface RequirementsResult extends NonNullable<TaskDefinition["requirements"]> {
  status: "valid" | "problematic" | "insufficient";
  overallScore: number;
}

interface ConnectorDiscovery {
  name: string;
  rootPath: string;
  files: string[];
  hasTransformerDir: boolean;
  hasTypesFile: boolean;
  mainConnectorFile?: string;
}

async function discoverConnectorFiles(
  projectRoot: string,
  connectorName: string
): Promise<ConnectorDiscovery | null> {
  // Common paths where connectors might live in hyperswitch-prism
  const searchPaths = [
    "crates/integrations/connector-integration/src/connectors",
    "crates/router/src/connector",
    "src/connectors",
    "connectors",
  ];

  for (const basePath of searchPaths) {
    const fullPath = path.join(projectRoot, basePath);
    try {
      const entries = await fs.readdir(fullPath, { withFileTypes: true });

      // Look for connector directory or file
      const connectorDir = entries.find(
        (e) =>
          e.isDirectory() &&
          e.name.toLowerCase() === connectorName.toLowerCase()
      );

      const connectorFile = entries.find(
        (e) =>
          e.isFile() &&
          e.name.toLowerCase().startsWith(connectorName.toLowerCase()) &&
          e.name.endsWith(".rs")
      );

      if (connectorDir || connectorFile) {
        const rootPath = connectorDir
          ? path.join(fullPath, connectorDir.name)
          : fullPath;

        const files: string[] = [];
        let hasTransformerDir = false;
        let hasTypesFile = false;
        let mainConnectorFile: string | undefined;

        if (connectorDir) {
          // Recursively list files in connector directory
          const walkDir = async (dir: string, prefix: string) => {
            const items = await fs.readdir(dir, { withFileTypes: true });
            for (const item of items) {
              const relativePath = path.join(prefix, item.name);
              if (item.isDirectory()) {
                if (item.name === "transformers") hasTransformerDir = true;
                await walkDir(path.join(dir, item.name), relativePath);
              } else if (item.isFile() && item.name.endsWith(".rs")) {
                files.push(relativePath);
                if (
                  item.name.toLowerCase().includes("transformer") &&
                  !mainConnectorFile
                ) {
                  mainConnectorFile = relativePath;
                }
                if (
                  item.name.toLowerCase().includes("type") &&
                  !hasTypesFile
                ) {
                  hasTypesFile = true;
                }
              }
            }
          };
          await walkDir(rootPath, "");
        }

        if (connectorFile && files.length === 0) {
          files.push(connectorFile.name);
          mainConnectorFile = connectorFile.name;
        }

        return {
          name: connectorName,
          rootPath,
          files,
          hasTransformerDir,
          hasTypesFile,
          mainConnectorFile,
        };
      }
    } catch {
      // Directory doesn't exist or can't be read, try next path
      continue;
    }
  }

  return null;
}

async function readSampleFiles(
  discovery: ConnectorDiscovery,
  maxFiles = 5
): Promise<Array<{ path: string; content: string }>> {
  const samples: Array<{ path: string; content: string }> = [];

  // Prioritize important files
  const priorityPatterns = [
    /transformers?\.rs$/i,
    /types?\.rs$/i,
    /mod\.rs$/i,
    new RegExp(`${discovery.name.toLowerCase()}\.rs$`, "i"),
    /lib\.rs$/i,
  ];

  const sortedFiles = [...discovery.files].sort((a, b) => {
    const aScore = priorityPatterns.findIndex((p) => p.test(a));
    const bScore = priorityPatterns.findIndex((p) => p.test(b));
    return (aScore === -1 ? 999 : aScore) - (bScore === -1 ? 999 : bScore);
  });

  for (const file of sortedFiles.slice(0, maxFiles)) {
    try {
      const content = await fs.readFile(
        path.join(discovery.rootPath, file),
        "utf-8"
      );
      // Truncate large files
      const truncated =
        content.length > 5000 ? content.slice(0, 5000) + "\n..." : content;
      samples.push({ path: file, content: truncated });
    } catch {
      // Skip unreadable files
    }
  }

  return samples;
}

const SYSTEM = `You are a payment method implementation researcher for the hyperswitch-prism connector system.

Your task is to analyze connector codebases and identify requirements for implementing a specific payment method.

## INPUT DATA PROVIDED
You will receive:
- paymentMethod: The payment method to implement (e.g., "ApplePay", "GooglePay", "Card")
- targetConnectors: Array of connector names to analyze
- projectRoot: The project root path
- discoveredConnectors: Actual file system data for each connector including:
  - name: Connector name
  - rootPath: Absolute path to connector files
  - files: List of Rust source files found
  - hasTransformerDir: Whether transformers subdirectory exists
  - hasTypesFile: Whether a types file exists
  - sampleFiles: Contents of key files (truncated to 5000 chars)

## ANALYSIS TASK

For each discovered connector, analyze the sample files to understand:

1. **Current Payment Methods**: Look for PaymentMethodType enums, match arms, or registrations
2. **Registration Pattern**: How are payment methods added to the connector?
3. **Transformer Pattern**: How are requests/responses transformed? Look for TryFrom implementations
4. **Type Definitions**: What types are used for payment methods?
5. **Error Handling**: What error patterns are used?
6. **Authentication**: How is auth handled?

## SCORING (Grace-style 10-Point)

Score each connector (0-10):

| # | Element | Score |
|---|---------|-------|
| 1 | Connector files located | 0-1 |
| 2 | Current payment methods identified | 0-1 |
| 3 | Registration pattern understood | 0-1 |
| 4 | Transformer pattern identified | 0-1 |
| 5 | Files to modify clearly identified | 0-1 |
| 6 | Similar payment method found as reference | 0-1 |
| 7 | Type definitions understood | 0-1 |
| 8 | Error handling pattern identified | 0-1 |
| 9 | Authentication pattern understood | 0-1 |
| 10 | Complete implementation path clear | 0-1 |

**Overall Status**:
- Score >= 7 → "valid" — Ready for implementation
- Score >= 4 and < 7 → "problematic" — Some gaps exist
- Score < 4 → "insufficient" — Not enough information

## OUTPUT FORMAT

Return ONLY a JSON object (no markdown fences, no code blocks):

{
  "status": "valid" | "problematic" | "insufficient",
  "overallScore": number (0-10),
  "connectors": [
    {
      "name": "Stripe",
      "files": {
        "root": "src/connectors/stripe/",
        "mainFiles": ["src/connectors/stripe.rs"],
        "transformers": ["src/connectors/stripe/transformers.rs"],
        "types": ["src/connectors/stripe/types.rs"]
      },
      "currentPaymentMethods": ["Card", "Wallet"],
      "filesToModify": [
        {
          "path": "src/connectors/stripe/transformers.rs",
          "reason": "Add payment method request/response transformers",
          "changeType": "modify"
        }
      ],
      "patterns": {
        "existingMethod": "Card transformer with TryFrom<RouterDataV2>",
        "registrationPattern": "create_all_connectors! macro or match statement",
        "transformerPattern": "RouterDataV2 → ConnectorRequest → ConnectorResponse → RouterDataV2"
      },
      "score": 8,
      "keyGaps": []
    }
  ],
  "commonPatterns": {
    "registration": "Description of how connectors register payment methods",
    "transformers": "Description of transformer patterns found"
  },
  "recommendations": [
    "Specific recommendation 1",
    "Specific recommendation 2"
  ]
}

CRITICAL RULES:
- Return ONLY the JSON object
- NO markdown code fences
- NO explanatory text
- overallScore MUST be a number (0-10)
- status MUST be exactly: "valid", "problematic", or "insufficient"`;

export const requirementsCheckpoint: Checkpoint = {
  id: "requirements",
  name: "Requirements Discovery",
  description:
    "Analyze connector codebases to identify implementation requirements.",
  retryFrom: "requirements",
  maxRetries: 2,
  timeout: 30 * 60 * 1000,
  async run(ctx) {
    const task = ctx.artifacts.task;
    if (!task?.paymentMethod) {
      return { passed: false, errors: ["Missing paymentMethod in task"] };
    }

    const connectors = task.targetConnectors || [];
    if (connectors.length === 0) {
      return { passed: false, errors: ["No target connectors specified"] };
    }

    ctx.log(
      `[requirements] Analyzing ${task.paymentMethod} for connectors: ${connectors.join(", ")}`,
      "info"
    );

    // Discover actual connector files on disk
    const discoveredConnectors: Array<{
      discovery: ConnectorDiscovery;
      samples: Array<{ path: string; content: string }>;
    }> = [];

    for (const connectorName of connectors) {
      ctx.log(`[requirements] Discovering files for ${connectorName}...`, "info");
      const discovery = await discoverConnectorFiles(
        task.projectRoot,
        connectorName
      );

      if (discovery) {
        ctx.log(
          `[requirements] Found ${discovery.files.length} files for ${connectorName}`,
          "info"
        );
        const samples = await readSampleFiles(discovery, 5);
        discoveredConnectors.push({ discovery, samples });
      } else {
        ctx.log(
          `[requirements] Could not find connector files for ${connectorName}`,
          "warn"
        );
      }
    }

    if (discoveredConnectors.length === 0) {
      return {
        passed: false,
        errors: [
          `Could not find any connector files for: ${connectors.join(", ")}. ` +
          `Searched common paths like crates/integrations/connector-integration/src/connectors/`,
        ],
      };
    }

    // Prepare context for LLM
    const llmContext = {
      paymentMethod: task.paymentMethod,
      targetConnectors: connectors,
      projectRoot: task.projectRoot,
      discoveredConnectors: discoveredConnectors.map(({ discovery, samples }) => ({
        name: discovery.name,
        rootPath: discovery.rootPath,
        files: discovery.files,
        hasTransformerDir: discovery.hasTransformerDir,
        hasTypesFile: discovery.hasTypesFile,
        mainConnectorFile: discovery.mainConnectorFile,
        sampleFiles: samples.map((s) => ({
          path: s.path,
          content: s.content,
        })),
      })),
    };

    try {
      const raw = await callLlm({
        system: SYSTEM,
        user: JSON.stringify(llmContext, null, 2),
        label: "requirements_discovery",
      });

      const parsed = safeParseJson<RequirementsResult>(raw);
      if (!parsed || typeof parsed.overallScore !== "number") {
        ctx.log(
          `[requirements] LLM returned invalid format. Raw response (first 1000 chars): ${raw.slice(
            0,
            1000
          )}`,
          "error"
        );
        return {
          passed: false,
          errors: [
            "Invalid requirements format - LLM did not return valid JSON with overallScore",
          ],
        };
      }

      ctx.log(
        `[requirements] Score: ${parsed.overallScore}/10, Status: ${parsed.status}`,
        parsed.status === "valid" ? "success" : "warn"
      );

      return {
        passed: parsed.status !== "insufficient",
        artifacts: {
          requirements: parsed,
        },
        errors:
          parsed.status === "insufficient"
            ? ["Insufficient connector information for implementation"]
            : undefined,
      };
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      return {
        passed: false,
        errors: [`Requirements discovery failed: ${msg}`],
      };
    }
  },
};
