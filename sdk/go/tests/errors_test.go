package payments_test

import (
	"testing"

	"google.golang.org/protobuf/proto"
	pb "github.com/juspay/hyperswitch-prism/sdk/go/generated/payments"
	"github.com/juspay/hyperswitch-prism/sdk/go/payments"
)

func TestIntegrationError_Error(t *testing.T) {
	e := &payments.IntegrationError{
		Proto: &pb.IntegrationError{
			ErrorCode:    "IE001",
			ErrorMessage: "amount is required",
		},
	}
	if got := e.Error(); got != "amount is required" {
		t.Errorf("Error() = %q, want %q", got, "amount is required")
	}
}

func TestConnectorError_Error(t *testing.T) {
	e := &payments.ConnectorError{
		Proto: &pb.ConnectorError{
			ErrorCode:      "CE001",
			ErrorMessage:   "bad request",
			HttpStatusCode: proto.Uint32(400),
		},
	}
	if got := e.Error(); got != "bad request" {
		t.Errorf("Error() = %q, want %q", got, "bad request")
	}
}

func TestNetworkError_Error(t *testing.T) {
	e := &payments.NetworkError{
		Code:    pb.NetworkErrorCode_NETWORK_FAILURE,
		Message: "connection refused",
		Status:  0,
	}
	if got := e.Error(); got != "connection refused" {
		t.Errorf("Error() = %q, want %q", got, "connection refused")
	}
}

func TestNetworkError_WithStatus(t *testing.T) {
	e := &payments.NetworkError{
		Code:    pb.NetworkErrorCode_NETWORK_FAILURE,
		Message: "internal server error",
		Status:  500,
	}
	if got := e.Error(); got != "internal server error" {
		t.Errorf("Error() = %q, want %q", got, "internal server error")
	}
	if e.Status != 500 {
		t.Errorf("Status = %d, want 500", e.Status)
	}
}
