import dns from "node:dns/promises";
import http from "node:http";
import net from "node:net";
import type { Duplex } from "node:stream";
import ipaddr from "ipaddr.js";
import { navigationUrl } from "./validation";

export function isPublicAddress(address: string): boolean {
  try {
    // Normalize IPv4-mapped IPv6 before classifying private/link-local ranges.
    const parsed = ipaddr.process(address);
    return parsed.range() === "unicast";
  } catch {
    return false;
  }
}

export async function publicDestination(
  input: string,
  resolve: typeof dns.lookup = dns.lookup
): Promise<{ url: URL; address: string; family: number }> {
  const url = new URL(navigationUrl(input));
  if (url.port && !["80", "443"].includes(url.port)) throw new Error("Destination port is blocked");
  const hostname = url.hostname.replace(/^\[|\]$/g, "");
  const addresses = net.isIP(hostname)
    ? [{ address: hostname, family: net.isIP(hostname) }]
    : await resolve(hostname, { all: true, verbatim: true });
  if (addresses.length === 0 || addresses.some(entry => !isPublicAddress(entry.address))) {
    throw new Error("Private, local and reserved destinations are blocked");
  }
  // The proxy connects to this checked IP, avoiding a second DNS lookup/rebinding.
  return { url, ...addresses[0] };
}

/** Per-job forward proxy. Its ephemeral loopback port never accepts automation jobs. */
export async function startNetworkGuard(): Promise<{ server: string; blocked(): boolean; close(): Promise<void> }> {
  let blocked = false;
  const sockets = new Set<Duplex>();
  const track = <T extends Duplex>(socket: T): T => {
    sockets.add(socket);
    socket.once("close", () => sockets.delete(socket));
    if (socket instanceof net.Socket) socket.setTimeout(30_000, () => socket.destroy());
    return socket;
  };
  const proxy = http.createServer((request, response) => {
    void (async () => {
      const { url, address, family } = await publicDestination(request.url ?? "");
      if (url.protocol !== "http:") throw new Error("Use CONNECT for HTTPS");
      const headers: http.OutgoingHttpHeaders = { ...request.headers, host: url.host };
      delete headers["proxy-authorization"];
      delete headers["proxy-connection"];
      const upstream = http.request({
        hostname: address,
        family,
        port: url.port || 80,
        path: url.pathname + url.search,
        method: request.method,
        headers,
        timeout: 30_000
      }, incoming => {
        response.writeHead(incoming.statusCode ?? 502, incoming.headers);
        incoming.pipe(response);
      });
      upstream.on("socket", track);
      upstream.on("timeout", () => upstream.destroy());
      upstream.on("error", () => { if (!response.headersSent) response.writeHead(502); response.end(); });
      request.on("aborted", () => upstream.destroy());
      response.on("close", () => upstream.destroy());
      request.pipe(upstream);
    })().catch(() => { blocked = true; response.writeHead(403); response.end("Destination blocked"); });
  });
  proxy.on("connection", track);
  proxy.on("connect", (request, client, head) => {
    void (async () => {
      const { url, address, family } = await publicDestination(`https://${request.url ?? ""}`);
      if (url.pathname !== "/" || url.search || url.hash) throw new Error("Invalid CONNECT destination");
      const upstream = track(net.connect({ host: address, family, port: Number(url.port || 443) }));
      upstream.once("connect", () => {
        if (client.destroyed) { upstream.destroy(); return; }
        client.write("HTTP/1.1 200 Connection Established\r\n\r\n");
        if (head.length) upstream.write(head);
        client.pipe(upstream).pipe(client);
      });
      upstream.on("error", () => client.destroy());
      client.on("close", () => upstream.destroy());
      client.on("error", () => upstream.destroy());
    })().catch(() => { blocked = true; client.end("HTTP/1.1 403 Forbidden\r\nConnection: close\r\n\r\n"); });
  });
  proxy.on("clientError", (_error, socket) => socket.destroy());
  await new Promise<void>((resolve, reject) => {
    proxy.once("error", reject);
    proxy.listen(0, "127.0.0.1", () => { proxy.off("error", reject); resolve(); });
  });
  const address = proxy.address();
  if (!address || typeof address === "string") throw new Error("Could not start browser network guard");
  return {
    server: `http://127.0.0.1:${address.port}`,
    blocked: () => blocked,
    close: async () => {
      for (const socket of sockets) socket.destroy();
      await new Promise<void>(resolve => proxy.close(() => resolve()));
    }
  };
}
