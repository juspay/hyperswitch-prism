/**
 * Build-time knowledge indexer for the hs-prism-learn-mcp server.
 *
 * Walks the hyperswitch-prism repo's real docs/skills/grace/connector-registry and the
 * hand-authored content/ (concept cards, learning paths, repo map, known discrepancies),
 * and emits committed JSON files consumed at runtime by the MCP tools:
 *
 *   src/data/docs.index.generated.json        — one searchable entry per markdown doc
 *   src/data/docs.bodies.generated.json        — path -> full markdown body (verbatim read_doc)
 *   src/data/skills.generated.json             — the .skills/* catalog (frontmatter + references)
 *   src/data/glossary.generated.json           — term -> definition (from docs-generated/glossary.md)
 *   src/data/connectors.registry.generated.json— connector name -> impl/transformers file paths
 *   src/data/coverage.generated.json           — best-effort flow/PM coverage (from all_connector.md)
 *   src/data/repomap.generated.json            — top-level dir map + curated topic -> path
 *   src/data/concepts.generated.json           — hand-authored plain-language concept cards
 *   src/data/paths.generated.json              — hand-authored role-based learning paths
 *   src/data/discrepancies.generated.json      — known cross-doc contradictions to surface
 *   src/data/meta.generated.json               — counts + provenance
 *
 * The repo is NOT shipped in the published npm package — these JSON files are. So the
 * server answers correctly even when run via `npx` outside the repo. Re-run in-repo with
 * `npm run gen:knowledge` (wired into `prebuild`) to refresh. Source of truth so the
 * server never hallucinates a path, term, connector, or status code.
 */
import {
  readFileSync,
  writeFileSync,
  existsSync,
  readdirSync,
  statSync,
  realpathSync,
  mkdirSync,
} from "node:fs";
import { dirname, join, resolve, relative, basename, extname } from "node:path";
import { fileURLToPath } from "node:url";

const SCRIPT_DIR = dirname(fileURLToPath(import.meta.url));
const PKG_ROOT = resolve(SCRIPT_DIR, "..");
const DATA_DIR = join(PKG_ROOT, "src", "data");
const CONTENT_DIR = join(PKG_ROOT, "content");

// ---------------------------------------------------------------------------
// Repo-root discovery (mirrors integration-mcp/scripts/gen-connectors.ts).
// ---------------------------------------------------------------------------
const ROOT_CANDIDATES = [
  process.env.PRISM_REPO_ROOT,
  resolve(PKG_ROOT, ".."), // hs-prism-learn-mcp/ is a sibling of the repo's top-level dirs
  resolve(PKG_ROOT, "..", ".."),
].filter((p): p is string => Boolean(p));

function isRepoRoot(dir: string): boolean {
  return (
    existsSync(join(dir, "docs", "SUMMARY.md")) &&
    existsSync(join(dir, ".skills")) &&
    existsSync(join(dir, "crates"))
  );
}

function findRepoRoot(): string | null {
  for (const candidate of ROOT_CANDIDATES) {
    if (existsSync(candidate) && isRepoRoot(candidate)) return candidate;
  }
  return null;
}

// ---------------------------------------------------------------------------
// Small helpers.
// ---------------------------------------------------------------------------
const SKIP_DIRS = new Set(["node_modules", "target", "dist", ".git", ".github", "build"]);

function expect(cond: boolean, msg: string): void {
  if (!cond) {
    console.error(`❌ gen-knowledge sanity check failed: ${msg}`);
    process.exit(1);
  }
}

function repoRel(root: string, absPath: string): string {
  return relative(root, absPath).split("\\").join("/");
}

function walkMarkdown(absDir: string, out: string[] = []): string[] {
  if (!existsSync(absDir)) return out;
  for (const name of readdirSync(absDir)) {
    if (SKIP_DIRS.has(name)) continue;
    const full = join(absDir, name);
    let st;
    try {
      st = statSync(full);
    } catch {
      continue;
    }
    if (st.isDirectory()) walkMarkdown(full, out);
    else if (extname(name).toLowerCase() === ".md") out.push(full);
  }
  return out;
}

function slugifyAnchor(text: string): string {
  return text
    .toLowerCase()
    .replace(/[^a-z0-9\s-]/g, "")
    .trim()
    .replace(/\s+/g, "-");
}

function humanizeFilename(path: string): string {
  return basename(path, extname(path))
    .replace(/[-_]+/g, " ")
    .replace(/\b\w/g, (c) => c.toUpperCase());
}

const STOPWORDS = new Set([
  "the", "and", "for", "with", "this", "that", "from", "into", "your", "you", "are", "how",
  "what", "why", "use", "using", "guide", "reference", "overview", "readme", "docs", "doc",
]);

function keywordize(...texts: string[]): string[] {
  const set = new Set<string>();
  for (const t of texts) {
    for (const tok of t.toLowerCase().split(/[^a-z0-9_]+/)) {
      if (tok.length > 2 && !STOPWORDS.has(tok)) set.add(tok);
    }
  }
  return [...set];
}

function categorize(path: string): string {
  if (path.startsWith("docs/architecture/concepts/")) return "architecture-concept";
  if (path.startsWith("docs/architecture/frameworks/")) return "architecture-framework";
  if (path.startsWith("docs/architecture")) return "architecture";
  if (path.startsWith("docs/getting-started/payment-methods/")) return "payment-method";
  if (path.startsWith("docs/getting-started/")) return "getting-started";
  if (path.startsWith("docs/blogs/")) return "blog";
  if (path.startsWith("docs/")) return "docs";
  if (path.startsWith(".skills/_shared/references/flow-patterns/")) return "flow-pattern";
  if (path.startsWith(".skills/_shared/references/")) return "skill-reference";
  if (path.startsWith(".skills/")) return "skill";
  if (path.startsWith("grace/")) return "grace";
  if (path.startsWith("docs-generated/")) return "generated";
  return "overview";
}

// ---------------------------------------------------------------------------
// Doc indexing.
// ---------------------------------------------------------------------------
interface Heading {
  level: number;
  text: string;
  anchor: string;
}
interface DocEntry {
  path: string;
  title: string;
  category: string;
  headings: Heading[];
  summary: string;
  keywords: string[];
  wordCount: number;
  githubUrl: string;
  aliases: string[]; // other repo paths (symlinks) that point at this same file
}

const GITHUB_BLOB = "https://github.com/juspay/hyperswitch-prism/blob/main";

function firstTitle(text: string, fallbackPath: string): string {
  const m = text.match(/^#\s+(.+)$/m);
  return m ? m[1]!.trim() : humanizeFilename(fallbackPath);
}

function extractHeadings(text: string): Heading[] {
  const out: Heading[] = [];
  for (const line of text.split("\n")) {
    const m = line.match(/^(#{2,3})\s+(.+?)\s*$/);
    if (m) {
      const level = m[1]!.length;
      const htext = m[2]!.replace(/[#*`]/g, "").trim();
      if (htext) out.push({ level, text: htext, anchor: slugifyAnchor(htext) });
    }
  }
  return out;
}

function extractLede(text: string): string {
  // Skip frontmatter, HTML comments, headings, and blank lines; take the first prose paragraph.
  let body = text;
  if (body.startsWith("---\n")) {
    const end = body.indexOf("\n---", 4);
    if (end >= 0) body = body.slice(end + 4);
  }
  body = body.replace(/<!--[\s\S]*?-->/g, "");
  const lines = body.split("\n");
  const paras: string[] = [];
  let cur = "";
  for (const raw of lines) {
    const line = raw.trim();
    if (!line) {
      if (cur) {
        paras.push(cur);
        cur = "";
      }
      continue;
    }
    if (/^#{1,6}\s/.test(line)) continue;
    if (/^[|>`-]/.test(line) || /^\d+\.\s/.test(line)) continue;
    cur = cur ? `${cur} ${line}` : line;
    if (cur.length > 280) break;
  }
  if (cur) paras.push(cur);
  const lede = (paras[0] ?? "").replace(/[*_`]/g, "").trim();
  return lede.length > 300 ? `${lede.slice(0, 297)}...` : lede;
}

// ---------------------------------------------------------------------------
// Glossary parsing (docs-generated/glossary.md).
// ---------------------------------------------------------------------------
interface GlossaryTerm {
  term: string;
  definition: string;
  sourcePath: string;
}

function parseGlossary(text: string, sourcePath: string): GlossaryTerm[] {
  const out: GlossaryTerm[] = [];
  // Skip the leading @doc-guidance HTML comment block: start at "# Glossary".
  const start = text.indexOf("# Glossary");
  const body = start >= 0 ? text.slice(start) : text;
  for (const line of body.split("\n")) {
    const m = line.match(/^\*\*(.+?)\*\*\s*[—–-]\s*(.+)$/);
    if (m) {
      const term = m[1]!.trim();
      const definition = m[2]!.replace(/\s+/g, " ").trim();
      if (term && definition) out.push({ term, definition, sourcePath });
    }
  }
  return out;
}

// ---------------------------------------------------------------------------
// Skill parsing (.skills/<name>/SKILL.md frontmatter + references).
// ---------------------------------------------------------------------------
interface SkillEntry {
  name: string;
  description: string;
  domain: string;
  skillMdPath: string;
  references: { path: string; aliasedFrom?: string }[];
  triggers: string[];
}

function parseFrontmatter(text: string): Record<string, string> {
  const out: Record<string, string> = {};
  if (!text.startsWith("---")) return out;
  const end = text.indexOf("\n---", 3);
  if (end < 0) return out;
  const fm = text.slice(text.indexOf("\n") + 1, end);
  const lines = fm.split("\n");
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i]!;
    const kv = line.match(/^([a-zA-Z_][\w-]*):\s*(.*)$/);
    if (!kv) continue;
    const key = kv[1]!;
    let val = kv[2]!.trim();
    if (val === ">" || val === "|" || val === ">-" || val === "|-") {
      // folded/literal block: gather subsequent more-indented lines.
      const parts: string[] = [];
      for (let j = i + 1; j < lines.length; j++) {
        if (/^\s+\S/.test(lines[j]!)) parts.push(lines[j]!.trim());
        else break;
      }
      val = parts.join(" ");
    }
    out[key] = val;
    // shallow nested metadata.domain
    if (key === "metadata") {
      for (let j = i + 1; j < lines.length; j++) {
        const dm = lines[j]!.match(/^\s+domain:\s*(.+)$/);
        if (dm) out["domain"] = dm[1]!.trim();
        if (!/^\s+\S/.test(lines[j]!)) break;
      }
    }
  }
  return out;
}

function extractTriggers(description: string): string[] {
  const triggers: string[] = [];
  const re = /\bUse when ([^.]+)\./gi;
  let m;
  while ((m = re.exec(description))) triggers.push(m[1]!.trim());
  return triggers;
}

// ---------------------------------------------------------------------------
// Connector registry parsing (connectors.rs).
// ---------------------------------------------------------------------------
interface RegistryConnector {
  name: string;
  implPath: string;
  transformersPath: string | null;
  inRegistry: boolean;
}

function parseRegistry(root: string): RegistryConnector[] {
  const regPath = join(root, "crates/integrations/connector-integration/src/connectors.rs");
  const text = readFileSync(regPath, "utf8");
  const names = new Set<string>();
  for (const line of text.split("\n")) {
    const m = line.match(/^pub mod (\w+);/);
    if (m) names.add(m[1]!);
  }
  const connectorsDir = "crates/integrations/connector-integration/src/connectors";
  const out: RegistryConnector[] = [];
  for (const name of [...names].sort()) {
    const implRel = `${connectorsDir}/${name}.rs`;
    const xformRel = `${connectorsDir}/${name}/transformers.rs`;
    out.push({
      name,
      implPath: implRel,
      transformersPath: existsSync(join(root, xformRel)) ? xformRel : null,
      inRegistry: true,
    });
  }
  return out;
}

// ---------------------------------------------------------------------------
// Coverage parsing (best-effort, never sanity-gated) from all_connector.md.
// ---------------------------------------------------------------------------
interface Coverage {
  sourcePath: string;
  flows: Record<string, string[]>; // flow -> connectors with >=1 supported PM
  connectors: Record<string, string[]>; // connector -> flows it supports
  note: string;
}

function cellConnectorName(cell: string): string {
  const m = cell.match(/\[([^\]]+)\]/); // strip markdown link: [Stripe](...) -> Stripe
  return (m ? m[1]! : cell).trim().toLowerCase();
}

function parseCoverage(root: string): Coverage {
  const sourcePath = "docs-generated/all_connector.md";
  const empty: Coverage = {
    sourcePath,
    flows: {},
    connectors: {},
    note: "Best-effort parse of docs-generated/all_connector.md. For 'Authorize', ✓ = at least one payment method supported. See the source file for the full payment-method matrix.",
  };
  try {
    const text = readFileSync(join(root, sourcePath), "utf8");
    const lines = text.split("\n");
    const flows: Record<string, Set<string>> = {};
    const connectors: Record<string, Set<string>> = {};
    const addSupport = (flow: string, connector: string) => {
      flows[flow] = flows[flow] ?? new Set();
      flows[flow]!.add(connector);
      connectors[connector] = connectors[connector] ?? new Set();
      connectors[connector]!.add(flow);
    };

    let section: string | null = null;
    let header: string[] | null = null; // column labels (after the "Connector" col)
    for (const raw of lines) {
      const fh = raw.match(/^###\s+(.+?)\s*$/);
      if (fh) {
        section = fh[1]!.trim();
        header = null;
        continue;
      }
      if (raw.startsWith("## ")) {
        section = null;
        header = null;
        continue;
      }
      if (!section || !raw.startsWith("|")) continue;
      const cells = raw.split("|").map((c) => c.trim());
      cells.shift(); // leading empty
      if (cells[cells.length - 1] === "") cells.pop(); // trailing empty
      if (/^:?-{3,}/.test(cells[0] ?? "")) continue; // separator row
      if ((cells[0] ?? "").toLowerCase() === "connector") {
        header = cells.slice(1);
        continue;
      }
      if (!header) continue;
      const name = cellConnectorName(cells[0] ?? "");
      if (!name) continue;
      const cols = cells.slice(1);
      const isOtherFlows = /other flows/i.test(section);
      if (isOtherFlows) {
        // Columns ARE flow names; mark each flow the connector supports.
        cols.forEach((c, i) => {
          if (c.includes("✓") && header![i]) addSupport(header![i]!, name);
        });
      } else {
        // Columns are payment methods; the section heading IS the flow.
        if (cols.some((c) => c.includes("✓"))) addSupport(section, name);
      }
    }

    const flowsOut: Record<string, string[]> = {};
    for (const [k, v] of Object.entries(flows)) flowsOut[k] = [...v].sort();
    const connOut: Record<string, string[]> = {};
    for (const [k, v] of Object.entries(connectors)) connOut[k] = [...v].sort();
    return { ...empty, flows: flowsOut, connectors: connOut };
  } catch (err) {
    console.warn(`⚠️ coverage parse failed (${String(err)}); emitting empty coverage.`);
    return empty;
  }
}

// ---------------------------------------------------------------------------
// Hand-authored content compilation (content/).
// ---------------------------------------------------------------------------
interface ConceptCard {
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

function parseCard(absPath: string): ConceptCard {
  const text = readFileSync(absPath, "utf8");
  if (!text.startsWith("---")) {
    throw new Error(`Concept card ${basename(absPath)} must start with a --- JSON frontmatter fence.`);
  }
  const end = text.indexOf("\n---", 3);
  if (end < 0) throw new Error(`Concept card ${basename(absPath)} has no closing --- fence.`);
  const fmRaw = text.slice(text.indexOf("\n") + 1, end);
  let fm: any;
  try {
    fm = JSON.parse(fmRaw);
  } catch (e) {
    throw new Error(`Concept card ${basename(absPath)} frontmatter is not valid JSON: ${String(e)}`);
  }
  const body = text.slice(end + 4).trim();
  return {
    slug: fm.slug,
    title: fm.title,
    tier: fm.tier ?? "misc",
    audience: fm.audience ?? "everyone",
    one_liner: fm.one_liner ?? "",
    analogy: fm.analogy ?? "",
    depth: fm.depth ?? { tldr: fm.one_liner ?? "" },
    prerequisites: fm.prerequisites ?? [],
    related: fm.related ?? [],
    go_deeper: fm.go_deeper ?? [],
    verify_anchors: fm.verify_anchors ?? [],
    body,
  };
}

function loadConcepts(): ConceptCard[] {
  const dir = join(CONTENT_DIR, "concepts");
  if (!existsSync(dir)) return [];
  return readdirSync(dir)
    .filter((f) => f.endsWith(".md"))
    .sort()
    .map((f) => parseCard(join(dir, f)));
}

function loadJsonContent<T>(name: string, fallback: T): T {
  const p = join(CONTENT_DIR, name);
  if (!existsSync(p)) return fallback;
  return JSON.parse(readFileSync(p, "utf8")) as T;
}

// ---------------------------------------------------------------------------
// Main.
// ---------------------------------------------------------------------------
function keepCommittedOrThrow(): never {
  const hasData = existsSync(join(DATA_DIR, "docs.index.generated.json"));
  if (hasData) {
    console.warn(
      "⚠️  Could not locate the hyperswitch-prism repo root; keeping the committed generated JSON.\n" +
        `   Searched: ${ROOT_CANDIDATES.join(", ")}\n` +
        "   Set PRISM_REPO_ROOT to regenerate against a checkout.",
    );
    process.exit(0);
  }
  console.error(
    "❌ Could not locate the hyperswitch-prism repo root and no committed JSON exists.\n" +
      `   Searched: ${ROOT_CANDIDATES.join(", ")}\n` +
      "   Set PRISM_REPO_ROOT to the repo checkout.",
  );
  process.exit(1);
}

function main(): void {
  const root = findRepoRoot();
  if (!root) keepCommittedOrThrow();
  const REPO = root as string;
  console.log(`gen-knowledge: indexing repo at ${REPO}`);

  // --- Collect markdown files (explicit roots, bounded). ---
  const fileSet = new Set<string>();
  const add = (rel: string) => {
    const abs = join(REPO, rel);
    if (existsSync(abs) && statSync(abs).isFile()) fileSet.add(abs);
  };
  const addDir = (rel: string) => {
    for (const f of walkMarkdown(join(REPO, rel))) fileSet.add(f);
  };

  addDir("docs");
  addDir(".skills");
  addDir("grace/rulesbook/codegen/guides");
  addDir("grace/rulesbook/codegen/patterns");
  add("grace/rulesbook/codegen/README.md");
  add("grace/README.md");
  add("README.md");
  add("setup.md");
  add("docs-generated/glossary.md");
  add("docs-generated/all_connector.md");

  // --- Index docs with symlink dedup by canonical (real) path. ---
  const byCanonical = new Map<string, DocEntry>();
  const bodies: Record<string, string> = {};
  for (const abs of [...fileSet].sort()) {
    let canonicalAbs: string;
    try {
      canonicalAbs = realpathSync(abs);
    } catch {
      canonicalAbs = abs;
    }
    const aliasRel = repoRel(REPO, abs);
    const canonicalRel = canonicalAbs.startsWith(REPO) ? repoRel(REPO, canonicalAbs) : aliasRel;

    const existing = byCanonical.get(canonicalRel);
    if (existing) {
      if (aliasRel !== canonicalRel && !existing.aliases.includes(aliasRel)) {
        existing.aliases.push(aliasRel);
      }
      continue;
    }
    const text = readFileSync(canonicalAbs, "utf8");
    const title = firstTitle(text, canonicalRel);
    const headings = extractHeadings(text);
    const entry: DocEntry = {
      path: canonicalRel,
      title,
      category: categorize(canonicalRel),
      headings,
      summary: extractLede(text),
      keywords: keywordize(title, headings.map((h) => h.text).join(" ")),
      wordCount: text.split(/\s+/).filter(Boolean).length,
      githubUrl: `${GITHUB_BLOB}/${canonicalRel}`,
      aliases: aliasRel !== canonicalRel ? [aliasRel] : [],
    };
    byCanonical.set(canonicalRel, entry);
    bodies[canonicalRel] = text;
  }
  const docs = [...byCanonical.values()].sort((a, b) => a.path.localeCompare(b.path));

  // --- Glossary. ---
  const glossaryPath = "docs-generated/glossary.md";
  const glossary = parseGlossary(bodies[glossaryPath] ?? readFileSync(join(REPO, glossaryPath), "utf8"), glossaryPath);

  // --- Skills. ---
  const skills: SkillEntry[] = [];
  const skillsRoot = join(REPO, ".skills");
  for (const name of readdirSync(skillsRoot).sort()) {
    if (name.startsWith("_")) continue;
    const skillMd = join(skillsRoot, name, "SKILL.md");
    if (!existsSync(skillMd)) continue;
    const text = readFileSync(skillMd, "utf8");
    const fm = parseFrontmatter(text);
    const refsDir = join(skillsRoot, name, "references");
    const references: { path: string; aliasedFrom?: string }[] = [];
    if (existsSync(refsDir)) {
      for (const r of walkMarkdown(refsDir)) {
        let canon = r;
        try {
          canon = realpathSync(r);
        } catch {
          /* keep */
        }
        const canonRel = canon.startsWith(REPO) ? repoRel(REPO, canon) : repoRel(REPO, r);
        const aliasRel = repoRel(REPO, r);
        references.push(aliasRel === canonRel ? { path: canonRel } : { path: canonRel, aliasedFrom: aliasRel });
      }
    }
    skills.push({
      name: fm["name"] ?? name,
      description: (fm["description"] ?? "").replace(/\s+/g, " ").trim(),
      domain: fm["domain"] ?? "",
      skillMdPath: repoRel(REPO, skillMd),
      references: references.sort((a, b) => a.path.localeCompare(b.path)),
      triggers: extractTriggers(fm["description"] ?? ""),
    });
  }

  // --- Connector registry + coverage. ---
  const registry = parseRegistry(REPO);
  const coverage = parseCoverage(REPO);

  // --- Connector count discrepancy inputs. ---
  let llmsTxtCount: number | null = null;
  const llmsPath = join(REPO, "docs-generated/llms.txt");
  if (existsSync(llmsPath)) {
    const m = readFileSync(llmsPath, "utf8").match(/Connectors:\s*(\d+)/i);
    if (m) llmsTxtCount = Number(m[1]);
  }

  // --- Hand-authored content. ---
  const concepts = loadConcepts();
  const paths = loadJsonContent<any[]>("paths.json", []);
  const repoMapAreas = loadJsonContent<any[]>("repo-map.json", []);
  const discrepancies = loadJsonContent<any[]>("known-discrepancies.json", []);

  // repomap = curated areas (validated downstream) + computed top-level dir listing.
  const topDirs = readdirSync(REPO)
    .filter((n) => {
      if (SKIP_DIRS.has(n) || n.startsWith(".")) return n === ".skills";
      try {
        return statSync(join(REPO, n)).isDirectory();
      } catch {
        return false;
      }
    })
    .sort();
  const repomap = { areas: repoMapAreas, topDirs };

  // --- Sanity guards. ---
  expect(docs.length > 25, `expected >25 indexed docs, got ${docs.length}`);
  expect(skills.length === 8, `expected 8 skills, got ${skills.length} (${skills.map((s) => s.name).join(", ")})`);
  expect(glossary.length >= 10 && glossary.length <= 80, `expected 10-80 glossary terms, got ${glossary.length}`);
  expect(registry.length >= 80 && registry.length <= 130, `expected 80-130 registry connectors, got ${registry.length}`);
  expect(concepts.length >= 20, `expected >=20 concept cards, got ${concepts.length}`);
  expect(paths.length >= 4, `expected >=4 learning paths, got ${paths.length}`);

  // --- Write. ---
  mkdirSync(DATA_DIR, { recursive: true });
  const write = (name: string, data: unknown) => {
    writeFileSync(join(DATA_DIR, name), JSON.stringify(data, null, 2) + "\n");
    console.log(`  wrote data/${name}`);
  };

  const generatedNote = "GENERATED by scripts/gen-knowledge.ts from the hyperswitch-prism repo. Do not edit by hand.";

  write("docs.index.generated.json", { _comment: generatedNote, count: docs.length, docs });
  write("docs.bodies.generated.json", { _comment: generatedNote, bodies });
  write("skills.generated.json", { _comment: generatedNote, count: skills.length, skills });
  write("glossary.generated.json", { _comment: generatedNote, count: glossary.length, terms: glossary });
  write("connectors.registry.generated.json", {
    _comment: generatedNote,
    count: registry.length,
    connectors: registry,
  });
  write("coverage.generated.json", { _comment: generatedNote, ...coverage });
  write("repomap.generated.json", { _comment: generatedNote, ...repomap });
  write("concepts.generated.json", { _comment: generatedNote, count: concepts.length, concepts });
  write("paths.generated.json", { _comment: generatedNote, count: paths.length, paths });
  write("discrepancies.generated.json", { _comment: generatedNote, count: discrepancies.length, discrepancies });
  write("meta.generated.json", {
    _comment: generatedNote,
    connectorCounts: { registry: registry.length, llmsTxt: llmsTxtCount },
    docCount: docs.length,
    skillCount: skills.length,
    glossaryCount: glossary.length,
    conceptCount: concepts.length,
    pathCount: paths.length,
  });

  console.log(
    `gen-knowledge: done. ${docs.length} docs, ${skills.length} skills, ${glossary.length} terms, ` +
      `${registry.length} connectors (llms.txt says ${llmsTxtCount}), ${concepts.length} concept cards, ${paths.length} paths.`,
  );
}

main();
