# Documentation Sync Setup

This guide explains how the automatic documentation sync from `connector-service` to `hyperswitch-docs` works and how to configure it.

## Overview

Documentation in the `connector-service/docs/` folder is automatically synced to the `juspay/hyperswitch-docs` repository whenever changes are pushed to the `main` branch.

## How It Works

### Workflow Trigger

The sync workflow (`.github/workflows/docs-sync.yml`) runs on:
- Push to `main` branch with changes in `docs/**`
- Manual trigger via `workflow_dispatch` with optional dry-run

### Sync Process

1. **Checkout both repositories**
   - `connector-service` (source)
   - `juspay/hyperswitch-docs` (target)

2. **Copy documentation**
   - Source: `docs/`
   - Target: `hyperswitch-docs/connector-service/`

3. **Filtered sync**
   The following patterns from `docs/gitbook.yml` are excluded:
   - `rules/**`
   - `plans/**`
   - `CODE_OF_CONDUCT.md`
   - `specs-self-managing-documentation-system.md`

4. **Commit and push**
   - Commit message includes source commit SHA
   - Pushes to `juspay/hyperswitch-docs:main`

## Prerequisites

### 1. PAT Configuration (REQUIRED)

The workflow requires a Personal Access Token (PAT) with write access to `juspay/hyperswitch-docs`.

**Secret Name:** `CONNECTOR_SERVICE_CI_PAT`

**Setup Instructions:**

1. Go to GitHub Settings → Developer settings → Personal access tokens → Tokens (classic)
2. Generate new token with these scopes:
   - `repo` (full repository access)
   - `workflow` (if modifying workflows)

3. Add the token as a repository secret:
   - Repository: `juspay/connector-service`
   - Name: `CONNECTOR_SERVICE_CI_PAT`
   - Value: Your generated PAT

### 2. Target Repository Setup

Ensure `juspay/hyperswitch-docs` exists and you have write access. The workflow will create the `connector-service/` directory on first sync.

## Verification

### Dry Run Mode

Test the sync without pushing changes:

1. Go to Actions → "Sync Docs to Hyperswitch Docs"
2. Click "Run workflow"
3. Enable "Dry run" option
4. Check logs for files that would be synced

### Check Last Sync

View workflow runs at:
`https://github.com/juspay/connector-service/actions/workflows/docs-sync.yml`

## Troubleshooting

### "Permission denied" errors

The `CONNECTOR_SERVICE_CI_PAT` secret may be missing or expired. Regenerate and update the secret.

### "Repository not found" errors

Verify:
- Repository `juspay/hyperswitch-docs` exists
- The PAT has access to the repository
- The repository name is correct in the workflow

### Changes not appearing

Check workflow logs for:
- Sync completed successfully
- No files excluded by gitbook.yml patterns
- Target directory structure is correct

## Manual Sync

If automatic sync fails, manually trigger from GitHub Actions:

1. Go to Actions → "Sync Docs to Hyperswitch Docs"
2. Click "Run workflow"
3. Select branch: `main`
4. Click "Run workflow"

## Related Files

- Workflow: `.github/workflows/docs-sync.yml`
- GitBook config: `docs/gitbook.yml`
- This guide: `.github/DOCUMENTATION_SYNC.md`
