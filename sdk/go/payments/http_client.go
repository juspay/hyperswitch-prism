package payments
//
import (
	"bytes"
	"context"
	"crypto/tls"
	"crypto/x509"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"strings"
	"time"

	pb "github.com/juspay/hyperswitch-prism/sdk/go/generated/payments"
)

// HTTPResponse is a simplified HTTP response struct.
type HTTPResponse struct {
	StatusCode int
	Headers    map[string]string
	Body       []byte
}

// HTTPClient wraps net/http with connector-specific configuration.
type HTTPClient struct {
	client *http.Client
}

// NewHTTPClient creates an HTTP client from HttpConfig proto.
func NewHTTPClient(cfg *pb.HttpConfig) *HTTPClient {
	transport := &http.Transport{
		MaxIdleConns:        100,
		MaxIdleConnsPerHost: 10,
		IdleConnTimeout:     90 * time.Second,
	}

	if cfg != nil {
		if cfg.GetProxy() != nil && cfg.GetProxy().GetHttpUrl() != "" {
			proxyURL, err := url.Parse(cfg.GetProxy().GetHttpUrl())
			if err == nil {
				transport.Proxy = http.ProxyURL(proxyURL)
			}
		}

		if cfg.GetCaCert() != nil {
			certPool := x509.NewCertPool()
			caCert := cfg.GetCaCert()
			if caCert.GetPem() != "" {
				if certPool.AppendCertsFromPEM([]byte(caCert.GetPem())) {
					transport.TLSClientConfig = &tls.Config{
						RootCAs: certPool,
					}
				}
			} else if len(caCert.GetDer()) > 0 {
				if certPool.AppendCertsFromPEM(caCert.GetDer()) {
					transport.TLSClientConfig = &tls.Config{
						RootCAs: certPool,
					}
				}
			}
		}
	}

	timeout := 30 * time.Second
	if cfg != nil && cfg.GetTotalTimeoutMs() > 0 {
		timeout = time.Duration(cfg.GetTotalTimeoutMs()) * time.Millisecond
	}

	return &HTTPClient{
		client: &http.Client{
			Transport: transport,
			Timeout:   timeout,
		},
	}
}

// Execute performs an HTTP request from an FfiConnectorHttpRequest proto.
func (c *HTTPClient) Execute(ctx context.Context, reqProto *pb.FfiConnectorHttpRequest) (*HTTPResponse, error) {
	var body io.Reader
	if reqProto.GetBody() != nil && len(reqProto.GetBody()) > 0 {
		body = bytes.NewReader(reqProto.GetBody())
	}

	req, err := http.NewRequestWithContext(ctx, reqProto.GetMethod(), reqProto.GetUrl(), body)
	if err != nil {
		return nil, fmt.Errorf("failed to create HTTP request: %w", err)
	}

	for k, v := range reqProto.GetHeaders() {
		req.Header.Set(k, v)
	}

	resp, err := c.client.Do(req)
	if err != nil {
		return nil, &NetworkError{Message: err.Error()}
	}
	defer resp.Body.Close()

	respBody, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, fmt.Errorf("failed to read response body: %w", err)
	}

	headers := make(map[string]string)
	for k, v := range resp.Header {
		if len(v) > 0 {
			headers[k] = strings.Join(v, ", ")
		}
	}

	return &HTTPResponse{
		StatusCode: resp.StatusCode,
		Headers:    headers,
		Body:       respBody,
	}, nil
}
