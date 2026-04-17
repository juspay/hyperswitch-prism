#!/usr/bin/env node
/**
 * Client Sanity Certification Runner (Optimized)
 *
 * Orchestrates the execution of client sanity tests across all SDK languages.
 * OPTIMIZED: Runs all SDKs in parallel for each scenario.
 */
const fs = require('fs');
const path = require('path');
const { spawn, execFileSync } = require('child_process');

const CLIENT_SANITY_DIR = __dirname;
const PROJECT_ROOT = path.join(CLIENT_SANITY_DIR, '..', '..', '..');
const ARTIFACTS_DIR = path.join(CLIENT_SANITY_DIR, 'artifacts');
const MANIFEST_PATH = path.join(CLIENT_SANITY_DIR, 'manifest.json');
const ECHO_SERVER_PATH = path.join(CLIENT_SANITY_DIR, 'echo_server.js');
const JUDGE_PATH = path.join(CLIENT_SANITY_DIR, 'judge.js');

// Ensure artifacts directory exists
if (!fs.existsSync(ARTIFACTS_DIR)) {
  fs.mkdirSync(ARTIFACTS_DIR, { recursive: true });
}

async function runCommand(cmd, args, input = null, opts = {}) {
  const cwd = opts.cwd || PROJECT_ROOT;
  return new Promise((resolve) => {
    const proc = spawn(cmd, args, { stdio: ['pipe', 'pipe', 'inherit'], cwd });
    let stdout = '';
    if (input) {
      proc.stdin.write(input);
      proc.stdin.end();
    }
    proc.stdout.on('data', (data) => { stdout += data; });
    proc.on('close', (code) => resolve({ code, stdout }));
  });
}

async function runSdkScenario(lang, scenario, runnerInput) {
  const sourceId = `${lang}_${scenario.id}`;
  const actualStore = path.join(ARTIFACTS_DIR, `actual_${sourceId}.json`);
  const captureFile = path.join(ARTIFACTS_DIR, `capture_${sourceId}.json`);

  // Clean up old files
  if (fs.existsSync(actualStore)) fs.unlinkSync(actualStore);
  if (fs.existsSync(captureFile)) fs.unlinkSync(captureFile);

  // Execute Thin Runner
  let cmd, args, runOpts = {};
  if (lang === 'rust') {
    cmd = path.join(PROJECT_ROOT, 'target/debug/client_sanity_runner');
    args = [];
    runOpts = { cwd: path.join(PROJECT_ROOT, 'sdk/rust') };
  } else if (lang === 'python') {
    cmd = 'python3';
    args = ['tests/client_sanity_runner.py'];
    runOpts = { cwd: path.join(PROJECT_ROOT, 'sdk/python') };
  } else if (lang === 'node') {
    cmd = 'npx';
    args = ['ts-node', 'tests/client_sanity_runner.ts'];
    runOpts = { cwd: path.join(PROJECT_ROOT, 'sdk/javascript') };
  } else if (lang === 'kotlin') {
    cmd = './gradlew';
    args = ['runClientSanity', '--quiet'];
    runOpts = { cwd: path.join(PROJECT_ROOT, 'sdk/java') };
  }

  const { stdout } = await runCommand(cmd, args, runnerInput, runOpts);

  let sdkOutput;
  try {
    const lines = stdout.trim().split('\n').filter(l => l.trim());
    const jsonLine = lines[lines.length - 1] || stdout.trim();
    sdkOutput = JSON.parse(jsonLine);
  } catch (e) {
    sdkOutput = { error: { code: 'RUNNER_CRASH', message: stdout || 'No output' } };
  }

  fs.writeFileSync(actualStore, JSON.stringify(sdkOutput, null, 2));
  return { lang, scenario: scenario.id };
}

async function startEchoServer() {
  console.log('📡 Starting Echo Server...');
  const server = spawn('node', [ECHO_SERVER_PATH], { stdio: 'inherit' });
  await new Promise(r => setTimeout(r, 1000));
  return server;
}

async function main() {
  const manifest = JSON.parse(fs.readFileSync(MANIFEST_PATH, 'utf8'));
  const languages = process.argv.slice(2);
  if (languages.length === 0) {
    console.error('Usage: node run_client_certification.js <rust|python|node|kotlin>');
    process.exit(1);
  }

  const echoServer = await startEchoServer();

  for (const scenario of manifest.scenarios) {
    console.log(`\n[SCENARIO]: ${scenario.id}`);
    
    const runnerInput = JSON.stringify({
      scenario_id: scenario.id,
      request: scenario.request,
      proxy: scenario.proxy,
      client_timeout_ms: scenario.client_timeout_ms,
      client_response_timeout_ms: scenario.client_response_timeout_ms
    });

    // Run all SDKs in parallel for this scenario
    const promises = languages.map(lang => {
      // Override source_id per language
      const input = JSON.stringify({
        ...JSON.parse(runnerInput),
        source_id: `${lang}_${scenario.id}`
      });
      return runSdkScenario(lang, scenario, input);
    });

    const results = await Promise.all(promises);
    results.forEach(r => console.log(`   ✓ ${r.lang.toUpperCase()}`));

    // Reduced wait time (was 200ms, now 100ms) - just enough for file writes
    const waitMs = scenario.server_delay_ms ? scenario.server_delay_ms + 100 : 100;
    await new Promise(r => setTimeout(r, waitMs));
  }

  echoServer.kill();
  console.log('\n⚖️ Starting Judge...');
  execFileSync('node', [JUDGE_PATH], { stdio: 'inherit' });
}

main().catch(err => {
  console.error(err);
  process.exit(1);
});
