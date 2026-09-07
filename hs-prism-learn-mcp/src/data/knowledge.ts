/**
 * Runtime accessors over the committed knowledge JSON (built by scripts/gen-knowledge.ts).
 *
 * JSON is loaded via fs (not `import ... with { type: "json" }`) so the module works
 * identically under tsx (src/) and compiled (dist/) on Node 18+. Source of truth so the
 * server never invents a path, term, connector, status code, or concept.
 */
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

// ---------------------------------------------------------------------------
// Shapes (must match what gen-knowledge.ts emits).
// ---------------------------------------------------------------------------
export interface Heading {
  level: number;
  text: string;
  anchor: string;
}
export interface DocEntry {
  path: string;
  title: string;
  category: string;
  headings: Heading[];
  summary: string;
  keywords: string[];
  wordCount: number;
  githubUrl: string;
  aliases: string[];
}
export interface ConceptCard {
  slug: string;
  title: string;
  tier: string;
  audience: string;
  one_liner: string;
  analogy: string;
  depth: { tldr: string; standard?: string; deep?: string };
  prerequisites: string[];
  related: string[];
  go_deeper: { path: string; why: string }[];
  verify_anchors: { path: string; must_contain: string }[];
  body: string;
}
export interface SkillEntry {
  name: string;
  description: string;
  domain: string;
  skillMdPath: string;
  references: { path: string; aliasedFrom?: string }[];
  triggers: string[];
}
export interface GlossaryTerm {
  term: string;
  definition: string;
  sourcePath: string;
}
export interface RegistryConnector {
  name: string;
  implPath: string;
  transformersPath: string | null;
  inRegistry: boolean;
}
export interface Coverage {
  sourcePath: string;
  flows: Record<string, string[]>;
  connectors: Record<string, string[]>;
  note: string;
}
export interface RepoArea {
  topic: string;
  path: string;
  what: string;
  keywords: string[];
}
export interface LearningStep {
  slug: string;
  file?: string;
  why: string;
}
export interface LearningPath {
  id: string;
  title: string;
  role: string;
  audience: string;
  summary: string;
  steps: LearningStep[];
}
export interface Discrepancy {
  id: string;
  title: string;
  topic: string;
  keywords: string[];
  summary: string;
  sources: { path: string; says: string }[];
  authoritative: string;
  resolution: string;
}
export interface Meta {
  connectorCounts: { registry: number; llmsTxt: number | null };
  docCount: number;
  skillCount: number;
  glossaryCount: number;
  conceptCount: number;
  pathCount: number;
}

function load<T>(relative: string): T {
  const url = new URL(relative, import.meta.url);
  return JSON.parse(readFileSync(fileURLToPath(url), "utf8")) as T;
}

const docsIndex = load<{ docs: DocEntry[] }>("./docs.index.generated.json").docs;
const docsBodies = load<{ bodies: Record<string, string> }>("./docs.bodies.generated.json").bodies;
const concepts = load<{ concepts: ConceptCard[] }>("./concepts.generated.json").concepts;
const skills = load<{ skills: SkillEntry[] }>("./skills.generated.json").skills;
const glossary = load<{ terms: GlossaryTerm[] }>("./glossary.generated.json").terms;
const registry = load<{ connectors: RegistryConnector[] }>("./connectors.registry.generated.json").connectors;
const coverage = load<Coverage>("./coverage.generated.json");
const repomap = load<{ areas: RepoArea[]; topDirs: string[] }>("./repomap.generated.json");
const learningPaths = load<{ paths: LearningPath[] }>("./paths.generated.json").paths;
const discrepancies = load<{ discrepancies: Discrepancy[] }>("./discrepancies.generated.json").discrepancies;
export const META = load<Meta>("./meta.generated.json");

// ---------------------------------------------------------------------------
// Indexes.
// ---------------------------------------------------------------------------
const DOC_BY_PATH = new Map<string, DocEntry>();
for (const d of docsIndex) {
  DOC_BY_PATH.set(d.path, d);
  for (const a of d.aliases) DOC_BY_PATH.set(a, d);
}
const CONCEPT_BY_SLUG = new Map<string, ConceptCard>();
for (const c of concepts) CONCEPT_BY_SLUG.set(c.slug, c);
const SKILL_BY_NAME = new Map<string, SkillEntry>();
for (const s of skills) SKILL_BY_NAME.set(s.name, s);
const CONNECTOR_BY_NAME = new Map<string, RegistryConnector>();
for (const c of registry) CONNECTOR_BY_NAME.set(c.name, c);

const norm = (s: string) => s.trim().toLowerCase();

// ---------------------------------------------------------------------------
// Docs.
// ---------------------------------------------------------------------------
export function listDocs(): DocEntry[] {
  return docsIndex;
}
export function getDoc(path: string): DocEntry | undefined {
  const p = path.replace(/^\.\//, "").replace(/^\/+/, "").trim();
  return DOC_BY_PATH.get(p);
}
export function getBody(path: string): string | undefined {
  const d = getDoc(path);
  return d ? docsBodies[d.path] : docsBodies[path.replace(/^\.\//, "").replace(/^\/+/, "").trim()];
}

/** Return the markdown of one section (a heading and everything under it until the next same/higher heading). */
export function getDocSection(path: string, headingQuery: string): { heading: string; text: string } | null {
  const body = getBody(path);
  if (!body) return null;
  const lines = body.split("\n");
  const q = norm(headingQuery);
  let startIdx = -1;
  let startLevel = 0;
  let heading = "";
  for (let i = 0; i < lines.length; i++) {
    const m = lines[i]!.match(/^(#{1,6})\s+(.+?)\s*$/);
    if (m) {
      const text = m[2]!.replace(/[#*`]/g, "").trim();
      if (norm(text).includes(q)) {
        startIdx = i;
        startLevel = m[1]!.length;
        heading = text;
        break;
      }
    }
  }
  if (startIdx < 0) return null;
  const out = [lines[startIdx]!];
  for (let i = startIdx + 1; i < lines.length; i++) {
    const m = lines[i]!.match(/^(#{1,6})\s+/);
    if (m && m[1]!.length <= startLevel) break;
    out.push(lines[i]!);
  }
  return { heading, text: out.join("\n").trim() };
}
export function suggestDocPaths(path: string, limit = 5): string[] {
  const q = norm(path);
  return docsIndex
    .map((d) => ({ p: d.path, score: d.path.toLowerCase().includes(q) ? 2 : d.title.toLowerCase().includes(q) ? 1 : 0 }))
    .filter((x) => x.score > 0)
    .sort((a, b) => b.score - a.score)
    .slice(0, limit)
    .map((x) => x.p);
}

// ---------------------------------------------------------------------------
// Concepts.
// ---------------------------------------------------------------------------
export function listConcepts(): ConceptCard[] {
  return concepts;
}
export function getConcept(slug: string): ConceptCard | undefined {
  const s = norm(slug).replace(/\s+/g, "-");
  if (CONCEPT_BY_SLUG.has(s)) return CONCEPT_BY_SLUG.get(s);
  // tolerate title/alias-ish lookups: exact title match, then keyword match
  const byTitle = concepts.find((c) => norm(c.title) === norm(slug));
  if (byTitle) return byTitle;
  return undefined;
}
export function suggestConcepts(slug: string, limit = 6): string[] {
  const q = norm(slug).replace(/\s+/g, "-");
  const bare = q.replace(/-/g, " ");
  const scored = concepts
    .map((c) => {
      const n = c.slug;
      let score = 0;
      if (n.startsWith(q) || q.startsWith(n)) score = 4;
      else if (n.includes(q) || q.includes(n)) score = 3;
      else if (norm(c.title).includes(bare)) score = 2;
      else if (c.one_liner.toLowerCase().includes(bare) || n[0] === q[0]) score = 1;
      return { n, score };
    })
    .filter((s) => s.score > 0)
    .sort((a, b) => b.score - a.score)
    .slice(0, limit)
    .map((s) => s.n);
  return scored;
}

// ---------------------------------------------------------------------------
// Glossary.
// ---------------------------------------------------------------------------
export function listGlossary(): GlossaryTerm[] {
  return glossary;
}
export function getGlossaryTerm(term: string): GlossaryTerm | undefined {
  const q = norm(term);
  return glossary.find((t) => norm(t.term) === q) ?? glossary.find((t) => norm(t.term).includes(q));
}
export function searchGlossary(query: string): GlossaryTerm[] {
  const q = norm(query);
  return glossary.filter((t) => norm(t.term).includes(q) || t.definition.toLowerCase().includes(q));
}

// ---------------------------------------------------------------------------
// Skills.
// ---------------------------------------------------------------------------
export function listSkills(): SkillEntry[] {
  return skills;
}
export function getSkill(name: string): SkillEntry | undefined {
  return SKILL_BY_NAME.get(norm(name));
}

/** Curated phrase -> skill name, to make routing robust for common newcomer phrasings. */
const TASK_ALIASES: { match: RegExp; skill: string }[] = [
  { match: /\b(new|add|create|implement|integrate).*(connector|processor|gateway)\b/, skill: "new-connector" },
  { match: /\bconnector\b.*\b(from scratch|new)\b/, skill: "new-connector" },
  { match: /\b(add|implement).*(flow)\b/, skill: "add-connector-flow" },
  { match: /\b(refund|capture|void|psync|rsync|webhook).*(flow|support)\b/, skill: "add-connector-flow" },
  { match: /\b(payment method|wallet|card|upi|bnpl|bank transfer)\b/, skill: "add-payment-method" },
  { match: /\b(tech spec|technical specification|spec)\b/, skill: "generate-tech-spec" },
  { match: /\b(review|pr|pull request)\b/, skill: "pr-reviewer" },
  { match: /\b(coverage|metrics|report|stakeholder|meeting)\b/, skill: "coverage-report" },
  { match: /\b(sdk|integrate.*app|use.*sdk|client library|python|node|java)\b/, skill: "sdk-integration" },
  { match: /\b(demo|embed)\b/, skill: "demo-integration" },
];

export function findSkillForTask(task: string): { skill: SkillEntry; score: number } | undefined {
  const q = norm(task);
  for (const a of TASK_ALIASES) {
    if (a.match.test(q)) {
      const s = SKILL_BY_NAME.get(a.skill);
      if (s) return { skill: s, score: 100 };
    }
  }
  // fallback: token overlap with name + description + triggers
  const tokens = q.split(/[^a-z0-9]+/).filter((t) => t.length > 2);
  let best: { skill: SkillEntry; score: number } | undefined;
  for (const s of skills) {
    const hay = `${s.name} ${s.description} ${s.triggers.join(" ")}`.toLowerCase();
    let score = 0;
    for (const t of tokens) if (hay.includes(t)) score += 1;
    if (score > 0 && (!best || score > best.score)) best = { skill: s, score };
  }
  return best;
}

// ---------------------------------------------------------------------------
// Connectors + coverage.
// ---------------------------------------------------------------------------
export function listConnectors(): RegistryConnector[] {
  return registry;
}
export function getConnector(name: string): RegistryConnector | undefined {
  return CONNECTOR_BY_NAME.get(norm(name));
}
export function suggestConnectors(name: string, limit = 6): string[] {
  const q = norm(name);
  if (!q) return [];
  return registry
    .map((c) => {
      const n = c.name;
      let score = 0;
      if (n.startsWith(q) || q.startsWith(n)) score = 3;
      else if (n.includes(q) || q.includes(n)) score = 2;
      else if (n[0] === q[0]) score = 1;
      return { n, score };
    })
    .filter((s) => s.score > 0)
    .sort((a, b) => b.score - a.score)
    .slice(0, limit)
    .map((s) => s.n);
}
export function getCoverage(): Coverage {
  return coverage;
}
export function flowsForConnector(name: string): string[] {
  return coverage.connectors[norm(name)] ?? [];
}
/** Connectors supporting a flow; matches the flow label loosely (e.g. "refund" -> Pay.Refund / Refund.Get). */
export function connectorsForFlow(flowQuery: string): { flow: string; connectors: string[] }[] {
  const q = norm(flowQuery);
  const out: { flow: string; connectors: string[] }[] = [];
  for (const [flow, conns] of Object.entries(coverage.flows)) {
    if (norm(flow).includes(q)) out.push({ flow, connectors: conns });
  }
  return out;
}
export function listFlows(): string[] {
  return Object.keys(coverage.flows);
}

// ---------------------------------------------------------------------------
// Repo map.
// ---------------------------------------------------------------------------
export function listRepoAreas(): RepoArea[] {
  return repomap.areas;
}
export function topDirs(): string[] {
  return repomap.topDirs;
}
export function searchRepoMap(query: string): RepoArea[] {
  const q = norm(query);
  if (!q) return repomap.areas;
  const tokens = q.split(/[^a-z0-9]+/).filter(Boolean);
  return repomap.areas
    .map((a) => {
      const hay = `${a.topic} ${a.what} ${a.path} ${a.keywords.join(" ")}`.toLowerCase();
      let score = 0;
      for (const t of tokens) if (hay.includes(t)) score += 1;
      return { a, score };
    })
    .filter((x) => x.score > 0)
    .sort((x, y) => y.score - x.score)
    .map((x) => x.a);
}

// ---------------------------------------------------------------------------
// Learning paths.
// ---------------------------------------------------------------------------
export function listPaths(): LearningPath[] {
  return learningPaths;
}
export function getPath(id: string): LearningPath | undefined {
  const q = norm(id);
  return learningPaths.find((p) => p.id === q) ?? learningPaths.find((p) => norm(p.title).includes(q));
}
export function getPathForRole(role: string): LearningPath | undefined {
  const q = norm(role);
  return learningPaths.find((p) => p.role === q);
}

// ---------------------------------------------------------------------------
// Discrepancies.
// ---------------------------------------------------------------------------
export function listDiscrepancies(): Discrepancy[] {
  return discrepancies;
}
export function findDiscrepancies(query: string): Discrepancy[] {
  const q = norm(query);
  if (!q) return discrepancies;
  const tokens = q.split(/[^a-z0-9]+/).filter(Boolean);
  return discrepancies.filter((d) => {
    const hay = `${d.title} ${d.summary} ${d.topic} ${d.keywords.join(" ")} ${d.resolution}`.toLowerCase();
    return tokens.some((t) => hay.includes(t));
  });
}
