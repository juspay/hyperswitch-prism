/** Zod input shapes for every tool (passed to registerTool as inputSchema). */
import { z } from "zod";
import { ROLES, DEPTHS, SEARCH_AREAS } from "./constants.js";

export const startHereShape = {
  role: z.enum(ROLES).optional().describe("Who you are: explorer (just understanding), contributor (changing code), reviewer (reviewing PRs), or integrator (using the SDK in an app)."),
  goal: z.string().optional().describe("Optional free-text goal, e.g. 'add a connector' or 'understand refunds'."),
};

export const architectureOverviewShape = {
  depth: z.enum(DEPTHS).optional().describe("tldr (non-technical), standard (default), or deep (engineer detail)."),
  topic: z.string().optional().describe("Optional: zoom into one concept slug instead of the whole tour."),
};

export const explainConceptShape = {
  concept: z.string().describe("A concept to explain, e.g. 'connector', 'transformer', 'authorize', 'RouterDataV2', 'grace'."),
  depth: z.enum(DEPTHS).optional().describe("tldr (plain, for anyone), standard (default), or deep (engineer detail)."),
};

export const glossaryShape = {
  term: z.string().optional().describe("A term to define. Omit to list all glossary terms."),
};

export const repoMapShape = {
  query: z.string().optional().describe("A topic or symbol to locate, e.g. 'stripe', 'proto', 'macros', 'flow dependencies'. Omit for the full map."),
};

export const searchShape = {
  query: z.string().describe("Keywords to search across the repo's docs, skills, and grace guides."),
  area: z.enum(SEARCH_AREAS).optional().describe("Scope: docs, skills, grace, or all (default)."),
  limit: z.number().int().min(1).max(25).optional().describe("Max results (default 8)."),
};

export const readDocShape = {
  path: z.string().describe("Repo-relative path of a doc to read verbatim, e.g. 'docs/architecture/concepts/error-handling.md'."),
  heading: z.string().optional().describe("Optional heading text to return just that section."),
};

export const howToShape = {
  task: z.string().describe("What you want to do, e.g. 'add a connector', 'add a refund flow', 'add a wallet payment method', 'review a PR'."),
};

export const explainFlowShape = {
  flow: z.string().describe("A payment flow: authorize, capture, void, refund, psync, rsync (or an advanced flow name)."),
};

export const connectorWalkthroughShape = {
  connector: z.string().optional().describe("Connector machine name to walk through (default 'stripe')."),
};

export const learningPathShape = {
  role: z.enum(ROLES).describe("Which curriculum: explorer, contributor, reviewer, or integrator."),
};

export const troubleshootShape = {
  symptom: z.string().describe("What went wrong, e.g. 'RouterData not found', 'status is a number', 'refund won't compile', 'payment method dropped'."),
};

export const coverageShape = {
  flow: z.string().optional().describe("A flow to list supporting connectors for, e.g. 'refund', 'capture'."),
  connector: z.string().optional().describe("A connector to list supported flows for, e.g. 'stripe'."),
};

export const faqShape = {
  query: z.string().optional().describe("A question or keywords to match against the FAQ. Omit to list all questions."),
};
