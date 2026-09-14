#!/usr/bin/env bash
# 10XGRACE one-command setup.
#
# Usage:
#   pnpm bootstrap                   interactive: probe -> install -> build -> .env prompts -> optional pnpm dev
#   pnpm bootstrap -- --yes          non-interactive: same path, skips all prompts (accepts defaults)
#   pnpm check                       read-only: probe + .env validation, NO install/build/changes
#
# (`pnpm setup` and `pnpm doctor` are reserved by pnpm itself — that's why we
#  use `bootstrap` and `check` instead. Both delegate to this script.)
#
# Exit codes:
#   0 = all good
#   1 = a required prereq is missing OR a step failed
set -u

# ─── tty colours ─────────────────────────────────────────────────────────────
if [ -t 1 ]; then
  C_RESET=$'\033[0m'; C_DIM=$'\033[90m'; C_BOLD=$'\033[1m'
  C_OK=$'\033[32m'; C_WARN=$'\033[33m'; C_ERR=$'\033[31m'; C_INFO=$'\033[36m'
else
  C_RESET=''; C_DIM=''; C_BOLD=''; C_OK=''; C_WARN=''; C_ERR=''; C_INFO=''
fi

YES=0; DOCTOR=0
for arg in "$@"; do
  case "$arg" in
    -y|--yes) YES=1 ;;
    --doctor) DOCTOR=1 ;;
    -h|--help)
      sed -n '2,12p' "$0"
      exit 0
      ;;
    --) : ;;  # pnpm sometimes inserts the arg separator; silently swallow
    *) echo "${C_WARN}unknown arg: $arg${C_RESET}" >&2 ;;
  esac
done

WARNINGS=0
ERRORS=0

note()  { printf "%s\n" "$1"; }
ok()    { printf "  ${C_OK}✓${C_RESET} %s\n" "$1"; }
warn()  { printf "  ${C_WARN}⚠${C_RESET} %s\n" "$1"; WARNINGS=$((WARNINGS + 1)); }
err()   { printf "  ${C_ERR}✗${C_RESET} %s\n" "$1"; ERRORS=$((ERRORS + 1)); }
hint()  { printf "    ${C_DIM}→ %s${C_RESET}\n" "$1"; }

# ─── locate the workspace root ───────────────────────────────────────────────
# Mirrors findWorkspaceRoot() in packages/core/src/utils.ts: walk up looking
# for pnpm-workspace.yaml. Lets the user run this script from anywhere.
locate_workspace() {
  local dir
  dir="$(pwd)"
  while [ "$dir" != "/" ]; do
    if [ -f "$dir/pnpm-workspace.yaml" ]; then
      printf "%s" "$dir"
      return 0
    fi
    dir="$(dirname "$dir")"
  done
  return 1
}

WORKSPACE_ROOT="$(locate_workspace)" || {
  printf "${C_ERR}error:${C_RESET} pnpm-workspace.yaml not found in any parent of %s\n" "$(pwd)" >&2
  printf "       cd into grace-workspace/ (or any subdirectory) and re-run.\n" >&2
  exit 1
}
cd "$WORKSPACE_ROOT"

# ─── banner ──────────────────────────────────────────────────────────────────
printf "\n${C_BOLD}10XGRACE setup${C_RESET} · ${C_DIM}%s${C_RESET}\n" "$WORKSPACE_ROOT"
if [ "$DOCTOR" -eq 1 ]; then
  printf "${C_DIM}(doctor mode — read-only, no install or build)${C_RESET}\n"
fi
echo

# ─── OS detection (for hints only) ───────────────────────────────────────────
case "$(uname -s)" in
  Darwin) OS="macos" ;;
  Linux)  OS="linux" ;;
  MINGW*|MSYS*|CYGWIN*) OS="windows" ;;
  *) OS="other" ;;
esac

install_hint_for() {
  # $1 = tool name. Prints one OS-appropriate install line.
  case "$1:$OS" in
    node:macos)     hint "brew install node@20" ;;
    node:linux)     hint "nvm install 20  (or your distro's package manager)" ;;
    node:windows)   hint "https://nodejs.org/  (LTS)" ;;
    pnpm:*)         hint "corepack enable && corepack prepare pnpm@latest --activate" ;;
    git:macos)      hint "xcode-select --install" ;;
    git:linux)      hint "sudo apt install git  (or distro equivalent)" ;;
    sqlite3:macos)  hint "brew install sqlite" ;;
    sqlite3:linux)  hint "sudo apt install sqlite3" ;;
    claude:*)       hint "https://docs.anthropic.com/claude/docs/claude-code  (then run: claude /login)" ;;
    gh:macos)       hint "brew install gh  (then: gh auth login)" ;;
    gh:linux)       hint "https://cli.github.com/  (then: gh auth login)" ;;
    opencode:*)     hint "https://opencode.ai/  (only needed if llm.runner is 'opencode' in config.yml)" ;;
    cargo:macos)    hint "https://rustup.rs/  (only needed when running the compiler/grpc_test checkpoints)" ;;
    cargo:linux)    hint "https://rustup.rs/  (only needed when running the compiler/grpc_test checkpoints)" ;;
    *)              hint "see the project README" ;;
  esac
}

# ─── version helpers ────────────────────────────────────────────────────────
node_major() {
  node -v 2>/dev/null | sed -E 's/^v([0-9]+)\..*/\1/'
}
pnpm_major() {
  pnpm -v 2>/dev/null | sed -E 's/^([0-9]+)\..*/\1/'
}

# ─── prereq probe ────────────────────────────────────────────────────────────
note "${C_BOLD}Prerequisites${C_RESET}"

# Required: Node 20+
if command -v node >/dev/null 2>&1; then
  v=$(node_major)
  if [ -n "$v" ] && [ "$v" -ge 20 ]; then
    ok "node v$(node -v | tr -d v) (>= 20)"
  else
    err "node $(node -v) is older than 20"
    install_hint_for node
  fi
else
  err "node not found"
  install_hint_for node
fi

# Required: pnpm 9+. Try to auto-install via corepack if missing.
#
# Corepack rescue is split into two non-fatal commands: `enable` may fail with
# EEXIST on stale shims (e.g. /usr/local/bin/pnpx left from a prior
# `npm install -g pnpm`), but `prepare --activate` can still install pnpm
# independently. We tolerate either command's failure and re-check via
# `command -v pnpm` to determine actual outcome.
if command -v pnpm >/dev/null 2>&1; then
  v=$(pnpm_major)
  if [ -n "$v" ] && [ "$v" -ge 9 ]; then
    ok "pnpm $(pnpm -v) (>= 9)"
  else
    warn "pnpm $(pnpm -v) is older than 9 — attempting upgrade via corepack"
    if [ "$DOCTOR" -eq 0 ] && command -v corepack >/dev/null 2>&1; then
      corepack enable >/dev/null 2>&1 || true
      corepack prepare pnpm@latest --activate >/dev/null 2>&1 || true
      new_v=$(pnpm_major)
      if [ -n "$new_v" ] && [ "$new_v" -ge 9 ]; then
        ok "pnpm upgraded to $(pnpm -v) via corepack"
      else
        err "corepack upgrade failed — run 'corepack prepare pnpm@latest --activate' manually to see the error"
        install_hint_for pnpm
      fi
    else
      install_hint_for pnpm
    fi
  fi
else
  err "pnpm not found"
  if [ "$DOCTOR" -eq 0 ] && command -v corepack >/dev/null 2>&1; then
    note "    attempting corepack auto-install…"
    corepack enable >/dev/null 2>&1 || true
    corepack prepare pnpm@latest --activate >/dev/null 2>&1 || true
    if command -v pnpm >/dev/null 2>&1; then
      ERRORS=$((ERRORS - 1))  # un-count the err above; corepack rescued us
      ok "pnpm $(pnpm -v) (installed via corepack)"
    else
      warn "corepack auto-install did not produce a working pnpm. Run 'corepack prepare pnpm@latest --activate' manually and check the output."
      install_hint_for pnpm
    fi
  else
    install_hint_for pnpm
  fi
fi

# Required: git
if command -v git >/dev/null 2>&1; then
  ok "git $(git --version | awk '{print $3}')"
else
  err "git not found"
  install_hint_for git
fi

# Optional: sqlite3 CLI is *not* required for grace to run. Storage goes
# through better-sqlite3 (npm pkg with bundled native binding at
# node_modules/.pnpm/better-sqlite3@*/.../better_sqlite3.node). Even
# `pnpm clear` only uses `rm -f`. The CLI is purely for ad-hoc human
# DB inspection: `sqlite3 ~/.tenxgrace/pipeline.sqlite '.schema'` etc.
if command -v sqlite3 >/dev/null 2>&1; then
  ok "sqlite3 $(sqlite3 --version | awk '{print $1}')  (CLI present — handy for ad-hoc DB inspection)"
else
  warn "sqlite3 CLI not found  (optional — grace's storage uses better-sqlite3's bundled binding; install the CLI only if you want to inspect ~/.tenxgrace/pipeline.sqlite by hand)"
  install_hint_for sqlite3
fi

# Strongly recommended: claude CLI
if command -v claude >/dev/null 2>&1; then
  ok "claude $(claude --version 2>/dev/null | head -1)"
else
  warn "claude CLI not found  (required for connector auto-discovery in the dashboard wizard and the claude-code runner)"
  install_hint_for claude
fi

# Optional: gh
if command -v gh >/dev/null 2>&1; then
  if gh auth status >/dev/null 2>&1; then
    ok "gh $(gh --version | head -1 | awk '{print $3}') (authenticated)"
  else
    warn "gh installed but not authenticated  (run: gh auth login)"
  fi
else
  warn "gh not found  (parity autopilot + PR resolver need it; otherwise optional)"
  install_hint_for gh
fi

# Optional: opencode
if command -v opencode >/dev/null 2>&1; then
  ok "opencode $(opencode --version 2>/dev/null | head -1 | awk '{print $NF}')"
else
  warn "opencode not found  (only required when config.yml sets runner: opencode)"
  install_hint_for opencode
fi

# Optional: cargo
if command -v cargo >/dev/null 2>&1; then
  ok "cargo $(cargo --version | awk '{print $2}')"
else
  warn "cargo not found  (compiler/grpc_test checkpoints will fail without it)"
  install_hint_for cargo
fi

echo

# ─── doctor mode stops here ──────────────────────────────────────────────────
if [ "$DOCTOR" -eq 1 ]; then
  note "${C_BOLD}.env status${C_RESET}"
  if [ -f .env ]; then
    ok ".env exists at $WORKSPACE_ROOT/.env"
    # Show which fields are set (presence-only, never the values)
    grep -E '^[A-Z_]+=' .env | while read -r line; do
      key="${line%%=*}"; val="${line#*=}"
      if [ -n "$val" ]; then
        printf "    ${C_DIM}%s: set${C_RESET}\n" "$key"
      else
        printf "    ${C_DIM}%s: blank${C_RESET}\n" "$key"
      fi
    done
  else
    warn ".env is missing — run \`pnpm bootstrap\` (without --doctor) to create it from .env.example"
  fi

  echo
  note "${C_BOLD}Runner status${C_RESET}"
  if [ -f config.yml ]; then
    runner=$(grep -E "^runner:" config.yml 2>/dev/null | awk '{print $2}')
    if [ -n "$runner" ]; then
      ok "runner=$runner (config.yml)"
    else
      warn "no 'runner:' line in config.yml (engine will default to opencode at runtime)"
      runner="opencode"
    fi
    # Runner CLI reachability
    if [ "$runner" = "claude-code" ]; then
      if command -v claude >/dev/null 2>&1; then
        ok "claude CLI $(claude --version 2>/dev/null | head -1) reachable"
      else
        err "claude CLI not found — runner=claude-code requires it"
      fi
      # Identify claude auth mode from current shell env
      if [ -n "${ANTHROPIC_BASE_URL:-}" ]; then
        ok "claude auth mode: LiteLLM gateway (ANTHROPIC_BASE_URL=$ANTHROPIC_BASE_URL${ANTHROPIC_MODEL:+, ANTHROPIC_MODEL=$ANTHROPIC_MODEL})"
        if [ -z "${ANTHROPIC_AUTH_TOKEN:-}" ]; then
          warn "ANTHROPIC_BASE_URL set but ANTHROPIC_AUTH_TOKEN blank — claude calls will 401"
        fi
        if [ -z "${ANTHROPIC_MODEL:-}" ]; then
          warn "ANTHROPIC_MODEL not set — claude CLI may use a hardcoded default not on your gateway"
        fi
      elif [ -n "${ANTHROPIC_API_KEY:-}" ]; then
        ok "claude auth mode: bare ANTHROPIC_API_KEY (routes to api.anthropic.com)"
      else
        # OAuth mode — probe ~/.claude/.credentials.json for login state.
        if [ -f "$HOME/.claude/.credentials.json" ]; then
          ok "claude auth mode: OAuth (~/.claude/.credentials.json present)"
        else
          warn "claude auth mode: assumed OAuth but ~/.claude/.credentials.json NOT found — run 'claude /login'"
        fi
      fi
    elif [ "$runner" = "opencode" ]; then
      if command -v opencode >/dev/null 2>&1; then
        ok "opencode CLI $(opencode --version 2>/dev/null | head -1 | awk '{print $NF}') installed"
      else
        err "opencode CLI not found — runner=opencode requires it"
      fi
      # Probe opencode auth state via its bundled creds file.
      if [ -f "$HOME/.local/share/opencode/auth.json" ] && [ -s "$HOME/.local/share/opencode/auth.json" ] && ! grep -q '^{}$' "$HOME/.local/share/opencode/auth.json" 2>/dev/null; then
        ok "opencode auth: ~/.local/share/opencode/auth.json has credentials"
      else
        warn "opencode auth: no providers configured — run 'opencode providers login' (alias: 'opencode auth login')"
      fi
      # Reachability check of opencode serve sidecar
      attach_url=$(grep -A5 "^opencode:" config.yml 2>/dev/null | grep -E "attachUrl:" | head -1 | sed -E 's/.*attachUrl:\s*"?([^"]+)"?.*/\1/')
      attach_url="${attach_url:-http://127.0.0.1:4096}"
      if curl -sf --max-time 2 "$attach_url" >/dev/null 2>&1; then
        ok "opencode sidecar reachable at $attach_url"
      else
        warn "opencode sidecar NOT reachable at $attach_url — run 'opencode serve' in a sidecar terminal"
      fi
    fi
    # Supplementary gateway consistency
    if [ -f .env ]; then
      gw_url=$(grep -E "^TENXGRACE_LLM_BASE_URL=" .env | head -1 | sed 's/[^=]*=//')
      gw_key=$(grep -E "^TENXGRACE_LLM_API_KEY=" .env | head -1 | sed 's/[^=]*=//')
      gw_model=$(grep -E "^TENXGRACE_LLM_MODEL=" .env | head -1 | sed 's/[^=]*=//')
      set_count=0
      [ -n "$gw_url" ] && set_count=$((set_count + 1))
      [ -n "$gw_key" ] && set_count=$((set_count + 1))
      [ -n "$gw_model" ] && set_count=$((set_count + 1))
      if [ "$set_count" -eq 0 ]; then
        ok "TENXGRACE_LLM_* all blank (no supplementary gateway configured)"
      elif [ "$set_count" -eq 3 ]; then
        ok "TENXGRACE_LLM_* all 3 set (supplementary gateway active)"
      else
        warn "TENXGRACE_LLM_* partially set ($set_count/3) — gateway calls will fail. Set all 3 or none."
      fi
    fi
  else
    warn "config.yml not found in workspace root — runner unknown"
  fi

  echo
  if [ "$ERRORS" -eq 0 ]; then
    printf "${C_OK}${C_BOLD}Check OK${C_RESET}  ·  %d warning(s)\n\n" "$WARNINGS"
    exit 0
  else
    printf "${C_ERR}${C_BOLD}Check found %d error(s)${C_RESET}  ·  %d warning(s)\n\n" "$ERRORS" "$WARNINGS"
    exit 1
  fi
fi

# ─── from here on: full setup mode ───────────────────────────────────────────
if [ "$ERRORS" -gt 0 ]; then
  printf "${C_ERR}${C_BOLD}Stopping:${C_RESET} %d required prereq(s) missing. Fix the items marked ✗ above and re-run.\n\n" "$ERRORS"
  exit 1
fi

# ─── pnpm install ────────────────────────────────────────────────────────────
echo
note "${C_BOLD}Installing dependencies${C_RESET}  (pnpm install)"
if pnpm install; then
  ok "dependencies installed"
else
  err "pnpm install failed"
  exit 1
fi

# ─── build ───────────────────────────────────────────────────────────────────
echo
note "${C_BOLD}Building packages${C_RESET}  (pnpm build)"
if pnpm build; then
  ok "all packages built"
else
  err "pnpm build failed"
  exit 1
fi

# ─── .env bootstrap ──────────────────────────────────────────────────────────
echo
note "${C_BOLD}Configuring .env${C_RESET}"
if [ -f .env ]; then
  ok ".env already exists — leaving as-is. Edit it by hand to change values."
else
  if [ ! -f .env.example ]; then
    err ".env.example missing — cannot bootstrap .env"
    exit 1
  fi
  cp .env.example .env
  ok "copied .env.example -> .env"

  if [ "$YES" -eq 1 ]; then
    warn "--yes mode: skipping prompts. Edit .env by hand to fill in TENXGRACE_LLM_API_KEY and other paths."
  else
    # Auto-detect TENXGRACE_PROJECT_ROOT as the workspace's grandparent
    # (canonical layout: hyperswitch-prism/grace/grace-workspace/).
    DETECTED_ROOT="$(cd "$WORKSPACE_ROOT/../.." && pwd)"
    printf "  Detected ${C_BOLD}TENXGRACE_PROJECT_ROOT${C_RESET} as %s\n" "$DETECTED_ROOT"
    printf "  Accept? [Y/n] "
    read -r ans
    if [ -z "$ans" ] || [ "$ans" = "y" ] || [ "$ans" = "Y" ]; then
      # Replace the placeholder line in .env. BSD/GNU sed compatible.
      tmp="$(mktemp)"
      sed -E "s|^TENXGRACE_PROJECT_ROOT=.*|TENXGRACE_PROJECT_ROOT=$DETECTED_ROOT|" .env > "$tmp" && mv "$tmp" .env
      ok "wrote TENXGRACE_PROJECT_ROOT=$DETECTED_ROOT"
    else
      printf "  Enter the absolute path to the target repo (blank to leave unset): "
      read -r custom_root
      # Always rewrite — blank input must clear the .env.example placeholder
      # (otherwise the user thinks "blank" but .env retains a fake path like
      # /absolute/path/to/hyperswitch-prism that downstream code would honour).
      tmp="$(mktemp)"
      sed -E "s|^TENXGRACE_PROJECT_ROOT=.*|TENXGRACE_PROJECT_ROOT=$custom_root|" .env > "$tmp" && mv "$tmp" .env
      if [ -n "$custom_root" ]; then
        ok "wrote TENXGRACE_PROJECT_ROOT=$custom_root"
      else
        ok "cleared TENXGRACE_PROJECT_ROOT (left blank)"
      fi
    fi

    # ─── Runner choice ────────────────────────────────────────────────
    # Determines which CLI drives tool-using checkpoints (L2/L3/impl/pr_review).
    # The runner choice is stored in config.yml (sed-edited below), NOT .env —
    # config.yml is the canonical source for runner: and the *Code/opencode blocks.
    echo
    note "${C_BOLD}Runner choice${C_RESET}"
    current_runner=$(grep -E "^runner:" config.yml 2>/dev/null | awk '{print $2}')
    if [ "$current_runner" = "opencode" ]; then
      default_runner="opencode"
    else
      default_runner="claude-code"
    fi
    printf "  Which runner drives tool-using checkpoints (L2/L3/impl/pr_review)?\n"
    printf "    [1] claude-code  — uses local 'claude' CLI (Anthropic OAuth, bare API key, or LiteLLM redirect)\n"
    printf "    [2] opencode     — uses local 'opencode serve' sidecar (opencode handles its own auth)\n"
    printf "  Choice [1/2, default=%s]: " "$default_runner"
    read -r runner_in
    case "$runner_in" in
      2|opencode) runner_choice="opencode" ;;
      1|claude-code) runner_choice="claude-code" ;;
      "") runner_choice="$default_runner" ;;
      *) warn "unknown choice '$runner_in' — keeping current '$default_runner'"; runner_choice="$default_runner" ;;
    esac
    if [ "$runner_choice" != "$current_runner" ]; then
      tmp="$(mktemp)"
      sed -E "s|^runner:.*|runner: $runner_choice|" config.yml > "$tmp" && mv "$tmp" config.yml
      ok "updated config.yml: runner: $runner_choice"
    else
      ok "config.yml runner already $runner_choice — no change"
    fi

    # ─── Per-runner auth detection + hints ────────────────────────────
    if [ "$runner_choice" = "claude-code" ]; then
      # The claude CLI inherits process.env from the supervisor → engine →
      # spawn('claude', ..., {env: process.env}). So whatever ANTHROPIC_*
      # vars exist in the user's shell at `pnpm dev` time are what claude
      # actually uses. Three auth modes are possible; identify which one
      # we're in by inspecting shell env right now.
      if [ -n "${ANTHROPIC_BASE_URL:-}" ]; then
        ok "Detected claude auth mode: LiteLLM gateway"
        printf "    ${C_DIM}ANTHROPIC_BASE_URL=%s${C_RESET}\n" "$ANTHROPIC_BASE_URL"
        [ -n "${ANTHROPIC_MODEL:-}" ] && printf "    ${C_DIM}ANTHROPIC_MODEL=%s${C_RESET}\n" "$ANTHROPIC_MODEL"
        if [ -z "${ANTHROPIC_AUTH_TOKEN:-}" ]; then
          warn "ANTHROPIC_BASE_URL is set but ANTHROPIC_AUTH_TOKEN is empty — claude calls will 401"
          hint "export ANTHROPIC_AUTH_TOKEN=<your-gateway-key>  (add to ~/.zshrc or ~/.bashrc)"
        fi
        if [ -z "${ANTHROPIC_MODEL:-}" ]; then
          warn "ANTHROPIC_MODEL not set — claude CLI may use a hardcoded default not available on your gateway"
          hint "export ANTHROPIC_MODEL=<litellm-model-id>  (see your gateway's /models endpoint)"
        fi
      elif [ -n "${ANTHROPIC_API_KEY:-}" ]; then
        ok "Detected claude auth mode: bare ANTHROPIC_API_KEY (routes to api.anthropic.com)"
      else
        # OAuth path — probe login state by file presence.
        if [ -f "$HOME/.claude/.credentials.json" ]; then
          ok "Assumed claude auth mode: Anthropic OAuth (~/.claude/.credentials.json present)"
        else
          warn "Assumed claude auth mode: OAuth, but ~/.claude/.credentials.json NOT found"
          hint "Run 'claude /login' before 'pnpm dev' — otherwise claude calls will fail"
        fi
        hint "On a corporate LiteLLM gateway? Export ANTHROPIC_BASE_URL + ANTHROPIC_AUTH_TOKEN + ANTHROPIC_MODEL in your shell rc and re-run this bootstrap."
      fi
    else
      # opencode runner — probe auth state, then nudge about the sidecar.
      if [ -f "$HOME/.local/share/opencode/auth.json" ] && [ -s "$HOME/.local/share/opencode/auth.json" ] && ! grep -q '^{}$' "$HOME/.local/share/opencode/auth.json" 2>/dev/null; then
        ok "Detected opencode credentials at ~/.local/share/opencode/auth.json"
      else
        warn "No opencode credentials found (~/.local/share/opencode/auth.json missing or empty)"
        hint "Run 'opencode providers login' (alias: 'opencode auth login') to authenticate before 'pnpm dev'"
      fi
      hint "Run 'opencode serve' in a sidecar terminal before 'pnpm dev' (listens on 127.0.0.1:4096)"
    fi

    # ─── Supplementary LLM HTTP-gateway prompts (optional) ────────────
    # The llm.* config powers direct LLM calls for non-tool-using checkpoints.
    # Separate from the runner choice above. Most users don't need this.
    echo
    printf "  ${C_BOLD}Direct LLM HTTP gateway${C_RESET} (optional — for the few non-tool-using checkpoints)\n"
    printf "  Configure now? [y/N]: "
    read -r want_llm
    case "$want_llm" in
      y|Y|yes|YES)
        printf "    ${C_BOLD}TENXGRACE_LLM_BASE_URL${C_RESET} (e.g. https://api.openai.com/v1/chat/completions): "
        read -r base_url
        printf "    ${C_BOLD}TENXGRACE_LLM_MODEL${C_RESET} (e.g. gpt-4o, kimi-latest, claude-sonnet-4-6): "
        read -r llm_model
        printf "    ${C_BOLD}TENXGRACE_LLM_API_KEY${C_RESET} (input hidden): "
        if read -rs api_key 2>/dev/null; then echo; else read -r api_key; fi
        for kv in "TENXGRACE_LLM_BASE_URL=$base_url" "TENXGRACE_LLM_MODEL=$llm_model" "TENXGRACE_LLM_API_KEY=$api_key"; do
          key="${kv%%=*}"
          val="${kv#*=}"
          tmp="$(mktemp)"
          sed -E "s|^${key}=.*|${key}=${val}|" .env > "$tmp" && mv "$tmp" .env
        done
        ok "wrote 3 TENXGRACE_LLM_* values to .env (api key hidden)"
        ;;
      *)
        # Make sure any stale .env.example placeholders are cleared in this skip-path too.
        for k in TENXGRACE_LLM_BASE_URL TENXGRACE_LLM_MODEL TENXGRACE_LLM_API_KEY; do
          tmp="$(mktemp)"
          sed -E "s|^${k}=.*|${k}=|" .env > "$tmp" && mv "$tmp" .env
        done
        ok "left TENXGRACE_LLM_* blank (no direct-gateway calls)"
        ;;
    esac

    # ─── Misconfig detection ──────────────────────────────────────────
    if [ "$runner_choice" = "claude-code" ] && ! command -v claude >/dev/null 2>&1; then
      err "runner=claude-code but 'claude' CLI is not installed — nothing will run"
      install_hint_for claude
    fi
    if [ "$runner_choice" = "opencode" ] && ! command -v opencode >/dev/null 2>&1; then
      err "runner=opencode but 'opencode' CLI is not installed — nothing will run"
      install_hint_for opencode
    fi

    # ─── TENXGRACE_CREDS_PATH ─────────────────────────────────────────
    echo
    printf "  ${C_BOLD}TENXGRACE_CREDS_PATH${C_RESET} (absolute path to creds.json; blank to skip): "
    read -r creds_path
    tmp="$(mktemp)"
    sed -E "s|^TENXGRACE_CREDS_PATH=.*|TENXGRACE_CREDS_PATH=$creds_path|" .env > "$tmp" && mv "$tmp" .env
    if [ -n "$creds_path" ]; then
      ok "wrote TENXGRACE_CREDS_PATH=$creds_path"
    else
      ok "cleared TENXGRACE_CREDS_PATH (left blank — placeholder removed)"
    fi

    printf "  ${C_DIM}(PARITY_* and PR_RESOLVER_* env vars left blank — uncomment in .env if/when you use those features)${C_RESET}\n"
  fi
fi

# ─── one-shot migration (~/.10xgrace -> ~/.tenxgrace) ────────────────────────
# StateManager's constructor runs the migration as a side effect. `status`
# is the cheapest command that constructs StateManager and exits cleanly.
echo
note "${C_BOLD}Running engine smoke check${C_RESET}  (also triggers home-dir migration if needed)"
if node packages/cli/dist/index.js status >/dev/null 2>&1; then
  ok "engine boots cleanly"
else
  warn "engine smoke check failed — usually safe to continue, see the error with: node packages/cli/dist/index.js status"
fi

# ─── summary ─────────────────────────────────────────────────────────────────
echo
if [ "$WARNINGS" -eq 0 ]; then
  printf "${C_OK}${C_BOLD}Setup OK${C_RESET}\n"
else
  printf "${C_OK}${C_BOLD}Setup OK${C_RESET}  ·  ${C_WARN}%d warning(s)${C_RESET} (review above; usually safe to continue)\n" "$WARNINGS"
fi

# ─── hand-off ────────────────────────────────────────────────────────────────
echo
if [ "$YES" -eq 1 ]; then
  printf "Run ${C_BOLD}pnpm dev${C_RESET} to start the supervisor + dashboard.\n\n"
  exit 0
fi
printf "Start ${C_BOLD}pnpm dev${C_RESET} now? [Y/n] "
read -r start
if [ -z "$start" ] || [ "$start" = "y" ] || [ "$start" = "Y" ]; then
  exec pnpm dev
fi
printf "\nWhen you're ready: ${C_BOLD}pnpm dev${C_RESET}  (then open http://localhost:3141)\n\n"
exit 0
