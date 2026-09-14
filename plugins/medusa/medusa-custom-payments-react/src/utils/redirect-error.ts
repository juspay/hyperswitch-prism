/**
 * Next.js signals `redirect()` and `notFound()` by THROWING a control-flow
 * error rather than returning. In Server Actions these surface client-side with
 * a `digest` (and sometimes a `message`) of the form "NEXT_REDIRECT;..." or
 * "NEXT_NOT_FOUND". They are not real errors — they must be re-thrown so Next
 * can complete the navigation, instead of being swallowed by a `try/catch` and
 * rendered as a payment error.
 */
export function isNextControlFlowError(err: unknown): boolean {
  const candidate = err as { digest?: unknown; message?: unknown } | null;
  const probe = [candidate?.digest, candidate?.message].find(
    (value): value is string => typeof value === "string"
  );
  return (
    !!probe && (probe.startsWith("NEXT_REDIRECT") || probe === "NEXT_NOT_FOUND")
  );
}
