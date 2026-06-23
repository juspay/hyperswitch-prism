import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { doctorShape } from "../schemas.js";
import { sdkInstalled, probeNative, tryNativeInit } from "../services/prism.js";
import { SDK_PACKAGE, CONFIG_ENV_VAR } from "../constants.js";
import { result } from "../util.js";

interface Check {
  name: string;
  ok: boolean;
  detail: string;
  fix?: string;
}

export function registerDoctor(server: McpServer): void {
  server.registerTool(
    "prism_doctor",
    {
      title: "Prism environment doctor",
      description:
        "Diagnose the local environment for running hyperswitch-prism: Node version, OS/arch, whether the SDK is " +
        "installed & resolvable, whether the shipped native FFI binary matches this platform/arch (reads the actual " +
        "ELF/Mach-O architecture from the binary), whether it loads, and whether PRISM_CONNECTOR_CONFIG is set. " +
        "Returns a checklist with concrete fixes — notably warns when running on linux-arm64 (no binary ships).",
      inputSchema: doctorShape,
      annotations: { readOnlyHint: true, openWorldHint: false },
    },
    async () => {
      const checks: Check[] = [];

      // Node version.
      const nodeMajor = Number(process.versions.node.split(".")[0]);
      checks.push({
        name: "Node.js >= 18",
        ok: nodeMajor >= 18,
        detail: `Node ${process.versions.node}`,
        fix: nodeMajor >= 18 ? undefined : "Upgrade to Node 18+.",
      });

      // SDK installed.
      const sdk = sdkInstalled();
      checks.push({
        name: `${SDK_PACKAGE} installed`,
        ok: sdk.installed,
        detail: sdk.installed ? `v${sdk.version} at ${sdk.path}` : "not found",
        fix: sdk.installed ? undefined : `Run: npm install ${SDK_PACKAGE}`,
      });

      // Native binary probe.
      const probe = probeNative();
      checks.push({
        name: "Native FFI binary supports this platform/arch",
        ok: probe.willLoad,
        detail:
          `platform=${probe.platform} arch=${probe.arch}; ` +
          `lib=${probe.expectedLibFile ?? "(none)"} exists=${probe.libExists} libArch=${probe.libArch} ` +
          `archMatch=${probe.archMatch}`,
        fix: probe.willLoad ? undefined : probe.reason,
      });

      // Actually load the native FFI lib (construct a PaymentClient — no network).
      // This catches loader failures that a plain import() misses (e.g. old glibc).
      let nativeInit: { ok: boolean; error: string | null; code: string | null } = {
        ok: false,
        error: null,
        code: null,
      };
      if (probe.supportedPlatform && probe.libExists) {
        nativeInit = await tryNativeInit();
        const fixFor = (code: string | null): string => {
          if (code === "GLIBC_TOO_OLD")
            return "The shipped linux .so is built against a newer glibc than this host. Run on a newer base image (e.g. Debian 12 / Ubuntu 24.04) or a glibc >= the required version.";
          if (code === "ARCH_MISMATCH") return "The native binary's architecture does not match this CPU.";
          return "Native library failed to load. Reinstall the package and verify platform compatibility.";
        };
        checks.push({
          name: "Native FFI library actually loads (constructs a client)",
          ok: nativeInit.ok,
          detail: nativeInit.ok ? "PaymentClient constructed; native lib loaded" : `load failed: ${nativeInit.error}`,
          fix: nativeInit.ok ? undefined : fixFor(nativeInit.code),
        });
      }

      // Config env presence (informational — not a hard failure).
      const hasConfig = Boolean(process.env[CONFIG_ENV_VAR]);
      checks.push({
        name: `${CONFIG_ENV_VAR} set (needed only for prism_test_charge)`,
        ok: hasConfig,
        detail: hasConfig ? "present" : "not set",
        fix: hasConfig ? undefined : `Set ${CONFIG_ENV_VAR} to your sandbox connectorConfig JSON before running a test charge.`,
      });

      const blocking = checks.filter((c) => !c.ok && c.name !== `${CONFIG_ENV_VAR} set (needed only for prism_test_charge)`);
      const allGood = blocking.length === 0;

      const lines = checks.map((c) => `${c.ok ? "✅" : "❌"} ${c.name}\n    ${c.detail}${c.fix ? `\n    fix: ${c.fix}` : ""}`);
      const text =
        `# Prism doctor\n${lines.join("\n")}\n\n` +
        (allGood
          ? "All systems go — you can run a sandbox test charge."
          : `${blocking.length} blocking issue(s). Resolve the ❌ items above.`) +
        (probe.platform === "linux" && probe.arch === "arm64"
          ? "\n\n⚠️ linux-arm64 detected: hyperswitch-prism does not ship an ARM-Linux native binary, so it cannot run here. Use linux-x64 or macOS."
          : "");

      return result(text, { ok: true, healthy: allGood, checks, native: probe, nativeInit, sdk });
    },
  );
}
