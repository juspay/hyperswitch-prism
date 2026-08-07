// @ts-ignore
import { types } from "./generated/proto";

/**
 * Error classes for FFI-level errors.
 * 
 * These wrap the protobuf error types and provide proper Error inheritance
 * for use with instanceof checks and stack traces.
 */

/**
 * Exception raised when req_transformer fails (integration error).
 * Wraps IntegrationError proto and provides access to proto fields.
 */
export class IntegrationError extends Error {
  constructor(public proto: types.IIntegrationError) {
    super(proto.errorMessage || (proto as any).error_message);
  }

  get errorCode(): string { return this.proto.errorCode || (this.proto as any).error_code || "UNKNOWN"; }
  get suggestedAction(): string | undefined { return this.proto.suggestedAction || (this.proto as any).suggested_action; }
  get docUrl(): string | undefined { return this.proto.docUrl || (this.proto as any).doc_url; }
}

/**
 * Exception raised when res_transformer fails (response transformation error).
 * Wraps ConnectorError proto and provides access to proto fields.
 */
export class ConnectorError extends Error {
  constructor(public proto: types.IConnectorError) {
    super(proto.errorMessage || (proto as any).error_message);
  }

  get errorCode(): string { return this.proto.errorCode || (this.proto as any).error_code || "UNKNOWN"; }
  get httpStatusCode(): number | undefined { return this.proto.httpStatusCode || (this.proto as any).http_status_code; }
}
