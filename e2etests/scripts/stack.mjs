import { spawnSync } from "node:child_process";
import { mkdirSync, existsSync, readFileSync, writeFileSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const runtime = resolve(root, ".runtime");
const checkout = process.env.HS_SOURCE_DIR
  ? resolve(process.env.HS_SOURCE_DIR)
  : resolve(runtime, "hyperswitch");
const override = resolve(runtime, "compose.e2e.json");
const action = process.argv[2];
if (!["up", "down", "prepare"].includes(action))
  throw new Error("Use stack.mjs up|down|prepare");
function run(command, args, options = {}) {
  const result = spawnSync(command, args, { stdio: "inherit", ...options });
  if (result.error) throw result.error;
  if (result.status !== 0)
    throw new Error(`${command} exited ${result.status}`);
}
mkdirSync(runtime, { recursive: true });
if (action !== "down") {
  // Checkout is solely for the matching config, migrations, and upstream Compose dependencies.
  // The router itself always runs from an image.
  if (!existsSync(checkout)) {
    run("git", [
      "clone",
      "--depth",
      "1",
      "--branch",
      process.env.HS_REF ?? "main",
      "https://github.com/juspay/hyperswitch.git",
      checkout,
    ]);
  }
  const config = resolve(runtime, "hs.toml");
  let contents = readFileSync(
    resolve(checkout, "config/docker_compose.toml"),
    "utf8",
  );
  // Support both existing and absent UCS sections without duplicate TOML tables.
  contents = contents.replace(
    /^\[grpc_client\.unified_connector_service\][\s\S]*?(?=^\[|$(?![\s\S]))/m,
    "",
  );
  contents +=
    "\n[grpc_client.unified_connector_service]\n" +
    `base_url = ${JSON.stringify(process.env.UCS_BASE_URL ?? "http://host.docker.internal:8000")}\n` +
    "connection_timeout = 10\nrequest_timeout = 60\n" +
    'ucs_only_connectors = "adyen,stripe,paypal,authorizedotnet,cybersource,checkout"\n';
  writeFileSync(config, contents);
  writeFileSync(
    override,
    JSON.stringify(
      {
        services: {
          "hyperswitch-server": {
            image:
              process.env.HS_IMAGE ??
              "docker.juspay.io/juspaydotin/hyperswitch-router:latest",
            pull_policy: "always",
            command: ["/local/bin/router", "-f", "/local/config/e2e.toml"],
            volumes: [`${config}:/local/config/e2e.toml:ro`],
            extra_hosts: ["host.docker.internal:host-gateway"],
            healthcheck: { start_period: "60s", retries: 30 },
          },
        },
      },
      null,
      2,
    ),
  );
  writeFileSync(
    resolve(runtime, "versions.json"),
    JSON.stringify(
      {
        image:
          process.env.HS_IMAGE ??
          "docker.juspay.io/juspaydotin/hyperswitch-router:latest",
        ucs: process.env.UCS_BASE_URL ?? "http://host.docker.internal:8000",
        hsCommit: spawnSync("git", ["-C", checkout, "rev-parse", "HEAD"], {
          encoding: "utf8",
        }).stdout.trim(),
      },
      null,
      2,
    ),
  );
} else if (!existsSync(override)) {
  throw new Error("No E2E stack has been prepared");
}
if (action === "prepare") {
  console.log(`Prepared ${override}`);
  process.exit(0);
}
const args = [
  "compose",
  "--project-name",
  "ucs-e2e",
  "--project-directory",
  checkout,
  "-f",
  resolve(checkout, "docker-compose.yml"),
  "-f",
  override,
];
// Compose starts the router and its declared dependencies only (no dashboard/SDK yet).
run("docker", [
  ...args,
  ...(action === "up"
    ? ["up", "-d", "--wait", "--wait-timeout", "600", "hyperswitch-server"]
    : ["down"]),
]);
