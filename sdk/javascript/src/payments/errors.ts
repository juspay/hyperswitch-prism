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
  constructor(public proto: any) {
    super(proto.errorMessage || proto.error_message);
  }

  get errorCode(): string { return this.proto.errorCode || this.proto.error_code; }
  get suggestedAction(): string | undefined { return this.proto.suggestedAction || this.proto.suggested_action; }
  get docUrl(): string | undefined { return this.proto.docUrl || this.proto.doc_url; }
}

/**
 * Exception raised when res_transformer fails (response transformation error).
 * Wraps ConnectorError proto and provides access to proto fields.
 */
export class ConnectorError extends Error {
  constructor(public proto: any) {
    super(proto.errorMessage || proto.error_message);
  }

  get errorCode(): string { return this.proto.errorCode || this.proto.error_code; }
  get httpStatusCode(): number | undefined { return this.proto.httpStatusCode || this.proto.http_status_code; }
}
