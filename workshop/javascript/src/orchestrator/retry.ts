// Payment retry / fallback (orchestrator use-case B)
// ─────────────────────────────────────────────────────────────────────────────
// Try a payment across an ordered list of PSPs, stopping at the first success.
// This is the classic "if Stripe declines, try Adyen" pattern.
//
// `withRetry` is kept pure and processor-agnostic by accepting an `attempt`
// callback: `(psp) => Promise<AttemptOutcome>`. The real demo passes in our
// `authorize()` wrapper; the tests pass in a fake. That separation is what makes
// the retry logic robust AND unit-testable without a network.
// ─────────────────────────────────────────────────────────────────────────────

import type { PspName } from '../../config/psp-registry.js';

// The minimum shape `withRetry` needs from each attempt's result.
export interface AttemptOutcome {
  ok: boolean;
  statusText: string;
  error?: string;
}

export interface RetryOptions<T extends AttemptOutcome> {
  // PSPs to try, in priority order. First success wins.
  plan: PspName[];
  // How to attempt a payment with a given PSP.
  attempt: (psp: PspName) => Promise<T>;
  // Optional cap on attempts (defaults to the full plan length).
  maxAttempts?: number;
  // Optional hook for logging each attempt (used by the demo).
  onAttempt?: (psp: PspName, index: number, outcome: T) => void;
}

export interface RetryResult<T extends AttemptOutcome> {
  succeeded: boolean;
  finalResult: T | null;
  winningPsp: PspName | null;
  attempts: Array<{ psp: PspName; outcome: T }>;
}

export async function withRetry<T extends AttemptOutcome>(
  opts: RetryOptions<T>,
): Promise<RetryResult<T>> {
  const limit = Math.min(opts.maxAttempts ?? opts.plan.length, opts.plan.length);
  const attempts: Array<{ psp: PspName; outcome: T }> = [];

  for (let i = 0; i < limit; i++) {
    const psp = opts.plan[i];
    const outcome = await opts.attempt(psp);
    attempts.push({ psp, outcome });
    opts.onAttempt?.(psp, i, outcome);

    if (outcome.ok) {
      return { succeeded: true, finalResult: outcome, winningPsp: psp, attempts };
    }
  }

  const last = attempts.length > 0 ? attempts[attempts.length - 1] : null;
  return {
    succeeded: false,
    finalResult: last ? last.outcome : null,
    winningPsp: null,
    attempts,
  };
}

// Build a de-duplicated retry plan that tries `primary` first, then the rest.
// Handy when routing picks the primary PSP and you want the others as fallbacks.
export function buildPlan(primary: PspName, fallbacks: PspName[]): PspName[] {
  const seen = new Set<PspName>();
  const plan: PspName[] = [];
  for (const psp of [primary, ...fallbacks]) {
    if (!seen.has(psp)) {
      seen.add(psp);
      plan.push(psp);
    }
  }
  return plan;
}
