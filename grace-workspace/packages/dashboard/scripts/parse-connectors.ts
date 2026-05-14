#!/usr/bin/env tsx
/**
 * Parse all_connector.md into structured JSON for dashboard consumption.
 * Reads two tables from the same markdown file:
 *   - `### PaymentService.Authorize` → paymentMethods[] + stats per connector.
 *   - `### Other Flows`              → flows[] (minus Authorize) + flowStats.
 * Authorize itself is then prepended to flows[] as a roll-up of paymentMethods.
 * Run during build: `tsx scripts/parse-connectors.ts`
 */
import * as fs from "fs";
import * as path from "path";
import { fileURLToPath } from "url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

type Status = "supported" | "not_supported" | "not_implemented" | "error";

interface Stats {
  total: number;
  supported: number;
  notImplemented: number;
  notSupported: number;
  error: number;
}

interface ConnectorData {
  name: string;
  filePath: string;
  paymentMethods: Array<{
    category: string;
    method: string;
    status: Status;
  }>;
  stats: Stats;
  flows: Array<{ name: string; status: Status }>;
  flowStats: Stats;
}

const STATUS_MAP: Record<string, Status> = {
  "✓": "supported",
  "x": "not_supported",
  "⚠": "not_implemented",
  "?": "error",
};

/**
 * Roll up a set of cell statuses into a single flow-level status.
 * Used for the Authorize flow (which fans out to ~100 payment methods).
 * Priority: any supported → supported; else any not_implemented →
 * not_implemented; else any not_supported → not_supported; else error.
 */
function rollupStatus(cells: Status[]): Status {
  if (cells.length === 0) return "error";
  if (cells.some((c) => c === "supported")) return "supported";
  if (cells.some((c) => c === "not_implemented")) return "not_implemented";
  if (cells.some((c) => c === "not_supported")) return "not_supported";
  return "error";
}

function emptyStats(): Stats {
  return { total: 0, supported: 0, notImplemented: 0, notSupported: 0, error: 0 };
}

function bumpStats(s: Stats, status: Status) {
  s.total++;
  if (status === "supported") s.supported++;
  else if (status === "not_implemented") s.notImplemented++;
  else if (status === "not_supported") s.notSupported++;
  else s.error++;
}

/**
 * Locate a markdown table that sits under a section heading.
 * Returns `{ headers, rows }` where rows are raw `| ... |` strings, or null
 * if the section/table can't be found.
 */
function findTableUnderHeading(
  lines: string[],
  heading: string
): { headers: string[]; rows: string[] } | null {
  const headingIdx = lines.findIndex((l) => l.trim() === heading);
  if (headingIdx < 0) return null;

  // Walk forward to the first table header row starting with `| Connector`.
  let headerIdx = -1;
  for (let i = headingIdx + 1; i < lines.length; i++) {
    if (lines[i].startsWith("| Connector")) {
      headerIdx = i;
      break;
    }
    // Bail out if we crossed into the next section before finding a header.
    if (lines[i].startsWith("## ")) return null;
  }
  if (headerIdx < 0) return null;

  const headers = lines[headerIdx]
    .split("|")
    .map((h) => h.trim())
    .filter((h) => h && h !== "Connector");

  // Skip the separator line (|-----|), then collect data rows.
  const rows: string[] = [];
  for (let i = headerIdx + 2; i < lines.length; i++) {
    const line = lines[i];
    if (line.startsWith("| [")) {
      rows.push(line);
      continue;
    }
    // Empty line or non-row content terminates the table.
    if (!line.trim() || line.startsWith("#")) break;
  }
  return { headers, rows };
}

function parseRowCells(row: string): { name: string; filePath: string; cells: string[] } | null {
  const parts = row.split("|").map((p) => p.trim());
  if (parts.length < 2) return null;
  const nameMatch = parts[1]?.match(/\[([^\]]+)\]\(([^)]+)\)/);
  if (!nameMatch) return null;
  // parts[0] is "" (before first |), parts[1] is connector cell, parts[2..N+1] are status cells.
  // The trailing "" after the final | is dropped by filtering, but we want to keep cells aligned with headers.
  const cells = parts.slice(2, -1);
  return { name: nameMatch[1], filePath: nameMatch[2], cells };
}

function parseAllConnectorMd(content: string): ConnectorData[] {
  const lines = content.split("\n");

  // ── Pass 1: Authorize table → paymentMethods + stats ───────────────────
  const authorizeTable = findTableUnderHeading(lines, "### PaymentService.Authorize");
  if (!authorizeTable) {
    throw new Error("Could not locate `### PaymentService.Authorize` table in all_connector.md");
  }

  const connectors: ConnectorData[] = [];
  const byName = new Map<string, ConnectorData>();

  for (const row of authorizeTable.rows) {
    const parsed = parseRowCells(row);
    if (!parsed) continue;

    const paymentMethods: ConnectorData["paymentMethods"] = [];
    const stats = emptyStats();

    for (let i = 0; i < authorizeTable.headers.length; i++) {
      const header = authorizeTable.headers[i];
      const symbol = parsed.cells[i];
      if (!header || !symbol) continue;
      const [category, method] = header.split(" / ").map((s) => s.trim());
      const status = STATUS_MAP[symbol] || "error";
      paymentMethods.push({
        category: category || "Unknown",
        method: method || category || "Unknown",
        status,
      });
      bumpStats(stats, status);
    }

    const entry: ConnectorData = {
      name: parsed.name,
      filePath: parsed.filePath,
      paymentMethods,
      stats,
      flows: [],          // filled in pass 2
      flowStats: emptyStats(),
    };
    connectors.push(entry);
    byName.set(parsed.name, entry);
  }

  // ── Pass 2: Other Flows table → flows[] + flowStats ────────────────────
  const otherFlowsTable = findTableUnderHeading(lines, "### Other Flows");
  if (!otherFlowsTable) {
    throw new Error("Could not locate `### Other Flows` table in all_connector.md");
  }

  for (const row of otherFlowsTable.rows) {
    const parsed = parseRowCells(row);
    if (!parsed) continue;
    const entry = byName.get(parsed.name);
    if (!entry) {
      // A connector listed in Other Flows but missing from Authorize is unexpected;
      // fail loud rather than silently drop it.
      throw new Error(
        `Connector "${parsed.name}" appears in Other Flows table but not in PaymentService.Authorize table`
      );
    }

    const flows: ConnectorData["flows"] = [];
    const flowStats = emptyStats();

    // Authorize is rolled up from this connector's payment-method statuses,
    // prepended so flows[] always leads with the Authorize entry.
    const authorizeStatus = rollupStatus(entry.paymentMethods.map((pm) => pm.status));
    flows.push({ name: "PaymentService.Authorize", status: authorizeStatus });
    bumpStats(flowStats, authorizeStatus);

    for (let i = 0; i < otherFlowsTable.headers.length; i++) {
      const flowName = otherFlowsTable.headers[i];
      const symbol = parsed.cells[i];
      if (!flowName || !symbol) continue;
      const status = STATUS_MAP[symbol] || "error";
      flows.push({ name: flowName, status });
      bumpStats(flowStats, status);
    }

    entry.flows = flows;
    entry.flowStats = flowStats;
  }

  // Sanity: every connector must have at least the Authorize roll-up.
  for (const c of connectors) {
    if (c.flows.length === 0) {
      throw new Error(`Connector "${c.name}" has no flows after Other Flows pass`);
    }
  }

  return connectors;
}

function main() {
  const mdPath = path.resolve(
    __dirname,
    "../../../../docs-generated/all_connector.md"
  );
  const outputPath = path.resolve(__dirname, "../src/data/connectors.json");

  if (!fs.existsSync(mdPath)) {
    console.error(`❌ Input file not found: ${mdPath}`);
    process.exit(1);
  }

  console.log(`📖 Reading ${mdPath}...`);
  const content = fs.readFileSync(mdPath, "utf-8");

  console.log("🔍 Parsing connector data...");
  const connectors = parseAllConnectorMd(content);

  if (connectors.length === 0) {
    console.error("❌ No connectors parsed! Check the markdown format.");
    process.exit(1);
  }

  console.log(`✅ Parsed ${connectors.length} connectors`);

  // Ensure output directory exists
  const outputDir = path.dirname(outputPath);
  if (!fs.existsSync(outputDir)) {
    fs.mkdirSync(outputDir, { recursive: true });
  }

  fs.writeFileSync(outputPath, JSON.stringify(connectors, null, 2));
  console.log(`💾 Written to ${outputPath}`);

  // Summary stats
  const totalNotImplemented = connectors.reduce(
    (sum, c) => sum + c.stats.notImplemented,
    0
  );
  const totalNotImplementedFlows = connectors.reduce(
    (sum, c) => sum + c.flowStats.notImplemented,
    0
  );
  const flowsPerConnector = connectors[0]?.flows.length ?? 0;
  console.log(`📊 Total not-implemented payment methods: ${totalNotImplemented}`);
  console.log(`📊 Total not-implemented flows: ${totalNotImplementedFlows} (${flowsPerConnector} flows tracked per connector)`);
}

main();
