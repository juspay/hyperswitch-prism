/**
 * ConnectorClient — high-level wrapper using UniFFI bindings via koffi.
 *
 * Handles the full round-trip for any payment flow:
 *   1. Serialize protobuf request to bytes
 *   2. Build connector HTTP request via UniffiClient.callReq (generic FFI dispatch)
 *   3. Execute the HTTP request via our standardized HttpClient
 *   4. Parse the connector response via UniffiClient.callRes (generic FFI dispatch)
 *   5. Deserialize protobuf response from bytes
 *
 * Flow methods (authorize, capture, void, refund, …) are in _generated_connector_client_flows.ts.
 * To add a new flow: edit sdk/flows.yaml and run `make codegen`.
 */

import { Dispatcher } from "undici";
import { UniffiClient } from "./uniffi_client";
import { execute, createDispatcher, HttpRequest, NetworkError } from "../http_client";
// @ts-ignore - protobuf generated files might not have types yet
import { types } from "./generated/proto";

const v2 = types;

/**
 * Exception raised when req_transformer fails (integration error).
 * Wraps IntegrationError and provides access to proto fields.
 */
export class IntegrationError extends Error {
  constructor(public proto: any) {
    super(proto.errorMessage || proto.error_message);
    this.name = 'IntegrationError';
  }
}

/**
 * Exception raised when res_transformer fails (response transformation error).
 * Wraps ConnectorResponseTransformationError and provides access to proto fields.
 */
export class ConnectorResponseTransformationError extends Error {
  constructor(public proto: any) {
    super(proto.errorMessage || proto.error_message);
    this.name = 'ConnectorResponseTransformationError';
  }
}

export class ConnectorClient {
  private uniffi: UniffiClient;
  private config: types.ConnectorConfig;
  private defaults: types.IRequestConfig;
  private dispatcher: Dispatcher;

  /**
   * Initialize the client with mandatory config and optional request defaults.
   *
   * @param config - Immutable connector config and environment (ConnectorSpecificConfig, SdkOptions).
   * @param defaults - Optional per-request defaults (Http, Vault).
   * @param libPath - optional path to the UniFFI shared library.
   */
  constructor(
    config: types.IConnectorConfig,
    defaults: types.IRequestConfig = {},
    libPath?: string
  ) {
    this.uniffi = new UniffiClient(libPath);
    this.config = types.ConnectorConfig.create(config);
    this.defaults = defaults;

    const hasConnectorVariant = !!config.connectorConfig
      && Object.values(config.connectorConfig).some((value) => value != null);

    if (!hasConnectorVariant) {
      throw new NetworkError(
        "connectorConfig with a connector variant is required in ConnectorConfig",
        types.NetworkErrorCode.CLIENT_INITIALIZATION_FAILURE,
        400
      );
    }

    // Instance-level cache: create the primary connection pool at startup
    this.dispatcher = createDispatcher(defaults.http || {});
  }

  /**
   * Merges request-level options with client defaults. Environment comes from config (immutable).
   */
  private _resolveConfig(overrides?: types.IRequestConfig | null): {
    ffi: types.FfiOptions;
    http: types.IHttpConfig;
  } {
    const opt = overrides || {};
    const clientHttp = this.defaults.http || {};
    const overrideHttp = opt.http || {};

    const http: types.IHttpConfig = {
      totalTimeoutMs: overrideHttp.totalTimeoutMs ?? clientHttp.totalTimeoutMs,
      connectTimeoutMs: overrideHttp.connectTimeoutMs ?? clientHttp.connectTimeoutMs,
      responseTimeoutMs: overrideHttp.responseTimeoutMs ?? clientHttp.responseTimeoutMs,
      keepAliveTimeoutMs: overrideHttp.keepAliveTimeoutMs ?? clientHttp.keepAliveTimeoutMs,
      proxy: overrideHttp.proxy ?? clientHttp.proxy,
      caCert: overrideHttp.caCert ?? clientHttp.caCert,
    };

    const ffi = types.FfiOptions.create({
      environment: this.config.options?.environment ?? types.Environment.SANDBOX,
      connectorConfig: this.config.connectorConfig,
    });

    return { ffi, http };
  }

  /**
   * Execute a full round-trip for any registered payment flow.
   *
   * @param flow - Flow name matching the FFI transformer prefix (e.g. "authorize").
   * @param requestMsg - Protobuf request message object.
   * @param options - Optional ConfigOptions override (Environment, Http).
   */
  async _executeFlow(
    flow: string,
    requestMsg: object,
    options?: types.IRequestConfig | null,
    reqTypeName?: string,
    resTypeName?: string
  ): Promise<unknown> {
    const reqType = reqTypeName ? (v2 as any)[reqTypeName] : undefined;
    const resType = resTypeName ? (v2 as any)[resTypeName] : undefined;

    if (!reqType || !resType) {
      throw new Error(`Unknown flow: '${flow}' or missing type names.`);
    }

    // 1. Resolve final configuration
    const { ffi, http } = this._resolveConfig(options);
    const optionsBytes = Buffer.from(v2.FfiOptions.encode(ffi).finish());

    // 2. Serialize domain request
    const requestBytes = Buffer.from(reqType.encode(requestMsg).finish());

    // 3. Build connector HTTP request via FFI
    const resultBytes = this.uniffi.callReq(flow, requestBytes, optionsBytes);
    const connectorReq = v2.FfiConnectorHttpRequest.decode(resultBytes);

    const connectorRequest: HttpRequest = {
      url: connectorReq.url,
      method: connectorReq.method,
      headers: connectorReq.headers || {},
      body: connectorReq.body ?? undefined
    };

    // 4. Execute HTTP using the instance-owned connection pool
    const response = await execute(
      connectorRequest,
      http,
      this.dispatcher
    );

    // 5. Encode HTTP response for FFI
    const resProto = v2.FfiConnectorHttpResponse.create({
      statusCode: response.statusCode,
      headers: response.headers,
      body: response.body
    });
    const resBytes = Buffer.from(v2.FfiConnectorHttpResponse.encode(resProto).finish());

    // 6. Parse connector response via FFI and decode
    const resultBytesRes = this.uniffi.callRes(flow, resBytes, requestBytes, optionsBytes);
    // callRes returns FfiConnectorHttpResponse, extract domain response from body
    const httpResponse = v2.FfiConnectorHttpResponse.decode(resultBytesRes);
    return resType.decode(httpResponse.body);
  }

  /**
   * Execute a single-step flow directly via FFI (no HTTP round-trip).
   * Used for inbound flows like webhook processing where the connector sends data to us.
   */
  async _executeDirect(
    flow: string,
    requestMsg: object,
    options?: types.IRequestConfig | null,
    reqTypeName?: string,
    resTypeName?: string
  ): Promise<unknown> {
    const reqType = reqTypeName ? (v2 as any)[reqTypeName] : undefined;
    const resType = resTypeName ? (v2 as any)[resTypeName] : undefined;

    if (!reqType || !resType) {
      throw new Error(`Unknown flow: '${flow}' or missing type names.`);
    }

    // 1. Serialize request
    const requestBytes = Buffer.from(reqType.encode(requestMsg).finish());

    // 2. Resolve FFI options from identity + defaults + request override
    const { ffi } = this._resolveConfig(options);
    const optionsBytes = Buffer.from(v2.FfiOptions.encode(ffi).finish());

    // 3. Call the single-step transformer directly (no HTTP)
    const resultBytes = this.uniffi.callDirect(flow, requestBytes, optionsBytes);
    return resType.decode(resultBytes);
  }
}
