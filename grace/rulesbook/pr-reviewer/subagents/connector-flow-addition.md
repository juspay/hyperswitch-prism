# Connector Flow Addition Subagent

Review a PR classified as `connector-flow-addition`.

## Read First

- `grace/rulesbook/pr-reviewer/reviewers/connector.md`
- `grace/rulesbook/pr-reviewer/config/rubric.yaml`

## Focus

- new or changed `ConnectorIntegrationV2` flow implementations
- request and response transformer completeness
- status, ID, amount, and currency semantics for the added flow
- prerequisite flow assumptions and cross-flow consistency
- targeted test or scenario coverage for the new flow

## Extra Checks

- no silent support claims for payment methods or edge cases that are not implemented
- no flow-specific failures hidden behind generic error mapping
- required companion trait and flow markers line up with the new implementation

## Output

Use the standard structured finding format from `grace/rulesbook/pr-reviewer/reviewers/connector.md`.
