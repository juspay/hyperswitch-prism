package payments
//
import (
	"fmt"

	"google.golang.org/protobuf/proto"
	pb "github.com/juspay/hyperswitch-prism/sdk/go/generated/payments"
)

// CheckReq parses raw bytes from a req_transformer FFI call.
// Returns an FfiConnectorHttpRequest on success, or an error.
func CheckReq(resultBytes []byte) (*pb.FfiConnectorHttpRequest, error) {
	result := &pb.FfiResult{}
	if err := proto.Unmarshal(resultBytes, result); err != nil {
		return nil, fmt.Errorf("failed to unmarshal FfiResult: %w", err)
	}

	switch result.GetType() {
	case pb.FfiResult_HTTP_REQUEST:
		return result.GetHttpRequest(), nil
	case pb.FfiResult_INTEGRATION_ERROR:
		return nil, &IntegrationError{Proto: result.GetIntegrationError()}
	case pb.FfiResult_CONNECTOR_ERROR:
		return nil, &ConnectorError{Proto: result.GetConnectorError()}
	default:
		return nil, fmt.Errorf("unhandled result type: %v", result.GetType())
	}
}

// CheckRes parses raw bytes from a res_transformer FFI call.
// Returns an FfiConnectorHttpResponse on success, or an error.
func CheckRes(resultBytes []byte) (*pb.FfiConnectorHttpResponse, error) {
	result := &pb.FfiResult{}
	if err := proto.Unmarshal(resultBytes, result); err != nil {
		return nil, fmt.Errorf("failed to unmarshal FfiResult: %w", err)
	}

	switch result.GetType() {
	case pb.FfiResult_HTTP_RESPONSE:
		return result.GetHttpResponse(), nil
	case pb.FfiResult_CONNECTOR_ERROR:
		return nil, &ConnectorError{Proto: result.GetConnectorError()}
	case pb.FfiResult_INTEGRATION_ERROR:
		return nil, &IntegrationError{Proto: result.GetIntegrationError()}
	default:
		return nil, fmt.Errorf("unhandled result type: %v", result.GetType())
	}
}

// CheckDirect parses raw bytes from a direct transformer call (no HTTP round-trip).
// Returns raw proto response bytes on success, or an error.
func CheckDirect(resultBytes []byte) ([]byte, error) {
	result := &pb.FfiResult{}
	if err := proto.Unmarshal(resultBytes, result); err != nil {
		return nil, fmt.Errorf("failed to unmarshal FfiResult: %w", err)
	}

	switch result.GetType() {
	case pb.FfiResult_PROTO_RESPONSE:
		return result.GetProtoResponse(), nil
	case pb.FfiResult_CONNECTOR_ERROR:
		return nil, &ConnectorError{Proto: result.GetConnectorError()}
	case pb.FfiResult_INTEGRATION_ERROR:
		return nil, &IntegrationError{Proto: result.GetIntegrationError()}
	default:
		return nil, fmt.Errorf("unhandled result type: %v for direct flow", result.GetType())
	}
}
