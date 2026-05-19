package payments_test

import (
	"context"
	"encoding/pem"
	"errors"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"

	pb "github.com/juspay/hyperswitch-prism/sdk/go/generated/payments"
	"github.com/juspay/hyperswitch-prism/sdk/go/payments"
)

func TestNewHTTPClient_Defaults(t *testing.T) {
	client := payments.NewHTTPClient(nil)
	if client == nil {
		t.Fatal("NewHTTPClient(nil) returned nil")
	}
}

func TestNewHTTPClient_CustomTimeout(t *testing.T) {
	timeoutMs := uint32(5000)
	cfg := &pb.HttpConfig{
		TotalTimeoutMs: &timeoutMs,
	}
	client := payments.NewHTTPClient(cfg)
	// We can only verify it was created without panic;
	// the inner timeout field is not exported.
	if client == nil {
		t.Fatal("NewHTTPClient returned nil")
	}
}

func TestNewHTTPClient_WithProxy(t *testing.T) {
	proxyURL := "http://proxy.example.com:8080"
	cfg := &pb.HttpConfig{
		Proxy: &pb.ProxyOptions{HttpUrl: &proxyURL},
	}
	client := payments.NewHTTPClient(cfg)
	if client == nil {
		t.Fatal("NewHTTPClient returned nil")
	}
}

func TestNewHTTPClient_WithCACertPEM(t *testing.T) {
	server := httptest.NewTLSServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
	}))
	defer server.Close()

	cert := server.Certificate()
	pemBytes := pemEncodeCert(cert.Raw)

	caCert := &pb.CaCert{Format: &pb.CaCert_Pem{Pem: string(pemBytes)}}
	cfg := &pb.HttpConfig{CaCert: caCert}
	client := payments.NewHTTPClient(cfg)
	if client == nil {
		t.Fatal("NewHTTPClient returned nil")
	}

	reqProto := &pb.FfiConnectorHttpRequest{
		Method: "GET",
		Url:    server.URL,
	}
	_, err := client.Execute(context.Background(), reqProto)
	if err != nil {
		t.Fatalf("Execute failed against TLS server: %v", err)
	}
}

func TestHTTPClient_Execute_Get(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.Method != "GET" {
			t.Errorf("method = %q, want GET", r.Method)
		}
		if r.URL.Path != "/test" {
			t.Errorf("path = %q, want /test", r.URL.Path)
		}
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusOK)
		w.Write([]byte(`{"ok": true}`))
	}))
	defer server.Close()

	client := payments.NewHTTPClient(nil)
	reqProto := &pb.FfiConnectorHttpRequest{
		Method: "GET",
		Url:    server.URL + "/test",
		Headers: map[string]string{
			"Accept": "application/json",
		},
	}

	resp, err := client.Execute(context.Background(), reqProto)
	if err != nil {
		t.Fatalf("Execute: %v", err)
	}
	if resp.StatusCode != 200 {
		t.Errorf("status code = %d, want 200", resp.StatusCode)
	}
	if string(resp.Body) != `{"ok": true}` {
		t.Errorf("body = %q, want %q", string(resp.Body), `{"ok": true}`)
	}
	if resp.Headers["Content-Type"] != "application/json" {
		t.Errorf("content-type header = %q, want application/json", resp.Headers["Content-Type"])
	}
}

func TestHTTPClient_Execute_EmptyBody(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.Body != nil {
			buf := make([]byte, 1)
			n, _ := r.Body.Read(buf)
			if n > 0 {
				t.Error("expected empty body")
			}
		}
		w.WriteHeader(http.StatusNoContent)
	}))
	defer server.Close()

	client := payments.NewHTTPClient(nil)
	reqProto := &pb.FfiConnectorHttpRequest{
		Method: "GET",
		Url:    server.URL,
	}

	resp, err := client.Execute(context.Background(), reqProto)
	if err != nil {
		t.Fatalf("Execute: %v", err)
	}
	if resp.StatusCode != 204 {
		t.Errorf("status code = %d, want 204", resp.StatusCode)
	}
}

func TestHTTPClient_Execute_PostWithBody(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.Method != "POST" {
			t.Errorf("method = %q, want POST", r.Method)
		}
		body := make([]byte, r.ContentLength)
		r.Body.Read(body)
		if string(body) != `{"amount":100}` {
			t.Errorf("body = %q, want %q", string(body), `{"amount":100}`)
		}
		w.WriteHeader(http.StatusCreated)
		w.Write([]byte(`{"id":"pay_123"}`))
	}))
	defer server.Close()

	client := payments.NewHTTPClient(nil)
	reqProto := &pb.FfiConnectorHttpRequest{
		Method: "POST",
		Url:    server.URL,
		Headers: map[string]string{
			"Content-Type": "application/json",
		},
		Body: []byte(`{"amount":100}`),
	}

	resp, err := client.Execute(context.Background(), reqProto)
	if err != nil {
		t.Fatalf("Execute: %v", err)
	}
	if resp.StatusCode != 201 {
		t.Errorf("status code = %d, want 201", resp.StatusCode)
	}
}

func TestHTTPClient_Execute_ContextCancellation(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		time.Sleep(100 * time.Millisecond)
		w.WriteHeader(http.StatusOK)
	}))
	defer server.Close()

	client := payments.NewHTTPClient(nil)
	reqProto := &pb.FfiConnectorHttpRequest{
		Method: "GET",
		Url:    server.URL,
	}

	ctx, cancel := context.WithCancel(context.Background())
	cancel()

	_, err := client.Execute(ctx, reqProto)
	if err == nil {
		t.Fatal("expected error for cancelled context, got nil")
	}
}

func TestHTTPClient_Execute_NetworkError(t *testing.T) {
	client := payments.NewHTTPClient(nil)
	reqProto := &pb.FfiConnectorHttpRequest{
		Method: "GET",
		Url:    "http://localhost:1",
	}

	_, err := client.Execute(context.Background(), reqProto)
	if err == nil {
		t.Fatal("expected network error, got nil")
	}

	var netErr *payments.NetworkError
	if !errors.As(err, &netErr) {
		t.Fatalf("expected *payments.NetworkError, got %T", err)
	}
}

func pemEncodeCert(der []byte) []byte {
	block := &pem.Block{Type: "CERTIFICATE", Bytes: der}
	return pem.EncodeToMemory(block)
}
