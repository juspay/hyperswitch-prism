/**
 * Runtime access to the hyperswitch-prism SDK and host environment.
 *
 * The SDK (and its native FFI binary) is imported LAZILY so the data-only tools
 * work even when the SDK is absent or the platform is unsupported. This module also
 * provides native-binary probing (reading the actual ELF/Mach-O arch from the shipped
 * library), error normalization, and a cached llm.txt fetch.
 */
import { createRequire } from "node:module";
import { readFileSync, existsSync, openSync, readSync, closeSync } from "node:fs";
import { dirname, join } from "node:path";
import { LLM_TXT_URL } from "../constants.js";
import { unknownToMessage } from "../util.js";

const require = createRequire(import.meta.url);

// ---------------------------------------------------------------------------
// SDK module loading (lazy + cached)
// ---------------------------------------------------------------------------
let sdkModulePromise: Promise<unknown> | null = null;

export async function loadSdk(): Promise<Record<string, unknown>> {
  if (!sdkModulePromise) {
    sdkModulePromise = import("hyperswitch-prism");
  }
  return (await sdkModulePromise) as Record<string, unknown>;
}

export function sdkInstalled(): { installed: boolean; version: string | null; path: string | null } {
  try {
    const pkgPath = require.resolve("hyperswitch-prism/package.json");
    const pkg = JSON.parse(readFileSync(pkgPath, "utf8")) as { version?: string };
    return { installed: true, version: pkg.version ?? null, path: dirname(pkgPath) };
  } catch {
    return { installed: false, version: null, path: null };
  }
}

// ---------------------------------------------------------------------------
// Native binary probing
// ---------------------------------------------------------------------------
export type ArchName = "x64" | "arm64" | "unknown";

function archFromBinary(path: string): ArchName {
  let fd: number | null = null;
  try {
    fd = openSync(path, "r");
    const buf = Buffer.alloc(20);
    readSync(fd, buf, 0, 20, 0);
    // ELF: 0x7F 'E' 'L' 'F'
    if (buf[0] === 0x7f && buf[1] === 0x45 && buf[2] === 0x4c && buf[3] === 0x46) {
      const eMachine = buf.readUInt16LE(18);
      if (eMachine === 0x3e) return "x64"; // x86-64
      if (eMachine === 0xb7) return "arm64"; // aarch64
      return "unknown";
    }
    // Mach-O 64-bit: magic FEEDFACF (LE on disk -> CF FA ED FE)
    const magic = buf.readUInt32BE(0);
    if (magic === 0xfeedfacf || magic === 0xcffaedfe || magic === 0xcafebabe || magic === 0xbebafeca) {
      // cputype at offset 4 (little-endian for thin); arm64 = 0x0100000C, x86_64 = 0x01000007
      const cpu = buf.readUInt32LE(4);
      if (cpu === 0x0100000c) return "arm64";
      if (cpu === 0x01000007) return "x64";
      // Fat binary — can't tell a single arch cheaply.
      return "unknown";
    }
    return "unknown";
  } catch {
    return "unknown";
  } finally {
    if (fd !== null) closeSync(fd);
  }
}

export interface NativeProbe {
  platform: NodeJS.Platform;
  arch: string;
  expectedLibFile: string | null;
  libPath: string | null;
  libExists: boolean;
  libArch: ArchName;
  archMatch: boolean | null;
  supportedPlatform: boolean;
  willLoad: boolean;
  reason: string;
}

export function probeNative(): NativeProbe {
  const platform = process.platform;
  const arch = process.arch;
  const sdk = sdkInstalled();

  let expectedLibFile: string | null = null;
  if (platform === "linux") expectedLibFile = "libconnector_service_ffi.so";
  else if (platform === "darwin") expectedLibFile = "libconnector_service_ffi.dylib";
  // win32 / others: no shipped binary.

  const supportedPlatform = expectedLibFile !== null;

  let libPath: string | null = null;
  let libExists = false;
  let libArch: ArchName = "unknown";

  if (sdk.path && expectedLibFile) {
    const candidate = join(sdk.path, "dist", "src", "payments", "generated", expectedLibFile);
    if (existsSync(candidate)) {
      libPath = candidate;
      libExists = true;
      libArch = archFromBinary(candidate);
    }
  }

  const hostArch: ArchName = arch === "x64" ? "x64" : arch === "arm64" ? "arm64" : "unknown";
  const archMatch = libExists && libArch !== "unknown" && hostArch !== "unknown" ? libArch === hostArch : null;

  let willLoad = false;
  let reason = "";
  if (!sdk.installed) {
    reason = "hyperswitch-prism is not installed. Run: npm install hyperswitch-prism";
  } else if (!supportedPlatform) {
    reason = `No native binary ships for platform '${platform}'. Supported: linux (x64) and macOS (arm64/x64).`;
  } else if (!libExists) {
    reason = `Expected native lib '${expectedLibFile}' was not found in the installed package.`;
  } else if (archMatch === false) {
    reason =
      `Native lib architecture (${libArch}) does not match host (${hostArch}). ` +
      (platform === "linux" && hostArch === "arm64"
        ? "There is no linux-arm64 binary — Prism cannot run on ARM Linux (e.g. Graviton / some CI runners)."
        : "Install on a matching architecture.");
  } else {
    willLoad = true;
    reason = `Native lib present (${libArch}) and matches host (${hostArch}).`;
  }

  return {
    platform,
    arch,
    expectedLibFile,
    libPath,
    libExists,
    libArch,
    archMatch,
    supportedPlatform,
    willLoad,
    reason,
  };
}

/**
 * Actually load the native FFI library by constructing a PaymentClient with a
 * throwaway config. This is the only reliable way to detect loader failures
 * (wrong arch, missing/old glibc, corrupt binary) — importing the module alone
 * does NOT load the .so/.dylib. No network is performed by construction.
 */
export async function tryNativeInit(): Promise<{ ok: boolean; error: string | null; code: string | null }> {
  try {
    const sdk = await loadSdk();
    const PaymentClient = sdk.PaymentClient as new (cfg: unknown) => unknown;
    new PaymentClient({ connectorConfig: { stripe: { apiKey: { value: "diagnostic_dummy" } } } });
    return { ok: true, error: null, code: null };
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    let code: string | null = null;
    if (/GLIBC|version `?GLIBC/i.test(msg)) code = "GLIBC_TOO_OLD";
    else if (/shared library|cannot open shared object|dlopen|\.so|\.dylib/i.test(msg)) code = "NATIVE_LIB_LOAD_FAILED";
    else if (/wrong ELF|Exec format|architecture/i.test(msg)) code = "ARCH_MISMATCH";
    return { ok: false, error: msg, code };
  }
}

// ---------------------------------------------------------------------------
// Error normalization
// ---------------------------------------------------------------------------
export interface NormalizedError {
  errorClass: "IntegrationError" | "ConnectorError" | "NetworkError" | "Unknown";
  code: string | null;
  message: string;
  retriable: boolean;
}

export function normalizeError(err: unknown): NormalizedError {
  const name = err instanceof Error ? err.constructor?.name ?? err.name : "Unknown";
  const message = unknownToMessage(err);
  const codeFromErr =
    err && typeof err === "object" && "errorCode" in err
      ? String((err as { errorCode?: unknown }).errorCode ?? "")
      : null;

  if (name === "NetworkError") {
    return { errorClass: "NetworkError", code: codeFromErr || null, message, retriable: true };
  }
  if (name === "ConnectorError") {
    return { errorClass: "ConnectorError", code: codeFromErr || null, message, retriable: false };
  }
  if (name === "IntegrationError") {
    return { errorClass: "IntegrationError", code: codeFromErr || null, message, retriable: false };
  }
  return { errorClass: "Unknown", code: codeFromErr || null, message, retriable: false };
}

// ---------------------------------------------------------------------------
// llm.txt fetch + cache
// ---------------------------------------------------------------------------
interface LlmCache {
  text: string;
  fetchedAt: number;
}
let llmCache: LlmCache | null = null;
const LLM_TTL_MS = 60 * 60 * 1000; // 1 hour

export async function fetchLlmTxt(force = false): Promise<{ text: string; cached: boolean; url: string }> {
  const now = Date.now();
  if (!force && llmCache && now - llmCache.fetchedAt < LLM_TTL_MS) {
    return { text: llmCache.text, cached: true, url: LLM_TXT_URL };
  }
  const res = await fetch(LLM_TXT_URL);
  if (!res.ok) {
    throw new Error(`Failed to fetch llm.txt: HTTP ${res.status}`);
  }
  const text = await res.text();
  llmCache = { text, fetchedAt: now };
  return { text, cached: false, url: LLM_TXT_URL };
}
