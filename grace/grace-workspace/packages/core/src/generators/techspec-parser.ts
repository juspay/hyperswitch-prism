/**
 * Techspec Parser
 *
 * Parses the markdown output from grace techspec CLI into L2Spec JSON structure.
 */

import type { L2Spec, L2ResearchFinding } from "../types.js";

/**
 * Parse grace techspec markdown output into L2Spec
 */
export function parseTechspecToL2(
  specContent: string,
  connector: string,
  paymentMethod: string
): L2Spec {
  // Extract sections from the markdown
  const sections = extractSections(specContent);

  // Build L2Spec structure
  const l2Spec: L2Spec = {
    summary: generateSummary(sections, connector, paymentMethod),
    scope: sections.implementation_scope || sections.overview || specContent.slice(0, 2000),
    outOfScope: sections.out_of_scope || sections.limitations || "Not specified in techspec",
    technicalConstraints: extractConstraints(sections),
    estimatedComplexity: estimateComplexity(sections, specContent),
    researchFindings: extractResearchFindings(sections, connector),
  };

  return l2Spec;
}

/**
 * Extract markdown sections into a map
 */
function extractSections(content: string): Record<string, string> {
  const sections: Record<string, string> = {};

  // Match ## headings and their content
  const sectionRegex = /^##\s+(.+)\n([\s\S]*?)(?=^##\s|$)/gm;
  let match;

  while ((match = sectionRegex.exec(content)) !== null) {
    const heading = match[1].trim().toLowerCase().replace(/\s+/g, "_");
    const sectionContent = match[2].trim();
    sections[heading] = sectionContent;
  }

  // Also try to find # Title
  const titleMatch = content.match(/^#\s+(.+)$/m);
  if (titleMatch) {
    sections.title = titleMatch[1].trim();
  }

  // Extract overview/introduction if no specific section
  if (!sections.overview && !sections.summary) {
    const introMatch = content.match(/^(?!#).*?(?=\n##|\n###|$)/s);
    if (introMatch) {
      sections.overview = introMatch[0].trim();
    }
  }

  return sections;
}

/**
 * Generate a summary from the techspec content
 */
function generateSummary(
  sections: Record<string, string>,
  connector: string,
  paymentMethod: string
): string {
  // Use explicit summary if available
  if (sections.summary || sections.overview) {
    const summary = sections.summary || sections.overview || "";
    // Take first 2-3 sentences
    const sentences = summary.split(/[.!?]+/).filter(s => s.trim().length > 10);
    return sentences.slice(0, 3).join(". ").trim() + ".";
  }

  // Generate from title and connector info
  const title = sections.title || `${connector} ${paymentMethod} Integration`;
  return `Implement ${paymentMethod} payment method for ${connector} connector. ${title}`;
}

/**
 * Extract technical constraints from sections
 */
function extractConstraints(sections: Record<string, string>): string[] {
  const constraints: string[] = [];

  // Look for constraints/requirements/limitations sections
  const constraintSections = [
    sections.technical_constraints,
    sections.constraints,
    sections.requirements,
    sections.limitations,
    sections.dependencies,
  ];

  for (const section of constraintSections) {
    if (section) {
      // Extract bullet points or numbered items
      const items = section
        .split(/\n/)
        .map(line => line.trim())
        .filter(line => line.startsWith("-") || line.startsWith("*") || /^\d+\./.test(line))
        .map(line => line.replace(/^[-*\d.\s]+/, "").trim())
        .filter(line => line.length > 5);

      constraints.push(...items);
    }
  }

  // If no constraints found, add a default
  if (constraints.length === 0) {
    constraints.push("Follow existing connector patterns in the codebase");
  }

  return [...new Set(constraints)]; // Dedupe
}

/**
 * Estimate complexity from the spec content
 */
function estimateComplexity(
  sections: Record<string, string>,
  fullContent: string
): "low" | "medium" | "high" {
  // Check for explicit complexity mention
  const complexitySection = sections.complexity || sections.estimated_complexity || "";
  const lowerContent = (complexitySection + " " + fullContent.slice(0, 2000)).toLowerCase();

  if (lowerContent.includes("high") || lowerContent.includes("complex")) {
    return "high";
  }
  if (lowerContent.includes("low") || lowerContent.includes("simple")) {
    return "low";
  }

  // Estimate based on content length and sections
  const sectionCount = Object.keys(sections).length;
  const contentLength = fullContent.length;

  if (contentLength > 10000 || sectionCount > 10) {
    return "high";
  }
  if (contentLength < 3000 && sectionCount < 5) {
    return "low";
  }

  return "medium";
}

/**
 * Extract research findings from the spec
 */
function extractResearchFindings(
  sections: Record<string, string>,
  connector: string
): L2ResearchFinding {
  const urls: string[] = [];
  const keyDetails: string[] = [];

  // Extract URLs from references or links sections
  const refSections = [
    sections.references,
    sections.documentation,
    sections.api_references,
    sections.links,
  ];

  for (const section of refSections) {
    if (section) {
      // Find URLs
      const urlMatches = section.match(/https?:\/\/[^\s\)\]>]+/g);
      if (urlMatches) {
        urls.push(...urlMatches);
      }

      // Find key points
      const points = section
        .split(/\n/)
        .map(line => line.trim())
        .filter(line => line.startsWith("-") || line.startsWith("*"))
        .map(line => line.replace(/^[-*\s]+/, "").trim())
        .filter(line => line.length > 10);

      keyDetails.push(...points);
    }
  }

  // Extract implementation patterns if mentioned
  const patterns: string[] = [];
  const patternSections = [sections.implementation, sections.patterns, sections.architecture];

  for (const section of patternSections) {
    if (section) {
      const sectionPatterns = section
        .split(/\n/)
        .map(line => line.trim())
        .filter(line => line.startsWith("-") || line.startsWith("*"))
        .map(line => line.replace(/^[-*\s]+/, "").trim())
        .filter(line => line.length > 10 && line.length < 200);

      patterns.push(...sectionPatterns);
    }
  }

  return {
    connectorDocs: [
      {
        connector,
        urls: [...new Set(urls)].slice(0, 10), // Dedupe and limit
        keyDetails: keyDetails.slice(0, 5).join("; ") || "See techspec for details",
        verificationScore: 7.0, // Default score since parser doesn't do 10-point verification
        verificationStatus: "valid" as const, // Default status
      },
    ],
    paymentMethodInfo: {
      source: urls[0] || "grace techspec generation",
      details:
        sections.payment_method_details ||
        sections.payment_flow ||
        "See techspec document for payment flow details",
    },
    implementationPatterns: patterns.length > 0
      ? patterns.slice(0, 5)
      : ["Follow existing connector transformer patterns"],
  };
}

/**
 * Validates if the parsed L2Spec is complete enough
 */
export function validateL2Spec(spec: L2Spec): { valid: boolean; errors: string[] } {
  const errors: string[] = [];

  if (!spec.summary || spec.summary.length < 10) {
    errors.push("Summary is missing or too short");
  }

  if (!spec.scope || spec.scope.length < 50) {
    errors.push("Scope is missing or too short");
  }

  if (!spec.technicalConstraints || spec.technicalConstraints.length === 0) {
    errors.push("Technical constraints are missing");
  }

  if (!["low", "medium", "high"].includes(spec.estimatedComplexity)) {
    errors.push("Invalid complexity value");
  }

  return {
    valid: errors.length === 0,
    errors,
  };
}
