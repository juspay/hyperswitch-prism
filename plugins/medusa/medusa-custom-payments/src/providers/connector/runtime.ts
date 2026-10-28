/** Fail closed before Node.js decodes an HTTP response with vulnerable Brotli. */
const MINIMUM_PATCH: Record<number, [number, number]> = {
  22: [23, 4],
  24: [22, 1],
  26: [12, 1],
}

export function assertPatchedNodeRuntime(version = process.versions.node): void {
  const match = /^(\d+)\.(\d+)\.(\d+)$/.exec(version)
  const major = match ? Number(match[1]) : NaN
  const minor = match ? Number(match[2]) : NaN
  const patch = match ? Number(match[3]) : NaN
  const minimum = MINIMUM_PATCH[major]

  if (minimum && (minor > minimum[0] || (minor === minimum[0] && patch >= minimum[1]))) {
    return
  }

  throw new Error(
    `Node.js ${version} is not supported for HTTP requests: its bundled Brotli decoder may be vulnerable to CVE-2026-111497. ` +
      "Use Node.js 22.23.4+, 24.22.1+, or 26.12.1+ on the corresponding release line."
  )
}
