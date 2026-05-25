import net from "node:net";

/**
 * Ask the OS for a free TCP port via the bind-to-0 trick. We open a socket
 * on port 0 (kernel assigns a free ephemeral), read the chosen port, close
 * the socket, then hand the number back to the caller.
 *
 * There's a brief race window between `close()` and whoever binds the
 * returned port next — on a quiet machine with N≤16 workers this has been
 * empirically reliable (matches what Playwright, vitest, and CI harnesses
 * do). If you hit EADDRINUSE in practice, wrap the call in
 * `withRetry(pickFreePort, { attempts: 3 })`.
 *
 * Used by the PR Resolver to allocate a fresh port for every grpc-server
 * instance, so N concurrent verification steps don't collide on a static
 * `cfg.grpcPort` value.
 */
export async function pickFreePort(): Promise<number> {
  return new Promise<number>((resolve, reject) => {
    const srv = net.createServer();
    srv.unref();
    srv.on("error", (err) => {
      reject(err);
    });
    srv.listen(0, "127.0.0.1", () => {
      const addr = srv.address();
      if (typeof addr !== "object" || addr === null || addr.port === 0) {
        srv.close();
        reject(new Error("pickFreePort: server did not yield a port"));
        return;
      }
      const port = addr.port;
      srv.close((closeErr) => {
        if (closeErr) reject(closeErr);
        else resolve(port);
      });
    });
  });
}
