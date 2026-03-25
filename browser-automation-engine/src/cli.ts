import fs from "node:fs/promises";
import path from "node:path";
import { PlaywrightDriverFactory } from "./drivers/playwrightDriver";
import { AutomationEngine } from "./engine/automationEngine";
import type { RunRequest } from "./types/api";
import { parseRunRequest, RequestValidationError } from "./utils/validation";

interface CliOptions {
  inputPath: string;
  outputPath?: string;
  headed: boolean;
  slowMoMs?: number;
  pretty: boolean;
}

function printUsage(): void {
  const usage = [
    "Usage:",
    "  npm run cli -- --input <path> [--headed] [--slow-mo <ms>] [--pretty] [--output <path>]",
    "",
    "Flags:",
    "  --input <path>    Required. JSON file containing { url, rules, options? }",
    "  --headed          Run browser with UI (sets headless=false)",
    "  --slow-mo <ms>    Add Playwright slow motion delay in milliseconds",
    "  --pretty          Pretty-print JSON output",
    "  --output <path>   Write response JSON to file",
    "  --help            Show this help"
  ];

  console.log(usage.join("\n"));
}

function parseCliOptions(argv: string[]): CliOptions {
  let inputPath: string | undefined;
  let outputPath: string | undefined;
  let headed = false;
  let slowMoMs: number | undefined;
  let pretty = false;

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];

    if (arg === "--help") {
      printUsage();
      process.exit(0);
    }

    if (arg === "--input") {
      const value = argv[i + 1];
      if (!value || value.startsWith("--")) {
        throw new Error("--input requires a file path");
      }
      inputPath = value;
      i += 1;
      continue;
    }

    if (arg === "--output") {
      const value = argv[i + 1];
      if (!value || value.startsWith("--")) {
        throw new Error("--output requires a file path");
      }
      outputPath = value;
      i += 1;
      continue;
    }

    if (arg === "--headed") {
      headed = true;
      continue;
    }

    if (arg === "--pretty") {
      pretty = true;
      continue;
    }

    if (arg === "--slow-mo") {
      const value = argv[i + 1];
      if (!value || value.startsWith("--")) {
        throw new Error("--slow-mo requires a number in milliseconds");
      }

      const parsed = Number(value);
      if (!Number.isFinite(parsed) || parsed < 0) {
        throw new Error("--slow-mo must be a non-negative number");
      }
      slowMoMs = parsed;
      i += 1;
      continue;
    }

    throw new Error(`Unknown argument: ${arg}`);
  }

  if (!inputPath) {
    throw new Error("--input is required");
  }

  return {
    inputPath,
    outputPath,
    headed,
    slowMoMs,
    pretty
  };
}

async function loadRequestFromFile(filePath: string): Promise<RunRequest> {
  const contents = await fs.readFile(filePath, "utf8");
  let parsed: unknown;

  try {
    parsed = JSON.parse(contents);
  } catch {
    throw new Error(`Invalid JSON in input file: ${filePath}`);
  }

  try {
    return parseRunRequest(parsed);
  } catch (error) {
    if (error instanceof RequestValidationError) {
      throw new Error(`Invalid run request: ${error.message}`);
    }
    throw error;
  }
}

async function main(): Promise<void> {
  const options = parseCliOptions(process.argv.slice(2));

  const request = await loadRequestFromFile(options.inputPath);
  request.options = {
    ...(request.options ?? {}),
    ...(options.headed ? { headless: false } : {}),
    ...(options.slowMoMs !== undefined ? { slowMoMs: options.slowMoMs } : {})
  };

  const engine = new AutomationEngine(new PlaywrightDriverFactory());
  const result = await engine.run(request);

  const output = JSON.stringify(result, null, options.pretty ? 2 : undefined);
  console.log(output);

  if (options.outputPath) {
    await fs.mkdir(path.dirname(options.outputPath), { recursive: true });
    await fs.writeFile(options.outputPath, JSON.stringify(result, null, 2));
  }

  if (!result.success) {
    process.exitCode = 1;
  }
}

main().catch((error) => {
  const message = error instanceof Error ? error.message : String(error);
  console.error(`cli error: ${message}`);
  process.exit(1);
});
