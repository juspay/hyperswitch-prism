#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
umask 077
: "${CONNECTOR_CREDS_S3_BUCKET_URI:?Use the same S3 bucket URI secret as HS main}"
: "${CONNECTOR_AUTH_PASSPHRASE:?Use the same decryption secret as HS main}"
mkdir -p .secrets
# Same source object as HS main's cypress-tests-runner.yml. Override for a rotated object.
source_file="${S3_SOURCE_FILE_NAME:-b06ec672-ca83-405a-a834-856c3fe18dad.json.gpg}"
aws s3 cp "${CONNECTOR_CREDS_S3_BUCKET_URI%/}/${source_file}" .secrets/creds.json.gpg --only-show-errors
# Pass the secret on stdin, never as a process argument.
printf '%s' "$CONNECTOR_AUTH_PASSPHRASE" | gpg --quiet --batch --yes --pinentry-mode loopback --passphrase-fd 0 --decrypt --output .secrets/creds.json .secrets/creds.json.gpg
chmod 600 .secrets/creds.json
printf '%s\n' 'Credentials ready. export CONNECTOR_AUTH_FILE_PATH="$PWD/.secrets/creds.json"'
