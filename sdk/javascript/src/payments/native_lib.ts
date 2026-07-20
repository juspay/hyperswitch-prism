/**
 * Resolves the path to a bundled native FFI library for the current runtime.
 *
 * Native binaries are selected by BOTH platform and architecture. They are
 * bundled with a `${process.platform}-${process.arch}` suffix so that a single
 * package can carry libraries for several architectures without the loader
 * accidentally picking a binary built for the wrong CPU — e.g. loading the
 * x86-64 `.so` on arm64 Linux (AWS Graviton, Ampere, arm64 CI runners), which
 * fails with a cryptic ELF-architecture error.
 *
 * Naming scheme (preferred):
 *   libconnector_service_ffi-linux-x64.so       (Linux x86-64)
 *   libconnector_service_ffi-linux-arm64.so     (Linux aarch64)
 *   libconnector_service_ffi-darwin-arm64.dylib (macOS Apple Silicon)
 *
 * For backward compatibility with packages that shipped a single unsuffixed
 * binary per OS (`.so` = Linux x86-64, `.dylib` = macOS arm64), the legacy flat
 * name is tried as a fallback — but only on the platform/arch it was historically
 * built for, so a mismatched-arch legacy binary is never loaded.
 */
import fs from "fs";
import path from "path";

/**
 * Returns the absolute path to the native library named `baseName` (without
 * extension) inside `dir`, choosing the binary that matches the current
 * platform and architecture. Throws a descriptive error if none is bundled.
 */
export function resolveNativeLibPath(dir: string, baseName: string): string {
  const platform = process.platform;
  const arch = process.arch;
  const ext = platform === "darwin" ? "dylib" : "so";

  // Preferred: architecture-aware filename.
  const candidates: string[] = [
    path.join(dir, `${baseName}-${platform}-${arch}.${ext}`),
  ];

  // Legacy fallback: historically the package shipped one unsuffixed binary per
  // OS. Only trust it on the arch it was actually built for so we never load a
  // wrong-architecture library.
  const legacyMatchesArch =
    (platform === "linux" && arch === "x64") ||
    (platform === "darwin" && arch === "arm64");
  if (legacyMatchesArch) {
    candidates.push(path.join(dir, `${baseName}.${ext}`));
  }

  for (const candidate of candidates) {
    if (fs.existsSync(candidate)) return candidate;
  }

  throw new Error(
    `hyperswitch-prism: no native '${baseName}' library bundled for ` +
      `${platform}-${arch}. Looked for:\n` +
      candidates.map((c) => `  - ${c}`).join("\n") +
      `\nSupported platforms: linux-x64, linux-arm64, darwin-arm64. ` +
      `If you are on an unsupported platform, please open an issue at ` +
      `https://github.com/juspay/hyperswitch-prism/issues`
  );
}
