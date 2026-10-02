/**
 * Locates the native libraries bundled with this package.
 *
 * Natives are staged under generated/<platform>-<arch>/ (matching Node's
 * process.platform + process.arch — e.g. linux-x64, linux-arm64, darwin-arm64)
 * so a single package serves every supported runtime. Selecting on platform
 * alone picks the x86-64 binary on aarch64 hosts, so both parts are needed.
 */

import fs from "fs";
import path from "path";

/** Shared library extension per platform; anything else follows ELF naming. */
const LIB_EXTENSION: Record<string, string | undefined> = {
  darwin: "dylib",
  win32: "dll",
};

/**
 * Absolute path to `libName` for the platform and architecture we are running on.
 *
 * When nothing matches, reports the target we looked for and the ones this
 * package does carry — otherwise the failure surfaces from dlopen as a "wrong
 * ELF class" message that never names the architecture it expected.
 */
export function resolveNativeLib(generatedDir: string, libName: string): string {
  const target = `${process.platform}-${process.arch}`;
  const ext = LIB_EXTENSION[process.platform] ?? "so";
  const libPath = path.join(generatedDir, target, `${libName}.${ext}`);
  if (fs.existsSync(libPath)) return libPath;

  let bundled: string[] = [];
  try {
    bundled = fs
      .readdirSync(generatedDir, { withFileTypes: true })
      .filter((entry) => entry.isDirectory())
      .map((entry) => entry.name)
      .sort();
  } catch {
    // generated/ absent entirely; reported as "bundles no native libraries" below.
  }

  throw new Error(
    `hyperswitch-prism: ${libName} is not bundled for ${target} (expected ${libPath}). ` +
      (bundled.length > 0
        ? `This package bundles: ${bundled.join(", ")}.`
        : "This package bundles no native libraries.")
  );
}
