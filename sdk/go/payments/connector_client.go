package payments

import (
	"context"
	"fmt"

	"google.golang.org/protobuf/proto"
	pb "github.com/juspay/hyperswitch-prism/sdk/go/generated/payments"
)

// ConnectorClient is the base client for all connector service flows.
// It handles the full round-trip: FFI req transformer → HTTP → FFI res transformer.
type ConnectorClient struct {
	config     *pb.ConnectorConfig
	defaults   *pb.RequestConfig
	httpClient *HTTPClient
}

// NewConnectorClient creates a new ConnectorClient.
func NewConnectorClient(config *pb.ConnectorConfig, defaults *pb.RequestConfig) *ConnectorClient {
	if defaults == nil {
		defaults = &pb.RequestConfig{}
	}
	return &ConnectorClient{
		config:     config,
		defaults:   defaults,
		httpClient: NewHTTPClient(MergeHTTPConfig(nil, defaults.GetHttp())),
	}
}

// ExecuteFlow performs a full payment flow round-trip.
//
// 1. Calls reqTransformer to build an HTTP request envelope.
// 2. Executes the HTTP request via net/http.
// 3. Calls resTransformer to parse the connector response.
// 4. Deserializes the domain response into the provided proto message.
func (c *ConnectorClient) ExecuteFlow(
	ctx context.Context,
	reqTransformer func([]byte, []byte) []byte,
	resTransformer func([]byte, []byte, []byte) []byte,
	request proto.Message,
	response proto.Message,
	options *pb.RequestConfig,
) error {
	// 1. Serialize request protobuf.
	reqBytes, err := proto.Marshal(request)
	if err != nil {
		return fmt.Errorf("failed to marshal request: %w", err)
	}

	// 2. Build FFI options.
	ffiOpts := c.ResolveOptions(options)
	optsBytes, err := proto.Marshal(ffiOpts)
	if err != nil {
		return fmt.Errorf("failed to marshal FFI options: %w", err)
	}

	// 3. Call req_transformer via FFI.
	resultBytes := reqTransformer(reqBytes, optsBytes)

	// 4. Parse FfiResult → HTTP request envelope (or error).
	httpReq, err := CheckReq(resultBytes)
	if err != nil {
		return err
	}

	// 5. Execute HTTP call.
	httpRes, err := c.httpClient.Execute(ctx, httpReq)
	if err != nil {
		return err
	}

	// 6. Build FfiConnectorHttpResponse proto.
	resProto := &pb.FfiConnectorHttpResponse{
		StatusCode: uint32(httpRes.StatusCode),
		Headers:    httpRes.Headers,
		Body:       httpRes.Body,
	}
	resBytes, err := proto.Marshal(resProto)
	if err != nil {
		return fmt.Errorf("failed to marshal HTTP response: %w", err)
	}

	// 7. Call res_transformer via FFI.
	resultBytesRes := resTransformer(resBytes, reqBytes, optsBytes)

	// 8. Parse FfiResult → HTTP response envelope (or error).
	ffiRes, err := CheckRes(resultBytesRes)
	if err != nil {
		return err
	}

	// 9. Deserialize domain response.
	if err := proto.Unmarshal(ffiRes.GetBody(), response); err != nil {
		return fmt.Errorf("failed to unmarshal domain response: %w", err)
	}

	return nil
}

// ExecuteDirect performs a single-step flow with no HTTP round-trip.
// Used for inbound flows like webhook processing.
func (c *ConnectorClient) ExecuteDirect(
	ctx context.Context,
	transformer func([]byte, []byte) []byte,
	request proto.Message,
	response proto.Message,
	options *pb.RequestConfig,
) error {
	// 1. Serialize request protobuf.
	reqBytes, err := proto.Marshal(request)
	if err != nil {
		return fmt.Errorf("failed to marshal request: %w", err)
	}

	// 2. Build FFI options.
	ffiOpts := c.ResolveOptions(options)
	optsBytes, err := proto.Marshal(ffiOpts)
	if err != nil {
		return fmt.Errorf("failed to marshal FFI options: %w", err)
	}

	// 3. Call direct transformer via FFI.
	resultBytes := transformer(reqBytes, optsBytes)

	// 4. Parse FfiResult → proto response bytes (or error).
	protoResBytes, err := CheckDirect(resultBytes)
	if err != nil {
		return err
	}

	// 5. Deserialize domain response.
	if err := proto.Unmarshal(protoResBytes, response); err != nil {
		return fmt.Errorf("failed to unmarshal domain response: %w", err)
	}

	return nil
}

// ResolveOptions merges per-request options over client defaults.
func (c *ConnectorClient) ResolveOptions(options *pb.RequestConfig) *pb.FfiOptions {
	environment := c.config.GetOptions().GetEnvironment()

	return &pb.FfiOptions{
		Environment:      environment,
		ConnectorConfig:  c.config.GetConnectorConfig(),
	}
}

// MergeHTTPConfig merges override HTTP config over base config.
// Returns the effective HTTP configuration.
func MergeHTTPConfig(base, override *pb.HttpConfig) *pb.HttpConfig {
	if override == nil {
		return base
	}
	if base == nil {
		return override
	}

	result := proto.Clone(base).(*pb.HttpConfig)

	if override.GetTotalTimeoutMs() > 0 {
		result.TotalTimeoutMs = override.TotalTimeoutMs
	}
	if override.GetConnectTimeoutMs() > 0 {
		result.ConnectTimeoutMs = override.ConnectTimeoutMs
	}
	if override.GetResponseTimeoutMs() > 0 {
		result.ResponseTimeoutMs = override.ResponseTimeoutMs
	}
	if override.GetProxy() != nil {
		result.Proxy = override.Proxy
	}
	if override.GetCaCert() != nil {
		result.CaCert = override.CaCert
	}

	return result
}
