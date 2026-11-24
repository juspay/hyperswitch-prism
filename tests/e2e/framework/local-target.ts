import { isAbsolute } from "node:path";

const loopbackHosts = new Set(["localhost", "127.0.0.1", "::1", "[::1]"]);

export type LocalTargets = {
  hyperswitchBaseUrl: string;
  databaseUrl: string;
  connectorAuthFilePath: string;
};

/**
 * Resolve and validate the external services used by the local-only runner.
 * The ordinary `npm test` and Jenkins paths do not call this guard.
 */
export function localTargets(env: NodeJS.ProcessEnv): LocalTargets {
  if (
    isEnabled(env.CI) ||
    [
      env.JENKINS_URL,
      env.JENKINS_HOME,
      env.JENKINS_NODE_COOKIE,
      env.BUILD_TAG,
    ].some(isEnabled)
  ) {
    throw new Error("test:local is disabled in CI");
  }
  if (env.UCS_ENVIRONMENT?.toLowerCase() === "production") {
    throw new Error("test:local refuses UCS_ENVIRONMENT=production");
  }

  const hyperswitchBaseUrl =
    env.HS_BASE_URL ?? `http://127.0.0.1:${env.HS_PORT ?? "8080"}`;
  const databaseUrl =
    env.HS_DATABASE_URL ??
    `postgresql://db_user:db_pass@127.0.0.1:${env.HS_DB_PORT ?? "15432"}/hyperswitch_db`;
  const connectorAuthFilePath = env.CONNECTOR_AUTH_FILE_PATH;

  assertLoopbackUrl(hyperswitchBaseUrl, new Set(["http:", "https:"]), "HS");
  assertLoopbackUrl(
    databaseUrl,
    new Set(["postgres:", "postgresql:"]),
    "HS database",
  );
  if (!connectorAuthFilePath) {
    throw new Error("test:local requires CONNECTOR_AUTH_FILE_PATH");
  }
  if (!isAbsolute(connectorAuthFilePath)) {
    throw new Error("CONNECTOR_AUTH_FILE_PATH must be an absolute path");
  }

  return { hyperswitchBaseUrl, databaseUrl, connectorAuthFilePath };
}

function assertLoopbackUrl(
  raw: string,
  protocols: ReadonlySet<string>,
  label: string,
): void {
  let url: URL;
  try {
    url = new URL(raw);
  } catch {
    throw new Error(`${label} URL is invalid`);
  }
  if (!protocols.has(url.protocol)) {
    throw new Error(`${label} URL uses an unsupported protocol`);
  }
  if (!loopbackHosts.has(url.hostname)) {
    throw new Error(`${label} must use localhost or a loopback address`);
  }
}

function isEnabled(value: string | undefined): boolean {
  return (
    value !== undefined &&
    !new Set(["", "0", "false"]).has(value.toLowerCase())
  );
}
