// Re-export client classes flat (high-level API)
export * from "./payments/_generated_connector_client_flows";
export { UniffiClient } from "./payments/_generated_uniffi_client_flows";
export type { RustBuffer, RustCallStatus } from "./payments/uniffi_client";
export * from "./http_client";
export * from './payments/generated/proto.js';
// Re-export types namespace explicitly for both runtime and type access
export { types } from './payments/generated/proto.js';
// gRPC client (Rust-backed via hyperswitch_grpc_ffi native library)
export { GrpcClient } from "./payments/grpc_client";
export type {
  GrpcConfig,
  GrpcPaymentClient,
  GrpcCustomerClient,
  GrpcPaymentMethodClient,
  GrpcPaymentMethodAuthenticationClient,
  GrpcEventClient,
  GrpcMerchantAuthenticationClient,
  GrpcRecurringPaymentClient,
} from "./payments/grpc_client";
// Export error classes
export { IntegrationError, ConnectorError } from './payments/connector_client';

// ---------------------------------------------------------------------------
// Domain namespaces — runtime values
// Usage: import { payments, payment_methods, configs } from '@juspay/connector-service-sdk';
//        const config: configs.IConnectorConfig = { ... };
//        const client = new ConnectorClient(identity);
// ---------------------------------------------------------------------------
