#!/usr/bin/env node
/**
 * Cucumber BDD Certification Runner
 *
 * Orchestrates Cucumber/Gherkin-based client sanity tests across all SDK languages.
 * Each language has its own Cucumber step definitions that share the same .feature files.
 *
 * Usage: node sdk/tests/client_sanity/run_cucumber_certification.js <rust|python|node|kotlin>
 */
const fs = require('fs');
const path = require('path');
const { spawn, execFileSync } = require('child_process');

const CLIENT_SANITY_DIR = __dirname;
const PROJECT_ROOT = path.join(CLIENT_SANITY_DIR, '..', '..', '..');
const ARTIFACTS_DIR = path.join(CLIENT_SANITY_DIR, 'artifacts');
const MANIFEST_PATH = path.join(CLIENT_SANITY_DIR, 'manifest.json');
const ECHO_SERVER_PATH = path.join(CLIENT_SANITY_DIR, 'echo_server.js');

// Ensure artifacts directory exists
if (!fs.existsSync(ARTIFACTS_DIR)) {
  fs.mkdirSync(ARTIFACTS_DIR, { recursive: true });
}

async function startEchoServer() {
  console.log('📡 Starting Echo Server...');
  const server = spawn('node', [ECHO_SERVER_PATH], { stdio: 'inherit' });
  await new Promise(r => setTimeout(r, 1000));
  return server;
}

function runLangCucumber(lang) {
  return new Promise((resolve) => {
    let cmd, args, cwd;

    if (lang === 'node') {
      const cucumberBin = path.join(PROJECT_ROOT, 'sdk/javascript/node_modules/.bin/cucumber-js');
      if (fs.existsSync(cucumberBin)) {
        cmd = cucumberBin;
        args = ['--config', path.join(PROJECT_ROOT, 'sdk/javascript/cucumber.js')];
      } else {
        cmd = 'npx';
        args = [
          '--yes',
          '--package=@cucumber/cucumber',
          'cucumber-js',
          '--config',
          path.join(PROJECT_ROOT, 'sdk/javascript/cucumber.js'),
        ];
      }
      cwd = path.join(PROJECT_ROOT, 'sdk/javascript');
    } else if (lang === 'python') {
      cmd = 'python3';
      args = [
        '-m', 'behave',
        '--no-capture',
        '-f', 'json', '-o', path.join(ARTIFACTS_DIR, 'cucumber_python.json'),
        '-f', 'pretty',
        '--tags', '~@skip_python',
        path.join(CLIENT_SANITY_DIR, 'features'),
      ];
      cwd = path.join(PROJECT_ROOT, 'sdk/python/tests/cucumber');
    } else if (lang === 'rust') {
      cmd = 'cargo';
      args = ['test', '--test', 'cucumber'];
      cwd = path.join(PROJECT_ROOT, 'sdk/rust');
    } else if (lang === 'kotlin') {
      cmd = './gradlew';
      args = ['runCucumber', '--quiet'];
      cwd = path.join(PROJECT_ROOT, 'sdk/java');
    } else {
      console.error(`Unknown language: ${lang}`);
      resolve({ lang, exitCode: 1 });
      return;
    }

    console.log(`\n🥒 [CUCUMBER]: Running ${lang.toUpperCase()} SDK tests...`);
    const proc = spawn(cmd, args, { stdio: 'inherit', cwd });
    proc.on('close', (code) => {
      resolve({ lang, exitCode: code || 0 });
    });
  });
}

function generateReport(results) {
  const manifest = JSON.parse(fs.readFileSync(MANIFEST_PATH, 'utf8'));
  const totalLangs = results.length;
  const passedLangs = results.filter(r => r.exitCode === 0).length;
  const failedLangs = results.filter(r => r.exitCode !== 0);
  const verdict = failedLangs.length === 0 ? 'PASS' : 'FAIL';

  let markdown = `# HTTP Client Sanity Report (Cucumber/Gherkin)\n\n`;
  markdown += `**Verdict**: ${verdict}\n\n`;
  markdown += `## Language Results\n`;
  markdown += `| Language | Runner | Status |\n| :--- | :--- | :--- |\n`;

  results.forEach(r => {
    const status = r.exitCode === 0 ? 'PASS' : 'FAIL';
    markdown += `| **${r.lang.toUpperCase()}** | Cucumber | ${status} |\n`;
  });

  markdown += `\n## Test Suite\n`;
  markdown += `All tests are defined in shared Gherkin feature files at \`sdk/tests/client_sanity/features/\`.\n`;
  markdown += `Each language implements step definitions that exercise its SDK HTTP client.\n\n`;

  markdown += `### Scenarios (${manifest.scenarios.length} total)\n`;
  markdown += `| ID | Description | Tags |\n| :--- | :--- | :--- |\n`;
  manifest.scenarios.forEach(s => {
    const tags = [];
    if (s.optional) tags.push('optional');
    if (s.skip_langs) tags.push(...s.skip_langs.map(l => `skip_${l}`));
    markdown += `| **${s.id}** | ${s.description} | ${tags.join(', ') || '-'} |\n`;
  });

  if (failedLangs.length > 0) {
    markdown += `\n## Failures\n`;
    failedLangs.forEach(r => {
      markdown += `- **${r.lang.toUpperCase()}**: Cucumber tests failed (exit code ${r.exitCode})\n`;
    });
  }

  // Collect optional/skipped notes
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
    markdown += `\n## Optional / Skipped Scenarios\n\nThese failures do not block certification.\n\n`;
    markdown += optionalNotes.join('\n') + '\n';
  }

  const reportPath = path.join(ARTIFACTS_DIR, 'REPORT.md');
  fs.writeFileSync(reportPath, markdown);
  console.log('\nCertification Complete. Report: sdk/tests/client_sanity/artifacts/REPORT.md');
  return verdict;
}

async function main() {
  const languages = process.argv.slice(2);
  if (languages.length === 0) {
    console.error('Usage: node sdk/tests/client_sanity/run_cucumber_certification.js <rust|python|node|kotlin>');
    process.exit(1);
  }

  const echoServer = await startEchoServer();

  const results = [];
  for (const lang of languages) {
    const result = await runLangCucumber(lang);
    results.push(result);
  }

  echoServer.kill();

  const verdict = generateReport(results);
  process.exit(verdict === 'PASS' ? 0 : 1);
}

main().catch(err => {
  console.error(err);
  process.exit(1);
});
