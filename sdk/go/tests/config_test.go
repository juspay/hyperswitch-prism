package payments_test

import (
	"testing"

	pb "github.com/juspay/hyperswitch-prism/sdk/go/generated/payments"
	"github.com/juspay/hyperswitch-prism/sdk/go/payments"
)

func TestNewConnectorConfig(t *testing.T) {
	connConfig := &pb.ConnectorSpecificConfig{}
	cfg := payments.NewConnectorConfig(connConfig, pb.Environment_SANDBOX)

	if cfg.GetConnectorConfig() != connConfig {
		t.Error("ConnectorConfig mismatch")
	}
	if cfg.GetOptions().GetEnvironment() != pb.Environment_SANDBOX {
		t.Errorf("environment = %v, want Sandbox", cfg.GetOptions().GetEnvironment())
	}
}

func TestNewRequestConfig(t *testing.T) {
	req := payments.NewRequestConfig()
	if req == nil {
		t.Fatal("NewRequestConfig returned nil")
	}
}

func TestWithHTTPConfig(t *testing.T) {
	req := payments.NewRequestConfig()
	httpCfg := payments.NewHTTPConfig(5000)
	result := payments.WithHTTPConfig(req, httpCfg)

	if result.GetHttp().GetTotalTimeoutMs() != 5000 {
		t.Errorf("total timeout = %d, want 5000", result.GetHttp().GetTotalTimeoutMs())
	}
}

func TestWithVaultOptions(t *testing.T) {
	req := payments.NewRequestConfig()
	vaultOpts := &pb.VaultOptions{}
	result := payments.WithVaultOptions(req, vaultOpts)

	if result.GetVault() != vaultOpts {
		t.Error("Vault options mismatch")
	}
}

func TestNewHTTPConfig(t *testing.T) {
	cfg := payments.NewHTTPConfig(10000)
	if cfg.GetTotalTimeoutMs() != 10000 {
		t.Errorf("total timeout = %d, want 10000", cfg.GetTotalTimeoutMs())
	}
}

func TestWithConnectTimeout(t *testing.T) {
	cfg := payments.NewHTTPConfig(10000)
	result := payments.WithConnectTimeout(cfg, 3000)
	if result.GetConnectTimeoutMs() != 3000 {
		t.Errorf("connect timeout = %d, want 3000", result.GetConnectTimeoutMs())
	}
}

func TestWithResponseTimeout(t *testing.T) {
	cfg := payments.NewHTTPConfig(10000)
	result := payments.WithResponseTimeout(cfg, 5000)
	if result.GetResponseTimeoutMs() != 5000 {
		t.Errorf("response timeout = %d, want 5000", result.GetResponseTimeoutMs())
	}
}

func TestWithProxy(t *testing.T) {
	cfg := payments.NewHTTPConfig(10000)
	result := payments.WithProxy(cfg, "http://proxy.example.com:8080")
	if result.GetProxy().GetHttpUrl() != "http://proxy.example.com:8080" {
		t.Errorf("proxy url = %q, want %q", result.GetProxy().GetHttpUrl(), "http://proxy.example.com:8080")
	}
}

func TestWithProxy_NilConfig(t *testing.T) {
	cfg := &pb.HttpConfig{}
	result := payments.WithProxy(cfg, "http://localhost:3128")
	if result.GetProxy() == nil {
		t.Fatal("proxy is nil")
	}
	if result.GetProxy().GetHttpUrl() != "http://localhost:3128" {
		t.Errorf("proxy url = %q", result.GetProxy().GetHttpUrl())
	}
}
