#!/usr/bin/env bash
set +x
set -euo pipefail
umask 077

: "${CONNECTOR_AUTH_S3_URI:?Set the encrypted credentials S3 URI in Jenkins}"
if [[ -z "${CONNECTOR_AUTH_PASSPHRASE_FILE:-}" && -z "${CONNECTOR_AUTH_PASSPHRASE:-}" ]]; then
  echo 'Set CONNECTOR_AUTH_PASSPHRASE_FILE or CONNECTOR_AUTH_PASSPHRASE in Jenkins' >&2
  exit 1
fi

credentials_dir="$(mktemp -d "${TMPDIR:-/tmp}/ucs-e2e-credentials.XXXXXX")"
cleanup() {
  rm -f "$credentials_dir/creds.json" "$credentials_dir/creds.json.gpg"
  rmdir "$credentials_dir"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

aws s3 cp "$CONNECTOR_AUTH_S3_URI" "$credentials_dir/creds.json.gpg" --only-show-errors
gpg_args=(--batch --quiet --pinentry-mode loopback --output "$credentials_dir/creds.json")
if [[ -n "${CONNECTOR_AUTH_PASSPHRASE_FILE:-}" ]]; then
  gpg "${gpg_args[@]}" --passphrase-file "$CONNECTOR_AUTH_PASSPHRASE_FILE" \
    --decrypt "$credentials_dir/creds.json.gpg"
else
  printf '%s' "$CONNECTOR_AUTH_PASSPHRASE" | gpg "${gpg_args[@]}" --passphrase-fd 0 \
    --decrypt "$credentials_dir/creds.json.gpg"
fi

unset CONNECTOR_AUTH_PASSPHRASE
CONNECTOR_AUTH_FILE_PATH="$credentials_dir/creds.json" npm test -- "$@"
