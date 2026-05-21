import type { Checkpoint, L3Analysis } from "../types.js";
import { runAI } from "../tools/runner-factory.js";
import {
  deriveClaudeSessionId,
  friendlySessionName,
} from "./session-id.js";
import {
  L3_ANALYSIS_SYSTEM,
  buildL3AnalysisPayload,
  type L3AnalysisOptions,
} from "../generators/l3-analysis-prompt.js";
import * as path from "node:path";
import * as fs from "node:fs/promises";

function valid(analysis: unknown): analysis is L3Analysis {
  if (!analysis || typeof analysis !== "object") return false;
  const a = analysis as Record<string, unknown>;

  // Check top-level fields
  if (
    typeof a.connector !== "string" ||
    typeof a.flow !== "string" ||
    typeof a.implementationNotes !== "string"
  ) {
    return false;
  }

  // Check implementationType if provided (new field)
  if (a.implementationType) {
    const validTypes = [
      "new_flow",
      "payment_method_addition",
      "flow_completion",
    ];
    if (!validTypes.includes(a.implementationType as string)) {
      return false;
    }
  }

  // For payment_method_addition, validate required fields
  if (a.implementationType === "payment_method_addition") {
    if (!a.parentFlow || typeof a.parentFlow !== "string") {
      return false;
    }
    if (!a.paymentMethod || typeof a.paymentMethod !== "string") {
      return false;
    }
  }

  // Check analysis object
  const analysisObj = a.analysis as Record<string, unknown>;
  if (!analysisObj || typeof analysisObj !== "object") return false;

  // Check arrays
  if (
    !Array.isArray(analysisObj.patternsIdentified) ||
    !Array.isArray(analysisObj.filesToModify) ||
    !Array.isArray(analysisObj.existingFlows)
  ) {
    return false;
  }

  // Check prerequisitesStatus
  if (
    analysisObj.prerequisitesStatus !== "complete" &&
    analysisObj.prerequisitesStatus !== "incomplete"
  ) {
    return false;
  }

  // Check specification object (required)
  const spec = a.specification as Record<string, unknown>;
  if (!spec || typeof spec !== "object") return false;

  // For payment_method_addition, requestStruct/responseStruct may be omitted
  // For new_flow/flow_completion, they are required
  const implType = a.implementationType as string | undefined;
  if (implType !== "payment_method_addition") {
    if (!spec.requestStruct || !spec.responseStruct || !spec.connectorChanges) {
      return false;
    }
  }

  // Check filesChangedPreview (required)
  if (!Array.isArray(spec.filesChangedPreview)) return false;

  // Check supportingTypes (required for payment method additions with extra structs)
  if (!Array.isArray(spec.supportingTypes)) return false;

  return true;
}

/**
 * Check if a file contains a specific string
 */
async function checkFileContains(
  filePath: string,
  pattern: string,
): Promise<boolean> {
  try {
    const content = await fs.readFile(filePath, "utf-8");
    return content.includes(pattern);
  } catch {
    return false;
  }
}

/**
 * Cross-connector validation to detect misclassified flows
 */
async function validateAgainstReferenceConnectors(
  projectRoot: string,
  flow: string,
  logFn: (
    msg: string,
    level?: "info" | "warn" | "error" | "success" | "debug",
  ) => void,
): Promise<{
  isLikelyPaymentMethod: boolean;
  flowCount: number;
  pmCount: number;
}> {
  const refConnectors = ["adyen", "stripe", "checkout"];
  let flowCount = 0;
  let pmCount = 0;

  for (const ref of refConnectors) {
    const refFile = path.join(
      projectRoot,
      `crates/integrations/connector-integration/src/connectors/${ref}.rs`,
    );
    const transformersFile = path.join(
      projectRoot,
      `crates/integrations/connector-integration/src/connectors/${ref}/transformers.rs`,
    );

    const hasFlow = await checkFileContains(refFile, `flow: ${flow}`);
    const hasPm = await checkFileContains(transformersFile, flow);

    if (hasFlow) flowCount++;
    if (hasPm) pmCount++;
  }

  // If most connectors have it as PM but not as Flow, it's likely a payment method
  const isLikelyPaymentMethod = pmCount >= 2 && flowCount === 0;

  if (isLikelyPaymentMethod) {
    logFn(
      `[l3_analysis] WARNING: '${flow}' appears to be a payment method in ${pmCount} connectors, ` +
        `but not a flow in any. Consider using PAYMENT_METHOD=${flow} with FLOW=Authorize`,
      "warn",
    );
  }

  return { isLikelyPaymentMethod, flowCount, pmCount };
}

/**
 * Ensure tech spec is saved to disk for the L3 Analysis Agent to read
 */
async function ensureTechSpecFile(
  projectRoot: string,
  connector: string,
  flow: string,
  specContent: string | undefined,
): Promise<string> {
  const techSpecsDir = path.join(projectRoot, "techspecs");
  const specPath = path.join(techSpecsDir, `${connector}_${flow}_spec.md`);

  // Ensure directory exists
  await fs.mkdir(techSpecsDir, { recursive: true });

  // If we have specContent, write it to file
  if (specContent) {
    await fs.writeFile(specPath, specContent, "utf-8");
    return specPath;
  }

  // If file already exists, return its path
  try {
    await fs.access(specPath);
    return specPath;
  } catch {
    // File doesn't exist and no content provided
    throw new Error(
      `Tech spec not found at ${specPath} and no specContent provided`,
    );
  }
}

export const l3AnalysisCheckpoint: Checkpoint = {
  id: "l3_analysis",
  name: "L3 Analysis",
  description:
    "Phase 4 from 2.3_codegen.md: Read and analyze 6 reference files (tech spec, patterns, macros, domain types, existing connector code, transformers).",
  retryFrom: "l3_analysis",
  timeout: 40 * 60 * 1000, // 50 min for reading and analyzing files

  async run(ctx) {
    const l2 = ctx.artifacts.l2;

    if (!l2) {
      return { passed: false, errors: ["Missing L2 plan"] };
    }

    const task = ctx.artifacts.task;
    if (!task?.targetConnectors?.[0]) {
      return { passed: false, errors: ["Missing connector in task"] };
    }

    const connector = task.targetConnectors[0];
    const flow = task.paymentMethod || "Unknown";
    const paymentMethod = task.paymentMethod;
    const isPaymentMethodAddition = !!paymentMethod;
    const projectRoot = task.projectRoot;

    ctx.log(
      "[l3_analysis] ╔═══════════════════════════════════════════════════════════╗",
      "info",
    );
    ctx.log(
      "[l3_analysis] ║  L3 Analysis (2.3_codegen.md Phase 4)                    ║",
      "info",
    );
    ctx.log(
      "[l3_analysis] ╚═══════════════════════════════════════════════════════════╝",
      "info",
    );
    ctx.log(`[l3_analysis] Connector: ${connector}`, "info");
    ctx.log(`[l3_analysis] Flow: ${flow}`, "info");
    if (paymentMethod) {
      ctx.log(`[l3_analysis] Payment Method: ${paymentMethod}`, "info");
      ctx.log(`[l3_analysis] Type: payment_method_addition`, "info");
    }

    // Cross-connector validation for potential misclassification
    if (!isPaymentMethodAddition && flow !== "Unknown") {
      const validation = await validateAgainstReferenceConnectors(
        projectRoot,
        flow,
        ctx.log,
      );
      if (validation.isLikelyPaymentMethod) {
        ctx.log(
          `[l3_analysis] RECOMMENDATION: Set task.paymentMethod="${flow}" and use FLOW="Authorize"`,
          "warn",
        );
      }
    }

    // Ensure tech spec is available on disk
    let techSpecPath: string;
    try {
      techSpecPath = await ensureTechSpecFile(
        projectRoot,
        connector,
        flow,
        l2?.specContent,
      );
      ctx.log(`[l3_analysis] Tech spec: ${techSpecPath}`, "info");
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      ctx.log(`[l3_analysis] Failed to locate tech spec: ${msg}`, "error");
      return {
        passed: false,
        errors: [`Tech spec not available: ${msg}`],
      };
    }

    // Build payload for L3 Analysis Agent
    const options: L3AnalysisOptions = {
      paymentMethod,
      isPaymentMethodAddition,
    };
    const payload = buildL3AnalysisPayload(
      connector,
      flow,
      techSpecPath,
      projectRoot,
      path.join(projectRoot, "grace/workflow/2.3_codegen.md"),
      l2,
      options,
      task.description,
      task.acceptanceCriteria,
    );

    ctx.log(
      "[l3_analysis] Starting Phase 4 analysis (reading 6 files)...",
      "warn",
    );
    ctx.log("[l3_analysis]   1. Tech Spec", "info");
    ctx.log("[l3_analysis]   2. Pattern Guide", "info");
    ctx.log("[l3_analysis]   3. Macro Reference", "info");
    ctx.log("[l3_analysis]   4. Domain Types", "info");
    ctx.log("[l3_analysis]   5. Existing Connector Code", "info");
    ctx.log("[l3_analysis]   6. Existing Transformers", "info");

    let result: L3Analysis;
    // Phase 12: persistent per-phase Claude session. First call generates a
    // uuid (returned in `sessionId`); retries resume that conversation with the
    // reviewer's regenerate feedback as an incremental message, so the model
    // keeps its memory of the codebase / L2 spec it already read.
    const l3SessionId = ctx.artifacts.l3SessionId as string | undefined;
    const l3RegenPrompt = ctx.artifacts.l3RegeneratePrompt as string | undefined;
    try {
      // Phase 15: deterministic session id from (connector, flow, phase).
      const l3Friendly = friendlySessionName(connector, flow, "l3analysis");
      const l3Derived = deriveClaudeSessionId(connector, flow, "l3analysis");

      const aiCall = l3SessionId
        ? {
            claudeSessionId: l3SessionId,
            incremental: true,
            userPayload:
              `The reviewer requested revisions to your L3 analysis:\n\n` +
              `${l3RegenPrompt ?? "(no specific feedback supplied — refine your specification, particularly around ambiguities and edge cases you previously flagged)"}\n\n` +
              `Revise the analysis. The L2 spec, codebase patterns, and macro reference you already read are unchanged. Re-read specific files only if your new conclusions depend on them. Reply with ONLY the same L3Analysis JSON shape as your first reply (first char \`{\`, last char \`}\`).`,
            skillBody: "",
          }
        : {
            skillBody: L3_ANALYSIS_SYSTEM,
            userPayload: payload,
            preferredSessionId: l3Derived,
          };

      const { result: rawResult, sessionId: nextL3SessionId } =
        await runAI<L3Analysis>({
          ...aiCall,
          cwd: projectRoot,
          label: l3SessionId ? "l3:analysis:resume" : "l3:analysis",
          timeoutMs: 35 * 60 * 1000, // 35 min (5 min buffer below the 40 min checkpoint wrapper)
          sessionLabel: l3Friendly,
        });
      result = rawResult;
      ctx.artifacts.l3SessionId = nextL3SessionId;

      // Handle auto-wrapped results (when JSON parsing failed in runner)
      if (
        (result as unknown as Record<string, unknown>).contents &&
        !(result as unknown as Record<string, unknown>).analysis
      ) {
        ctx.log(
          "[l3_analysis] Result was auto-wrapped, attempting to extract JSON from contents",
          "warn",
        );
        const wrapped = result as unknown as {
          contents: string;
          notes?: string;
        };
        try {
          // Try to extract JSON from the contents string
          let extracted = wrapped.contents;
          // Remove markdown fences if present
          extracted = extracted
            .replace(/^```(?:json)?\s*/i, "")
            .replace(/```\s*$/i, "");
          // Find the JSON object
          const match = extracted.match(/\{[\s\S]*\}/);
          if (match) {
            result = JSON.parse(match[0]) as L3Analysis;
            ctx.log(
              "[l3_analysis] Successfully extracted L3 analysis from wrapped contents",
              "info",
            );
          }
        } catch (extractErr) {
          ctx.log(
            `[l3_analysis] Failed to extract from wrapped contents: ${extractErr}`,
            "error",
          );
        }
      }

      // Save L3 spec to file for downstream checkpoints
      const l3SpecPath = path.join(
        projectRoot,
        "techspecs",
        `${connector}_${flow}_l3.json`,
      );
      try {
        await fs.mkdir(path.dirname(l3SpecPath), { recursive: true });
        await fs.writeFile(l3SpecPath, JSON.stringify(result, null, 2));
        ctx.log(`[l3_analysis] L3 spec saved to: ${l3SpecPath}`, "info");
      } catch (writeErr) {
        ctx.log(
          `[l3_analysis] Warning: Failed to save L3 spec: ${writeErr}`,
          "warn",
        );
      }
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      ctx.log(`[l3_analysis] L3 Analysis Agent failed: ${msg}`, "error");
      return {
        passed: false,
        errors: [`L3 Analysis failed: ${msg}`],
      };
    }

    // Validate result
    if (!valid(result)) {
      ctx.log("[l3_analysis] ✗ Validation failed", "error");
      return {
        passed: false,
        errors: ["L3 Analysis result failed validation"],
        artifacts: { l3: result },
      };
    }

    // Check if flow already exists
    // For payment_method_addition, the parent flow SHOULD exist - don't skip
    if (
      result.analysis.flowAlreadyExists &&
      result.implementationType !== "payment_method_addition"
    ) {
      ctx.log("[l3_analysis] ⚠ Flow already implemented - SKIPPING", "warn");
      return {
        passed: false,
        errors: ["Flow already implemented - SKIPPED"],
        artifacts: { l3: result },
      };
    }

    // For payment_method_addition, log the intent
    if (result.implementationType === "payment_method_addition") {
      ctx.log(
        `[l3_analysis] ℹ Payment method addition: extending '${result.parentFlow}' with '${result.paymentMethod}'`,
        "info",
      );
    }

    // Check prerequisites status
    if (result.analysis.prerequisitesStatus === "incomplete") {
      ctx.log(
        `[l3_analysis] ✗ Prerequisites incomplete: ${result.analysis.missingPrerequisites?.join(", ")}`,
        "error",
      );
      return {
        passed: false,
        errors: [
          `Prerequisites incomplete: ${result.analysis.missingPrerequisites?.join(", ")}`,
        ],
        artifacts: { l3: result },
      };
    }

    // Check specification completeness
    if (!result.specification) {
      ctx.log("[l3_analysis] ✗ Missing implementation specification", "error");
      return {
        passed: false,
        errors: ["L3 Analysis did not produce implementation specification"],
        artifacts: { l3: result },
      };
    }

    // Check files changed preview
    if (
      !result.specification.filesChangedPreview ||
      result.specification.filesChangedPreview.length === 0
    ) {
      ctx.log("[l3_analysis] ✗ Missing files changed preview", "error");
      return {
        passed: false,
        errors: ["L3 Analysis did not produce filesChangedPreview"],
        artifacts: { l3: result },
      };
    }

    // Log ambiguities if any
    if (
      result.specification.ambiguities &&
      result.specification.ambiguities.length > 0
    ) {
      ctx.log(
        `[l3_analysis] ⚠ ${result.specification.ambiguities.length} ambiguous specification(s)`,
        "warn",
      );
      for (const ambiguity of result.specification.ambiguities) {
        ctx.log(
          `[l3_analysis]   • ${ambiguity.field}: ${ambiguity.issue}`,
          "warn",
        );
      }
    }

    // Log results
    ctx.log(
      `[l3_analysis] ✓ Patterns identified: ${result.analysis.patternsIdentified.length}`,
      "success",
    );
    for (const pattern of result.analysis.patternsIdentified) {
      ctx.log(`[l3_analysis]   • ${pattern}`, "info");
    }

    ctx.log(
      `[l3_analysis] ✓ Files to modify: ${result.analysis.filesToModify.length}`,
      "success",
    );
    for (const file of result.analysis.filesToModify) {
      ctx.log(`[l3_analysis]   • ${file}`, "info");
    }

    ctx.log(
      `[l3_analysis] ✓ Existing flows: ${result.analysis.existingFlows.join(", ") || "None"}`,
      "info",
    );

    // Log files changed preview
    const preview = result.specification.filesChangedPreview;
    const totalAdditions = preview.reduce((sum, f) => sum + f.linesAdded, 0);
    const totalDeletions = preview.reduce((sum, f) => sum + f.linesRemoved, 0);
    ctx.log(
      `[l3_analysis] ✓ Files to change: ${preview.length} (+${totalAdditions}/-${totalDeletions})`,
      "success",
    );
    for (const file of preview) {
      const icon =
        file.changeType === "created"
          ? "+"
          : file.changeType === "deleted"
            ? "−"
            : "•";
      ctx.log(
        `[l3_analysis]   ${icon} ${file.path} (+${file.linesAdded}/-${file.linesRemoved})`,
        "info",
      );
    }

    if (result.riskAssessment && result.riskAssessment.length > 0) {
      ctx.log(
        `[l3_analysis] ⚠ Risks identified: ${result.riskAssessment.length}`,
        "warn",
      );
      for (const risk of result.riskAssessment) {
        ctx.log(`[l3_analysis]   • ${risk}`, "warn");
      }
    }

    ctx.log(
      "[l3_analysis] ╔═══════════════════════════════════════════════════════════╗",
      "success",
    );
    ctx.log(
      "[l3_analysis] ║  ✓ Phase 4 Analysis Complete                             ║",
      "success",
    );
    ctx.log(
      "[l3_analysis] ╚═══════════════════════════════════════════════════════════╝",
      "success",
    );

    delete ctx.artifacts.l3RegeneratePrompt;

    // Get the l3SpecPath we saved earlier
    const l3SpecPath = path.join(
      projectRoot,
      "techspecs",
      `${connector}_${flow}_l3.json`,
    );

    return {
      passed: true,
      artifacts: {
        l3: result,
        l3SpecPath: l3SpecPath,
        // Phase 15: belt-and-suspenders echo (see l2-planning for rationale)
        l3SessionId: ctx.artifacts.l3SessionId,
      },
    };
  },
};
