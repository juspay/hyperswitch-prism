#!/usr/bin/env bash
# scripts/setup-connector-tests.sh
#
# One-time (idempotent) setup for the connector integration test suite.
#
# What it does:
#   1. Checks that Node ≥18 and npm are available
#   2. Runs `npm install` inside browser-automation-engine/ (skipped if up-to-date)
#   3. Installs Playwright browser binaries (chromium + webkit) if not present
#   4. Checks/installs grpcurl for gRPC backend testing
#   5. Auto-installs Netlify CLI locally if not already available (optional)
#   6. Deploys the GPay/APay token-generator pages to Netlify and writes
#      GPAY_HOSTED_URL to .env.connector-tests (skipped if already deployed)
#   7. Verifies credentials file is present (creds.json)
#   8. Installs test-prism launcher to PATH
#
# Re-running this script is safe — every step checks whether work is needed
# before doing it.
#
# Environment variables (all optional):
#   CONNECTOR_AUTH_FILE_PATH  Path to creds.json (overrides repo default)
#   GPAY_HOSTED_URL           Skip Netlify deploy if already set
#   SKIP_NETLIFY_DEPLOY       Set to 1 to skip the Netlify deploy step (disables Google Pay tests)
#   NETLIFY_AUTH_TOKEN        Required for unattended Netlify deploys (CI/CD environments)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
BAE_DIR="${REPO_ROOT}/browser-automation-engine"
ENV_FILE="${REPO_ROOT}/.env.connector-tests"
DEFAULT_CREDS="${REPO_ROOT}/creds.json"
UCS_CONFIG_DIR="${HOME}/.config/integration-tests"
SETUP_SENTINEL="${UCS_CONFIG_DIR}/setup.done"

# ── Colours ────────────────────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Colour

info()    { echo -e "${BLUE}[setup]${NC} $*"; }
success() { echo -e "${GREEN}[setup]${NC} $*"; }
warn()    { echo -e "${YELLOW}[setup]${NC} $*"; }
error()   { echo -e "${RED}[setup]${NC} $*" >&2; }

# ── Step 1: Check Node / npm ───────────────────────────────────────────────────
info "Checking Node.js and npm..."

if ! command -v node &>/dev/null; then
  error "Node.js not found. Install Node ≥18 from https://nodejs.org and re-run."
  exit 1
fi

NODE_VERSION=$(node --version | sed 's/v//')
NODE_MAJOR=$(echo "${NODE_VERSION}" | cut -d. -f1)
if [[ "${NODE_MAJOR}" -lt 18 ]]; then
  error "Node ${NODE_VERSION} is too old. Node ≥18 is required."
  exit 1
fi
success "Node ${NODE_VERSION} OK"

if ! command -v npm &>/dev/null; then
  error "npm not found. It should come with Node — please reinstall Node."
  exit 1
fi
success "npm $(npm --version) OK"

# ── Step 2: npm install ────────────────────────────────────────────────────────
info "Installing browser-automation-engine dependencies..."

LOCK_FILE="${BAE_DIR}/package-lock.json"
NODE_MODULES="${BAE_DIR}/node_modules"

# Check whether node_modules is up-to-date with package-lock.json
needs_install=true
if [[ -d "${NODE_MODULES}" && -f "${LOCK_FILE}" ]]; then
  # node_modules newer than package-lock.json → already installed
  if [[ "${NODE_MODULES}" -nt "${LOCK_FILE}" ]]; then
    needs_install=false
  fi
fi

if "${needs_install}"; then
  (cd "${BAE_DIR}" && npm install 2>&1)
  success "npm install complete"
else
  success "node_modules up-to-date, skipping npm install"
fi

# ── Step 3: Install Playwright browsers ───────────────────────────────────────
info "Checking Playwright browser binaries..."

# Use a sentinel file to avoid re-installing on every run
PLAYWRIGHT_SENTINEL="${NODE_MODULES}/.playwright-browsers-installed"

if [[ ! -f "${PLAYWRIGHT_SENTINEL}" ]]; then
  info "Installing Playwright browsers (chromium + webkit)..."
  (cd "${BAE_DIR}" && npm run install:browsers 2>&1)
  touch "${PLAYWRIGHT_SENTINEL}"
  success "Playwright browsers installed"
else
  success "Playwright browsers already installed"
fi

# ── Step 3.5: Check/Install grpcurl ───────────────────────────────────────────
info "Checking grpcurl..."

if command -v grpcurl &>/dev/null; then
  success "grpcurl already installed ($(grpcurl --version 2>&1 | head -1))"
else
  warn "grpcurl not found — attempting to install..."

  # Detect platform
  OS="$(uname -s)"
  ARCH="$(uname -m)"

  case "${OS}" in
    Darwin)
      if command -v brew &>/dev/null; then
        info "Installing grpcurl via Homebrew..."
        brew install grpcurl && success "grpcurl installed via Homebrew"
      else
        warn "Homebrew not found. Please install grpcurl manually:"
        warn "  brew install grpcurl"
        warn "  OR download from: https://github.com/fullstorydev/grpcurl/releases"
        warn ""
        warn "grpcurl is required for gRPC backend testing."
      fi
      ;;
    Linux)
      info "Installing grpcurl from GitHub releases..."
      GRPCURL_VERSION="1.9.1"
      DOWNLOAD_URL="https://github.com/fullstorydev/grpcurl/releases/download/v${GRPCURL_VERSION}/grpcurl_${GRPCURL_VERSION}_linux_x86_64.tar.gz"

      TEMP_DIR=$(mktemp -d)
      if curl -L -o "${TEMP_DIR}/grpcurl.tar.gz" "${DOWNLOAD_URL}" 2>&1; then
        tar -xzf "${TEMP_DIR}/grpcurl.tar.gz" -C "${TEMP_DIR}"

        # Try to install to /usr/local/bin or ~/bin
        if [[ -w "/usr/local/bin" ]]; then
          mv "${TEMP_DIR}/grpcurl" /usr/local/bin/
          success "grpcurl installed to /usr/local/bin/grpcurl"
        elif mkdir -p "${HOME}/bin" 2>/dev/null; then
          mv "${TEMP_DIR}/grpcurl" "${HOME}/bin/"
          success "grpcurl installed to ~/bin/grpcurl"

          # Check if ~/bin is in PATH
          if [[ ":${PATH}:" != *":${HOME}/bin:"* ]]; then
            warn "~/bin is not in your PATH. Add it to your shell profile:"
            warn "  echo 'export PATH=\"\${HOME}/bin:\${PATH}\"' >> ~/.bashrc"
            warn "  source ~/.bashrc"
          fi
        else
          warn "Could not install grpcurl to system path."
          warn "Binary available at: ${TEMP_DIR}/grpcurl"
          warn "Move it manually: sudo mv ${TEMP_DIR}/grpcurl /usr/local/bin/"
        fi
      else
        warn "Failed to download grpcurl. Please install manually:"
        warn "  https://github.com/fullstorydev/grpcurl/releases"
      fi
      rm -rf "${TEMP_DIR}"
      ;;
    *)
      warn "Unsupported OS: ${OS}. Please install grpcurl manually:"
      warn "  https://github.com/fullstorydev/grpcurl/releases"
      warn ""
      warn "grpcurl is required for gRPC backend testing."
      ;;
  esac
fi

# ── Step 4: Install Netlify CLI (if needed) ───────────────────────────────────
info "Checking Netlify CLI..."

# Check if Netlify CLI is already installed globally or can run via npx
NETLIFY_AVAILABLE=false
if command -v netlify &>/dev/null; then
  NETLIFY_AVAILABLE=true
  success "Netlify CLI already installed (global)"
elif (cd "${BAE_DIR}" && npx --no -- netlify --version &>/dev/null 2>&1); then
  NETLIFY_AVAILABLE=true
  success "Netlify CLI available via npx"
fi

# If not available and user hasn't explicitly skipped, offer to install
if [[ "${NETLIFY_AVAILABLE}" == "false" && "${SKIP_NETLIFY_DEPLOY:-0}" != "1" ]]; then
  echo ""
  warn "Netlify CLI is not installed."
  echo ""
  echo "  Netlify CLI is used to deploy Google Pay token generator pages."
  echo "  This enables Google Pay payment testing."
  echo ""
  echo "  Options:"
  echo "    1) Install globally (recommended):  npm install -g netlify-cli"
  echo "    2) Install locally in project:      (auto-installed below)"
  echo "    3) Skip (Google Pay tests disabled): export SKIP_NETLIFY_DEPLOY=1"
  echo ""

  # Auto-install locally in the project for convenience
  info "Installing Netlify CLI locally in browser-automation-engine..."
  if (cd "${BAE_DIR}" && npm install --save-dev netlify-cli 2>&1); then
    success "Netlify CLI installed locally"
    # Add to package.json scripts for easy access
    info "You can now use: cd browser-automation-engine && npx netlify"
  else
    warn "Failed to install Netlify CLI locally"
    warn "Google Pay tests will be skipped unless you install manually:"
    warn "  npm install -g netlify-cli"
  fi
fi

# ── Step 5: Netlify deploy (GPAY_HOSTED_URL) ──────────────────────────────────
# Source .env.connector-tests if it exists so we pick up a previously saved URL
if [[ -f "${ENV_FILE}" ]]; then
  # shellcheck disable=SC1090
  source "${ENV_FILE}" || true
fi

SKIP_NETLIFY="${SKIP_NETLIFY_DEPLOY:-0}"

if [[ -n "${GPAY_HOSTED_URL:-}" ]]; then
  success "GPAY_HOSTED_URL already set: ${GPAY_HOSTED_URL}"
  SKIP_NETLIFY=1
fi

# ── Pre-flight: obtain a Netlify auth token if not already set ────────────────
# Without a token the Netlify CLI blocks waiting for browser-based OAuth login,
# causing the script to hang indefinitely.
#
# We use the Netlify CLI ticket-based auth flow:
#   1. netlify login --request  → generates a one-time URL + ticket ID
#   2. User opens the URL in a browser and clicks "Authorize" (no sign-up form,
#      just one click if already logged in to netlify.com)
#   3. We poll netlify login --check <ticket-id> until the token arrives
#   4. Token is saved to NETLIFY_AUTH_TOKEN for the rest of this script
#
# The user never has to manually copy/paste a token.
if [[ "${SKIP_NETLIFY}" != "1" && -z "${NETLIFY_AUTH_TOKEN:-}" ]]; then

  # Resolve netlify command early so we can use it for auth
  NETLIFY_CMD_AUTH=""
  if command -v netlify &>/dev/null; then
    NETLIFY_CMD_AUTH="netlify"
  elif [[ -f "${BAE_DIR}/node_modules/.bin/netlify" ]]; then
    NETLIFY_CMD_AUTH="${BAE_DIR}/node_modules/.bin/netlify"
  else
    NETLIFY_CMD_AUTH="npx --no -- netlify"
  fi

  echo ""
  info "Netlify login required for Google Pay test setup."
  echo ""
  echo "  This is a one-time step. No account needed if you already have one."
  echo "  If you don't have a Netlify account, sign up free at:"
  echo "    https://app.netlify.com/signup"
  echo ""
  echo "  Press Enter to open the authorization URL, or type 's' to skip"
  echo "  Google Pay tests and continue:"
  echo ""
  printf "  > "
  read -r USER_INPUT </dev/tty

  if [[ "${USER_INPUT}" == "s" || "${USER_INPUT}" == "S" ]]; then
    warn "Skipping Netlify deploy — Google Pay tests will be disabled."
    SKIP_NETLIFY=1
  else
    # Generate a login ticket
    TICKET_JSON=$(cd "${BAE_DIR}" && ${NETLIFY_CMD_AUTH} login --request "integration-tests" --json 2>/dev/null || true)
    TICKET_ID=$(echo "${TICKET_JSON}" | grep -o '"ticket_id": *"[^"]*"' | sed 's/"ticket_id": *"//;s/"//' || true)
    AUTH_URL=$(echo "${TICKET_JSON}"  | grep -o '"url": *"[^"]*"'       | sed 's/"url": *"//;s/"//' || true)

    if [[ -z "${TICKET_ID}" || -z "${AUTH_URL}" ]]; then
      warn "Could not generate a Netlify login ticket."
      warn "Skipping Netlify deploy — Google Pay tests will be disabled."
      warn "To retry, re-run: make setup-connector-tests"
      SKIP_NETLIFY=1
    else
      echo ""
      info "Open this URL in your browser to authorize (one click):"
      echo ""
      echo "    ${AUTH_URL}"
      echo ""
      info "Waiting for authorization..."

      # Poll until the user authorizes (up to 5 minutes).
      # NOTE: netlify login --check returns {"status":"authorized","user":{...}}
      # after the user clicks Authorize — it does NOT include the token in the
      # JSON response.  The CLI writes the token to ~/.config/netlify/config.json
      # internally.  We detect "authorized" status, then read the token from there.
      POLL_TOKEN=""
      POLL_DEADLINE=$(( $(date +%s) + 300 ))
      while [[ $(date +%s) -lt ${POLL_DEADLINE} ]]; do
        POLL_RESULT=$(cd "${BAE_DIR}" && ${NETLIFY_CMD_AUTH} login --check "${TICKET_ID}" --json 2>/dev/null || true)
        POLL_STATUS=$(echo "${POLL_RESULT}" | grep -o '"status": *"[^"]*"' | sed 's/"status": *"//;s/"//' || true)

        if [[ "${POLL_STATUS}" == "authorized" ]]; then
          # Read the token from the netlify config file that the CLI just wrote.
          # The Netlify CLI uses env-paths which varies by OS:
          #   macOS:  ~/Library/Preferences/netlify/config.json
          #   Linux:  ${XDG_CONFIG_HOME:-~/.config}/netlify/config.json
          case "$(uname -s)" in
            Darwin*) NETLIFY_CONFIG="${HOME}/Library/Preferences/netlify/config.json" ;;
            *)       NETLIFY_CONFIG="${XDG_CONFIG_HOME:-${HOME}/.config}/netlify/config.json" ;;
          esac
          if [[ -f "${NETLIFY_CONFIG}" ]]; then
            POLL_TOKEN=$(python3 -c "
import json, sys
try:
  cfg = json.load(open('${NETLIFY_CONFIG}'))
  for uid, udata in cfg.get('users', {}).items():
    tok = udata.get('auth', {}).get('token', '')
    if tok:
      print(tok)
      sys.exit(0)
except Exception:
  pass
" 2>/dev/null || true)
          fi
          break
        fi

        if [[ -n "${POLL_STATUS}" && "${POLL_STATUS}" != "pending" ]]; then
          # denied or unknown — stop polling
          break
        fi

        printf "."
        sleep 3
      done
      echo ""

      if [[ -n "${POLL_TOKEN}" ]]; then
        export NETLIFY_AUTH_TOKEN="${POLL_TOKEN}"
        success "Netlify authorization successful."

        # Persist token to .env.connector-tests so future runs skip this step
        if [[ -f "${ENV_FILE}" ]]; then
          # Remove any existing token line before appending
          grep -v 'NETLIFY_AUTH_TOKEN' "${ENV_FILE}" > "${ENV_FILE}.tmp" && mv "${ENV_FILE}.tmp" "${ENV_FILE}"
        fi
        echo "export NETLIFY_AUTH_TOKEN=\"${NETLIFY_AUTH_TOKEN}\"" >> "${ENV_FILE}"
        success "Token saved to ${ENV_FILE} — future runs will skip this step."
      else
        warn "Authorization timed out or was not completed."
        warn "Skipping Netlify deploy — Google Pay tests will be disabled."
        warn "To retry, re-run: make setup-connector-tests"
        SKIP_NETLIFY=1
      fi
    fi
  fi
fi

if [[ "${SKIP_NETLIFY}" != "1" ]]; then
  info "Deploying GPay token-generator pages to Netlify..."

  # Determine which netlify command to use (global, local, or npx)
  NETLIFY_CMD=""
  if command -v netlify &>/dev/null; then
    NETLIFY_CMD="netlify"
    info "Using global Netlify CLI"
  elif [[ -f "${BAE_DIR}/node_modules/.bin/netlify" ]]; then
    NETLIFY_CMD="${BAE_DIR}/node_modules/.bin/netlify"
    info "Using locally installed Netlify CLI"
  elif (cd "${BAE_DIR}" && npx --no -- netlify --version &>/dev/null 2>&1); then
    NETLIFY_CMD="npx --no -- netlify"
    info "Using Netlify CLI via npx"
  else
    warn "Netlify CLI not found after installation attempt."
    warn "Manual workaround:"
    warn "  1) Install globally: npm install -g netlify-cli"
    warn "  2) Re-run setup"
    warn ""
    warn "Skipping Netlify deploy — Google Pay tests will be skipped at runtime."
    SKIP_NETLIFY=1
  fi
fi

if [[ "${SKIP_NETLIFY}" != "1" && -n "${NETLIFY_CMD}" ]]; then

  NETLIFY_STATE="${BAE_DIR}/.netlify/state.json"

  # ── Auto-create a Netlify site on first run if not already linked ────────────
  if [[ ! -f "${NETLIFY_STATE}" ]]; then
    info "No linked Netlify site found — creating one automatically..."

    # Generate a stable site name from the machine hostname + repo name
    SITE_SLUG="ucs-gpay-$(hostname -s | tr '[:upper:]' '[:lower:]' | tr -cs 'a-z0-9' '-' | sed 's/-*$//')-$(date +%s)"

    # Discover the user's Netlify account slug so sites:create doesn't prompt
    # interactively.  The API returns a JSON array; we pick the first account.
    ACCOUNT_SLUG=$(cd "${BAE_DIR}" && ${NETLIFY_CMD} api listAccountsForUser \
      --auth "${NETLIFY_AUTH_TOKEN}" 2>/dev/null \
      | python3 -c "import json,sys; print(json.load(sys.stdin)[0]['slug'])" 2>/dev/null || true)

    # Note: --json is not supported by all Netlify CLI versions for sites:create,
    # so we parse the plain-text output instead.  --disable-linking prevents the
    # CLI from writing its own .netlify/state.json in a potentially wrong location.
    CREATE_OUTPUT=$(cd "${BAE_DIR}" && NETLIFY_AUTH_TOKEN="${NETLIFY_AUTH_TOKEN}" \
      ${NETLIFY_CMD} sites:create \
        --name "${SITE_SLUG}" \
        --disable-linking \
        --auth "${NETLIFY_AUTH_TOKEN}" \
        ${ACCOUNT_SLUG:+--account-slug "${ACCOUNT_SLUG}"} \
        2>&1) || {
      warn "Could not create Netlify site automatically."
      warn "Create one manually at https://app.netlify.com and then link it:"
      warn "  cd browser-automation-engine && netlify link"
      warn ""
      warn "Skipping Netlify deploy — Google Pay tests will be skipped at runtime."
      SKIP_NETLIFY=1
    }

    if [[ "${SKIP_NETLIFY}" != "1" ]]; then
      # Plain-text output contains a line like:  "Project ID: <uuid>"
      SITE_ID=$(echo "${CREATE_OUTPUT}" | grep -Eo '[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}' | head -1 || true)
      if [[ -z "${SITE_ID}" ]]; then
        warn "Could not extract site ID from create output. Raw output:"
        echo "${CREATE_OUTPUT}" | sed 's/^/    /'
        warn "Skipping Netlify deploy — Google Pay tests will be skipped at runtime."
        SKIP_NETLIFY=1
      else
        mkdir -p "${BAE_DIR}/.netlify"
        echo "{\"siteId\": \"${SITE_ID}\"}" > "${NETLIFY_STATE}"
        success "Netlify site created: ${SITE_SLUG} (${SITE_ID})"
      fi
    fi
  fi
fi

if [[ "${SKIP_NETLIFY}" != "1" && -n "${NETLIFY_CMD}" ]]; then

  info "Running: netlify deploy --prod (in browser-automation-engine/)"
  DEPLOY_OUTPUT=$(cd "${BAE_DIR}" && NETLIFY_AUTH_TOKEN="${NETLIFY_AUTH_TOKEN}" \
    ${NETLIFY_CMD} deploy --prod --auth "${NETLIFY_AUTH_TOKEN}" 2>&1) || {
    warn "Netlify deploy failed. Google Pay tests will be skipped at runtime."
    warn "To fix, ensure NETLIFY_AUTH_TOKEN is valid and re-run setup."
    warn "Raw output:"
    echo "${DEPLOY_OUTPUT}" | sed 's/^/    /'
    SKIP_NETLIFY=1
  }

  if [[ "${SKIP_NETLIFY}" != "1" ]]; then
    # Extract the deployed URL from netlify output
    # Netlify prints lines like: "Website URL:  https://xxxx.netlify.app"
    DEPLOYED_URL=$(echo "${DEPLOY_OUTPUT}" | grep -Eo 'https://[a-zA-Z0-9._-]+\.netlify\.app' | head -1 || true)
    if [[ -z "${DEPLOYED_URL}" ]]; then
      warn "Could not extract Netlify URL from deploy output."
      warn "Set GPAY_HOSTED_URL manually in .env.connector-tests:"
      warn "  export GPAY_HOSTED_URL=https://<your-site>.netlify.app/gpay/gpay-token-gen.html"
    else
      GPAY_HOSTED_URL="${DEPLOYED_URL}/gpay/gpay-token-gen.html"
      success "Netlify deploy successful: ${GPAY_HOSTED_URL}"

      # Persist to .env.connector-tests
      {
        echo "# Auto-generated by scripts/setup-connector-tests.sh"
        echo "# Re-run 'make setup-connector-tests' to refresh"
        echo "export GPAY_HOSTED_URL=\"${GPAY_HOSTED_URL}\""
      } > "${ENV_FILE}"
      success "Saved GPAY_HOSTED_URL to ${ENV_FILE}"
    fi
  fi
fi

# ── Step 6: Verify credentials ─────────────────────────────────────────────────
info "Checking credentials file..."

CREDS_PATH="${CONNECTOR_AUTH_FILE_PATH:-${UCS_CREDS_PATH:-${DEFAULT_CREDS}}}"

if [[ -f "${CREDS_PATH}" ]]; then
  success "Credentials found: ${CREDS_PATH}"
else
  warn "Credentials file not found at: ${CREDS_PATH}"
  warn ""
  warn "Create it at creds.json in the repo root, or set one of:"
  warn "  export CONNECTOR_AUTH_FILE_PATH=/path/to/creds.json"
  warn "  export UCS_CREDS_PATH=/path/to/creds.json"
  warn ""
  warn "Connector tests that require credentials will be skipped."
fi

# ── Step 7: Install test-prism launcher ───────────────────────────────────────
info "Installing test-prism command..."

LAUNCHER_NAME="test-prism"
LAUNCHER_TARGET="${SCRIPT_DIR}/run-tests"

# Candidate directories — only consider those already on PATH
install_dir=""
candidates=(
  "/usr/local/bin"
  "/opt/homebrew/bin"
  "${HOME}/.local/bin"
  "${HOME}/bin"
)

for candidate in "${candidates[@]}"; do
  # Check if this candidate is on the user's PATH
  if [[ ":${PATH}:" == *":${candidate}:"* ]]; then
    # Create the directory if it doesn't exist (only for user-owned dirs)
    if [[ ! -d "${candidate}" && "${candidate}" == "${HOME}"* ]]; then
      mkdir -p "${candidate}" 2>/dev/null || continue
    fi
    # Check writable
    if [[ -w "${candidate}" ]]; then
      install_dir="${candidate}"
      break
    fi
  fi
done

if [[ -z "${install_dir}" ]]; then
  warn "Could not find a writable directory on your PATH to install ${LAUNCHER_NAME}."
  warn "Checked: ${candidates[*]}"
  warn ""
  warn "To install manually, run one of these after adding a bin dir to your PATH:"
  warn "  sudo cp \"${LAUNCHER_TARGET}\" /usr/local/bin/${LAUNCHER_NAME} && sudo chmod +x /usr/local/bin/${LAUNCHER_NAME}"
  warn "  -- or --"
  warn "  mkdir -p ~/.local/bin && cp \"${LAUNCHER_TARGET}\" ~/.local/bin/${LAUNCHER_NAME} && chmod +x ~/.local/bin/${LAUNCHER_NAME}"
  warn "  (then add ~/.local/bin to your PATH and re-run setup)"
else
  INSTALL_PATH="${install_dir}/${LAUNCHER_NAME}"

  # Write a small launcher that execs the repo script so the repo can be updated
  # independently of the installed command.
  cat > "${INSTALL_PATH}" <<LAUNCHER
#!/usr/bin/env bash
# Auto-generated by scripts/setup-connector-tests.sh — do not edit manually.
# Re-run setup to refresh this launcher after moving the repo.
exec "${LAUNCHER_TARGET}" "\$@"
LAUNCHER
  chmod +x "${INSTALL_PATH}"
  success "Installed ${LAUNCHER_NAME} → ${INSTALL_PATH}"
  success "You can now run:  test-prism"
fi

# ── Step 8: Google Pay session setup ──────────────────────────────────────────
GPAY_PROFILE_DIR="${BAE_DIR}/gpay/.webkit-profile"
GPAY_STORAGE_STATE="${GPAY_PROFILE_DIR}/storage-state.json"

if [[ -f "${GPAY_STORAGE_STATE}" ]]; then
  success "Google Pay session found: ${GPAY_STORAGE_STATE}"
else
  info "Google Pay requires a one-time Google sign-in."
  echo ""
  echo "  This opens a WebKit browser where you sign in to your Google account."
  echo "  The session is saved locally and reused for all future GPay test runs."
  echo ""

  # Only prompt if stdin is a terminal (interactive mode)
  if [[ -t 0 ]]; then
    read -rp "  Sign in to Google now? [Y/n] " gpay_login_answer
    gpay_login_answer="${gpay_login_answer:-Y}"
    if [[ "${gpay_login_answer}" =~ ^[Yy] ]]; then
      info "Launching Google sign-in browser..."
      (cd "${BAE_DIR}" && npm run gpay:login 2>&1) || {
        warn "Google sign-in failed or was cancelled."
        warn "You can run it later:  cd browser-automation-engine && npm run gpay:login"
      }
      if [[ -f "${GPAY_STORAGE_STATE}" ]]; then
        success "Google Pay session saved successfully."
      fi
    else
      warn "Skipped Google sign-in."
      warn "Run later:  cd browser-automation-engine && npm run gpay:login"
    fi
  else
    warn "Non-interactive terminal — skipping Google sign-in."
    warn "Run manually:  cd browser-automation-engine && npm run gpay:login"
  fi
fi

# ── Done ───────────────────────────────────────────────────────────────────────
# Write setup sentinel so test-prism knows setup has been completed.
mkdir -p "${UCS_CONFIG_DIR}"
echo "{\"repo\": \"${REPO_ROOT}\", \"completed_at\": \"$(date -u +%Y-%m-%dT%H:%M:%SZ)\"}" \
  > "${SETUP_SENTINEL}"

echo ""
success "Setup complete."
echo ""
echo "  To run all connector tests:                test-prism"
echo "  To run tests for a specific connector:     test-prism --connector stripe"
echo "  To run a specific scenario:                test-prism --connector stripe --suite authorize --scenario no3ds_auto_capture_credit_card"
echo "  Interactive wizard:                        test-prism --interactive"
echo "  Full usage:                                test-prism --help"
echo ""

if [[ -n "${GPAY_HOSTED_URL:-}" ]]; then
  echo "  Google Pay hosted URL: ENABLED (${GPAY_HOSTED_URL})"
else
  echo "  Google Pay hosted URL: DISABLED (GPAY_HOSTED_URL not set)"
  echo "  To enable: set GPAY_HOSTED_URL or allow Netlify deploy during setup"
fi

if [[ -f "${GPAY_STORAGE_STATE}" ]]; then
  echo "  Google Pay session:    SAVED (${GPAY_STORAGE_STATE})"
else
  echo "  Google Pay session:    NOT SET — run: cd browser-automation-engine && npm run gpay:login"
fi
echo ""
