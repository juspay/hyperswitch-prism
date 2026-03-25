<?php

declare(strict_types=1);

namespace Payments;

/**
 * PHP FFI wrapper for the UniFFI C ABI.
 *
 * UniFFI does not natively support PHP, so this file manually implements the
 * same RustBuffer protocol that the JavaScript SDK (koffi) and Kotlin SDK use.
 *
 * Uses PHP's built-in FFI extension (available since PHP 7.4, stable in PHP 8.0+)
 * to call the Rust shared library directly via its C ABI.
 *
 * The RustBuffer protocol:
 *   - All inputs are lowered: [i32 BE length prefix] + [raw protobuf bytes]
 *   - All outputs are lifted: strip the [i32 BE length prefix], return raw bytes
 *   - Panics (status.code === 2) surface as RuntimeException
 *   - Logical errors (status.code === 1) are encoded as protobuf error bytes in
 *     the returned RustBuffer, not in error_buf — the caller interprets them.
 *
 * Flow dispatch is generic: callReq()/callRes() look up the C symbol by name
 * at runtime. No flow names are hardcoded — add flows via codegen (make generate).
 */
class UniffiClient
{
    private \FFI $ffi;

    /**
     * Base C declarations for the UniFFI ABI (RustBuffer + alloc/free).
     * Flow-specific transformer declarations are appended dynamically from
     * GeneratedFlows::FLOWS and GeneratedFlows::SINGLE_FLOWS.
     */
    private const C_BASE = <<<'C'
        typedef struct {
            uint64_t capacity;
            uint64_t len;
            uint8_t *data;
        } RustBuffer;

        typedef struct {
            int8_t code;
            RustBuffer error_buf;
        } RustCallStatus;

        RustBuffer ffi_connector_service_ffi_rustbuffer_alloc(
            uint64_t size,
            RustCallStatus *call_status
        );
        void ffi_connector_service_ffi_rustbuffer_free(
            RustBuffer buf,
            RustCallStatus *call_status
        );
        C;

    /**
     * @param string|null $libPath Path to libconnector_service_ffi.{so,dylib}.
     *                             Defaults to Generated/libconnector_service_ffi.<ext>
     *                             in the same directory as this file.
     * @throws \RuntimeException if FFI extension is not loaded or library not found.
     */
    public function __construct(?string $libPath = null)
    {
        if (!extension_loaded('ffi')) {
            throw new \RuntimeException(
                'PHP FFI extension is required but not loaded. '
                . 'Ensure extension=ffi is in php.ini and ffi.enable=1.'
            );
        }

        if ($libPath === null) {
            $ext    = PHP_OS_FAMILY === 'Darwin' ? 'dylib' : 'so';
            $libPath = __DIR__ . '/Generated/libconnector_service_ffi.' . $ext;
        }

        if (!file_exists($libPath)) {
            throw new \RuntimeException(
                "FFI shared library not found: {$libPath}. "
                . "Run 'make build-ffi-lib' or 'make generate-all' first."
            );
        }

        $this->ffi = \FFI::cdef($this->buildCHeader(), $libPath);
    }

    // ── C header construction ────────────────────────────────────────────────

    /**
     * Build the complete C header string from the base declarations plus
     * one pair of req/res transformer functions per registered flow.
     */
    private function buildCHeader(): string
    {
        $header = self::C_BASE . "\n";

        foreach (array_keys(GeneratedFlows::FLOWS) as $flow) {
            $header .= sprintf(
                "RustBuffer uniffi_connector_service_ffi_fn_func_%s_req_transformer("
                . "RustBuffer request, RustBuffer options, RustCallStatus *call_status);\n",
                $flow
            );
            $header .= sprintf(
                "RustBuffer uniffi_connector_service_ffi_fn_func_%s_res_transformer("
                . "RustBuffer response, RustBuffer request, RustBuffer options, RustCallStatus *call_status);\n",
                $flow
            );
        }

        foreach (array_keys(GeneratedFlows::SINGLE_FLOWS) as $flow) {
            $header .= sprintf(
                "RustBuffer uniffi_connector_service_ffi_fn_func_%s_transformer("
                . "RustBuffer request, RustBuffer options, RustCallStatus *call_status);\n",
                $flow
            );
        }

        return $header;
    }

    // ── RustBuffer helpers ───────────────────────────────────────────────────

    /** @return \FFI\CData zero-initialised RustCallStatus */
    private function makeCallStatus(): \FFI\CData
    {
        return $this->ffi->new('RustCallStatus');
    }

    /**
     * Raise on Rust panics (status.code !== 0).
     * Logical errors (status.code === 1) do NOT appear here — they are encoded
     * as protobuf bytes in the returned RustBuffer and checked by the caller.
     *
     * @throws \RuntimeException on Rust panic.
     */
    private function checkCallStatus(\FFI\CData $status): void
    {
        if ($status->code === 0) {
            return;
        }

        // code 2 = Rust panic; error string is in error_buf as raw UTF-8
        if ($status->error_buf->len > 0 && !\FFI::isNull($status->error_buf->data)) {
            $msg = \FFI::string($status->error_buf->data, (int) $status->error_buf->len);
            $this->freeRustBufferRaw($status->error_buf);
            throw new \RuntimeException("Rust panic: {$msg}");
        }

        throw new \RuntimeException('Unknown Rust panic (no message in error_buf)');
    }

    /**
     * Lift a returned RustBuffer as UniFFI Vec<u8>:
     *   protocol: [i32 BE length] + [raw bytes]
     * Returns the raw bytes (without the 4-byte prefix).
     */
    private function liftBytes(\FFI\CData $buf): string
    {
        $len = (int) $buf->len;
        if ($len === 0 || \FFI::isNull($buf->data)) {
            return '';
        }
        $raw = \FFI::string($buf->data, $len);
        // First 4 bytes are a big-endian i32 giving the payload length
        $payloadLen = (int) unpack('N', substr($raw, 0, 4))[1];
        return (string) substr($raw, 4, $payloadLen);
    }

    /**
     * Allocate a RustBuffer via the FFI alloc function and copy $data into it.
     * The caller is responsible for ensuring Rust frees the buffer (either by
     * passing it to a transformer, which consumes it, or calling freeRustBuffer).
     *
     * @throws \RuntimeException on alloc failure.
     */
    private function allocRustBuffer(string $data): \FFI\CData
    {
        $len    = strlen($data);
        $status = $this->makeCallStatus();
        $buf    = $this->ffi->ffi_connector_service_ffi_rustbuffer_alloc($len, \FFI::addr($status));
        $this->checkCallStatus($status);

        \FFI::memcpy($buf->data, $data, $len);
        $buf->len = $len;

        return $buf;
    }

    /**
     * Lower raw bytes into a UniFFI-compliant input buffer.
     * Protocol: prepend [i32 BE length] to the raw bytes, then allocate.
     */
    private function lowerBytes(string $data): \FFI\CData
    {
        return $this->allocRustBuffer(pack('N', strlen($data)) . $data);
    }

    /**
     * Free a RustBuffer returned by a transformer.
     * Input buffers passed to transformers are consumed by Rust — do NOT free them.
     */
    private function freeRustBuffer(\FFI\CData $buf): void
    {
        $this->freeRustBufferRaw($buf);
    }

    /** Free helper that accepts the raw struct (used also for error_buf in status). */
    private function freeRustBufferRaw(\FFI\CData $buf): void
    {
        if ((int) $buf->len > 0 && !\FFI::isNull($buf->data)) {
            $status = $this->makeCallStatus();
            $this->ffi->ffi_connector_service_ffi_rustbuffer_free($buf, \FFI::addr($status));
        }
    }

    // ── Public API ───────────────────────────────────────────────────────────

    /**
     * Build the connector HTTP request for any registered flow.
     *
     * @param string $flow         Snake-case flow name (e.g. "authorize").
     * @param string $requestBytes Serialized protobuf request bytes.
     * @param string $optionsBytes Serialized FfiOptions protobuf bytes.
     * @return string              Serialized FfiConnectorHttpRequest protobuf bytes
     *                             (or RequestError bytes on logical failure).
     * @throws \InvalidArgumentException on unknown flow.
     * @throws \RuntimeException         on Rust panic.
     */
    public function callReq(string $flow, string $requestBytes, string $optionsBytes): string
    {
        $flows = array_keys(GeneratedFlows::FLOWS);
        if (!in_array($flow, $flows, true)) {
            throw new \InvalidArgumentException(
                "Unknown flow: '{$flow}'. Supported: " . implode(', ', $flows)
            );
        }

        $rbReq   = $this->lowerBytes($requestBytes);
        $rbOpts  = $this->lowerBytes($optionsBytes);
        $status  = $this->makeCallStatus();

        $fnName  = "uniffi_connector_service_ffi_fn_func_{$flow}_req_transformer";
        $result  = $this->ffi->$fnName($rbReq, $rbOpts, \FFI::addr($status));

        // $rbReq and $rbOpts are consumed by Rust — do not free them.
        try {
            $this->checkCallStatus($status);
            return $this->liftBytes($result);
        } finally {
            $this->freeRustBuffer($result);
        }
    }

    /**
     * Parse the connector HTTP response for any registered flow.
     *
     * @param string $flow          Snake-case flow name (e.g. "authorize").
     * @param string $responseBytes Serialized FfiConnectorHttpResponse protobuf bytes.
     * @param string $requestBytes  Serialized protobuf request bytes (for context).
     * @param string $optionsBytes  Serialized FfiOptions protobuf bytes.
     * @return string               Serialized response protobuf bytes
     *                              (or ResponseError bytes on logical failure).
     * @throws \InvalidArgumentException on unknown flow.
     * @throws \RuntimeException         on Rust panic.
     */
    public function callRes(
        string $flow,
        string $responseBytes,
        string $requestBytes,
        string $optionsBytes
    ): string {
        $flows = array_keys(GeneratedFlows::FLOWS);
        if (!in_array($flow, $flows, true)) {
            throw new \InvalidArgumentException(
                "Unknown flow: '{$flow}'. Supported: " . implode(', ', $flows)
            );
        }

        $rbRes   = $this->lowerBytes($responseBytes);
        $rbReq   = $this->lowerBytes($requestBytes);
        $rbOpts  = $this->lowerBytes($optionsBytes);
        $status  = $this->makeCallStatus();

        $fnName  = "uniffi_connector_service_ffi_fn_func_{$flow}_res_transformer";
        $result  = $this->ffi->$fnName($rbRes, $rbReq, $rbOpts, \FFI::addr($status));

        // Input buffers are consumed by Rust — do not free them.
        try {
            $this->checkCallStatus($status);
            return $this->liftBytes($result);
        } finally {
            $this->freeRustBuffer($result);
        }
    }

    /**
     * Execute a single-step transformer directly (no HTTP round-trip).
     * Used for inbound flows such as webhook processing.
     *
     * @param string $flow         Snake-case flow name (e.g. "handle_event").
     * @param string $requestBytes Serialized protobuf request bytes.
     * @param string $optionsBytes Serialized FfiOptions protobuf bytes.
     * @return string              Serialized response protobuf bytes.
     * @throws \InvalidArgumentException on unknown single-step flow.
     * @throws \RuntimeException         on Rust panic.
     */
    public function callDirect(string $flow, string $requestBytes, string $optionsBytes): string
    {
        $flows = array_keys(GeneratedFlows::SINGLE_FLOWS);
        if (!in_array($flow, $flows, true)) {
            throw new \InvalidArgumentException(
                "Unknown single-step flow: '{$flow}'. Supported: " . implode(', ', $flows)
            );
        }

        $rbReq  = $this->lowerBytes($requestBytes);
        $rbOpts = $this->lowerBytes($optionsBytes);
        $status = $this->makeCallStatus();

        $fnName = "uniffi_connector_service_ffi_fn_func_{$flow}_transformer";
        $result = $this->ffi->$fnName($rbReq, $rbOpts, \FFI::addr($status));

        try {
            $this->checkCallStatus($status);
            return $this->liftBytes($result);
        } finally {
            $this->freeRustBuffer($result);
        }
    }
}
