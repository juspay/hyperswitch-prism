/**
 * UniFFI client for Node.js — calls the same shared library as Python/Kotlin.
 *
 * Uses koffi to call the UniFFI C ABI directly, replacing NAPI entirely.
 * Handles RustBuffer serialization/deserialization for the UniFFI protocol.
 *
 * Flow dispatch is generic: callReq(flow, ...) and callRes(flow, ...) load
 * the corresponding C symbol dynamically from the flow list in _generated_flows.js.
 * No flow names are hardcoded here — add new flows to flows.yaml and run `make generate`.
 */

import koffi from "koffi";
import path from "path";
// @ts-ignore - generated CommonJS module
import { FLOWS, SINGLE_FLOWS } from "./_generated_flows.js";
// @ts-ignore - generated protobuf types
import { types } from "./generated/proto.js";
import { IntegrationError, ConnectorError } from "./errors";

// Standard Node.js __dirname
declare const __dirname: string;
const _dirname = __dirname;

const FLOW_NAMES: string[] = Object.keys(FLOWS as Record<string, unknown>);
const SINGLE_FLOW_NAMES: string[] = Object.keys((SINGLE_FLOWS || {}) as Record<string, unknown>);

// ── RustBuffer struct layout ────────────────────────────────────────────────
// UniFFI uses RustBuffer { capacity: u64, len: u64, data: *u8 } for all
// compound types.

export interface RustBuffer {
  capacity: bigint;
  len: bigint;
  data: Buffer | null;
}

export interface RustCallStatus {
  code: number;
  error_buf: RustBuffer;
}

const RustBufferStruct = koffi.struct("RustBuffer", {
  capacity: "uint64",
  len: "uint64",
  data: "void *",
});

const RustCallStatusStruct = koffi.struct("RustCallStatus", {
  code: "int8",
  error_buf: RustBufferStruct,
});

// ── Shared Library Interface ─────────────────────────────────────────────────

interface FfiFunctions {
  alloc: (len: bigint, status: any) => RustBuffer;
  free: (buf: RustBuffer, status: any) => void;
  [key: string]: (...args: any[]) => any;
}

function loadLib(libPath?: string): FfiFunctions {
  if (!libPath) {
    const ext = process.platform === "darwin" ? "dylib" : "so";
    libPath = path.join(_dirname, "generated", `libconnector_service_ffi.${ext}`);
  }

  const lib = koffi.load(libPath);

  const fns: Record<string, any> = {
    alloc: lib.func(
      "ffi_connector_service_ffi_rustbuffer_alloc",
      RustBufferStruct,
      ["uint64", koffi.out(koffi.pointer(RustCallStatusStruct))]
    ),
    free: lib.func(
      "ffi_connector_service_ffi_rustbuffer_free",
      "void",
      [RustBufferStruct, koffi.out(koffi.pointer(RustCallStatusStruct))]
    ),
  };

  // Load req and res transformer symbols for every registered flow.
  for (const flow of FLOW_NAMES) {
    fns[`${flow}_req`] = lib.func(
      `uniffi_connector_service_ffi_fn_func_${flow}_req_transformer`,
      RustBufferStruct,
      [RustBufferStruct, RustBufferStruct, koffi.out(koffi.pointer(RustCallStatusStruct))]
    );
    fns[`${flow}_res`] = lib.func(
      `uniffi_connector_service_ffi_fn_func_${flow}_res_transformer`,
      RustBufferStruct,
      [RustBufferStruct, RustBufferStruct, RustBufferStruct, koffi.out(koffi.pointer(RustCallStatusStruct))]
    );
  }

  // Load single-step transformer symbols (no HTTP round-trip, e.g. webhook processing).
  for (const flow of SINGLE_FLOW_NAMES) {
    fns[`${flow}_direct`] = lib.func(
      `uniffi_connector_service_ffi_fn_func_${flow}_transformer`,
      RustBufferStruct,
      [RustBufferStruct, RustBufferStruct, koffi.out(koffi.pointer(RustCallStatusStruct))]
    );
  }

  return fns as FfiFunctions;
}

// ── Helpers ──────────────────────────────────────────────────────

function makeCallStatus(): RustCallStatus {
  return { code: 0, error_buf: { capacity: 0n, len: 0n, data: null } };
}

function checkCallStatus(ffi: FfiFunctions, status: RustCallStatus): void {
  if (status.code === 0) return;

  // Only Rust panics should reach here now (status.code === 2)
  // Normal errors are encoded as protobuf IntegrationError/ConnectorError in returned bytes
  if (status.error_buf.len > 0n) {
    const msg = liftString(status.error_buf);
    freeRustBuffer(ffi, status.error_buf);
    throw new Error(`Rust panic: ${msg}`);
  }

  throw new Error("Unknown Rust panic");
}

/**
 * UniFFI Strings are serialized as raw UTF8 bytes when top-level in RustBuffer.
 */
function liftString(buf: RustBuffer): string {
  if (!buf.data || buf.len === 0n) return "";
  const raw = Buffer.from(koffi.decode(buf.data, "uint8", Number(buf.len)));
  return raw.toString("utf-8");
}

/**
 * UniFFI Vec<u8> (Bytes) as return values are serialized as [i32 length] + [raw bytes]
 */
function liftBytes(buf: RustBuffer): Buffer {
  if (!buf.data || buf.len === 0n) return Buffer.alloc(0);
  const raw = Buffer.from(koffi.decode(buf.data, "uint8", Number(buf.len)));

  // UniFFI protocol for return values: first 4 bytes are the length of the actual payload
  const len = raw.readInt32BE(0);
  return raw.subarray(4, 4 + len);
}

function freeRustBuffer(ffi: FfiFunctions, buf: RustBuffer): void {
  if (buf.data && buf.len > 0n) {
    ffi.free(buf, makeCallStatus());
  }
}

function allocRustBuffer(ffi: FfiFunctions, data: Buffer | Uint8Array): RustBuffer {
  const status = makeCallStatus();
  const buf = ffi.alloc(BigInt(data.length), status);
  checkCallStatus(ffi, status);

  koffi.encode(buf.data, "uint8", Array.from(data), data.length);
  buf.len = BigInt(data.length);
  return buf;
}

/**
 * Lowers raw bytes into a UniFFI-compliant buffer for top-level arguments.
 * Protocol: [i32 length prefix] + [raw bytes]
 */
function lowerBytes(ffi: FfiFunctions, data: Buffer | Uint8Array): RustBuffer {
  const buf = Buffer.alloc(4 + data.length);
  buf.writeInt32BE(data.length, 0);
  Buffer.from(data).copy(buf, 4);
  return allocRustBuffer(ffi, buf);
}

export class UniffiClient {
  private _ffi: FfiFunctions;

  constructor(libPath?: string) {
    this._ffi = loadLib(libPath);
  }

  /**
   * Build the connector HTTP request for any flow.
   * Returns protobuf-encoded FfiConnectorHttpRequest bytes.
   */
  callReq(
    flow: string,
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    const fn = this._ffi[`${flow}_req`];
    if (!fn) throw new Error(`Unknown flow: '${flow}'. Supported: ${FLOW_NAMES.join(", ")}`);

    const rbReq = lowerBytes(this._ffi, requestBytes);
    const rbOpts = lowerBytes(this._ffi, optionsBytes);
    const status = makeCallStatus();

    const result = fn(rbReq, rbOpts, status);

    try {
      checkCallStatus(this._ffi, status);
      const bytes = liftBytes(result);
      const resultMsg = types.FfiResult.decode(bytes);
      
      // Enum-based type checking
      switch (resultMsg.type) {
        case types.FfiResult.Type.HTTP_REQUEST:
          if (!resultMsg.httpRequest) {
            throw new Error("Expected httpRequest in FfiResult, but got null/undefined");
          }
          return Buffer.from(types.FfiConnectorHttpRequest.encode(resultMsg.httpRequest).finish());
        case types.FfiResult.Type.INTEGRATION_ERROR:
          if (!resultMsg.integrationError) {
            throw new Error("Expected integrationError in FfiResult, but got null/undefined");
          }
          throw new IntegrationError(resultMsg.integrationError);
        case types.FfiResult.Type.CONNECTOR_ERROR:
          if (!resultMsg.connectorError) {
            throw new Error("Expected connectorError in FfiResult, but got null/undefined");
          }
          throw new ConnectorError(resultMsg.connectorError);
        default:
          throw new Error(`Unknown result type: ${resultMsg.type}`);
      }
    } finally {
      freeRustBuffer(this._ffi, result);
    }
  }

  /**
   * Parse the connector HTTP response for any flow.
   * responseBytes: protobuf-encoded FfiConnectorHttpResponse.
   * Returns protobuf-encoded response bytes for the flow's response type.
   */
  callRes(
    flow: string,
    responseBytes: Buffer | Uint8Array,
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    const fn = this._ffi[`${flow}_res`];
    if (!fn) throw new Error(`Unknown flow: '${flow}'. Supported: ${FLOW_NAMES.join(", ")}`);

    const rbRes = lowerBytes(this._ffi, responseBytes);
    const rbReq = lowerBytes(this._ffi, requestBytes);
    const rbOpts = lowerBytes(this._ffi, optionsBytes);
    const status = makeCallStatus();

    const result = fn(rbRes, rbReq, rbOpts, status);

    try {
      checkCallStatus(this._ffi, status);
      const bytes = liftBytes(result);
      const resultMsg = types.FfiResult.decode(bytes);
      
      // Enum-based type checking
      switch (resultMsg.type) {
        case types.FfiResult.Type.HTTP_RESPONSE:
          if (!resultMsg.httpResponse) {
            throw new Error("Expected httpResponse in FfiResult, but got null/undefined");
          }
          return Buffer.from(types.FfiConnectorHttpResponse.encode(resultMsg.httpResponse).finish());
        case types.FfiResult.Type.CONNECTOR_ERROR:
          if (!resultMsg.connectorError) {
            throw new Error("Expected connectorError in FfiResult, but got null/undefined");
          }
          throw new ConnectorError(resultMsg.connectorError);
        case types.FfiResult.Type.INTEGRATION_ERROR:
          if (!resultMsg.integrationError) {
            throw new Error("Expected integrationError in FfiResult, but got null/undefined");
          }
          throw new IntegrationError(resultMsg.integrationError);
        default:
          throw new Error(`Unknown result type: ${resultMsg.type}`);
      }
    } finally {
      freeRustBuffer(this._ffi, result);
    }
  }

  /**
   * Execute a single-step transformer directly (no HTTP round-trip).
   * Used for inbound flows like webhook processing.
   * Returns protobuf-encoded response bytes.
   */
  callDirect(
    flow: string,
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    const fn = this._ffi[`${flow}_direct`];
    if (!fn) throw new Error(`Unknown single-step flow: '${flow}'. Supported: ${SINGLE_FLOW_NAMES.join(", ")}`);

    const rbReq = lowerBytes(this._ffi, requestBytes);
    const rbOpts = lowerBytes(this._ffi, optionsBytes);
    const status = makeCallStatus();

    const result = fn(rbReq, rbOpts, status);

    try {
      checkCallStatus(this._ffi, status);
      const bytes = liftBytes(result);
      const resultMsg = types.FfiResult.decode(bytes);
      
      // Enum-based type checking
      switch (resultMsg.type) {
        case types.FfiResult.Type.PROTO_RESPONSE:
          if (!resultMsg.protoResponse) {
            throw new Error("Expected protoResponse in FfiResult, but got null/undefined");
          }
          return Buffer.from(resultMsg.protoResponse);
        case types.FfiResult.Type.CONNECTOR_ERROR:
          if (!resultMsg.connectorError) {
            throw new Error("Expected connectorError in FfiResult, but got null/undefined");
          }
          throw new ConnectorError(resultMsg.connectorError);
        case types.FfiResult.Type.INTEGRATION_ERROR:
          if (!resultMsg.integrationError) {
            throw new Error("Expected integrationError in FfiResult, but got null/undefined");
          }
          throw new IntegrationError(resultMsg.integrationError);
        default:
          throw new Error(`Unknown result type: ${resultMsg.type}`);
      }
    } finally {
      freeRustBuffer(this._ffi, result);
    }
  }

}
