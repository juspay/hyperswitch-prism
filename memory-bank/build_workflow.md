---
description: 
globs: 
alwaysApply: true
---
## Workflow: Automating Connector Integration for (Authorize Flow) referencing the Hyperswitch implementation.


1.  **Start**: Begin the connector integration process. 

2.  **Add connector support**: Take reference from memory-bank/connector_implementation_guide.md for this step.

3.  **Take a reference from Hyperswitch**: Take references from the Hyperswitch connector implementation for request/response struct at connector level and transformers in the docs provided. Use this file as a context on how to transform the req and res body: memory-bank/transformer_memory_bank.md
    *   `https://github.com/juspay/hyperswitch/blob/main/crates/hyperswitch_connectors/src/connectors/{connector_name}/transformers.rs`
    *   `https://github.com/juspay/hyperswitch/blob/main/crates/hyperswitch_connectors/src/connectors/{connector_name}.rs`

4.  **Validation Step**: Implement the transformation logic to convert between the FlowCommonData (PaymentFlowData, PaymentsAuthorizeData)  and the connector-specific request/response types.
Transform each field logically from PaymentsAuthorizeData to connector end request backend/domain_types/src/connector_types.rs

5.  **Build using command `cargo build`**: Build the project to check for compilation errors. Build and log all errors along with their code references by generating a new file `error.md`.
