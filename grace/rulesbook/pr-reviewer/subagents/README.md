# Scenario Subagents

These are the default execution units for the PR reviewer.

The classifier maps each PR to one or more repo-specific scenarios, then the orchestrator spawns one subagent per matched scenario plus any metadata-specific subagents such as `grace-generated-pr`.

Each subagent should:

- read the scenario-specific prompt here
- read the referenced family reviewer prompt in `grace/rulesbook/pr-reviewer/reviewers/`
- read the companion files required by the classifier
- return structured findings only

The family reviewers in `grace/rulesbook/pr-reviewer/reviewers/` remain reusable deep-dive lenses. The scenario subagents in this folder are the primary dispatch targets.
