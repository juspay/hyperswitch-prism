package payments

import pb "github.com/juspay/hyperswitch-prism/sdk/go/generated/payments"

// NewConnectorConfig creates a ConnectorConfig with the given connector config and environment.
func NewConnectorConfig(connectorConfig *pb.ConnectorSpecificConfig, environment pb.Environment) *pb.ConnectorConfig {
	return &pb.ConnectorConfig{
		ConnectorConfig: connectorConfig,
		Options: &pb.SdkOptions{
			Environment: environment,
		},
	}
}

// NewRequestConfig creates a RequestConfig with default values.
func NewRequestConfig() *pb.RequestConfig {
	return &pb.RequestConfig{}
}

// WithHTTPConfig sets the HTTP configuration for a request.
func WithHTTPConfig(req *pb.RequestConfig, httpCfg *pb.HttpConfig) *pb.RequestConfig {
	req.Http = httpCfg
	return req
}

// WithVaultOptions sets the vault configuration for a request.
func WithVaultOptions(req *pb.RequestConfig, vaultOpts *pb.VaultOptions) *pb.RequestConfig {
	req.Vault = vaultOpts
	return req
}

// NewHTTPConfig creates an HttpConfig with the given timeout.
func NewHTTPConfig(totalTimeoutMs uint32) *pb.HttpConfig {
	return &pb.HttpConfig{
		TotalTimeoutMs: &totalTimeoutMs,
	}
}

// WithConnectTimeout sets the connection timeout for an HTTP config.
func WithConnectTimeout(httpCfg *pb.HttpConfig, timeoutMs uint32) *pb.HttpConfig {
	httpCfg.ConnectTimeoutMs = &timeoutMs
	return httpCfg
}

// WithResponseTimeout sets the response timeout for an HTTP config.
func WithResponseTimeout(httpCfg *pb.HttpConfig, timeoutMs uint32) *pb.HttpConfig {
	httpCfg.ResponseTimeoutMs = &timeoutMs
	return httpCfg
}

// WithProxy sets the proxy URL for an HTTP config.
func WithProxy(httpCfg *pb.HttpConfig, proxyURL string) *pb.HttpConfig {
	httpCfg.Proxy = &pb.ProxyOptions{HttpUrl: &proxyURL}
	return httpCfg
}
