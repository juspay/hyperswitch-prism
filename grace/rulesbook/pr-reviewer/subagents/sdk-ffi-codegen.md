# SDK FFI Codegen Subagent

Review a PR classified as `sdk-ffi-codegen`.

## Read First

- `grace/rulesbook/pr-reviewer/reviewers/sdk-ffi-codegen.md`
- `grace/rulesbook/pr-reviewer/config/rubric.yaml`

## Focus

- SDK client behavior and packaging
- FFI boundary alignment
- generated files and regeneration evidence
- multi-language drift and client sanity implications

## Extra Checks

- all affected language SDKs stay aligned
- generated files are not manually tweaked
- HTTP client changes have corresponding certification evidence

## Output

Use the standard structured finding format from `grace/rulesbook/pr-reviewer/reviewers/sdk-ffi-codegen.md`.
