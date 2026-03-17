#!/usr/bin/env node
/**
 * Client Sanity Certification Runner
 *
 * Orchestrates the execution of client sanity tests across all SDK languages.
 * For each scenario in manifest.json, this script:
 * 1. Starts the echo server
 * 2. Runs language-specific runners (Node, Python, Rust, Kotlin)
 * 3. Collects actual outputs and echo server captures
 * 4. Stores results for the judge to verify
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

async function startEchoServer() {
  console.log('📡 Starting Echo Server...');
  const server = spawn('node', [ECHO_SERVER_PATH], { stdio: 'inherit' });
  await new Promise(r => setTimeout(r, 1000)); // Wait for server to bind
  return server;
}

async function main() {
  const manifest = JSON.parse(fs.readFileSync(MANIFEST_PATH, 'utf8'));
  const languages = process.argv.slice(2);
  if (languages.length === 0) {
    console.error('Usage: node sdk/tests/client_sanity/run_client_certification.js <rust|python|node|kotlin>');
    process.exit(1);
  }

  const echoServer = await startEchoServer();

  for (const lang of languages) {
    console.log(`\n[ORCHESTRATOR]: Testing ${lang.toUpperCase()} SDK...`);

    for (const scenario of manifest.scenarios) {
      console.log(`   Scenario: ${scenario.id}`);

      const sourceId = `${lang}_${scenario.id}`;
      const actualStore = path.join(ARTIFACTS_DIR, `actual_${lang}_${scenario.id}.json`);
      const captureFile = path.join(ARTIFACTS_DIR, `capture_${sourceId}.json`);

      // Clean up old files
      if (fs.existsSync(actualStore)) fs.unlinkSync(actualStore);
      if (fs.existsSync(captureFile)) fs.unlinkSync(captureFile);

      // Prepare input for Thin Runner
      const runnerInput = JSON.stringify({
        scenario_id: scenario.id,
        source_id: sourceId,
        request: scenario.request,
        proxy: scenario.proxy,
        client_timeout_ms: scenario.client_timeout_ms,
        client_response_timeout_ms: scenario.client_response_timeout_ms
      });

      // Execute Thin Runner
      let cmd, args, runOpts = {};
      if (lang === 'rust') {
        cmd = 'cargo';
        args = ['run', '--bin', 'client_sanity_runner', '--quiet'];
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
        sdkOutput = JSON.parse(stdout.trim());
      } catch (e) {
        sdkOutput = { error: { code: 'RUNNER_CRASH', message: stdout || 'No output' } };
      }

      // Wait for Echo Server to write capture (for success/timeout scenarios where request reached server)
      let waitMs = 200;
      if (scenario.server_delay_ms) waitMs = Math.max(waitMs, scenario.server_delay_ms + 100);
      await new Promise(r => setTimeout(r, waitMs));

      // actual contains ONLY SDK outcome (response or error). Judge reads capture directly for Speaker.
      fs.writeFileSync(actualStore, JSON.stringify(sdkOutput, null, 2));
    }
  }

  echoServer.kill();
  console.log('\n⚖️ Starting Judge...');
  execFileSync('node', [JUDGE_PATH], { stdio: 'inherit' });
}

main().catch(err => {
  console.error(err);
  process.exit(1);
});
