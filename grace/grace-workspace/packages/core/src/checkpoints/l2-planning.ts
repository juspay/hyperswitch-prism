import type { Checkpoint, L2Plan, L2GenerationLog } from "../types.js";
import { runAI } from "../tools/runner-factory.js";
import {
  deriveClaudeSessionId,
  friendlySessionName,
} from "./session-id.js";
import { L2_SYSTEM, buildL2User } from "../generators/l2-prompt.js";
import {
  LINKS_AGENT_SYSTEM,
  resolveLinksAgentSystem,
  buildLinksAgentUserPayload,
  type LinksAgentResult,
} from "../generators/links-agent-prompt.js";
import {
  TECHSPEC_AGENT_SYSTEM,
  resolveTechspecAgentSystem,
  buildTechspecAgentUserPayload,
  type TechspecAgentResult,
} from "../generators/techspec-agent-prompt.js";
import { parseTechspecToL2 } from "../generators/techspec-parser.js";
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";

function valid(spec: unknown): spec is L2Plan {
  if (!spec || typeof spec !== "object") return false;
  const s = spec as Record<string, unknown>;

  // Basic validation
  const basicValid =
    typeof s.summary === "string" &&
    typeof s.scope === "string" &&
    typeof s.outOfScope === "string" &&
    Array.isArray(s.technicalConstraints) &&
    typeof s.estimatedComplexity === "string" &&
    ["low", "medium", "high"].includes(s.estimatedComplexity as string);

  if (!basicValid) return false;

  // Validate specContent exists and has 8 sections
  const specContent = s.specContent;
  if (!specContent || typeof specContent !== "string") {
    return false;
  }

  // Check for all 8 required sections in specContent
  const requiredSections = [
    "Connector Profile",
    "Authentication",
    "Supported Flows",
    "Request Schema",
    "Response Schema",
    "Error Handling",
    "Webhooks",
    "References",
  ];

  const missingSections = requiredSections.filter(
    (section) => !specContent.includes(section)
  );

  if (missingSections.length > 0) {
    return false;
  }

  // Validate researchFindings has connectorDocs with verification scores
  const researchFindings = s.researchFindings;
  if (researchFindings && typeof researchFindings === "object") {
    const rf = researchFindings as Record<string, unknown>;
    const connectorDocs = rf.connectorDocs;
    if (Array.isArray(connectorDocs)) {
      for (const doc of connectorDocs) {
        if (doc && typeof doc === "object") {
          const d = doc as Record<string, unknown>;
          // verificationScore and verificationStatus are now required
          if (typeof d.verificationScore !== "number") {
            return false;
          }
          if (!["valid", "problematic", "insufficient"].includes(d.verificationStatus as string)) {
            return false;
          }
        }
      }
    }
  }

  return true;
}

export const l2PlanningCheckpoint: Checkpoint = {
  id: "l2_planning",
  name: "L2 Planning",
  description: "Generate technical specification via documentation discovery and tech spec generation (2.1_links.md + 2.2_techspec.md).",
  retryFrom: "l2_planning",
  timeout: 45 * 60 * 1000, // 45 min (includes grace techspec ~20 min)
  async run(ctx) {
    if (!ctx.artifacts.task) {
      return { passed: false, errors: ["Missing task artifact"] };
    }

    const task = ctx.artifacts.task;
    const connector = task.targetConnectors?.[0] || "Unknown";
    const paymentMethod = task.paymentMethod || "Unknown";
    const projectRoot = task.projectRoot;

    // Initialize generation log
    const generationLog: L2GenerationLog = {
      workflowExecutions: [],
      webSearchQueries: [],
      filesCreated: [],
      commandsExecuted: [],
    };

    // ═══════════════════════════════════════════════════════════════════════
    // PHASE 0 (short-circuit): If a pre-generated tech spec exists on disk
    // at the canonical grace path (typically written by the wizard via
    // preflight), parse it directly and skip BOTH Links + Tech Spec agents.
    // Saves 5-20 minutes of LLM time per session.
    // ═══════════════════════════════════════════════════════════════════════
    if (connector !== "Unknown" && projectRoot) {
      const canonicalSpecPath = path.join(
        projectRoot,
        "grace",
        "rulesbook",
        "codegen",
        "references",
        "specs",
        `${connector}.md`,
      );
      if (existsSync(canonicalSpecPath)) {
        ctx.log(
          `[l2_planning] ✓ Pre-existing tech spec at ${canonicalSpecPath} — short-circuiting both phases`,
          "success",
        );
        try {
          const specContent = readFileSync(canonicalSpecPath, "utf-8");
          const l2Plan = parseTechspecToL2(specContent, connector, paymentMethod);
          // Attach the raw markdown so downstream checkpoints can reference it.
          l2Plan.specContent = specContent;
          l2Plan.generationLog = {
            workflowExecutions: [
              {
                phase: "techspec_generation",
                workflowFile: "wizard-pregenerated",
                readAt: new Date().toISOString(),
                output: `parsed from ${canonicalSpecPath}`,
                status: "success",
              },
            ],
            webSearchQueries: [],
            filesCreated: [
              {
                path: canonicalSpecPath,
                description: "Pre-generated tech spec from wizard",
                sizeBytes: specContent.length,
              },
            ],
            commandsExecuted: [],
          };
          if (valid(l2Plan)) {
            ctx.log(
              `[l2_planning] ✓ L2Plan parsed from existing spec (${specContent.length} chars)`,
              "success",
            );
            return {
              passed: true,
              artifacts: {
                l2: l2Plan,
                l2ShortCircuit: true,
              },
            };
          }
          ctx.log(
            `[l2_planning] Existing spec failed validation; falling back to LLM generation`,
            "warn",
          );
        } catch (parseErr) {
          const msg = parseErr instanceof Error ? parseErr.message : String(parseErr);
          ctx.log(
            `[l2_planning] Could not parse existing spec (${msg}); falling back to LLM generation`,
            "warn",
          );
        }
      }
    }

    // ═══════════════════════════════════════════════════════════════════════
    // PHASE 1: Links Agent - Discover documentation URLs
    // ═══════════════════════════════════════════════════════════════════════
    ctx.log("[l2_planning] ╔═══════════════════════════════════════════════════════╗", "info");
    ctx.log("[l2_planning] ║  PHASE 1: Links Discovery                              ║", "info");
    ctx.log("[l2_planning] ╚═══════════════════════════════════════════════════════╝", "info");
    ctx.log("[l2_planning] 📖 Reading workflow: grace/workflow/2.1_links.md", "info");

    const linksPayload = buildLinksAgentUserPayload(connector, paymentMethod, task);
    let linksResult: LinksAgentResult;

    // Phase 12: resume the links agent's persistent Claude session if we have
    // one from a prior run/retry of this checkpoint. The incremental message
    // carries the reviewer's regenerate feedback (set by l2_review on
    // "regenerate" verdict). On the very first call, no session exists and we
    // do a fresh `--session-id <uuid>` invocation; the uuid we capture goes on
    // ctx.artifacts.l2LinksSessionId for the next retry.
    const linksSessionId = ctx.artifacts.l2LinksSessionId as string | undefined;
    const regenPrompt = ctx.artifacts.l2RegeneratePrompt as string | undefined;

    try {
      // Phase 15: deterministic session id from (connector, flow, phase).
      // Same task → same uuid → cross-run resume of the same conversation.
      const linksFriendly = friendlySessionName(
        connector,
        paymentMethod,
        "l2planning-links"
      );
      const linksDerived = deriveClaudeSessionId(
        connector,
        paymentMethod,
        "l2planning-links"
      );

      const aiCall = linksSessionId
        ? {
            claudeSessionId: linksSessionId,
            incremental: true,
            userPayload:
              `The reviewer requested changes to the L2 documentation discovery for ${connector} / ${paymentMethod}:\n\n` +
              `${regenPrompt ?? "(no specific feedback supplied — perform another pass and improve coverage)"}\n\n` +
              `Revise the URL list to address this. Use your read tools as needed. Reply with ONLY the same JSON shape as your first reply (no markdown fences, first char \`{\` last char \`}\`).`,
            skillBody: "", // ignored when incremental
          }
        : {
            skillBody: resolveLinksAgentSystem(projectRoot),
            userPayload: linksPayload,
            preferredSessionId: linksDerived,
          };

      const { result: rawResult, sessionId: nextLinksSessionId } =
        await runAI<LinksAgentResult>({
          ...aiCall,
          cwd: projectRoot,
          label: linksSessionId ? "l2_gen:links:resume" : "l2_gen:links",
          timeoutMs: 15 * 60 * 1000, // 15 min for web search
          sessionLabel: linksFriendly,
        });
      linksResult = rawResult;
      // Persist the session id so a future retry can `--resume` this exact
      // conversation. If we just resumed, nextLinksSessionId echoes the input.
      ctx.artifacts.l2LinksSessionId = nextLinksSessionId;
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      ctx.log(`[l2_planning] Links Agent failed: ${msg}`, "error");
      return {
        passed: false,
        errors: [`Links discovery failed: ${msg}`],
        artifacts: { l2GenLog: generationLog },
      };
    }

    // Log workflow execution
    if (linksResult.executionLog) {
      generationLog.workflowExecutions.push({
        phase: "links_discovery",
        workflowFile: linksResult.executionLog.workflowFile || "grace/workflow/2.1_links.md",
        readAt: new Date().toISOString(),
        output: `Found ${linksResult.urlCount} URLs for ${linksResult.connector}`,
        status: linksResult.success ? "success" : "failed",
      });

      // Log web search queries
      if (linksResult.executionLog.webSearchQueries) {
        ctx.log("[l2_planning] 🔍 Web search queries executed:", "info");
        for (const query of linksResult.executionLog.webSearchQueries) {
          generationLog.webSearchQueries.push({
            query: query.query,
            timestamp: query.timestamp,
            results: query.results || [],
            resultCount: query.resultCount || query.results?.length || 0,
          });
          ctx.log(`[l2_planning]    "${query.query}" → ${query.resultCount || query.results?.length || 0} results`, "info");

          // Log first 3 URLs from each query
          const topResults = (query.results || []).slice(0, 3);
          for (const result of topResults) {
            ctx.log(`[l2_planning]       └─ ${result.url}`, "info");
          }
        }
      }

      // Log files created
      if (linksResult.executionLog.filesCreated) {
        for (const file of linksResult.executionLog.filesCreated) {
          generationLog.filesCreated.push(file);
          ctx.log(`[l2_planning] 📝 Created: ${file.path} (${file.description})`, "success");
        }
      }
    }

    if (!linksResult.success) {
      ctx.log("[l2_planning] Links discovery failed", "error");
      return {
        passed: false,
        errors: ["Links discovery failed"],
        artifacts: { l2GenLog: generationLog },
      };
    }

    ctx.log(`[l2_planning] ✓ Phase 1 complete: ${linksResult.urlCount} URLs discovered`, "success");

    // ═══════════════════════════════════════════════════════════════════════
    // PHASE 2: Tech Spec Agent - Generate spec via grace CLI
    // ═══════════════════════════════════════════════════════════════════════
    ctx.log("[l2_planning] ╔═══════════════════════════════════════════════════════╗", "info");
    ctx.log("[l2_planning] ║  PHASE 2: Techspec Generation                          ║", "info");
    ctx.log("[l2_planning] ╚═══════════════════════════════════════════════════════╝", "info");
    ctx.log("[l2_planning] 📖 Reading workflow: grace/workflow/2.2_techspec.md", "info");
    ctx.log(`[l2_planning] ⚙️  Running grace techspec for ${connector}...`, "info");
    ctx.log("[l2_planning]    (This may take 15-25 minutes)", "warn");

    const techspecPayload = buildTechspecAgentUserPayload(connector, paymentMethod, task);
    let techspecResult: TechspecAgentResult;
    const techspecSessionId = ctx.artifacts.l2TechspecSessionId as
      | string
      | undefined;

    try {
      const techspecFriendly = friendlySessionName(
        connector,
        paymentMethod,
        "l2planning-techspec"
      );
      const techspecDerived = deriveClaudeSessionId(
        connector,
        paymentMethod,
        "l2planning-techspec"
      );

      const aiCall = techspecSessionId
        ? {
            claudeSessionId: techspecSessionId,
            incremental: true,
            userPayload:
              `The reviewer requested changes to the L2 tech spec for ${connector} / ${paymentMethod}:\n\n` +
              `${regenPrompt ?? "(no specific feedback — refine the spec further)"}\n\n` +
              `Revise the spec content. The links agent's output (this conversation's sibling) may have changed — re-read the URL file if your prior assumptions about it need verification. Reply with ONLY the same JSON shape as your first reply.`,
            skillBody: "",
          }
        : {
            skillBody: resolveTechspecAgentSystem(projectRoot),
            userPayload: techspecPayload,
            preferredSessionId: techspecDerived,
          };

      const { result: rawResult, sessionId: nextTechspecSessionId } =
        await runAI<TechspecAgentResult>({
          ...aiCall,
          cwd: projectRoot,
          label: techspecSessionId ? "l2_gen:techspec:resume" : "l2_gen:techspec",
          timeoutMs: 30 * 60 * 1000, // 30 min for grace techspec
          sessionLabel: techspecFriendly,
        });
      techspecResult = rawResult;
      ctx.artifacts.l2TechspecSessionId = nextTechspecSessionId;
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      ctx.log(`[l2_planning] Tech Spec Agent failed: ${msg}`, "error");
      return {
        passed: false,
        errors: [`Tech spec generation failed: ${msg}`],
        artifacts: { l2GenLog: generationLog },
      };
    }

    // Normalize specPath if missing
    const specPath = techspecResult.specPath || `techspecs/${connector}_${paymentMethod}_spec.md`;

    // Normalize commands to object format (handle strings from agent)
    const normalizeCommand = (cmd: unknown): { command: string; workingDir: string; output?: string; durationMs?: number; status: "success" | "failed" } => {
      if (typeof cmd === "string") {
        return {
          command: cmd,
          workingDir: projectRoot,
          output: "",
          durationMs: 0,
          status: "success",
        };
      }
      if (cmd && typeof cmd === "object") {
        const c = cmd as Record<string, unknown>;
        return {
          command: String(c.command || ""),
          workingDir: String(c.workingDir || projectRoot),
          output: c.output ? String(c.output) : undefined,
          durationMs: typeof c.durationMs === "number" ? c.durationMs : undefined,
          status: (c.status === "failed" ? "failed" : "success") as "success" | "failed",
        };
      }
      return { command: "", workingDir: projectRoot, status: "success" };
    };

    // Log workflow execution
    if (techspecResult.executionLog) {
      generationLog.workflowExecutions.push({
        phase: "techspec_generation",
        workflowFile: techspecResult.executionLog.workflowFile || "grace/workflow/2.2_techspec.md",
        readAt: new Date().toISOString(),
        output: `Generated spec at ${specPath}`,
        status: techspecResult.success ? "success" : "failed",
      });

      // Log commands executed
      if (techspecResult.executionLog.commandsExecuted) {
        ctx.log("[l2_planning] ⚙️  Commands executed:", "info");
        for (const rawCmd of techspecResult.executionLog.commandsExecuted) {
          const cmd = normalizeCommand(rawCmd);
          if (cmd.command) { // Only add non-empty commands
            generationLog.commandsExecuted.push(cmd);
            ctx.log(`[l2_planning]    $ ${cmd.command}`, "info");
            ctx.log(
              `         CWD: ${cmd.workingDir} | Status: ${cmd.status} | Duration: ${
                cmd.durationMs ? `${(cmd.durationMs / 1000).toFixed(1)}s` : "unknown"
              }`,
              cmd.status === "success" ? "success" : "error"
            );
          }
        }
      }

      // Log files created
      if (techspecResult.executionLog.filesCreated) {
        for (const file of techspecResult.executionLog.filesCreated) {
          // Avoid duplicating the URLs file that was already logged
          if (!generationLog.filesCreated.some(f => f.path === file.path)) {
            generationLog.filesCreated.push(file);
          }
          ctx.log(`[l2_planning] 📄 ${file.path} (${file.description})`, "success");
        }
      }
    }

    if (!techspecResult.success) {
      ctx.log("[l2_planning] Tech spec generation failed", "error");
      return {
        passed: false,
        errors: ["Tech spec generation failed"],
        artifacts: { l2GenLog: generationLog },
      };
    }

    ctx.log(`[l2_planning] ✓ Phase 2 complete: Spec generated at ${techspecResult.specPath}`, "success");

    // ═══════════════════════════════════════════════════════════════════════
    // PHASE 3: Parse techspec into L2Spec format
    // ═══════════════════════════════════════════════════════════════════════
    ctx.log("[l2_planning] Parsing techspec into L2Spec format...", "info");

    let l2Plan: L2Plan;
    try {
      l2Plan = parseTechspecToL2(techspecResult.specContent, connector, paymentMethod);
      l2Plan.generationLog = generationLog;
      // Store full tech spec content for display in UI
      l2Plan.specContent = techspecResult.specContent;
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      ctx.log(`[l2_planning] Failed to parse techspec: ${msg}`, "error");
      return {
        passed: false,
        errors: [`Failed to parse techspec: ${msg}`],
        artifacts: { l2GenLog: generationLog },
      };
    }

    // Validate the parsed spec
    if (!valid(l2Plan)) {
      return {
        passed: false,
        errors: ["Parsed L2 plan failed validation"],
        artifacts: { l2: l2Plan, l2GenLog: generationLog },
      };
    }

    // Clear any regenerate prompt now that we've acted on it
    delete ctx.artifacts.l2RegeneratePrompt;

    // Final summary log
    ctx.log("[l2_planning] ╔═══════════════════════════════════════════════════════╗", "success");
    ctx.log("[l2_planning] ║  L2 Planning Complete                                 ║", "success");
    ctx.log("[l2_planning] ╚═══════════════════════════════════════════════════════╝", "success");
    ctx.log(`[l2_planning] Summary: ${l2Plan.summary.slice(0, 80)}...`, "success");
    ctx.log(`[l2_planning] Complexity: ${l2Plan.estimatedComplexity}`, "info");
    ctx.log(`[l2_planning] URLs discovered: ${generationLog.webSearchQueries.reduce((sum, q) => sum + q.resultCount, 0)}`, "info");
    ctx.log(`[l2_planning] Commands executed: ${generationLog.commandsExecuted.length}`, "info");

    return {
      passed: true,
      artifacts: {
        l2: l2Plan,
        l2GenLog: generationLog,
        // Phase 15: belt-and-suspenders echo. ctx.artifacts.l2*SessionId is
        // already set above via direct mutation, so this is technically
        // redundant — but it makes the persistence contract explicit so a
        // future refactor that derives artifacts from result rather than
        // ctx doesn't silently drop the session ids.
        l2LinksSessionId: ctx.artifacts.l2LinksSessionId,
        l2TechspecSessionId: ctx.artifacts.l2TechspecSessionId,
      },
    };
  },
};
