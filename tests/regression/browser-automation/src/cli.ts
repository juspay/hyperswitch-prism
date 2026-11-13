import { AutomationEngine } from "./engine/automationEngine";
import { PlaywrightDriverFactory } from "./drivers/playwrightDriver";
import { MAX_INPUT_BYTES, parseRunRequest } from "./utils/validation";

/** Internal one-job entry point. The harness runs this inside the worker container. */
async function main(): Promise<void> {
  if (process.argv.length > 2) throw new Error("Supply one JSON job through stdin; file/output arguments are unsupported");
  const factory = new PlaywrightDriverFactory();
  const engine = new AutomationEngine(factory);
  const timeoutMs = Number(process.env.WORKER_TIMEOUT_MS ?? 90000);
  if (!Number.isInteger(timeoutMs) || timeoutMs < 1000 || timeoutMs > 300000) {
    throw new Error("Invalid worker deadline");
  }
  let stopping = false;
  const stop = async (code: number): Promise<void> => {
    if (stopping) return;
    stopping = true;
    // The container supervisor also removes the entire job on timeout/cancellation.
    const hardStop = setTimeout(() => process.exit(code), 2000);
    hardStop.unref();
    await engine.cleanup().catch(() => undefined);
    process.exit(code);
  };
  const deadline = setTimeout(() => { void stop(124); }, timeoutMs);
  const onSignal = (): void => { void stop(130); };
  process.once("SIGTERM", onSignal);
  process.once("SIGINT", onSignal);
  try {
    const chunks: Buffer[] = [];
    let size = 0;
    for await (const chunk of process.stdin) {
      const buffer = Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk);
      size += buffer.length;
      if (size > MAX_INPUT_BYTES) throw new Error("Browser job exceeds the input limit");
      chunks.push(buffer);
    }
    let body: unknown;
    try { body = JSON.parse(Buffer.concat(chunks).toString("utf8")); }
    catch { throw new Error("Expected one JSON job on stdin"); }
    const result = await engine.run(parseRunRequest(body));
    const output = JSON.stringify(result);
    if (Buffer.byteLength(output) > MAX_INPUT_BYTES) throw new Error("Browser result exceeds the output limit");
    process.stdout.write(output + "\n");
    if (!result.success) process.exitCode = 1;
  } finally {
    clearTimeout(deadline);
    process.off("SIGTERM", onSignal);
    process.off("SIGINT", onSignal);
    await engine.cleanup();
  }
}

main().catch(() => {
  // Never echo raw payloads, URLs, page text or browser exception messages into CI logs.
  process.stderr.write("Browser worker rejected the job or could not complete it.\n");
  process.exitCode = 1;
});
