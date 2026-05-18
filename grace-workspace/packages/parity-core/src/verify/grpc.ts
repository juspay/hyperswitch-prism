import { execa, type ExecaError, type ResultPromise } from "execa";
import { createWriteStream } from "node:fs";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import type { ParityConfig } from "../config.js";
import type { Leaf, LocusTarget, VerifyResult } from "../types.js";
import { diffField, extractPayload, type PayloadSpec } from "./payloads.js";

interface ServerHandle {
  proc: ResultPromise;
  logPath: string;
  port: number;
}

async function waitForListening(logPath: string, timeoutMs: number): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try {
      const buf = await readFile(logPath, "utf8");
      if (/grpc server listening|listening on|server starting/i.test(buf)) return;
    } catch {
      // log may not exist yet
    }
    await delay(500);
  }
  throw new Error(`gRPC server did not report listening within ${timeoutMs}ms; see ${logPath}`);
}

export async function bootGrpcServer(cfg: ParityConfig, leafNumber: number): Promise<ServerHandle> {
  await mkdir(cfg.cache.dir, { recursive: true });
  const logPath = join(cfg.cache.dir, `grpc-server-${leafNumber}.log`);
  await writeFile(logPath, ""); // truncate

  const env = { ...process.env, CONNECTOR_AUTH_FILE_PATH: cfg.credsPath || process.env.CONNECTOR_AUTH_FILE_PATH || "" };
  const proc = execa("cargo", ["run", "-p", "grpc-server", "--release"], {
    cwd: cfg.prismPath,
    env,
    reject: false,
  });
  proc.stdout?.pipe(createWriteStream(logPath, { flags: "a" }));
  proc.stderr?.pipe(createWriteStream(logPath, { flags: "a" }));

  try {
    await waitForListening(logPath, cfg.grpc.bootTimeoutMs);
  } catch (e) {
    proc.kill("SIGTERM");
    throw e;
  }
  return { proc, logPath, port: cfg.grpc.port };
}

export async function teardown(h: ServerHandle): Promise<void> {
  h.proc.kill("SIGTERM");
  try {
    await Promise.race([h.proc, delay(5000)]);
  } catch {
    // ignore
  }
}

async function grpcurlCall(cfg: ParityConfig, h: ServerHandle, spec: PayloadSpec): Promise<{ stdout: string; stderr: string; exitCode: number }> {
  const payloadFile = join(tmpdir(), `parity-payload-${process.pid}-${Date.now()}.json`);
  await writeFile(payloadFile, JSON.stringify(spec.payload));
  const protoImport = join(cfg.prismPath, "crates/types-traits/grpc-api-types/proto");
  try {
    const r = await execa(
      "grpcurl",
      [
        "-plaintext",
        "-d", `@${payloadFile}`,
        "-import-path", protoImport,
        "-proto", spec.protoFile,
        "-max-time", String(Math.max(1, Math.floor(cfg.grpc.callTimeoutMs / 1000))),
        `localhost:${h.port}`,
        `${spec.service}/${spec.rpc}`,
      ],
      { reject: false },
    );
    return { stdout: r.stdout ?? "", stderr: r.stderr ?? "", exitCode: r.exitCode ?? 0 };
  } catch (err) {
    const e = err as ExecaError;
    return { stdout: e.stdout?.toString() ?? "", stderr: e.stderr?.toString() ?? "", exitCode: e.exitCode ?? 1 };
  }
}

function renderResult(opts: {
  spec: PayloadSpec;
  result: "PASS" | "FAIL";
  reason?: string;
  observedRaw: string;
  diffs: { path: string; match: boolean; expected: unknown; observed: unknown }[];
  logTail: string;
  prefixValue?: string;
  postfixValue?: string;
}): string {
  const { spec, result, reason, diffs, logTail, prefixValue, postfixValue } = opts;
  const lines: string[] = [];
  lines.push("## gRPC Verification");
  lines.push("");
  lines.push("### Replay");
  lines.push(`- Payload source: \`${spec.source}${spec.fixturePath ? ` (${spec.fixturePath})` : ""}\``);
  lines.push(`- Endpoint: \`localhost:<port>\` \`${spec.service}/${spec.rpc}\``);
  lines.push(`- Result: ${result}${reason ? ` — ${reason}` : ""}`);
  lines.push("");
  lines.push("### Targeted field diff");
  lines.push("| Field path | Oracle expected | Prism observed (post-fix) | Match |");
  lines.push("|------------|-----------------|---------------------------|-------|");
  for (const d of diffs) {
    lines.push(`| \`${d.path}\` | \`${JSON.stringify(d.expected)}\` | \`${JSON.stringify(d.observed)}\` | ${d.match ? "✅" : "❌"} |`);
  }
  lines.push("");
  lines.push("### Log evidence (post-fix code path executed)");
  lines.push("```");
  lines.push(logTail.slice(-4000));
  lines.push("```");
  if (prefixValue || postfixValue) {
    lines.push("");
    lines.push("### Pre-fix vs post-fix");
    if (prefixValue) lines.push(`- Pre-fix value (from leaf body): \`${prefixValue}\``);
    if (postfixValue) lines.push(`- Post-fix value: \`${postfixValue}\``);
  }
  return lines.join("\n");
}

export async function verifyLeaf(cfg: ParityConfig, leaf: Leaf, target: LocusTarget = "prism"): Promise<VerifyResult> {
  // hs-bridge fixes live in hyperswitch, not prism. Booting prism's grpc-server
  // would test the unchanged downstream transformer, not the bridge fix. Also
  // the `parity:<flow>:<field> hit` tracing marker the agent adds lives in
  // hyperswitch's binary, so prism's log can't possibly grep-match it.
  //
  // Instead we let the EXECUTE phase's `cargo nextest run -p external_services`
  // BE the verify gate — the EXECUTE prompt requires the agent to add a unit
  // test that exercises the patched serializer.
  if (target === "hyperswitch-bridge") {
    return {
      ok: true,
      markdown: [
        "## gRPC Verification",
        "",
        "**Skipped for hs-bridge target.**",
        "",
        "Bridge fixes are verified by the regression test the EXECUTE phase added in",
        "`crates/external_services/`, which `cargo nextest run -p external_services` already",
        "ran during EXECUTE. PR creation gate is satisfied by EXECUTE passing.",
        "",
        "_(Running prism's grpc-server here would test the unchanged downstream transformer,",
        "not the bridge fix. The tracing-marker check would also fail because the marker",
        "lives in hyperswitch's binary, not prism's.)_",
      ].join("\n"),
    };
  }

  const spec = await extractPayload(cfg, leaf);
  if (!spec) {
    return {
      ok: false,
      reason: "no payload available (neither leaf body nor fixture)",
      markdown: ["## gRPC Verification", "", "Result: FAIL — no payload available (neither leaf body JSON block nor fixtures/<connector>/<flow>.json present)"].join("\n"),
    };
  }

  let handle: ServerHandle | null = null;
  try {
    handle = await bootGrpcServer(cfg, leaf.number);
  } catch (e) {
    return {
      ok: false,
      reason: `server boot failure: ${(e as Error).message}`,
      markdown: ["## gRPC Verification", "", `Result: FAIL — ${(e as Error).message}`].join("\n"),
    };
  }

  try {
    const call = await grpcurlCall(cfg, handle, spec);
    const logBuf = await readFile(handle.logPath, "utf8").catch(() => "");
    if (call.exitCode !== 0) {
      return {
        ok: false,
        reason: "grpcurl rpc error",
        markdown: renderResult({
          spec,
          result: "FAIL",
          reason: `grpcurl exit=${call.exitCode}: ${call.stderr.slice(-500)}`,
          observedRaw: call.stdout,
          diffs: [],
          logTail: logBuf.split("\n").slice(-50).join("\n"),
        }),
      };
    }

    let observed: unknown;
    try {
      observed = JSON.parse(call.stdout || "{}");
    } catch {
      observed = call.stdout;
    }

    const diffs = Object.entries(spec.expected).map(([path, expected]) => diffField(observed, expected, path));
    const allMatch = diffs.length > 0 && diffs.every((d) => d.match);

    const tracingHit = /parity:[a-z0-9_]+:[a-z0-9_.]+ hit/i.test(logBuf);
    if (!tracingHit) {
      return {
        ok: false,
        reason: "tracing::debug parity marker not found in server log — patched code path did not execute",
        markdown: renderResult({
          spec,
          result: "FAIL",
          reason: "post-fix tracing marker not observed in server log",
          observedRaw: call.stdout,
          diffs,
          logTail: logBuf.split("\n").slice(-50).join("\n"),
        }),
      };
    }

    if (!allMatch) {
      return {
        ok: false,
        reason: "targeted field mismatch vs oracle",
        markdown: renderResult({
          spec,
          result: "FAIL",
          reason: "field mismatch (see table)",
          observedRaw: call.stdout,
          diffs,
          logTail: logBuf.split("\n").slice(-50).join("\n"),
        }),
        fieldDiffs: diffs,
      };
    }

    return {
      ok: true,
      markdown: renderResult({
        spec,
        result: "PASS",
        observedRaw: call.stdout,
        diffs,
        logTail: logBuf.split("\n").slice(-50).join("\n"),
      }),
      fieldDiffs: diffs,
    };
  } finally {
    if (handle) await teardown(handle);
  }
}
