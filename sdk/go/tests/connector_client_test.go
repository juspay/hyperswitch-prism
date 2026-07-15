package payments_test
//
import (
	"context"
	"errors"
	"net/http"
	"net/http/httptest"
	"testing"

	"google.golang.org/protobuf/proto"
	pb "github.com/juspay/hyperswitch-prism/sdk/go/generated/payments"
	"github.com/juspay/hyperswitch-prism/sdk/go/payments"
)

// stubReqTransformer returns an FfiResult containing the given HTTP request.
func stubReqTransformer(reqProto *pb.FfiConnectorHttpRequest) func([]byte, []byte) []byte {
	return func(_, _ []byte) []byte {
		result := &pb.FfiResult{
			Type:    pb.FfiResult_HTTP_REQUEST,
			Payload: &pb.FfiResult_HttpRequest{HttpRequest: reqProto},
		}
		b, _ := proto.Marshal(result)
		return b
	}
}

// stubResTransformer returns an FfiResult containing the given proto response.
func stubResTransformer(response proto.Message) func([]byte, []byte, []byte) []byte {
	return func(_, _, _ []byte) []byte {
		body, _ := proto.Marshal(response)
		result := &pb.FfiResult{
			Type:    pb.FfiResult_HTTP_RESPONSE,
			Payload: &pb.FfiResult_HttpResponse{HttpResponse: &pb.FfiConnectorHttpResponse{StatusCode: 200, Body: body}},
		}
		b, _ := proto.Marshal(result)
		return b
	}
}

// stubDirectTransformer returns an FfiResult with PROTO_RESPONSE.
func stubDirectTransformer(response proto.Message) func([]byte, []byte) []byte {
	return func(_, _ []byte) []byte {
		body, _ := proto.Marshal(response)
		result := &pb.FfiResult{
			Type:    pb.FfiResult_PROTO_RESPONSE,
			Payload: &pb.FfiResult_ProtoResponse{ProtoResponse: body},
		}
		b, _ := proto.Marshal(result)
		return b
	}
}

func TestExecuteFlow_Success(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
		w.Write([]byte(`{"status":"authorized"}`))
	}))
	defer server.Close()

	client := payments.NewConnectorClient(&pb.ConnectorConfig{}, &pb.RequestConfig{})

	reqProto := &pb.FfiConnectorHttpRequest{
		Method: "POST",
		Url:    server.URL,
	}

	req := &pb.PaymentServiceAuthorizeRequest{}
	res := &pb.PaymentServiceAuthorizeResponse{}

	stubReq := stubReqTransformer(reqProto)
	stubRes := stubResTransformer(res)

	err := client.ExecuteFlow(context.Background(), stubReq, stubRes, req, res, nil)
	if err != nil {
		t.Fatalf("ExecuteFlow: %v", err)
	}
}

func TestExecuteFlow_ReqTransformerError(t *testing.T) {
	client := payments.NewConnectorClient(&pb.ConnectorConfig{}, &pb.RequestConfig{})

	reqTransformer := func(_, _ []byte) []byte {
		result := &pb.FfiResult{
			Type: pb.FfiResult_INTEGRATION_ERROR,
			Payload: &pb.FfiResult_IntegrationError{
				IntegrationError: &pb.IntegrationError{
					ErrorCode:    "IE_TEST",
					ErrorMessage: "req transform failed",
				},
			},
		}
		b, _ := proto.Marshal(result)
		return b
	}

	req := &pb.PaymentServiceAuthorizeRequest{}
	res := &pb.PaymentServiceAuthorizeResponse{}

	err := client.ExecuteFlow(context.Background(), reqTransformer, nil, req, res, nil)
	if err == nil {
		t.Fatal("expected error, got nil")
	}

	var ie *payments.IntegrationError
	if !errors.As(err, &ie) {
		t.Fatalf("expected *payments.IntegrationError, got %T", err)
	}
}

func TestExecuteFlow_ResTransformerError(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
	}))
	defer server.Close()

	client := payments.NewConnectorClient(&pb.ConnectorConfig{}, &pb.RequestConfig{})

	reqProto := &pb.FfiConnectorHttpRequest{Method: "GET", Url: server.URL}
	stubReq := stubReqTransformer(reqProto)

	resTransformer := func(_, _, _ []byte) []byte {
		result := &pb.FfiResult{
			Type: pb.FfiResult_CONNECTOR_ERROR,
			Payload: &pb.FfiResult_ConnectorError{
				ConnectorError: &pb.ConnectorError{
					ErrorCode:    "CE_TEST",
					ErrorMessage: "res transform failed",
				},
			},
		}
		b, _ := proto.Marshal(result)
		return b
	}

	req := &pb.PaymentServiceAuthorizeRequest{}
	res := &pb.PaymentServiceAuthorizeResponse{}

	err := client.ExecuteFlow(context.Background(), stubReq, resTransformer, req, res, nil)
	if err == nil {
		t.Fatal("expected error, got nil")
	}

	var ce *payments.ConnectorError
	if !errors.As(err, &ce) {
		t.Fatalf("expected *payments.ConnectorError, got %T", err)
	}
}

func TestExecuteDirect_Success(t *testing.T) {
	client := payments.NewConnectorClient(&pb.ConnectorConfig{}, &pb.RequestConfig{})

	req := &pb.EventServiceHandleRequest{}
	res := &pb.EventServiceHandleResponse{}

	err := client.ExecuteDirect(context.Background(), stubDirectTransformer(res), req, res, nil)
	if err != nil {
		t.Fatalf("ExecuteDirect: %v", err)
	}
}

func TestExecuteDirect_TransformerError(t *testing.T) {
	client := payments.NewConnectorClient(&pb.ConnectorConfig{}, &pb.RequestConfig{})

	transformer := func(_, _ []byte) []byte {
		result := &pb.FfiResult{
			Type: pb.FfiResult_INTEGRATION_ERROR,
			Payload: &pb.FfiResult_IntegrationError{
				IntegrationError: &pb.IntegrationError{
					ErrorCode:    "IE_DIRECT",
					ErrorMessage: "direct transform failed",
				},
			},
		}
		b, _ := proto.Marshal(result)
		return b
	}

	req := &pb.EventServiceHandleRequest{}
	res := &pb.EventServiceHandleResponse{}

	err := client.ExecuteDirect(context.Background(), transformer, req, res, nil)
	if err == nil {
		t.Fatal("expected error, got nil")
	}
}

func TestResolveOptions(t *testing.T) {
	cfg := &pb.ConnectorConfig{
		ConnectorConfig: &pb.ConnectorSpecificConfig{},
		Options: &pb.SdkOptions{
			Environment: pb.Environment_SANDBOX,
		},
	}
	client := payments.NewConnectorClient(cfg, &pb.RequestConfig{})

	opts := client.ResolveOptions(nil)
	if opts.GetEnvironment() != pb.Environment_SANDBOX {
		t.Errorf("environment = %v, want SANDBOX", opts.GetEnvironment())
	}
}

func TestMergeHTTPConfig_NilOverride(t *testing.T) {
	base := &pb.HttpConfig{TotalTimeoutMs: proto.Uint32(1000)}
	result := payments.MergeHTTPConfig(base, nil)
	if result.GetTotalTimeoutMs() != 1000 {
		t.Errorf("total timeout = %d, want 1000", result.GetTotalTimeoutMs())
	}
}

func TestMergeHTTPConfig_NilBase(t *testing.T) {
	override := &pb.HttpConfig{TotalTimeoutMs: proto.Uint32(2000)}
	result := payments.MergeHTTPConfig(nil, override)
	if result.GetTotalTimeoutMs() != 2000 {
		t.Errorf("total timeout = %d, want 2000", result.GetTotalTimeoutMs())
	}
}

func TestMergeHTTPConfig_BothNil(t *testing.T) {
	result := payments.MergeHTTPConfig(nil, nil)
	if result != nil {
		t.Fatalf("expected nil, got %v", result)
	}
}

func TestMergeHTTPConfig_MergeFields(t *testing.T) {
	base := &pb.HttpConfig{
		TotalTimeoutMs: proto.Uint32(1000),
	}
	override := &pb.HttpConfig{
		TotalTimeoutMs:   proto.Uint32(2000),
		ConnectTimeoutMs: proto.Uint32(500),
	}
	result := payments.MergeHTTPConfig(base, override)
	if result.GetTotalTimeoutMs() != 2000 {
		t.Errorf("total timeout = %d, want 2000", result.GetTotalTimeoutMs())
	}
	if result.GetConnectTimeoutMs() != 500 {
		t.Errorf("connect timeout = %d, want 500", result.GetConnectTimeoutMs())
	}
}
