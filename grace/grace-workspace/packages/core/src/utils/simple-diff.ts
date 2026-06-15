/**
 * Minimal unified-diff generator. Produces a string with +/- lines
 * suitable for rendering a GitHub-style diff view. No external deps.
 *
 * Uses a basic LCS (longest common subsequence) approach — good enough
 * for files up to a few hundred lines. Not optimized for huge files.
 */

export function unifiedDiff(
  oldText: string,
  newText: string,
  oldLabel = "a",
  newLabel = "b"
): string {
  const oldLines = oldText.split("\n");
  const newLines = newText.split("\n");

  // LCS table
  const m = oldLines.length;
  const n = newLines.length;
  const dp: number[][] = Array.from({ length: m + 1 }, () =>
    Array(n + 1).fill(0)
  );
  for (let i = 1; i <= m; i++) {
    for (let j = 1; j <= n; j++) {
      if (oldLines[i - 1] === newLines[j - 1]) {
        dp[i]![j] = dp[i - 1]![j - 1]! + 1;
      } else {
        dp[i]![j] = Math.max(dp[i - 1]![j]!, dp[i]![j - 1]!);
      }
    }
  }

  // Backtrack to produce diff lines
  const lines: string[] = [];
  let i = m;
  let j = n;
  const stack: string[] = [];
  while (i > 0 || j > 0) {
    if (i > 0 && j > 0 && oldLines[i - 1] === newLines[j - 1]) {
      stack.push(` ${oldLines[i - 1]}`);
      i--;
      j--;
    } else if (j > 0 && (i === 0 || dp[i]![j - 1]! >= dp[i - 1]![j]!)) {
      stack.push(`+${newLines[j - 1]}`);
      j--;
    } else {
      stack.push(`-${oldLines[i - 1]}`);
      i--;
    }
  }
  stack.reverse();
  lines.push(...stack);

  // Build unified header
  const header = [
    `--- ${oldLabel}`,
    `+++ ${newLabel}`,
  ];

  return [...header, ...lines].join("\n");
}
