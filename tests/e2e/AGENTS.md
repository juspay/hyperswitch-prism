# E2E contribution instructions

Read `CONTRIBUTING.md` before changing this directory. The implemented shared spec is the reference for extensions.

Keep journeys in specs, setup in Playwright fixtures, individual calls in operations, native Playwright assertions in specs, and comparison attachments in the report helper. Connector configuration is data only. All connectors inherit shared scenarios; exclusions require reasons.

Before and after each push, perform the contributor-perspective review in `CONTRIBUTING.md`. Check the actual published diff after pushing. Question extra public extension paths, hidden sequences, unchecked casts, mutable shared state, retries around mutations, weakened assertions, and connector-specific scenario execution.

Use normal TypeScript types, standard Playwright fixtures, and review. Do not introduce regex/source-inspection enforcement or a custom scenario interpreter. Do not claim unimplemented coverage or guarantee that external sandbox behavior is deterministic.
