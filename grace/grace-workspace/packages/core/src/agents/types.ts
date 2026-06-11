import type { PipelineContext } from "../types.js";

export interface AgentReport {
  agent: string;
  findings: string[];
  citations: string[];
  notes?: string;
}

export type AgentFn = (ctx: PipelineContext) => Promise<AgentReport>;
