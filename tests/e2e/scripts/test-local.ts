import { spawnSync } from "node:child_process";
import { accessSync, constants, statSync } from "node:fs";
import { localTargets } from "../framework/local-target.js";

const targets = localTargets(process.env);
if (!isReadableFile(targets.connectorAuthFilePath)) {
  throw new Error("CONNECTOR_AUTH_FILE_PATH must identify a readable file");
}

const npm = process.platform === "win32" ? "npm.cmd" : "npm";
const result = spawnSync(
  npm,
  ["run", "test", "--", ...process.argv.slice(2)],
  {
    stdio: "inherit",
    env: {
      ...process.env,
      HS_BASE_URL: targets.hyperswitchBaseUrl,
      HS_DATABASE_URL: targets.databaseUrl,
      CONNECTOR_AUTH_FILE_PATH: targets.connectorAuthFilePath,
    },
  },
);

if (result.error) throw result.error;
process.exitCode = result.status ?? 1;

function isReadableFile(path: string): boolean {
  try {
    accessSync(path, constants.R_OK);
    return statSync(path).isFile();
  } catch {
    return false;
  }
}
