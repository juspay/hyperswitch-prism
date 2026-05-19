package payments_test

import (
	"errors"
	"testing"

	"google.golang.org/protobuf/proto"
	pb "github.com/juspay/hyperswitch-prism/sdk/go/generated/payments"
	"github.com/juspay/hyperswitch-prism/sdk/go/payments"
)

func TestCheckReq_HTTPRequest(t *testing.T) {
	want := &pb.FfiConnectorHttpRequest{
		Method: "POST",
		Url:    "https://api.example.com/v1/payments",
		Headers: map[string]string{
			"Content-Type": "application/json",
		},
		Body: []byte(`{"amount": 100}`),
	}

	result := &pb.FfiResult{
		Type:    pb.FfiResult_HTTP_REQUEST,
		Payload: &pb.FfiResult_HttpRequest{HttpRequest: want},
	}
	b, err := proto.Marshal(result)
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}

	got, err := payments.CheckReq(b)
	if err != nil {
		t.Fatalf("CheckReq: %v", err)
	}

	if got.GetMethod() != want.GetMethod() {
		t.Errorf("method = %q, want %q", got.GetMethod(), want.GetMethod())
	}
	if got.GetUrl() != want.GetUrl() {
		t.Errorf("url = %q, want %q", got.GetUrl(), want.GetUrl())
	}
	if string(got.GetBody()) != string(want.GetBody()) {
		t.Errorf("body = %q, want %q", got.GetBody(), want.GetBody())
	}
}

func TestCheckReq_IntegrationError(t *testing.T) {
	result := &pb.FfiResult{
		Type: pb.FfiResult_INTEGRATION_ERROR,
		Payload: &pb.FfiResult_IntegrationError{
			IntegrationError: &pb.IntegrationError{
				ErrorCode:    "IE001",
				ErrorMessage: "missing required field: amount",
			},
		},
	}
	b, err := proto.Marshal(result)
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}

	_, err = payments.CheckReq(b)
	if err == nil {
		t.Fatal("expected error, got nil")
	}

	var ie *payments.IntegrationError
	if !errors.As(err, &ie) {
		t.Fatalf("expected *payments.IntegrationError, got %T", err)
	}
	if ie.Proto.GetErrorCode() != "IE001" {
		t.Errorf("error code = %q, want IE001", ie.Proto.GetErrorCode())
	}
	if ie.Proto.GetErrorMessage() != "missing required field: amount" {
		t.Errorf("error message = %q, want %q", ie.Proto.GetErrorMessage(), "missing required field: amount")
	}
}

func TestCheckReq_ConnectorError(t *testing.T) {
	result := &pb.FfiResult{
		Type: pb.FfiResult_CONNECTOR_ERROR,
		Payload: &pb.FfiResult_ConnectorError{
			ConnectorError: &pb.ConnectorError{
				ErrorCode:      "CE001",
				ErrorMessage:   "connector rejected request",
				HttpStatusCode: proto.Uint32(400),
			},
		},
	}
	b, err := proto.Marshal(result)
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}

	_, err = payments.CheckReq(b)
	if err == nil {
		t.Fatal("expected error, got nil")
	}

	var ce *payments.ConnectorError
	if !errors.As(err, &ce) {
		t.Fatalf("expected *payments.ConnectorError, got %T", err)
	}
	if ce.Proto.GetErrorCode() != "CE001" {
		t.Errorf("error code = %q, want CE001", ce.Proto.GetErrorCode())
	}
	if ce.Proto.GetHttpStatusCode() != 400 {
		t.Errorf("http status = %d, want 400", ce.Proto.GetHttpStatusCode())
	}
}

func TestCheckRes_HTTPResponse(t *testing.T) {
	want := &pb.FfiConnectorHttpResponse{
		StatusCode: 200,
		Headers: map[string]string{
			"Content-Type": "application/json",
		},
		Body: []byte(`{"status": "succeeded"}`),
	}

	result := &pb.FfiResult{
		Type:    pb.FfiResult_HTTP_RESPONSE,
		Payload: &pb.FfiResult_HttpResponse{HttpResponse: want},
	}
	b, err := proto.Marshal(result)
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}

	got, err := payments.CheckRes(b)
	if err != nil {
		t.Fatalf("CheckRes: %v", err)
	}

	if got.GetStatusCode() != want.GetStatusCode() {
		t.Errorf("status code = %d, want %d", got.GetStatusCode(), want.GetStatusCode())
	}
	if string(got.GetBody()) != string(want.GetBody()) {
		t.Errorf("body = %q, want %q", got.GetBody(), want.GetBody())
	}
}

func TestCheckRes_IntegrationError(t *testing.T) {
	result := &pb.FfiResult{
		Type: pb.FfiResult_INTEGRATION_ERROR,
		Payload: &pb.FfiResult_IntegrationError{
			IntegrationError: &pb.IntegrationError{
				ErrorCode:    "IE002",
				ErrorMessage: "invalid response format",
			},
		},
	}
	b, err := proto.Marshal(result)
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}

	_, err = payments.CheckRes(b)
	if err == nil {
		t.Fatal("expected error, got nil")
	}

	var ie *payments.IntegrationError
	if !errors.As(err, &ie) {
		t.Fatalf("expected *payments.IntegrationError, got %T", err)
	}
}

func TestCheckRes_ConnectorError(t *testing.T) {
	result := &pb.FfiResult{
		Type: pb.FfiResult_CONNECTOR_ERROR,
		Payload: &pb.FfiResult_ConnectorError{
			ConnectorError: &pb.ConnectorError{
				ErrorCode:      "CE002",
				ErrorMessage:   "gateway timeout",
				HttpStatusCode: proto.Uint32(504),
			},
		},
	}
	b, err := proto.Marshal(result)
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}

	_, err = payments.CheckRes(b)
	if err == nil {
		t.Fatal("expected error, got nil")
	}

	var ce *payments.ConnectorError
	if !errors.As(err, &ce) {
		t.Fatalf("expected *payments.ConnectorError, got %T", err)
	}
}

func TestCheckDirect_ProtoResponse(t *testing.T) {
	payload := &pb.PaymentServiceAuthorizeResponse{
		Status: pb.PaymentStatus_STARTED,
	}
	payloadBytes, err := proto.Marshal(payload)
	if err != nil {
		t.Fatalf("marshal payload: %v", err)
	}

	result := &pb.FfiResult{
		Type:    pb.FfiResult_PROTO_RESPONSE,
		Payload: &pb.FfiResult_ProtoResponse{ProtoResponse: payloadBytes},
	}
	b, err := proto.Marshal(result)
	if err != nil {
		t.Fatalf("marshal result: %v", err)
	}

	got, err := payments.CheckDirect(b)
	if err != nil {
		t.Fatalf("CheckDirect: %v", err)
	}

	gotPayload := &pb.PaymentServiceAuthorizeResponse{}
	if err := proto.Unmarshal(got, gotPayload); err != nil {
		t.Fatalf("unmarshal payload: %v", err)
	}
	if gotPayload.GetStatus() != payload.GetStatus() {
		t.Errorf("status = %v, want %v", gotPayload.GetStatus(), payload.GetStatus())
	}
}

func TestCheckDirect_IntegrationError(t *testing.T) {
	result := &pb.FfiResult{
		Type: pb.FfiResult_INTEGRATION_ERROR,
		Payload: &pb.FfiResult_IntegrationError{
			IntegrationError: &pb.IntegrationError{
				ErrorCode:    "IE003",
				ErrorMessage: "transformation failed",
			},
		},
	}
	b, err := proto.Marshal(result)
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}

	_, err = payments.CheckDirect(b)
	if err == nil {
		t.Fatal("expected error, got nil")
	}

	var ie *payments.IntegrationError
	if !errors.As(err, &ie) {
		t.Fatalf("expected *payments.IntegrationError, got %T", err)
	}
}

func TestCheckDirect_UnmarshalError(t *testing.T) {
	_, err := payments.CheckDirect([]byte("not-valid-protobuf"))
	if err == nil {
		t.Fatal("expected error for invalid protobuf, got nil")
	}
}
