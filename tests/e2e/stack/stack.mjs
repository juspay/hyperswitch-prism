import { spawnSync } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  renameSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const e2eRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const runtime = resolve(e2eRoot, ".runtime");
const composeFile = resolve(e2eRoot, "stack/compose.yml");
const versionsFile = resolve(runtime, "versions.json");
const action = process.argv[2];
const hsImage =
  process.env.HS_IMAGE ??
  "docker.juspay.io/juspaydotin/hyperswitch-router:latest";
const ucsImage =
  process.env.UCS_IMAGE ?? "docker.io/juspaydotin/hyperswitch-prism:latest";

if (!new Set(["up", "down"]).has(action)) {
  throw new Error("Use stack.mjs up|down");
}

function run(command, args, options = {}) {
  const result = spawnSync(command, args, { stdio: "inherit", ...options });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`${command} exited ${result.status}`);
}

function output(command, args) {
  const result = spawnSync(command, args, { encoding: "utf8" });
  if (result.status !== 0) throw new Error(`${command} exited ${result.status}`);
  return result.stdout.trim();
}

async function sourceCheckout() {
  if (process.env.HS_SOURCE_DIR) return resolve(process.env.HS_SOURCE_DIR);

  if (action === "down" && existsSync(versionsFile)) {
    return JSON.parse(readFileSync(versionsFile, "utf8")).hsSourceDir;
  }

  run("docker", ["pull", hsImage]);
  const version = output("docker", [
    "run",
    "--rm",
    "--network",
    "none",
    "--entrypoint",
    "/local/bin/router",
    hsImage,
    "--version",
  ]);
  const reportedRef =
    process.env.HS_REF ?? version.match(/-[0-9a-f]{7,40}-/)?.[0].slice(1, -1);
  if (!reportedRef) {
    throw new Error(
      "Cannot identify the Hyperswitch image revision; set HS_REF or HS_SOURCE_DIR",
    );
  }

  const cached = readdirSync(runtime)
    .map((name) => name.match(/^hyperswitch-([0-9a-f]{40})$/)?.[1])
    .filter((sha) => sha?.startsWith(reportedRef));
  let commit = /^[0-9a-f]{40}$/.test(reportedRef) ? reportedRef : undefined;
  if (!commit && cached.length === 1) commit = cached[0];

  if (!commit) {
    const response = await fetch(
      `https://api.github.com/repos/juspay/hyperswitch/commits/${encodeURIComponent(reportedRef)}`,
      { signal: AbortSignal.timeout(30_000) },
    );
    if (!response.ok) {
      throw new Error(
        `Cannot resolve the Hyperswitch source revision (HTTP ${response.status})`,
      );
    }
    commit = (await response.json()).sha;
  }
  if (!/^[0-9a-f]{40}$/.test(commit)) throw new Error("Invalid HS revision");

  const checkout = resolve(runtime, `hyperswitch-${commit}`);
  if (!existsSync(checkout)) {
    const pending = `${checkout}.partial`;
    rmSync(pending, { recursive: true, force: true });
    run("git", ["init", "--quiet", pending]);
    run("git", [
      "-C",
      pending,
      "remote",
      "add",
      "origin",
      "https://github.com/juspay/hyperswitch.git",
    ]);
    run("git", [
      "-C",
      pending,
      "fetch",
      "--depth",
      "1",
      "--filter=blob:none",
      "origin",
      commit,
    ]);
    run("git", [
      "-C",
      pending,
      "sparse-checkout",
      "set",
      "config",
      "files",
      "migrations",
      "scripts",
    ]);
    run("git", ["-C", pending, "checkout", "--detach", "FETCH_HEAD"]);
    renameSync(pending, checkout);
  }
  return checkout;
}

mkdirSync(runtime, { recursive: true });
const checkout = await sourceCheckout();
if (!checkout || !existsSync(resolve(checkout, "docker-compose.yml"))) {
  throw new Error("The matching Hyperswitch source checkout is unavailable");
}

const hsConfig = resolve(runtime, "hs.toml");
const dieselConfig = resolve(runtime, "diesel.toml");
if (action === "up") {
  run("docker", ["pull", ucsImage]);
  let config = readFileSync(
    resolve(checkout, "config/docker_compose.toml"),
    "utf8",
  );
  config = config.replace(
    /^\[grpc_client\.unified_connector_service\][\s\S]*?(?=^\[|$(?![\s\S]))/m,
    "",
  );
  config +=
    "\n[grpc_client.unified_connector_service]\n" +
    'base_url = "http://ucs:8000"\n' +
    "connection_timeout = 10\n" +
    "request_timeout = 60\n" +
    'ucs_only_connectors = "adyen"\n';
  writeFileSync(hsConfig, config);
  writeFileSync(
    dieselConfig,
    '[migrations_directory]\ndir = "/app/migrations"\n',
  );
  writeFileSync(
    versionsFile,
    JSON.stringify({ hsImage, hsSourceDir: checkout }, null, 2),
  );
}

const compose = [
  "compose",
  "--project-name",
  "ucs-e2e",
  "--project-directory",
  checkout,
  "-f",
  resolve(checkout, "docker-compose.yml"),
  "-f",
  composeFile,
];
const env = {
  ...process.env,
  HS_IMAGE: hsImage,
  UCS_IMAGE: ucsImage,
  E2E_HS_CONFIG: hsConfig,
  E2E_DIESEL_CONFIG: dieselConfig,
};

run(
  "docker",
  [
    ...compose,
    ...(action === "up"
      ? ["up", "-d", "--wait", "--wait-timeout", "600", "hyperswitch-server"]
      : ["down"]),
  ],
  { env },
);
