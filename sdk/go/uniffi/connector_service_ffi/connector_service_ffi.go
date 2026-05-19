package connector_service_ffi
//
// #include <connector_service_ffi.h>
import "C"

import (
	"bytes"
	"encoding/binary"
	"fmt"
	"io"
	"math"
	"unsafe"
)

// This is needed, because as of go 1.24
// type RustBuffer C.RustBuffer cannot have methods,
// RustBuffer is treated as non-local type
type GoRustBuffer struct {
	inner C.RustBuffer
}

type RustBufferI interface {
	AsReader() *bytes.Reader
	Free()
	ToGoBytes() []byte
	Data() unsafe.Pointer
	Len() uint64
	Capacity() uint64
}

// C.RustBuffer fields exposed as an interface so they can be accessed in different Go packages.
// See https://github.com/golang/go/issues/13467
type ExternalCRustBuffer interface {
	Data() unsafe.Pointer
	Len() uint64
	Capacity() uint64
}

func RustBufferFromC(b C.RustBuffer) ExternalCRustBuffer {
	return GoRustBuffer{
		inner: b,
	}
}

func CFromRustBuffer(b ExternalCRustBuffer) C.RustBuffer {
	return C.RustBuffer{
		capacity: C.uint64_t(b.Capacity()),
		len:      C.uint64_t(b.Len()),
		data:     (*C.uchar)(b.Data()),
	}
}

func RustBufferFromExternal(b ExternalCRustBuffer) GoRustBuffer {
	return GoRustBuffer{
		inner: C.RustBuffer{
			capacity: C.uint64_t(b.Capacity()),
			len:      C.uint64_t(b.Len()),
			data:     (*C.uchar)(b.Data()),
		},
	}
}

func (cb GoRustBuffer) Capacity() uint64 {
	return uint64(cb.inner.capacity)
}

func (cb GoRustBuffer) Len() uint64 {
	return uint64(cb.inner.len)
}

func (cb GoRustBuffer) Data() unsafe.Pointer {
	return unsafe.Pointer(cb.inner.data)
}

func (cb GoRustBuffer) AsReader() *bytes.Reader {
	b := unsafe.Slice((*byte)(cb.inner.data), C.uint64_t(cb.inner.len))
	return bytes.NewReader(b)
}

func (cb GoRustBuffer) Free() {
	rustCall(func(status *C.RustCallStatus) bool {
		C.ffi_connector_service_ffi_rustbuffer_free(cb.inner, status)
		return false
	})
}

func (cb GoRustBuffer) ToGoBytes() []byte {
	return C.GoBytes(unsafe.Pointer(cb.inner.data), C.int(cb.inner.len))
}

func stringToRustBuffer(str string) C.RustBuffer {
	return bytesToRustBuffer([]byte(str))
}

func bytesToRustBuffer(b []byte) C.RustBuffer {
	if len(b) == 0 {
		return C.RustBuffer{}
	}
	// We can pass the pointer along here, as it is pinned
	// for the duration of this call
	foreign := C.ForeignBytes{
		len:  C.int(len(b)),
		data: (*C.uchar)(unsafe.Pointer(&b[0])),
	}

	return rustCall(func(status *C.RustCallStatus) C.RustBuffer {
		return C.ffi_connector_service_ffi_rustbuffer_from_bytes(foreign, status)
	})
}

type BufLifter[GoType any] interface {
	Lift(value RustBufferI) GoType
}

type BufLowerer[GoType any] interface {
	Lower(value GoType) C.RustBuffer
}

type BufReader[GoType any] interface {
	Read(reader io.Reader) GoType
}

type BufWriter[GoType any] interface {
	Write(writer io.Writer, value GoType)
}

func LowerIntoRustBuffer[GoType any](bufWriter BufWriter[GoType], value GoType) C.RustBuffer {
	// This might be not the most efficient way but it does not require knowing allocation size
	// beforehand
	var buffer bytes.Buffer
	bufWriter.Write(&buffer, value)

	bytes, err := io.ReadAll(&buffer)
	if err != nil {
		panic(fmt.Errorf("reading written data: %w", err))
	}
	return bytesToRustBuffer(bytes)
}

func LiftFromRustBuffer[GoType any](bufReader BufReader[GoType], rbuf RustBufferI) GoType {
	defer rbuf.Free()
	reader := rbuf.AsReader()
	item := bufReader.Read(reader)
	if reader.Len() > 0 {
		// TODO: Remove this
		leftover, _ := io.ReadAll(reader)
		panic(fmt.Errorf("Junk remaining in buffer after lifting: %s", string(leftover)))
	}
	return item
}

func rustCallWithError[E any, U any](converter BufReader[*E], callback func(*C.RustCallStatus) U) (U, *E) {
	var status C.RustCallStatus
	returnValue := callback(&status)
	err := checkCallStatus(converter, status)
	return returnValue, err
}

func checkCallStatus[E any](converter BufReader[*E], status C.RustCallStatus) *E {
	switch status.code {
	case 0:
		return nil
	case 1:
		return LiftFromRustBuffer(converter, GoRustBuffer{inner: status.errorBuf})
	case 2:
		// when the rust code sees a panic, it tries to construct a rustBuffer
		// with the message.  but if that code panics, then it just sends back
		// an empty buffer.
		if status.errorBuf.len > 0 {
			panic(fmt.Errorf("%s", FfiConverterStringINSTANCE.Lift(GoRustBuffer{inner: status.errorBuf})))
		} else {
			panic(fmt.Errorf("Rust panicked while handling Rust panic"))
		}
	default:
		panic(fmt.Errorf("unknown status code: %d", status.code))
	}
}

func checkCallStatusUnknown(status C.RustCallStatus) error {
	switch status.code {
	case 0:
		return nil
	case 1:
		panic(fmt.Errorf("function not returning an error returned an error"))
	case 2:
		// when the rust code sees a panic, it tries to construct a C.RustBuffer
		// with the message.  but if that code panics, then it just sends back
		// an empty buffer.
		if status.errorBuf.len > 0 {
			panic(fmt.Errorf("%s", FfiConverterStringINSTANCE.Lift(GoRustBuffer{
				inner: status.errorBuf,
			})))
		} else {
			panic(fmt.Errorf("Rust panicked while handling Rust panic"))
		}
	default:
		return fmt.Errorf("unknown status code: %d", status.code)
	}
}

func rustCall[U any](callback func(*C.RustCallStatus) U) U {
	returnValue, err := rustCallWithError[error](nil, callback)
	if err != nil {
		panic(err)
	}
	return returnValue
}

type NativeError interface {
	AsError() error
}

func writeInt8(writer io.Writer, value int8) {
	if err := binary.Write(writer, binary.BigEndian, value); err != nil {
		panic(err)
	}
}

func writeUint8(writer io.Writer, value uint8) {
	if err := binary.Write(writer, binary.BigEndian, value); err != nil {
		panic(err)
	}
}

func writeInt16(writer io.Writer, value int16) {
	if err := binary.Write(writer, binary.BigEndian, value); err != nil {
		panic(err)
	}
}

func writeUint16(writer io.Writer, value uint16) {
	if err := binary.Write(writer, binary.BigEndian, value); err != nil {
		panic(err)
	}
}

func writeInt32(writer io.Writer, value int32) {
	if err := binary.Write(writer, binary.BigEndian, value); err != nil {
		panic(err)
	}
}

func writeUint32(writer io.Writer, value uint32) {
	if err := binary.Write(writer, binary.BigEndian, value); err != nil {
		panic(err)
	}
}

func writeInt64(writer io.Writer, value int64) {
	if err := binary.Write(writer, binary.BigEndian, value); err != nil {
		panic(err)
	}
}

func writeUint64(writer io.Writer, value uint64) {
	if err := binary.Write(writer, binary.BigEndian, value); err != nil {
		panic(err)
	}
}

func writeFloat32(writer io.Writer, value float32) {
	if err := binary.Write(writer, binary.BigEndian, value); err != nil {
		panic(err)
	}
}

func writeFloat64(writer io.Writer, value float64) {
	if err := binary.Write(writer, binary.BigEndian, value); err != nil {
		panic(err)
	}
}

func readInt8(reader io.Reader) int8 {
	var result int8
	if err := binary.Read(reader, binary.BigEndian, &result); err != nil {
		panic(err)
	}
	return result
}

func readUint8(reader io.Reader) uint8 {
	var result uint8
	if err := binary.Read(reader, binary.BigEndian, &result); err != nil {
		panic(err)
	}
	return result
}

func readInt16(reader io.Reader) int16 {
	var result int16
	if err := binary.Read(reader, binary.BigEndian, &result); err != nil {
		panic(err)
	}
	return result
}

func readUint16(reader io.Reader) uint16 {
	var result uint16
	if err := binary.Read(reader, binary.BigEndian, &result); err != nil {
		panic(err)
	}
	return result
}

func readInt32(reader io.Reader) int32 {
	var result int32
	if err := binary.Read(reader, binary.BigEndian, &result); err != nil {
		panic(err)
	}
	return result
}

func readUint32(reader io.Reader) uint32 {
	var result uint32
	if err := binary.Read(reader, binary.BigEndian, &result); err != nil {
		panic(err)
	}
	return result
}

func readInt64(reader io.Reader) int64 {
	var result int64
	if err := binary.Read(reader, binary.BigEndian, &result); err != nil {
		panic(err)
	}
	return result
}

func readUint64(reader io.Reader) uint64 {
	var result uint64
	if err := binary.Read(reader, binary.BigEndian, &result); err != nil {
		panic(err)
	}
	return result
}

func readFloat32(reader io.Reader) float32 {
	var result float32
	if err := binary.Read(reader, binary.BigEndian, &result); err != nil {
		panic(err)
	}
	return result
}

func readFloat64(reader io.Reader) float64 {
	var result float64
	if err := binary.Read(reader, binary.BigEndian, &result); err != nil {
		panic(err)
	}
	return result
}

func init() {

	uniffiCheckChecksums()
}

func uniffiCheckChecksums() {
	// Get the bindings contract version from our ComponentInterface
	bindingsContractVersion := 29
	// Get the scaffolding contract version by calling the into the dylib
	scaffoldingContractVersion := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint32_t {
		return C.ffi_connector_service_ffi_uniffi_contract_version()
	})
	if bindingsContractVersion != int(scaffoldingContractVersion) {
		// If this happens try cleaning and rebuilding your project
		panic("connector_service_ffi: UniFFI contract version mismatch")
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_accept_req_transformer()
		})
		if checksum != 7766 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_accept_req_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_accept_res_transformer()
		})
		if checksum != 6217 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_accept_res_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_authenticate_req_transformer()
		})
		if checksum != 35858 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_authenticate_req_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_authenticate_res_transformer()
		})
		if checksum != 41029 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_authenticate_res_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_authorize_req_transformer()
		})
		if checksum != 611 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_authorize_req_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_authorize_res_transformer()
		})
		if checksum != 4632 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_authorize_res_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_capture_req_transformer()
		})
		if checksum != 52976 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_capture_req_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_capture_res_transformer()
		})
		if checksum != 42869 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_capture_res_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_charge_req_transformer()
		})
		if checksum != 56381 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_charge_req_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_charge_res_transformer()
		})
		if checksum != 64011 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_charge_res_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_create_client_authentication_token_req_transformer()
		})
		if checksum != 4584 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_create_client_authentication_token_req_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_create_client_authentication_token_res_transformer()
		})
		if checksum != 715 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_create_client_authentication_token_res_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_create_order_req_transformer()
		})
		if checksum != 36874 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_create_order_req_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_create_order_res_transformer()
		})
		if checksum != 44781 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_create_order_res_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_create_req_transformer()
		})
		if checksum != 17766 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_create_req_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_create_res_transformer()
		})
		if checksum != 22620 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_create_res_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_create_server_authentication_token_req_transformer()
		})
		if checksum != 50689 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_create_server_authentication_token_req_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_create_server_authentication_token_res_transformer()
		})
		if checksum != 34069 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_create_server_authentication_token_res_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_create_server_session_authentication_token_req_transformer()
		})
		if checksum != 56418 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_create_server_session_authentication_token_req_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_create_server_session_authentication_token_res_transformer()
		})
		if checksum != 12903 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_create_server_session_authentication_token_res_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_defend_req_transformer()
		})
		if checksum != 63685 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_defend_req_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_defend_res_transformer()
		})
		if checksum != 50376 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_defend_res_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_get_req_transformer()
		})
		if checksum != 52513 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_get_req_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_get_res_transformer()
		})
		if checksum != 29234 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_get_res_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_handle_event_transformer()
		})
		if checksum != 59147 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_handle_event_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_incremental_authorization_req_transformer()
		})
		if checksum != 54846 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_incremental_authorization_req_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_incremental_authorization_res_transformer()
		})
		if checksum != 48851 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_incremental_authorization_res_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_parse_event_transformer()
		})
		if checksum != 12008 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_parse_event_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_payout_create_link_req_transformer()
		})
		if checksum != 48060 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_payout_create_link_req_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_payout_create_link_res_transformer()
		})
		if checksum != 7340 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_payout_create_link_res_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_payout_create_recipient_req_transformer()
		})
		if checksum != 48456 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_payout_create_recipient_req_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_payout_create_recipient_res_transformer()
		})
		if checksum != 57952 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_payout_create_recipient_res_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_payout_create_req_transformer()
		})
		if checksum != 48589 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_payout_create_req_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_payout_create_res_transformer()
		})
		if checksum != 33888 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_payout_create_res_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_payout_enroll_disburse_account_req_transformer()
		})
		if checksum != 32410 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_payout_enroll_disburse_account_req_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_payout_enroll_disburse_account_res_transformer()
		})
		if checksum != 36813 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_payout_enroll_disburse_account_res_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_payout_get_req_transformer()
		})
		if checksum != 63854 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_payout_get_req_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_payout_get_res_transformer()
		})
		if checksum != 28933 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_payout_get_res_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_payout_stage_req_transformer()
		})
		if checksum != 16122 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_payout_stage_req_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_payout_stage_res_transformer()
		})
		if checksum != 62380 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_payout_stage_res_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_payout_transfer_req_transformer()
		})
		if checksum != 18807 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_payout_transfer_req_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_payout_transfer_res_transformer()
		})
		if checksum != 53099 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_payout_transfer_res_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_payout_void_req_transformer()
		})
		if checksum != 35208 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_payout_void_req_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_payout_void_res_transformer()
		})
		if checksum != 5624 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_payout_void_res_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_post_authenticate_req_transformer()
		})
		if checksum != 15980 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_post_authenticate_req_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_post_authenticate_res_transformer()
		})
		if checksum != 53484 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_post_authenticate_res_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_pre_authenticate_req_transformer()
		})
		if checksum != 26518 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_pre_authenticate_req_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_pre_authenticate_res_transformer()
		})
		if checksum != 31622 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_pre_authenticate_res_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_proxy_authorize_req_transformer()
		})
		if checksum != 44067 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_proxy_authorize_req_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_proxy_authorize_res_transformer()
		})
		if checksum != 45744 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_proxy_authorize_res_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_proxy_setup_recurring_req_transformer()
		})
		if checksum != 13702 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_proxy_setup_recurring_req_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_proxy_setup_recurring_res_transformer()
		})
		if checksum != 664 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_proxy_setup_recurring_res_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_recurring_revoke_req_transformer()
		})
		if checksum != 16266 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_recurring_revoke_req_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_recurring_revoke_res_transformer()
		})
		if checksum != 43716 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_recurring_revoke_res_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_refund_get_req_transformer()
		})
		if checksum != 3394 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_refund_get_req_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_refund_get_res_transformer()
		})
		if checksum != 57121 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_refund_get_res_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_refund_req_transformer()
		})
		if checksum != 34934 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_refund_req_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_refund_res_transformer()
		})
		if checksum != 64434 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_refund_res_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_reverse_req_transformer()
		})
		if checksum != 7626 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_reverse_req_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_reverse_res_transformer()
		})
		if checksum != 21254 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_reverse_res_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_setup_recurring_req_transformer()
		})
		if checksum != 56888 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_setup_recurring_req_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_setup_recurring_res_transformer()
		})
		if checksum != 8644 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_setup_recurring_res_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_submit_evidence_req_transformer()
		})
		if checksum != 25979 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_submit_evidence_req_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_submit_evidence_res_transformer()
		})
		if checksum != 21354 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_submit_evidence_res_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_token_authorize_req_transformer()
		})
		if checksum != 29753 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_token_authorize_req_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_token_authorize_res_transformer()
		})
		if checksum != 54640 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_token_authorize_res_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_token_setup_recurring_req_transformer()
		})
		if checksum != 17269 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_token_setup_recurring_req_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_token_setup_recurring_res_transformer()
		})
		if checksum != 25711 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_token_setup_recurring_res_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_tokenize_req_transformer()
		})
		if checksum != 35811 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_tokenize_req_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_tokenize_res_transformer()
		})
		if checksum != 47156 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_tokenize_res_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_verify_redirect_response_transformer()
		})
		if checksum != 43705 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_verify_redirect_response_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_void_req_transformer()
		})
		if checksum != 40925 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_void_req_transformer: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_connector_service_ffi_checksum_func_void_res_transformer()
		})
		if checksum != 14903 {
			// If this happens try cleaning and rebuilding your project
			panic("connector_service_ffi: uniffi_connector_service_ffi_checksum_func_void_res_transformer: UniFFI API checksum mismatch")
		}
	}
}

type FfiConverterString struct{}

var FfiConverterStringINSTANCE = FfiConverterString{}

func (FfiConverterString) Lift(rb RustBufferI) string {
	defer rb.Free()
	reader := rb.AsReader()
	b, err := io.ReadAll(reader)
	if err != nil {
		panic(fmt.Errorf("reading reader: %w", err))
	}
	return string(b)
}

func (FfiConverterString) Read(reader io.Reader) string {
	length := readInt32(reader)
	buffer := make([]byte, length)
	read_length, err := reader.Read(buffer)
	if err != nil && err != io.EOF {
		panic(err)
	}
	if read_length != int(length) {
		panic(fmt.Errorf("bad read length when reading string, expected %d, read %d", length, read_length))
	}
	return string(buffer)
}

func (FfiConverterString) Lower(value string) C.RustBuffer {
	return stringToRustBuffer(value)
}

func (c FfiConverterString) LowerExternal(value string) ExternalCRustBuffer {
	return RustBufferFromC(stringToRustBuffer(value))
}

func (FfiConverterString) Write(writer io.Writer, value string) {
	if len(value) > math.MaxInt32 {
		panic("String is too large to fit into Int32")
	}

	writeInt32(writer, int32(len(value)))
	write_length, err := io.WriteString(writer, value)
	if err != nil {
		panic(err)
	}
	if write_length != len(value) {
		panic(fmt.Errorf("bad write length when writing string, expected %d, written %d", len(value), write_length))
	}
}

type FfiDestroyerString struct{}

func (FfiDestroyerString) Destroy(_ string) {}

type FfiConverterBytes struct{}

var FfiConverterBytesINSTANCE = FfiConverterBytes{}

func (c FfiConverterBytes) Lower(value []byte) C.RustBuffer {
	return LowerIntoRustBuffer[[]byte](c, value)
}

func (c FfiConverterBytes) LowerExternal(value []byte) ExternalCRustBuffer {
	return RustBufferFromC(c.Lower(value))
}

func (c FfiConverterBytes) Write(writer io.Writer, value []byte) {
	if len(value) > math.MaxInt32 {
		panic("[]byte is too large to fit into Int32")
	}

	writeInt32(writer, int32(len(value)))
	write_length, err := writer.Write(value)
	if err != nil {
		panic(err)
	}
	if write_length != len(value) {
		panic(fmt.Errorf("bad write length when writing []byte, expected %d, written %d", len(value), write_length))
	}
}

func (c FfiConverterBytes) Lift(rb RustBufferI) []byte {
	return LiftFromRustBuffer[[]byte](c, rb)
}

func (c FfiConverterBytes) Read(reader io.Reader) []byte {
	length := readInt32(reader)
	buffer := make([]byte, length)
	read_length, err := reader.Read(buffer)
	if err != nil && err != io.EOF {
		panic(err)
	}
	if read_length != int(length) {
		panic(fmt.Errorf("bad read length when reading []byte, expected %d, read %d", length, read_length))
	}
	return buffer
}

type FfiDestroyerBytes struct{}

func (FfiDestroyerBytes) Destroy(_ []byte) {}

func AcceptReqTransformer(requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_accept_req_transformer(FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func AcceptResTransformer(responseBytes []byte, requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_accept_res_transformer(FfiConverterBytesINSTANCE.Lower(responseBytes), FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func AuthenticateReqTransformer(requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_authenticate_req_transformer(FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func AuthenticateResTransformer(responseBytes []byte, requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_authenticate_res_transformer(FfiConverterBytesINSTANCE.Lower(responseBytes), FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func AuthorizeReqTransformer(requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_authorize_req_transformer(FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func AuthorizeResTransformer(responseBytes []byte, requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_authorize_res_transformer(FfiConverterBytesINSTANCE.Lower(responseBytes), FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func CaptureReqTransformer(requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_capture_req_transformer(FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func CaptureResTransformer(responseBytes []byte, requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_capture_res_transformer(FfiConverterBytesINSTANCE.Lower(responseBytes), FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func ChargeReqTransformer(requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_charge_req_transformer(FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func ChargeResTransformer(responseBytes []byte, requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_charge_res_transformer(FfiConverterBytesINSTANCE.Lower(responseBytes), FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func CreateClientAuthenticationTokenReqTransformer(requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_create_client_authentication_token_req_transformer(FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func CreateClientAuthenticationTokenResTransformer(responseBytes []byte, requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_create_client_authentication_token_res_transformer(FfiConverterBytesINSTANCE.Lower(responseBytes), FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func CreateOrderReqTransformer(requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_create_order_req_transformer(FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func CreateOrderResTransformer(responseBytes []byte, requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_create_order_res_transformer(FfiConverterBytesINSTANCE.Lower(responseBytes), FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func CreateReqTransformer(requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_create_req_transformer(FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func CreateResTransformer(responseBytes []byte, requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_create_res_transformer(FfiConverterBytesINSTANCE.Lower(responseBytes), FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func CreateServerAuthenticationTokenReqTransformer(requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_create_server_authentication_token_req_transformer(FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func CreateServerAuthenticationTokenResTransformer(responseBytes []byte, requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_create_server_authentication_token_res_transformer(FfiConverterBytesINSTANCE.Lower(responseBytes), FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func CreateServerSessionAuthenticationTokenReqTransformer(requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_create_server_session_authentication_token_req_transformer(FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func CreateServerSessionAuthenticationTokenResTransformer(responseBytes []byte, requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_create_server_session_authentication_token_res_transformer(FfiConverterBytesINSTANCE.Lower(responseBytes), FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func DefendReqTransformer(requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_defend_req_transformer(FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func DefendResTransformer(responseBytes []byte, requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_defend_res_transformer(FfiConverterBytesINSTANCE.Lower(responseBytes), FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func GetReqTransformer(requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_get_req_transformer(FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func GetResTransformer(responseBytes []byte, requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_get_res_transformer(FfiConverterBytesINSTANCE.Lower(responseBytes), FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

// handle_event — synchronous webhook processing (single-step, no outgoing HTTP).
//
// Unlike req/res flows there is no split: the caller passes raw
// `EventServiceHandleRequest` proto bytes and receives encoded
// `EventServiceHandleResponse` bytes directly.
func HandleEventTransformer(requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_handle_event_transformer(FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func IncrementalAuthorizationReqTransformer(requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_incremental_authorization_req_transformer(FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func IncrementalAuthorizationResTransformer(responseBytes []byte, requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_incremental_authorization_res_transformer(FfiConverterBytesINSTANCE.Lower(responseBytes), FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

// parse_event — stateless webhook event type and resource reference extraction.
//
// No secrets, no context. The caller passes raw `EventServiceParseRequest` proto bytes
// and receives encoded `EventServiceParseResponse` bytes directly.
func ParseEventTransformer(requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_parse_event_transformer(FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func PayoutCreateLinkReqTransformer(requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_payout_create_link_req_transformer(FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func PayoutCreateLinkResTransformer(responseBytes []byte, requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_payout_create_link_res_transformer(FfiConverterBytesINSTANCE.Lower(responseBytes), FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func PayoutCreateRecipientReqTransformer(requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_payout_create_recipient_req_transformer(FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func PayoutCreateRecipientResTransformer(responseBytes []byte, requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_payout_create_recipient_res_transformer(FfiConverterBytesINSTANCE.Lower(responseBytes), FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func PayoutCreateReqTransformer(requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_payout_create_req_transformer(FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func PayoutCreateResTransformer(responseBytes []byte, requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_payout_create_res_transformer(FfiConverterBytesINSTANCE.Lower(responseBytes), FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func PayoutEnrollDisburseAccountReqTransformer(requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_payout_enroll_disburse_account_req_transformer(FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func PayoutEnrollDisburseAccountResTransformer(responseBytes []byte, requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_payout_enroll_disburse_account_res_transformer(FfiConverterBytesINSTANCE.Lower(responseBytes), FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func PayoutGetReqTransformer(requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_payout_get_req_transformer(FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func PayoutGetResTransformer(responseBytes []byte, requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_payout_get_res_transformer(FfiConverterBytesINSTANCE.Lower(responseBytes), FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func PayoutStageReqTransformer(requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_payout_stage_req_transformer(FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func PayoutStageResTransformer(responseBytes []byte, requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_payout_stage_res_transformer(FfiConverterBytesINSTANCE.Lower(responseBytes), FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func PayoutTransferReqTransformer(requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_payout_transfer_req_transformer(FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func PayoutTransferResTransformer(responseBytes []byte, requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_payout_transfer_res_transformer(FfiConverterBytesINSTANCE.Lower(responseBytes), FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func PayoutVoidReqTransformer(requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_payout_void_req_transformer(FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func PayoutVoidResTransformer(responseBytes []byte, requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_payout_void_res_transformer(FfiConverterBytesINSTANCE.Lower(responseBytes), FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func PostAuthenticateReqTransformer(requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_post_authenticate_req_transformer(FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func PostAuthenticateResTransformer(responseBytes []byte, requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_post_authenticate_res_transformer(FfiConverterBytesINSTANCE.Lower(responseBytes), FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func PreAuthenticateReqTransformer(requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_pre_authenticate_req_transformer(FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func PreAuthenticateResTransformer(responseBytes []byte, requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_pre_authenticate_res_transformer(FfiConverterBytesINSTANCE.Lower(responseBytes), FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func ProxyAuthorizeReqTransformer(requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_proxy_authorize_req_transformer(FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func ProxyAuthorizeResTransformer(responseBytes []byte, requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_proxy_authorize_res_transformer(FfiConverterBytesINSTANCE.Lower(responseBytes), FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func ProxySetupRecurringReqTransformer(requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_proxy_setup_recurring_req_transformer(FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func ProxySetupRecurringResTransformer(responseBytes []byte, requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_proxy_setup_recurring_res_transformer(FfiConverterBytesINSTANCE.Lower(responseBytes), FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func RecurringRevokeReqTransformer(requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_recurring_revoke_req_transformer(FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func RecurringRevokeResTransformer(responseBytes []byte, requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_recurring_revoke_res_transformer(FfiConverterBytesINSTANCE.Lower(responseBytes), FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func RefundGetReqTransformer(requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_refund_get_req_transformer(FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func RefundGetResTransformer(responseBytes []byte, requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_refund_get_res_transformer(FfiConverterBytesINSTANCE.Lower(responseBytes), FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func RefundReqTransformer(requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_refund_req_transformer(FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func RefundResTransformer(responseBytes []byte, requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_refund_res_transformer(FfiConverterBytesINSTANCE.Lower(responseBytes), FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func ReverseReqTransformer(requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_reverse_req_transformer(FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func ReverseResTransformer(responseBytes []byte, requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_reverse_res_transformer(FfiConverterBytesINSTANCE.Lower(responseBytes), FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func SetupRecurringReqTransformer(requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_setup_recurring_req_transformer(FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func SetupRecurringResTransformer(responseBytes []byte, requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_setup_recurring_res_transformer(FfiConverterBytesINSTANCE.Lower(responseBytes), FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func SubmitEvidenceReqTransformer(requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_submit_evidence_req_transformer(FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func SubmitEvidenceResTransformer(responseBytes []byte, requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_submit_evidence_res_transformer(FfiConverterBytesINSTANCE.Lower(responseBytes), FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func TokenAuthorizeReqTransformer(requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_token_authorize_req_transformer(FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func TokenAuthorizeResTransformer(responseBytes []byte, requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_token_authorize_res_transformer(FfiConverterBytesINSTANCE.Lower(responseBytes), FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func TokenSetupRecurringReqTransformer(requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_token_setup_recurring_req_transformer(FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func TokenSetupRecurringResTransformer(responseBytes []byte, requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_token_setup_recurring_res_transformer(FfiConverterBytesINSTANCE.Lower(responseBytes), FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func TokenizeReqTransformer(requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_tokenize_req_transformer(FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func TokenizeResTransformer(responseBytes []byte, requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_tokenize_res_transformer(FfiConverterBytesINSTANCE.Lower(responseBytes), FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

// verify_redirect_response — synchronous verification of redirect response (no outgoing HTTP call).
//
// Calls `decode_redirect_response_body`, `verify_redirect_response_source`, and
// `process_redirect_response` on the connector, mirroring what the gRPC server does.
func VerifyRedirectResponseTransformer(requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_verify_redirect_response_transformer(FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func VoidReqTransformer(requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_void_req_transformer(FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}

func VoidResTransformer(responseBytes []byte, requestBytes []byte, optionsBytes []byte) []byte {
	return FfiConverterBytesINSTANCE.Lift(rustCall(func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_connector_service_ffi_fn_func_void_res_transformer(FfiConverterBytesINSTANCE.Lower(responseBytes), FfiConverterBytesINSTANCE.Lower(requestBytes), FfiConverterBytesINSTANCE.Lower(optionsBytes), _uniffiStatus),
		}
	}))
}
