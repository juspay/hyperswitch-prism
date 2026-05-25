import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

/**
 * Skill markdown files live under packages/core/skills/. Each file has
 * YAML-ish frontmatter like:
 *
 *   ---
 *   name: l4-code-changes
 *   description: ...
 *   applies_to: l4_gen
 *   agents:
 *     - file-locator
 *     - signature-extractor
 *   ---
 *
 *   # L4 Skill — ...
 *   ...markdown body...
 *
 * This loader parses the frontmatter and returns the body so it can be
 * used as a system prompt by the corresponding checkpoint.
 */

export interface LoadedSkill {
  name: string;
  description: string;
  appliesTo?: string;
  agents: string[];
  body: string;
}

const HERE = path.dirname(fileURLToPath(import.meta.url));
// dist/tools/skill-loader.js → packages/core/skills
const SKILLS_DIR = path.resolve(HERE, "../../skills");

let cached: Record<string, LoadedSkill> | undefined;

function parseFrontmatter(raw: string): { meta: Record<string, unknown>; body: string } {
  const match = raw.match(/^---\s*\n([\s\S]*?)\n---\s*\n?([\s\S]*)$/);
  if (!match) return { meta: {}, body: raw };
  const metaBlock = match[1]!;
  const body = match[2] ?? "";
  const meta: Record<string, unknown> = {};
  const lines = metaBlock.split("\n");
  let currentListKey: string | null = null;
  for (const line of lines) {
    if (!line.trim()) continue;
    // List continuation: "  - item"
    const listItem = line.match(/^\s*-\s+(.+)$/);
    if (listItem && currentListKey) {
      const arr = meta[currentListKey];
      if (Array.isArray(arr)) arr.push(listItem[1]!.trim());
      continue;
    }
    const kv = line.match(/^([a-zA-Z_][a-zA-Z0-9_-]*):\s*(.*)$/);
    if (kv) {
      const key = kv[1]!;
      const value = kv[2]!.trim();
      if (value === "") {
        // Start of a list
        meta[key] = [];
        currentListKey = key;
      } else {
        meta[key] = value.replace(/^"(.*)"$/, "$1");
        currentListKey = null;
      }
    }
  }
  return { meta, body: body.trimStart() };
}

function loadAll(): Record<string, LoadedSkill> {
  if (cached) return cached;
  const out: Record<string, LoadedSkill> = {};
  try {
    const entries = fs.readdirSync(SKILLS_DIR, { withFileTypes: true });
    for (const e of entries) {
      if (!e.isFile() || !e.name.endsWith(".md")) continue;
      const full = path.join(SKILLS_DIR, e.name);
      const raw = fs.readFileSync(full, "utf-8");
      const { meta, body } = parseFrontmatter(raw);
      const skill: LoadedSkill = {
        name: String(meta.name ?? e.name.replace(/\.md$/, "")),
        description: String(meta.description ?? ""),
        appliesTo: meta.applies_to ? String(meta.applies_to) : undefined,
        agents: Array.isArray(meta.agents) ? (meta.agents as string[]) : [],
        body,
      };
      out[skill.name] = skill;
    }
  } catch (err) {
    // eslint-disable-next-line no-console
    console.error(`[skill-loader] failed to read ${SKILLS_DIR}:`, err);
  }
  cached = out;
  return out;
}

export function loadSkill(name: string): LoadedSkill | undefined {
  return loadAll()[name];
}

export function loadSkillFor(checkpointId: string): LoadedSkill | undefined {
  const all = loadAll();
  return Object.values(all).find((s) => s.appliesTo === checkpointId);
}

export function loadAgentSkill(agentName: string): LoadedSkill | undefined {
  // Agent skills live under packages/core/skills/agents/
  const agentDir = path.join(SKILLS_DIR, "agents");
  const file = path.join(agentDir, `${agentName}.md`);
  try {
    if (!fs.existsSync(file)) return undefined;
    const raw = fs.readFileSync(file, "utf-8");
    const { meta, body } = parseFrontmatter(raw);
    return {
      name: String(meta.name ?? agentName),
      description: String(meta.description ?? ""),
      appliesTo: undefined,
      agents: [],
      body,
    };
  } catch {
    return undefined;
  }
}
