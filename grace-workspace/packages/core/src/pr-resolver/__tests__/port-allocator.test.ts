import { describe, expect, it } from "vitest";
import net from "node:net";
import { pickFreePort } from "../port-allocator.js";

describe("pickFreePort", () => {
  it("returns a port the caller can bind to", async () => {
    const port = await pickFreePort();
    expect(port).toBeGreaterThan(0);
    expect(port).toBeLessThan(65_536);
    // Prove the port is free by actually binding to it.
    await new Promise<void>((resolve, reject) => {
      const srv = net.createServer();
      srv.once("error", reject);
      srv.listen(port, "127.0.0.1", () => {
        srv.close((err) => (err ? reject(err) : resolve()));
      });
    });
  });

  it("returns distinct ports across N concurrent calls", async () => {
    const N = 8;
    const ports = await Promise.all(
      Array.from({ length: N }, () => pickFreePort())
    );
    const unique = new Set(ports);
    // We can't guarantee 100% uniqueness because the OS may recycle a
    // just-closed port immediately. Empirically with N=8 we always get N
    // distinct ports; if this flakes in CI, loosen to expect.length >= N-1.
    expect(unique.size).toBe(N);
  });
});
