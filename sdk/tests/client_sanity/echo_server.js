/**
 * Manifest-driven echo server for HTTP client sanity certification.
 * Single source of truth: manifest.json in this directory.
 * Response is determined only by x-scenario-id header -> manifest.expected_response.
 */
const http = require('http');
const fs = require('fs');
const path = require('path');

const CLIENT_SANITY_DIR = __dirname;
const ARTIFACTS_DIR = path.join(CLIENT_SANITY_DIR, 'artifacts');
const PORT = 8081;
const MANIFEST_PATH = path.join(CLIENT_SANITY_DIR, 'manifest.json');

// Ensure artifacts directory exists
if (!fs.existsSync(ARTIFACTS_DIR)) {
  fs.mkdirSync(ARTIFACTS_DIR, { recursive: true });
}

let manifest = null;

function loadManifest() {
  if (manifest) return manifest;
  const raw = fs.readFileSync(MANIFEST_PATH, 'utf8');
  manifest = JSON.parse(raw);
  return manifest;
}

/**
 * Build response from manifest expected_response.
 * body: object -> JSON; string -> as-is; string "base64:..." -> decode and send binary (bodyForCapture = base64 for JSON-safe capture).
 * headers: optional; values can be string or string[] (e.g. Set-Cookie).
 * Returns { statusCode, headers, bodyWire, bodyForCapture } so binary can be sent as Buffer and stored as base64 in capture.
 */
function buildResponse(expectedResponse) {
  if (!expectedResponse) {
    const body = '{"status":"ok"}';
    return { statusCode: 200, headers: { 'content-type': 'application/json' }, bodyWire: body, bodyForCapture: body };
  }
  const statusCode = expectedResponse.status_code ?? 200;
  const headers = { ...expectedResponse.headers };
  const rawBody = expectedResponse.body;
  let bodyWire = '';
  let bodyForCapture = '';
  if (rawBody !== undefined && rawBody !== null) {
    if (typeof rawBody === 'object') {
      bodyWire = JSON.stringify(rawBody);
      bodyForCapture = bodyWire;
      if (!headers['content-type']) headers['content-type'] = 'application/json';
    } else {
      const str = String(rawBody);
      if (str.startsWith('base64:')) {
        const b64 = str.slice(7);
        bodyWire = Buffer.from(b64, 'base64');
        bodyForCapture = b64;
        if (!headers['content-type']) headers['content-type'] = 'application/octet-stream';
      } else {
        bodyWire = str;
        bodyForCapture = str;
      }
    }
  }
  return { statusCode, headers, bodyWire, bodyForCapture };
}

/**
 * Send response (handles multi-value headers like Set-Cookie; body can be string or Buffer).
 */
function sendResponse(res, statusCode, headers, bodyWire) {
  const outHeaders = {};
  for (const [key, value] of Object.entries(headers)) {
    outHeaders[key] = value;
  }
  res.writeHead(statusCode, outHeaders);
  res.end(bodyWire);
}

const server = http.createServer((req, res) => {
  const scenarioId = req.headers['x-scenario-id'];
  const rawSource = req.headers['x-source'] || 'unknown';
  const source = path.basename(String(rawSource)).replace(/[^a-zA-Z0-9_-]/g, '_') || 'unknown';

  let chunks = [];
  req.on('data', chunk => chunks.push(chunk));
  req.on('end', async () => {
    const rawBody = Buffer.concat(chunks);
    // Root cause: previously used toString('binary') (Latin-1), which corrupted UTF-8 and broke comparison.
    // Store as UTF-8 when valid, else base64 so capture matches golden (manifest) and judge can compare.
    let requestBodyForCapture = '';
    if (rawBody.length > 0) {
      // Use Content-Type to decide encoding: application/octet-stream means binary -> always base64.
      // For text types, decode as UTF-8 if valid, otherwise fall back to base64.
      const incomingContentType = (req.headers['content-type'] || '').toLowerCase();
      if (incomingContentType.includes('application/octet-stream')) {
        requestBodyForCapture = 'base64:' + rawBody.toString('base64');
      } else {
        const isUtf8 = typeof Buffer.isUtf8 === 'function'
          ? Buffer.isUtf8(rawBody)
          : (() => { try { new TextDecoder('utf8', { fatal: true }).decode(rawBody); return true; } catch { return false; } })();
        requestBodyForCapture = isUtf8 ? rawBody.toString('utf8') : 'base64:' + rawBody.toString('base64');
      }
    }

    const m = loadManifest();
    const scenario = m.scenarios.find(s => s.id === scenarioId);

    if (!scenarioId) {
      res.writeHead(400, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({ error: 'x-scenario-id header required' }));
      return;
    }
    if (!scenario) {
      res.writeHead(404, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({ error: 'Unknown scenario', scenario_id: scenarioId }));
      return;
    }

    if (scenario.server_delay_ms) {
      await new Promise(resolve => setTimeout(resolve, scenario.server_delay_ms));
    }

    const expectedResponse = scenario.expected_response;
    const { statusCode, headers, bodyWire, bodyForCapture } = buildResponse(expectedResponse);

    const scheme = req.socket.encrypted ? 'https' : 'http';
    const fullUrl = (req.url.startsWith('http://') || req.url.startsWith('https://'))
      ? req.url
      : `${scheme}://${req.headers.host || 'localhost:8081'}${req.url}`;

    const result = {
      method: req.method,
      url: fullUrl,
      headers: req.headers,
      body: requestBodyForCapture,
      response: { statusCode, headers, body: bodyForCapture },
    };

    const filename = path.join(ARTIFACTS_DIR, `capture_${source}.json`);
    try {
      fs.writeFileSync(filename, JSON.stringify(result, null, 2));
      sendResponse(res, statusCode, headers, bodyWire);
    } catch (err) {
      res.writeHead(500, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({ error: err.message }));
    }
  });
});

server.listen(PORT, () => {
  console.log(`Echo server (manifest-driven) listening on port ${PORT}`);
});
