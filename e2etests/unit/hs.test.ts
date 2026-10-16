import { test } from "node:test";
import assert from "node:assert/strict";
import { createServer } from "node:http";
import { Hyperswitch } from "../support/hs.js";

// Real Playwright HTTP transport against a local contract server. These tests
// validate bootstrap/error handling only; they do not stand in for sandbox payments.
for (const state of [
  "missing",
  "enabled",
  "disabled",
  "unauthorized",
] as const) {
  test(`UCS configuration: ${state}`, async () => {
    const calls: string[] = [];
    const server = createServer((req, res) => {
      calls.push(`${req.method} ${req.url}`);
      res.setHeader("content-type", "application/json");
      if (req.method === "POST") {
        res.end(JSON.stringify({ key: "ucs_enabled", value: "true" }));
      } else if (state === "missing") {
        res.writeHead(404).end("{}");
      } else if (state === "unauthorized") {
        res.writeHead(401).end('{"error":"secret must not appear"}');
      } else {
        res.end(
          JSON.stringify({
            key: "ucs_enabled",
            value: state === "enabled" ? "true" : "false",
          }),
        );
      }
    });
    await new Promise<void>((resolve) =>
      server.listen(0, "127.0.0.1", resolve),
    );
    const address = server.address() as { port: number };
    const previous = process.env.HS_BASE_URL;
    process.env.HS_BASE_URL = `http://127.0.0.1:${address.port}`;
    const hs = await Hyperswitch.create();
    try {
      if (state === "disabled" || state === "unauthorized") {
        await assert.rejects(
          hs.enableUcs("test-admin"),
          state === "disabled" ? /must be true/ : /HTTP 401/,
        );
        assert.equal(calls.length, 1);
      } else {
        await hs.enableUcs("test-admin");
        assert.equal(calls.length, state === "missing" ? 2 : 1);
      }
      if (state === "unauthorized") {
        await assert.rejects(
          hs.call("GET", "/failing", "test-admin"),
          (error) => {
            assert.match(String(error), /HTTP 401/);
            assert.doesNotMatch(
              String(error),
              /secret must not appear|test-admin/,
            );
            return true;
          },
        );
      }
    } finally {
      await hs.dispose();
      if (previous === undefined) delete process.env.HS_BASE_URL;
      else process.env.HS_BASE_URL = previous;
      await new Promise<void>((resolve, reject) =>
        server.close((error) => (error ? reject(error) : resolve())),
      );
    }
  });
}

test("transport failures omit request credentials", async () => {
  const server = createServer();
  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
  const { port } = server.address() as { port: number };
  await new Promise<void>((resolve) => server.close(() => resolve()));
  const previous = process.env.HS_BASE_URL;
  process.env.HS_BASE_URL = `http://127.0.0.1:${port}`;
  const hs = await Hyperswitch.create();
  try {
    await assert.rejects(
      hs.call("POST", "/payments", "never-print-this", {
        card_cvc: "sensitive",
      }),
      (error) => {
        assert.match(String(error), /HS transport failed/);
        assert.doesNotMatch(String(error), /never-print-this|sensitive/);
        return true;
      },
    );
  } finally {
    await hs.dispose();
    if (previous === undefined) delete process.env.HS_BASE_URL;
    else process.env.HS_BASE_URL = previous;
  }
});
