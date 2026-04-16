<!--
---
title: CI/CD Integration
description: Continuous integration and automated testing
last_updated: 2026-03-12
generated_from: backend/integration-tests/
auto_generated: false
reviewed_by: engineering
reviewed_at: 2026-03-12
approved: true
---
-->

# CI/CD Integration

## Overview

The Connector Service Test Suite is designed for continuous integration with a **snapshot testing strategy**. This approach ensures test stability while enabling comprehensive validation.

## Snapshot Testing Strategy

### How It Works

1. **Main Branch**: Maintains a certified snapshot of test results
2. **Pull Requests**: Validate against the snapshot (no live transactions during PR)
3. **Post-Merge**: Live transaction tests run to generate new snapshot
4. **Results**: Committed to repository (excluding credentials) in the docs section

### Benefits

- **Fast PR Validation**: No waiting for live transaction tests
- **Stable CI**: No flaky tests due to network/connector issues
- **Historical Tracking**: Git history shows test evolution
- **Safe Changes**: Detect unintended test modifications

## Pipeline Flow

```
Pull Request                    Main Branch (post-merge)
     │                                   │
     ▼                                   ▼
┌──────────┐                    ┌──────────────────┐
│ Checkout │                    │ Checkout         │
│ code     │                    │ merged code      │
└────┬─────┘                    └──────┬───────────┘
     │                                 │
     ▼                                 ▼
┌──────────┐                    ┌──────────────────┐
│ Run tests│                    │ Run tests with   │
│ against  │                    │ LIVE connectors  │
│ snapshot │                    │                  │
└────┬─────┘                    └──────┬───────────┘
     │                                 │
     ▼                                 ▼
┌──────────┐                    ┌──────────────────┐
│ Compare  │                    │ Generate new     │
│ results  │                    │ snapshot         │
└────┬─────┘                    └──────┬───────────┘
     │                                 │
     ▼                                 ▼
┌──────────┐                    ┌──────────────────┐
│ Pass/Fail│                    │ Commit snapshot  │
│ PR       │                    │ to repository    │
└──────────┘                    └──────────────────┘
```

## GitHub Actions Workflow

### Basic Workflow

```yaml
name: Connector Tests

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

env:
  CARGO_TERM_COLOR: always

jobs:
  test:
    runs-on: ubuntu-latest
    
    steps:
      - name: Checkout code
        uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-action@stable

      - name: Cache cargo dependencies
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/bin/
            ~/.cargo/registry/index/
            ~/.cargo/registry/cache/
            ~/.cargo/git/db/
            target/
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}

      - name: Run connector tests
        env:
          CONNECTOR_AUTH_FILE_PATH: ${{ secrets.CONNECTOR_AUTH_FILE_PATH }}
        run: |
          cargo run --bin suite_run_test -- --all

      - name: Upload test reports
        uses: actions/upload-artifact@v4
        if: always()
        with:
          name: test-reports
          path: |
            backend/integration-tests/report.json
            backend/integration-tests/test_report.md
```

### PR Validation Workflow

```yaml
name: PR Validation

on:
  pull_request:
    branches: [main]

jobs:
  validate:
    runs-on: ubuntu-latest
    
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-action@stable

      - name: Validate test snapshots
        run: |
          # Compare current test results with stored snapshot
          cargo run --bin suite_run_test -- --all --compare-snapshot
```

### Post-Merge Workflow

```yaml
name: Update Test Snapshots

on:
  push:
    branches: [main]

jobs:
  update-snapshots:
    runs-on: ubuntu-latest
    
    steps:
      - uses: actions/checkout@v4
        with:
          token: ${{ secrets.GITHUB_TOKEN }}

      - name: Install Rust
        uses: dtolnay/rust-action@stable

      - name: Run live tests
        env:
          CONNECTOR_AUTH_FILE_PATH: ${{ secrets.CONNECTOR_AUTH_FILE_PATH }}
        run: |
          cargo run --bin suite_run_test -- --all

      - name: Commit test results
        run: |
          git config --local user.email "action@github.com"
          git config --local user.name "GitHub Action"
          git add backend/integration-tests/docs/test-reports/
          git diff --staged --quiet || git commit -m "Update test snapshots [skip ci]"
          git push
```

## GitLab CI/CD

### Basic Pipeline

```yaml
stages:
  - test
  - report

variables:
  CARGO_HOME: $CI_PROJECT_DIR/.cargo
  CARGO_TARGET_DIR: $CI_PROJECT_DIR/target

cache:
  key: ${CI_COMMIT_REF_SLUG}
  paths:
    - .cargo/
    - target/

test:
  stage: test
  image: rust:latest
  script:
    - cargo run --bin suite_run_test -- --all
  artifacts:
    when: always
    paths:
      - backend/integration-tests/test_report.md
      - backend/integration-tests/report.json
    expire_in: 1 week
  only:
    - merge_requests
    - main
```

## Jenkins Pipeline

### Jenkinsfile

```groovy
pipeline {
    agent any

    environment {
        CONNECTOR_AUTH_FILE_PATH = credentials('ucs-connector-creds')
        CARGO_HOME = "${WORKSPACE}/.cargo"
    }

    stages {
        stage('Checkout') {
            steps {
                checkout scm
            }
        }

        stage('Test') {
            steps {
                sh 'cargo run --bin suite_run_test -- --all'
            }
        }

        stage('Archive Reports') {
            steps {
                archiveArtifacts artifacts: 'backend/integration-tests/*.md', allowEmptyArchive: true
                archiveArtifacts artifacts: 'backend/integration-tests/*.json', allowEmptyArchive: true
            }
        }
    }

    post {
        always {
            publishHTML(target: [
                allowMissing: false,
                alwaysLinkToLastBuild: true,
                keepAll: true,
                reportDir: 'backend/integration-tests',
                reportFiles: 'test_report.md',
                reportName: 'Test Report'
            ])
        }
    }
}
```

## Credentials Management

### GitHub Secrets

Store credentials securely in GitHub:

1. Go to **Settings** → **Secrets and variables** → **Actions**
2. Add secret: `CONNECTOR_AUTH_FILE_PATH`
3. Value: Path to credentials in secret storage

Or use GitHub's encrypted secrets:

```yaml
- name: Create credentials file
  env:
    CREDS_JSON: ${{ secrets.UCS_CONNECTOR_CREDS }}
  run: |
    echo "$CREDS_JSON" > /tmp/creds.json
    export CONNECTOR_AUTH_FILE_PATH=/tmp/creds.json
```

### Vault Integration

Fetch credentials from HashiCorp Vault:

```yaml
- name: Fetch credentials from Vault
  uses: hashicorp/vault-action@v3
  with:
    url: https://vault.example.com
    method: approle
    roleId: ${{ secrets.VAULT_ROLE_ID }}
    secretId: ${{ secrets.VAULT_SECRET_ID }}
    secrets: |
      secret/data/ucs/connectors creds_json | CREDS_JSON

- name: Run tests
  run: |
    echo "$CREDS_JSON" > /tmp/creds.json
    export CONNECTOR_AUTH_FILE_PATH=/tmp/creds.json
    cargo run --bin suite_run_test -- --all
```

### AWS Secrets Manager

```yaml
- name: Configure AWS credentials
  uses: aws-actions/configure-aws-credentials@v4
  with:
    aws-access-key-id: ${{ secrets.AWS_ACCESS_KEY_ID }}
    aws-secret-access-key: ${{ secrets.AWS_SECRET_ACCESS_KEY }}
    aws-region: us-east-1

- name: Get credentials from Secrets Manager
  run: |
    aws secretsmanager get-secret-value \
      --secret-id ucs/connector-creds \
      --query SecretString \
      --output text > /tmp/creds.json
    export CONNECTOR_AUTH_FILE_PATH=/tmp/creds.json
```

## Test Reports

### Artifact Upload

Upload test reports as CI artifacts:

```yaml
- name: Upload test reports
  uses: actions/upload-artifact@v4
  if: always()
  with:
    name: test-reports-${{ github.run_id }}
    path: |
      backend/integration-tests/report.json
      backend/integration-tests/test_report.md
    retention-days: 30
```

### GitHub PR Comments

Post test results as PR comments:

```yaml
- name: Post test results
  uses: actions/github-script@v7
  if: github.event_name == 'pull_request'
  with:
    script: |
      const fs = require('fs');
      const report = fs.readFileSync('backend/integration-tests/test_report.md', 'utf8');
      github.rest.issues.createComment({
        issue_number: context.issue.number,
        owner: context.repo.owner,
        repo: context.repo.repo,
        body: '## Test Results\n\n' + report
      });
```

### Slack Notifications

Send test results to Slack:

```yaml
- name: Notify Slack
  uses: slackapi/slack-github-action@v1
  if: always()
  with:
    payload: |
      {
        "text": "Connector Tests: ${{ job.status }}",
        "blocks": [
          {
            "type": "section",
            "text": {
              "type": "mrkdwn",
              "text": "*Connector Service Tests*\nStatus: ${{ job.status }}\n<${{ github.server_url }}/${{ github.repository }}/actions/runs/${{ github.run_id }}|View Run>"
            }
          }
        ]
      }
  env:
    SLACK_WEBHOOK_URL: ${{ secrets.SLACK_WEBHOOK_URL }}
```

## Performance Optimization

### Parallel Testing

Run tests in parallel across connectors:

```yaml
strategy:
  matrix:
    connector: [stripe, adyen, paypal, braintree]
  fail-fast: false

steps:
  - run: cargo run --bin suite_run_test -- --all --connector ${{ matrix.connector }}
```

### Caching

Cache cargo dependencies and build artifacts:

```yaml
- name: Cache cargo
  uses: actions/cache@v4
  with:
    path: |
      ~/.cargo/bin/
      ~/.cargo/registry/index/
      ~/.cargo/registry/cache/
      ~/.cargo/git/db/
      target/
    key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
    restore-keys: |
      ${{ runner.os }}-cargo-
```

## Best Practices

### CI/CD Checklist

- [ ] Store credentials in secrets manager
- [ ] Cache cargo dependencies
- [ ] Upload test artifacts
- [ ] Run tests on PR and merge
- [ ] Post results to PR/MR
- [ ] Alert on failures
- [ ] Archive old reports

### Security

- Never commit credentials to git
- Use short-lived credentials where possible
- Rotate API keys regularly
- Limit connector permissions to test environments
- Audit access to credential secrets

### Reliability

- Use deterministic test data
- Handle connector rate limits
- Retry transient failures
- Set appropriate timeouts
- Monitor test duration trends

## Next Steps

- [Usage](./usage.md) - Running tests locally
- [Best Practices](./best-practices.md) - Writing effective tests
- [Configuration](./configuration.md) - Environment setup
