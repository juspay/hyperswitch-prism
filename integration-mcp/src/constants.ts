/** Shared constants: package identity, reference URLs, env var names, defaults. */

export const SERVER_NAME = "hyperswitch-prism-integration";
export const SERVER_VERSION = "0.1.0";
export const SDK_PACKAGE = "hyperswitch-prism";

/** Env var holding the JSON connector config for prism_test_charge. */
export const CONFIG_ENV_VAR = "PRISM_CONNECTOR_CONFIG";

/** Official LLM reference, fetched at runtime by resources and cached. */
export const LLM_TXT_URL =
  "https://raw.githubusercontent.com/juspay/hyperswitch-prism/main/llm/llm.txt";
export const LLM_TXT_GENERATED_URL =
  "https://raw.githubusercontent.com/juspay/hyperswitch-prism/main/docs-generated/llms.txt";
export const REPO_URL = "https://github.com/juspay/hyperswitch-prism";

/** Native FFI binaries actually shipped by the SDK (platform-arch keys). */
export const SUPPORTED_NATIVE_TARGETS = [
  "linux-x64",
  "darwin-arm64",
  "darwin-x64",
] as const;

export const SUPPORTED_FRAMEWORKS = ["express", "next", "nestjs", "fastify", "node"] as const;
export type Framework = (typeof SUPPORTED_FRAMEWORKS)[number];

export const SUPPORTED_FLOWS = ["authorize", "capture", "void", "refund", "sync", "webhook"] as const;
export type Flow = (typeof SUPPORTED_FLOWS)[number];

export const SUPPORTED_LANGUAGES = ["ts", "js"] as const;
export type Language = (typeof SUPPORTED_LANGUAGES)[number];

export const CAPTURE_METHODS = ["AUTOMATIC", "MANUAL"] as const;
export type CaptureMethodName = (typeof CAPTURE_METHODS)[number];

/** Resource URIs. */
export const RESOURCE_URIS = {
  guide: "prism://guide",
  statusTable: "prism://status-table",
  llmTxt: "prism://llm-txt",
} as const;
