package payments

import com.sun.jna.*
import java.nio.ByteBuffer
import java.nio.ByteOrder

/**
 * UniFFI client for Kotlin/Java — handles the JNA bridge to the shared library.
 *
 * Implements the UniFFI binary protocol for:
 * 1. RustBuffer (Safe memory transport)
 * 2. RustCallStatus (Error handling)
 * 3. Scaffolding-style serialization
 */
internal class UniffiClient(libPath: String? = null) {

    private val lib: LibConnectorService

    init {
        val actualPath = libPath ?: System.getProperty("connector_service_ffi.path") 
            ?: throw IllegalStateException("Shared library path not provided")
        lib = Native.load(actualPath, LibConnectorService::class.java)
    }

    // ── UniFFI Structs ────────────────────────────────────────────────────────

    @Structure.FieldOrder("capacity", "len", "data")
    open class RustBuffer : Structure() {
        @JvmField var capacity: Long = 0
        @JvmField var len: Long = 0
        @JvmField var data: Pointer? = null

        class ByValue : RustBuffer(), Structure.ByValue
    }

    @Structure.FieldOrder("code", "error_buf")
    open class RustCallStatus : Structure() {
        @JvmField var code: Byte = 0
        @JvmField var error_buf: RustBuffer.ByValue = RustBuffer.ByValue()
    }

    // ── Shared Library Interface ──────────────────────────────────────────────

    interface LibConnectorService : Library {
        fun uniffi_connector_service_ffi_fn_func_authorize_req_transformer(
            req: RustBuffer.ByValue,
            meta: RustBuffer.ByValue,
            opts: RustBuffer.ByValue,
            status: RustCallStatus
        ): RustBuffer.ByValue

        fun uniffi_connector_service_ffi_fn_func_authorize_res_transformer(
            res: RustBuffer.ByValue,
            req: RustBuffer.ByValue,
            meta: RustBuffer.ByValue,
            opts: RustBuffer.ByValue,
            status: RustCallStatus
        ): RustBuffer.ByValue

        fun ffi_connector_service_ffi_rustbuffer_alloc(len: Long, status: RustCallStatus): RustBuffer.ByValue
        fun ffi_connector_service_ffi_rustbuffer_free(buf: RustBuffer.ByValue, status: RustCallStatus)
    }

    // ── Execution Logic ───────────────────────────────────────────────────────

    fun authorizeReq(
        requestBytes: ByteArray,
        metadata: Map<String, String>,
        optionsBytes: ByteArray
    ): ByteArray {
        val rbReq = lowerBytes(requestBytes)
        val rbMeta = lowerMap(metadata)
        val rbOpts = lowerBytes(optionsBytes)
        val status = RustCallStatus()

        val result = lib.uniffi_connector_service_ffi_fn_func_authorize_req_transformer(
            rbReq, rbMeta, rbOpts, status
        )

        try {
            checkCallStatus(status)
            return liftBytes(result)
        } finally {
            freeRustBuffer(result)
        }
    }

    fun authorizeRes(
        resBytes: ByteArray,
        requestBytes: ByteArray,
        metadata: Map<String, String>,
        optionsBytes: ByteArray
    ): ByteArray {
        val rbRes = lowerBytes(resBytes)
        val rbReq = lowerBytes(requestBytes)
        val rbMeta = lowerMap(metadata)
        val rbOpts = lowerBytes(optionsBytes)
        val status = RustCallStatus()

        val result = lib.uniffi_connector_service_ffi_fn_func_authorize_res_transformer(
            rbRes, rbReq, rbMeta, rbOpts, status
        )

        try {
            checkCallStatus(status)
            return liftBytes(result)
        } finally {
            freeRustBuffer(result)
        }
    }

    // ── Protocol Helpers ──────────────────────────────────────────────────────

    private fun checkCallStatus(status: RustCallStatus) {
        if (status.code.toInt() == 0) return
        
        // Simplified error lifting for Kotlin (Production code would parse error_buf)
        if (status.code.toInt() == 1) {
            throw RuntimeException("FFI Call Failed (Backend Error)")
        }
        throw RuntimeException("FFI Call Panic (Backend Crash)")
    }

    private fun liftBytes(rb: RustBuffer.ByValue): ByteArray {
        if (rb.len == 0L || rb.data == null) return ByteArray(0)
        
        // UniFFI protocol for Vec<u8> return: [i32 len] + [raw bytes]
        val totalBuf = rb.data!!.getByteBuffer(0, rb.len).order(ByteOrder.BIG_ENDIAN)
        val actualLen = totalBuf.getInt()
        val result = ByteArray(actualLen)
        totalBuf.get(result)
        return result
    }

    private fun lowerBytes(data: ByteArray): RustBuffer.ByValue {
        // UniFFI protocol for Vec<u8> arg: [i32 len] + [raw bytes]
        val totalSize = 4 + data.size
        val rb = allocRustBuffer(totalSize.toLong())
        
        val nativeBuf = rb.data!!.getByteBuffer(0, totalSize.toLong()).order(ByteOrder.BIG_ENDIAN)
        nativeBuf.putInt(data.size)
        nativeBuf.put(data)
        
        return rb
    }

    private fun lowerMap(map: Map<String, String>): RustBuffer.ByValue {
        val entries = map.entries.toList()
        var totalSize = 4 // count
        val encoded = entries.map { (k, v) ->
            val kb = k.toByteArray(Charsets.UTF_8)
            val vb = v.toByteArray(Charsets.UTF_8)
            totalSize += 4 + kb.size + 4 + vb.size
            kb to vb
        }

        val rb = allocRustBuffer(totalSize.toLong())
        val nativeBuf = rb.data!!.getByteBuffer(0, totalSize.toLong()).order(ByteOrder.BIG_ENDIAN)
        
        nativeBuf.putInt(entries.size)
        for ((kb, vb) in encoded) {
            nativeBuf.putInt(kb.size)
            nativeBuf.put(kb)
            nativeBuf.putInt(vb.size)
            nativeBuf.put(vb)
        }
        
        return rb
    }

    private fun allocRustBuffer(len: Long): RustBuffer.ByValue {
        val status = RustCallStatus()
        val rb = lib.ffi_connector_service_ffi_rustbuffer_alloc(len, status)
        if (status.code.toInt() != 0) throw RuntimeException("RustBuffer allocation failed")
        return rb
    }

    private fun freeRustBuffer(rb: RustBuffer.ByValue) {
        val status = RustCallStatus()
        lib.ffi_connector_service_ffi_rustbuffer_free(rb, status)
    }
}
