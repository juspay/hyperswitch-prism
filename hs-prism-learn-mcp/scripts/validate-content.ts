/**
 * Drift gate for hs-prism-learn-mcp. Run after gen-knowledge (wired into `prebuild`) and in CI.
 *
 * FAILS the build (exit 1) when the hand-authored content has drifted from the repo:
 *   - dead path:    any cited path (concept go_deeper, repo-map, path-step file,
 *                   discrepancy source, verify_anchor) no longer exists on disk
 *   - anchor drift: a verify_anchor's `must_contain` substring is gone from its file
 *   - slug break:   a card prerequisite/related or a learning-path step references a
 *                   slug that is not a real concept card
 *   - count drift:  the meta counts are missing/implausible
 *
 * Emits WARNINGS (does not fail) for style issues (hype words) so prose nudges don't
 * block a build. When the repo root cannot be found (e.g. a rebuild outside a checkout),
 * path/anchor checks are skipped with a warning; structural checks still run.
 */
import { readFileSync, existsSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const SCRIPT_DIR = dirname(fileURLToPath(import.meta.url));
const PKG_ROOT = resolve(SCRIPT_DIR, "..");
const DATA_DIR = join(PKG_ROOT, "src", "data");

const ROOT_CANDIDATES = [
  process.env.PRISM_REPO_ROOT,
  resolve(PKG_ROOT, ".."),
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
  for (const c of ROOT_CANDIDATES) if (existsSync(c) && isRepoRoot(c)) return c;
  return null;
}

function load<T>(name: string): T {
  return JSON.parse(readFileSync(join(DATA_DIR, name), "utf8")) as T;
}

const errors: string[] = [];
const warnings: string[] = [];
const fail = (m: string) => errors.push(m);
const warn = (m: string) => warnings.push(m);

const HYPE_WORDS = [
  "comprehensive", "robust", "seamless", "leverage", "utilize", "facilitate",
  "moreover", "furthermore",
];

function main(): void {
  const repoRoot = findRepoRoot();
  if (!repoRoot) {
    warn(`repo root not found (searched ${ROOT_CANDIDATES.join(", ")}); skipping path/anchor checks.`);
  }

  const concepts = load<{ concepts: any[] }>("concepts.generated.json").concepts;
  const paths = load<{ paths: any[] }>("paths.generated.json").paths;
  const repomap = load<{ areas: any[] }>("repomap.generated.json").areas;
  const discrepancies = load<{ discrepancies: any[] }>("discrepancies.generated.json").discrepancies;
  const meta = load<any>("meta.generated.json");

  const slugSet = new Set(concepts.map((c) => c.slug));

  // --- Collect every cited path with a label for good error messages. ---
  const citedPaths: { path: string; from: string }[] = [];
  const anchors: { path: string; must_contain: string; from: string }[] = [];

  for (const c of concepts) {
    if (!c.slug) fail(`a concept card is missing a slug`);
    if (!c.title) fail(`concept ${c.slug} is missing a title`);
    if (!c.depth?.tldr) fail(`concept ${c.slug} is missing depth.tldr`);
    for (const g of c.go_deeper ?? []) citedPaths.push({ path: g.path, from: `concept ${c.slug} go_deeper` });
    for (const a of c.verify_anchors ?? []) {
      citedPaths.push({ path: a.path, from: `concept ${c.slug} verify_anchors` });
      anchors.push({ path: a.path, must_contain: a.must_contain, from: `concept ${c.slug}` });
    }
    for (const slug of [...(c.prerequisites ?? []), ...(c.related ?? [])]) {
      if (!slugSet.has(slug)) fail(`concept ${c.slug} references unknown slug "${slug}"`);
    }
    // Style lint (warning only).
    const prose = [c.one_liner, c.analogy, c.depth?.tldr, c.depth?.standard, c.depth?.deep, c.body]
      .filter(Boolean)
      .join(" ")
      .toLowerCase();
    for (const w of HYPE_WORDS) {
      if (new RegExp(`\\b${w}\\b`).test(prose)) warn(`concept ${c.slug} uses hype word "${w}"`);
    }
  }

  for (const p of paths) {
    if (!p.id || !p.title) fail(`a learning path is missing id/title`);
    for (const step of p.steps ?? []) {
      if (step.slug && !slugSet.has(step.slug)) fail(`path ${p.id} step references unknown slug "${step.slug}"`);
      if (step.file) citedPaths.push({ path: step.file, from: `path ${p.id} step` });
    }
  }

  for (const a of repomap) {
    if (!a.path) fail(`a repo-map area is missing a path`);
    else citedPaths.push({ path: a.path, from: `repo-map "${a.topic}"` });
  }

  for (const d of discrepancies) {
    for (const s of d.sources ?? []) citedPaths.push({ path: s.path, from: `discrepancy ${d.id}` });
    if (d.authoritative) citedPaths.push({ path: d.authoritative, from: `discrepancy ${d.id} authoritative` });
  }

  // --- Path existence + anchor checks (only when repo root is available). ---
  if (repoRoot) {
    for (const { path, from } of citedPaths) {
      if (!path) {
        fail(`empty path cited from ${from}`);
        continue;
      }
      if (!existsSync(join(repoRoot, path))) fail(`dead path "${path}" (cited from ${from})`);
    }
    for (const { path, must_contain, from } of anchors) {
      const abs = join(repoRoot, path);
      if (!existsSync(abs)) continue; // already reported as dead path
      if (!must_contain) {
        fail(`empty must_contain anchor on ${path} (${from})`);
        continue;
      }
      const text = readFileSync(abs, "utf8");
      if (!text.includes(must_contain)) {
        fail(`anchor drift: "${must_contain}" no longer in ${path} (${from})`);
      }
    }
  }

  // --- Count sanity. ---
  if (!meta?.connectorCounts || typeof meta.connectorCounts.registry !== "number") {
    fail(`meta.connectorCounts.registry missing`);
  }
  if (!(meta.conceptCount >= 20)) fail(`expected >=20 concepts, meta says ${meta.conceptCount}`);
  if (!(meta.pathCount >= 4)) fail(`expected >=4 paths, meta says ${meta.pathCount}`);

  // --- Report. ---
  for (const w of warnings) console.warn(`⚠️  ${w}`);
  if (errors.length) {
    for (const e of errors) console.error(`❌ ${e}`);
    console.error(`\nvalidate-content: ${errors.length} error(s). Build blocked.`);
    process.exit(1);
  }
  console.log(
    `✅ validate-content: ${concepts.length} cards, ${paths.length} paths, ${repomap.length} repo-map areas, ` +
      `${citedPaths.length} cited paths, ${anchors.length} anchors all OK` +
      (warnings.length ? ` (${warnings.length} style warning(s))` : "") +
      (repoRoot ? "" : " [path checks skipped: no repo root]"),
  );
}

main();
