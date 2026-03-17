/**
 * Forward proxy for HTTP client sanity tests. Uses http-proxy for full-URL requests;
 * CONNECT is handled at raw TCP level (undici uses CONNECT for tunneling).
 */
const http = require('http');
const net = require('net');
const httpProxy = require('http-proxy');

const PROXY_PORT = 9082;
const TARGET_HOST = 'localhost';
const TARGET_PORT = 8081;

function handleConnect(clientSocket, firstChunk) {
  const line = firstChunk.toString().split('\r\n')[0];
  const match = line.match(/^CONNECT\s+([^:\s]+):(\d+)\s+/i);
  if (!match) {
    clientSocket.end('HTTP/1.1 400 Bad Request\r\n\r\n');
    return;
  }
  const host = match[1];
  const port = parseInt(match[2], 10);
  const targetSocket = net.connect(port, host, () => {
    clientSocket.write('HTTP/1.1 200 Connection Established\r\n\r\n');
    const headerEnd = firstChunk.indexOf('\r\n\r\n');
    if (headerEnd !== -1 && firstChunk.length > headerEnd + 4) {
      targetSocket.write(firstChunk.subarray(headerEnd + 4));
    }
    clientSocket.pipe(targetSocket);
    targetSocket.pipe(clientSocket);
  });
  targetSocket.on('error', () => clientSocket.destroy());
  clientSocket.on('error', () => targetSocket.destroy());
}

const proxy = httpProxy.createProxyServer({});
proxy.on('error', (err, req, res) => {
  if (!res.headersSent) {
    res.writeHead(502, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify({ error: err.message }));
  }
});
proxy.on('proxyReq', (proxyReq) => {
  proxyReq.setHeader('X-Via-Proxy', 'true');
});

const httpServer = http.createServer((clientReq, clientRes) => {
  // Always forward to the fixed local test target regardless of what the client sends.
  // Prevents SSRF: user-controlled URL/Host headers cannot redirect the proxy elsewhere.
  const target = `http://${TARGET_HOST}:${TARGET_PORT}`;
  if (clientReq.url.startsWith('http://') || clientReq.url.startsWith('https://')) {
    const parsed = new URL(clientReq.url);
    clientReq.url = parsed.pathname + parsed.search;
  }
  proxy.web(clientReq, clientRes, { target, changeOrigin: true });
});

const netServer = net.createServer((clientSocket) => {
  clientSocket.once('data', (firstChunk) => {
    if (firstChunk.toString().startsWith('CONNECT ')) {
      handleConnect(clientSocket, firstChunk);
    } else {
      clientSocket.unshift(firstChunk);
      httpServer.emit('connection', clientSocket);
    }
  });
});

netServer.listen(PROXY_PORT, () => {
  console.log(`Simple proxy listening on port ${PROXY_PORT} (forwarding to ${TARGET_HOST}:${TARGET_PORT})`);
});
netServer.on('error', (err) => {
  if (err.code === 'EADDRINUSE') {
    console.error(`Simple proxy: port ${PROXY_PORT} already in use. Stop the other process or change PROXY_PORT.`);
  } else {
    console.error('Simple proxy error:', err.message);
  }
  process.exit(1);
});
