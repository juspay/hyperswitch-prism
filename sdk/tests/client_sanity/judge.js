/**
 * HTTP Client Sanity Judge
 * Compares golden (from manifest) vs actual (SDK outcome) and capture (echo server request).
 */
const fs = require('fs');
const path = require('path');
const assert = require('assert');

const CLIENT_SANITY_DIR = __dirname;
const ARTIFACTS_DIR = path.join(CLIENT_SANITY_DIR, 'artifacts');
const REFERENCE_BOUNDARY = 'REFERENCE_BOUNDARY';

/**
 * Normalizes headers by lowercasing keys and ignoring transport-level noise.
 * Request: SDKs add accept; response: server adds keep-alive, date, transfer-encoding.
 */
function normalizeHeaders(headers) {
  const ignoredHeaders = [
    'user-agent', 'host', 'connection', 'accept-encoding',
    'content-length', 'x-source', 'x-scenario-id', 'accept-language',
    'sec-fetch-mode', 'sec-fetch-site', 'sec-fetch-dest',
    'priority',
    'accept',           // SDKs add */* by default
    'keep-alive', 'date', 'transfer-encoding'  // Server adds these
  ];
  const normalized = {};
  for (const [key, value] of Object.entries(headers)) {
    const lowerKey = key.toLowerCase();
    if (!ignoredHeaders.includes(lowerKey)) {
      normalized[lowerKey] = value;
    }
  }
  return normalized;
}

/**
 * Normalize URL for comparison so that café and caf%C3%A9 compare equal.
 */
function normalizeUrl(urlStr) {
  try {
    return new URL(urlStr).href;
  } catch {
    return urlStr;
  }
}

/**
 * Response headers: same as normalizeHeaders, plus set-cookie normalization.
 * Root cause: multiple Set-Cookie headers are represented differently by runtimes (array vs
 * comma-joined string). Flatten to sorted array so we compare semantic equality.
 */
function normalizeResponseHeaders(headers) {
  const n = normalizeHeaders(headers);
  if (n['set-cookie'] !== undefined) {
    const v = n['set-cookie'];
    const arr = Array.isArray(v) ? [...v] : (v != null && v !== '' ? [String(v)] : []);
    const flattened = arr.flatMap(s => String(s).split(/\s*,\s*/).filter(Boolean));
    n['set-cookie'] = flattened.sort();
  }
  return n;
}

/**
 * Normalizes multipart bodies by replacing random boundaries with a static reference.
 */
function normalizeBody(body, headers) {
  // Case-insensitive lookup: golden headers preserve manifest casing (e.g. 'Content-Type')
  // while capture headers from Node's http.IncomingMessage are always lowercase.
  const contentType = Object.entries(headers)
    .find(([k]) => k.toLowerCase() === 'content-type')?.[1] || '';
  if (contentType.includes('multipart/form-data')) {
    const boundaryMatch = contentType.match(/boundary=([^;]+)/);
    if (boundaryMatch) {
      const boundary = boundaryMatch[1];
      // Escape boundary for regex and replace with REFERENCE
      const regex = new RegExp(boundary.replace(/[-/\\^$*+?.()|[\]{}]/g, '\\$&'), 'g');
      return body.replace(regex, REFERENCE_BOUNDARY);
    }
  }
  return body;
}

function verifyScenario(lang, scenarioId, goldenPath, actualPath, capturePath, expectedError) {
  const goldenExists = fs.existsSync(goldenPath);
  const actualExists = fs.existsSync(actualPath);
  const captureExists = fs.existsSync(capturePath);

  // Error scenarios: certify the SDK error code only. No request reaches server, so no capture.
  // Supports NetworkError codes (CONNECT_TIMEOUT_EXCEEDED, URL_PARSING_FAILED, etc.) and other SDK error types.
  if (expectedError) {
    if (!actualExists) {
      return { status: 'MISSING', message: 'Actual error capture missing' };
    }
    const actual = JSON.parse(fs.readFileSync(actualPath, 'utf8'));
    const actualCode = actual?.error?.code;
    if (!actualCode) {
      return { status: 'FAILED', message: 'Actual error.code missing' };
    }
    try {
      assert.strictEqual(String(actualCode), String(expectedError), 'Error Code Mismatch');
      return { status: 'SUCCESS', message: 'Expected error code matched' };
    } catch (err) {
      return { status: 'FAILED', message: err.message, diff: { actual: err.actual, expected: err.expected } };
    }
  }

  if (!goldenExists) {
    return { status: 'MISSING', message: 'Golden capture missing (run node sdk/tests/client_sanity/generate_golden.js)' };
  }
  if (!actualExists) {
    return { status: 'MISSING', message: 'Actual capture missing' };
  }
  if (!captureExists) {
    return { status: 'MISSING', message: 'Echo server capture missing (request may not have reached server)' };
  }

  const golden = JSON.parse(fs.readFileSync(goldenPath, 'utf8'));
  const actual = JSON.parse(fs.readFileSync(actualPath, 'utf8'));
  const capture = JSON.parse(fs.readFileSync(capturePath, 'utf8'));

  try {
    // Speaker: request parity — compare echo server capture to golden request
    assert.strictEqual(capture.method, golden.method, 'Method Mismatch');
    assert.strictEqual(normalizeUrl(capture.url), normalizeUrl(golden.url), 'URL Mismatch');

    const normGoldenHeaders = normalizeHeaders(golden.headers);
    const normCaptureHeaders = normalizeHeaders(capture.headers);
    assert.deepStrictEqual(normCaptureHeaders, normGoldenHeaders, 'Headers Mismatch');

    const normGoldenBody = normalizeBody(golden.body, golden.headers);
    const normCaptureBody = normalizeBody(capture.body, capture.headers);
    assert.strictEqual(normCaptureBody, normGoldenBody, 'Body Content Mismatch');

    // Listener: response parity — compare SDK's actual response to golden expected response
    if (golden.response && actual.response) {
      assert.strictEqual(actual.response.statusCode, golden.response.statusCode, 'Response Status Mismatch');
      const normGoldenRespHeaders = normalizeResponseHeaders(golden.response.headers || {});
      const normActualRespHeaders = normalizeResponseHeaders(actual.response.headers || {});
      assert.deepStrictEqual(normActualRespHeaders, normGoldenRespHeaders, 'Response Headers Mismatch');
      const goldenRespBody = golden.response.body != null ? String(golden.response.body) : '';
      const actualRespBody = actual.response.body != null ? String(actual.response.body) : '';
      assert.strictEqual(actualRespBody, goldenRespBody, 'Response Body Mismatch');
    }

    return { status: 'SUCCESS', message: 'Perfect Parity' };
  } catch (err) {
    return { status: 'FAILED', message: err.message, diff: { actual: err.actual, expected: err.expected } };
  }
}

async function runCertification() {
  const manifestPath = path.join(CLIENT_SANITY_DIR, 'manifest.json');
  const manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8'));
  const languages = ['node', 'python', 'rust', 'kotlin'];

  const reportData = [];
  let totalCertified = 0;
  let totalFailed = 0;

  console.log('\n🛡️ [HTTP CLIENT SANITY JUDGE]: Starting certification...');

  for (const lang of languages) {
    // Check if any actual captures exist for this language
    const firstActual = path.join(ARTIFACTS_DIR, `actual_${lang}_${manifest.scenarios[0].id}.json`);
    if (!fs.existsSync(firstActual)) continue;

    console.log(`\n🏁 Certifying ${lang.toUpperCase()} SDK...`);
    totalCertified++;
    const langResults = [];

    for (const scenario of manifest.scenarios) {
      const golden = path.join(ARTIFACTS_DIR, `golden_${scenario.id}.json`);
      const actual = path.join(ARTIFACTS_DIR, `actual_${lang}_${scenario.id}.json`);
      const capture = path.join(ARTIFACTS_DIR, `capture_${lang}_${scenario.id}.json`);
      const expectedError = scenario.expected_error || null;
      const result = verifyScenario(lang, scenario.id, golden, actual, capture, expectedError);
      langResults.push({ id: scenario.id, ...result });

      // A scenario is optional if globally optional OR skipped for this specific language.
      const isOptional = scenario.optional || (Array.isArray(scenario.skip_langs) && scenario.skip_langs.includes(lang));
      const skipReason = scenario[`skip_reason_${lang}`] || scenario.skip_reason || '';

      if (result.status === 'SUCCESS') {
        console.log(`   PASS ${scenario.id}`);
      } else {
        const optionalMarker = isOptional ? ' [OPTIONAL]' : '';
        const reasonSuffix = skipReason ? ` — ${skipReason}` : '';
        const prefix = isOptional ? 'WARN' : 'FAIL';
        console.error(`   ${prefix} ${scenario.id}${optionalMarker}: ${result.status} (${result.message})${reasonSuffix}`);
      }
    }

    // Only non-optional failures block certification.
    const mandatoryFailedCount = langResults.filter(r => {
      if (r.status !== 'FAILED') return false;
      const scenario = manifest.scenarios.find(s => s.id === r.id);
      const isOptional = scenario?.optional || (Array.isArray(scenario?.skip_langs) && scenario.skip_langs.includes(lang));
      return !isOptional;
    }).length;
    if (mandatoryFailedCount > 0) totalFailed++;
    reportData.push({ lang, results: langResults });
  }

  // GENERATE MARKDOWN REPORT (plain text, neutral, no emojis)
  let markdown = `# HTTP Client Sanity Report\n\n**Verdict**: ${totalFailed === 0 ? 'PASS' : 'FAIL'}\n\n`;
  markdown += '## Language Scorecard\n| Language | Certified | Success Rate | Status |\n| :--- | :--- | :--- | :--- |\n';

  reportData.forEach(entry => {
    const passed = entry.results.filter(r => r.status === 'SUCCESS').length;
    const total = entry.results.length;
    const pct = Math.round((passed / total) * 100);
    markdown += `| **${entry.lang.toUpperCase()}** | ${passed}/${total} | ${pct}% | ${passed === total ? 'PASS' : 'WARN'} |\n`;
  });

  markdown += '\n## Detailed Scenario Audit\n| ID | Description | ' + languages.map(l => l.toUpperCase()).join(' | ') + ' |\n| :--- | :--- | ' + languages.map(() => ':---:').join(' | ') + ' |\n';

  manifest.scenarios.forEach(scenario => {
    const globallyOptional = scenario.optional;
    const optionalTag = globallyOptional ? ' *(optional)*' : '';
    let row = `| **${scenario.id}**${optionalTag} | ${scenario.description} | `;
    const statuses = reportData.map(entry => {
      const res = entry.results.find(r => r.id === scenario.id);
      if (!res) return '-';
      if (res.status === 'SUCCESS') return 'PASS';
      const isOptional = globallyOptional || (Array.isArray(scenario.skip_langs) && scenario.skip_langs.includes(entry.lang));
      return isOptional ? 'WARN' : 'FAIL';
    });
    markdown += row + statuses.join(' | ') + ' |\n';
  });

  // Collect all optional/skipped notes for the footer.
  const optionalNotes = [];
  manifest.scenarios.forEach(s => {
    if (s.optional && s.skip_reason) optionalNotes.push(`- **${s.id}** *(all languages)*: ${s.skip_reason}`);
    if (Array.isArray(s.skip_langs)) {
      s.skip_langs.forEach(lang => {
        const reason = s[`skip_reason_${lang}`] || s.skip_reason || 'Not supported for this SDK.';
        optionalNotes.push(`- **${s.id}** *(${lang.toUpperCase()})*: ${reason}`);
      });
    }
  });
  if (optionalNotes.length > 0) {
    markdown += '\n## Optional / Skipped Scenarios\n\nThese failures do not block certification.\n\n';
    markdown += optionalNotes.join('\n') + '\n';
  }

  const reportPath = path.join(ARTIFACTS_DIR, 'REPORT.md');
  fs.writeFileSync(reportPath, markdown);
  console.log('\nCertification Complete. Report: sdk/tests/client_sanity/artifacts/REPORT.md');

  process.exit(totalFailed === 0 ? 0 : 1);
}

runCertification().catch(console.error);
