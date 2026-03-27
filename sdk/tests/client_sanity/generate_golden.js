/**
 * Generate golden_<scenario.id>.json from manifest.json.
 * Golden has the same shape as echo server capture: method, url, headers, body, response.
 * Judge compares actual_<lang>_<id>.json (from echo server) to golden_<id>.json.
 * Only generates for scenarios that do NOT have expected_error (no request reaches server).
 */
const fs = require('fs');
const path = require('path');

const CLIENT_SANITY_DIR = __dirname;
const ARTIFACTS_DIR = path.join(CLIENT_SANITY_DIR, 'artifacts');
const manifestPath = path.join(CLIENT_SANITY_DIR, 'manifest.json');

// Ensure artifacts directory exists
if (!fs.existsSync(ARTIFACTS_DIR)) {
  fs.mkdirSync(ARTIFACTS_DIR, { recursive: true });
}
const manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8'));

function buildResponseBody(expectedResponse) {
  if (!expectedResponse) return '';
  const raw = expectedResponse.body;
  if (raw === undefined || raw === null) return '';
  if (typeof raw === 'object') return JSON.stringify(raw);
  const str = String(raw);
  if (str.startsWith('base64:')) return str.slice(7);
  return str;
}

manifest.scenarios.forEach((scenario) => {
  if (!scenario.expected_response) return;

  const req = scenario.request;
  const headers = { ...(req.headers || {}) };
  headers['x-source'] = `golden_${scenario.id}`;
  headers['x-scenario-id'] = scenario.id;
  if (scenario.proxy) headers['x-via-proxy'] = 'true';

  const expectedResponse = scenario.expected_response || { status_code: 200, body: { status: 'ok' } };
  const statusCode = expectedResponse.status_code ?? 200;
  const respHeaders = { ...(expectedResponse.headers || {}) };
  if (expectedResponse.body !== undefined && expectedResponse.body !== null && typeof expectedResponse.body === 'object' && !respHeaders['content-type']) {
    respHeaders['content-type'] = 'application/json';
  }
  const responseBody = buildResponseBody(expectedResponse);

  const golden = {
    method: req.method,
    url: req.url,
    headers,
    body: req.body != null ? req.body : '',
    response: {
      statusCode,
      headers: respHeaders,
      body: responseBody,
    },
  };

  const outPath = path.join(ARTIFACTS_DIR, `golden_${scenario.id}.json`);
  fs.writeFileSync(outPath, JSON.stringify(golden, null, 2));
  console.log(`   📄 golden_${scenario.id}.json`);
});

console.log('Golden captures generated from manifest.');
