import type { Checkpoint, L3Analysis } from "../types.js";
import { runHumanReview } from "./human-review.js";

function renderL3(analysis: L3Analysis) {
  const line = "─".repeat(60);
  // eslint-disable-next-line no-console
  console.log(`
┌${line}┐
│  L3 ANALYSIS REVIEW  (${analysis.connector} - ${analysis.flow})
├${line}┤
│  Patterns Identified (${analysis.analysis.patternsIdentified.length}):
│`);

  for (const pattern of analysis.analysis.patternsIdentified) {
    // eslint-disable-next-line no-console
    console.log(`│    • ${pattern}`);
  }

  // eslint-disable-next-line no-console
  console.log(`│
│  Files to Modify (${analysis.analysis.filesToModify.length}):
│`);

  for (const file of analysis.analysis.filesToModify) {
    // eslint-disable-next-line no-console
    console.log(`│    • ${file}`);
  }

  // eslint-disable-next-line no-console
  console.log(`│
│  Existing Flows: ${analysis.analysis.existingFlows.join(", ") || "None"}
│  Prerequisites: ${analysis.analysis.prerequisitesStatus}
│`);

  if (analysis.analysis.missingPrerequisites && analysis.analysis.missingPrerequisites.length > 0) {
    // eslint-disable-next-line no-console
    console.log(`│  Missing Prerequisites:`);
    for (const prereq of analysis.analysis.missingPrerequisites) {
      // eslint-disable-next-line no-console
      console.log(`│    ⚠ ${prereq}`);
    }
    // eslint-disable-next-line no-console
    console.log(`│`);
  }

  // eslint-disable-next-line no-console
  console.log(`│  Implementation Notes:
│    ${analysis.implementationNotes.slice(0, 150)}${analysis.implementationNotes.length > 150 ? "..." : ""}
│`);

  if (analysis.riskAssessment && analysis.riskAssessment.length > 0) {
    // eslint-disable-next-line no-console
    console.log(`│  Risk Assessment:`);
    for (const risk of analysis.riskAssessment) {
      // eslint-disable-next-line no-console
      console.log(`│    ⚠ ${risk}`);
    }
    // eslint-disable-next-line no-console
    console.log(`│`);
  }

  // eslint-disable-next-line no-console
  console.log(`└${line}┘`);
}

function validateL3(v: unknown): v is L3Analysis {
  if (!v || typeof v !== "object") return false;
  const a = v as Record<string, unknown>;

  // Check top-level fields
  if (
    typeof a.connector !== "string" ||
    typeof a.flow !== "string" ||
    typeof a.implementationNotes !== "string"
  ) {
    return false;
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

  return true;
}

function extraValidateL3(analysis: L3Analysis): string | null {
  // Check if flow already exists
  if (analysis.analysis.flowAlreadyExists) {
    return `Flow "${analysis.flow}" is already implemented on connector "${analysis.connector}"`;
  }

  // Check if prerequisites are complete
  if (analysis.analysis.prerequisitesStatus === "incomplete") {
    const missing = analysis.analysis.missingPrerequisites?.join(", ") || "unknown";
    return `Prerequisites incomplete: ${missing}`;
  }

  // Check if we have files to modify
  if (analysis.analysis.filesToModify.length === 0) {
    return "No files to modify identified";
  }

  return null;
}

export const l3ReviewCheckpoint: Checkpoint = {
  id: "l3_review",
  name: "Human review: L3 analysis",
  description: "Pauses for human sign-off on the L3 Phase 4 analysis before implementation.",
  retryFrom: "l3_analysis",
  timeout: 24 * 60 * 60 * 1000,
  maxRetries: 5,
  async run(ctx) {
    return runHumanReview<L3Analysis>(ctx, {
      checkpointId: "l3_review",
      specType: "l3",
      loadSpec: (c) => c.artifacts.l3,
      render: renderL3,
      validate: validateL3,
      extraValidate: extraValidateL3,
      applySpec: (c, s) => {
        c.artifacts.l3 = s;
      },
      regenerateKey: "l3RegeneratePrompt",
      previousKey: "previousL3",
      sessionKey: "l3Review",
    });
  },
};
