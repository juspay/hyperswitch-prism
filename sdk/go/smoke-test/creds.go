package main

import (
	"encoding/json"
	"fmt"
	"os"
	"strings"

	"google.golang.org/protobuf/proto"
	pb "github.com/juspay/hyperswitch-prism/sdk/go/generated/payments"
)

var placeholderValues = map[string]struct{}{
	"":                      {},
	"placeholder":           {},
	"test":                  {},
	"dummy":                 {},
	"sk_test_placeholder":   {},
	"<replace_with_your_value>": {},
	"<REPLACE_WITH_YOUR_VALUE>": {},
}

func isPlaceholder(value string) bool {
	if value == "" {
		return true
	}
	lower := strings.ToLower(value)
	if _, ok := placeholderValues[lower]; ok {
		return true
	}
	return strings.Contains(lower, "placeholder") || strings.Contains(lower, "<replace")
}

// loadCredentials loads connector credentials from a JSON file.
func loadCredentials(credsFile string) (map[string]map[string]interface{}, error) {
	data, err := os.ReadFile(credsFile)
	if err != nil {
		return nil, fmt.Errorf("failed to read credentials file: %w", err)
	}

	var creds map[string]map[string]interface{}
	if err := json.Unmarshal(data, &creds); err != nil {
		return nil, fmt.Errorf("failed to parse credentials JSON: %w", err)
	}

	return creds, nil
}

// hasValidCredentials checks if any field in the auth config has a non-placeholder value.
func hasValidCredentials(authConfig map[string]interface{}) bool {
	for key, value := range authConfig {
		if key == "metadata" || key == "_comment" {
			continue
		}
		if m, ok := value.(map[string]interface{}); ok {
			if v, ok := m["value"].(string); ok && !isPlaceholder(v) {
				return true
			}
		} else if s, ok := value.(string); ok && !isPlaceholder(s) {
			return true
		}
	}
	return false
}

// getStringValue extracts a string value from the auth config, handling both
// plain strings and {"value": "..."} objects.
func getStringValue(authConfig map[string]interface{}, key string) (string, bool) {
	v, ok := authConfig[key]
	if !ok {
		return "", false
	}
	if m, ok := v.(map[string]interface{}); ok {
		if s, ok := m["value"].(string); ok {
			return s, true
		}
		return "", false
	}
	if s, ok := v.(string); ok {
		return s, true
	}
	return "", false
}

// getSecretString extracts a SecretString from the auth config.
func getSecretString(authConfig map[string]interface{}, key string) *pb.SecretString {
	if s, ok := getStringValue(authConfig, key); ok && s != "" {
		return &pb.SecretString{Value: s}
	}
	return nil
}

// getOptionalString extracts an optional string pointer from the auth config.
func getOptionalString(authConfig map[string]interface{}, key string) *string {
	if s, ok := getStringValue(authConfig, key); ok && s != "" {
		return proto.String(s)
	}
	return nil
}

// connectorConfigBuilder is a function that builds a ConnectorSpecificConfig
// from an auth config map.
type connectorConfigBuilder func(map[string]interface{}) *pb.ConnectorSpecificConfig

// configBuilders maps connector names to their config builder functions.
var configBuilders = map[string]connectorConfigBuilder{
	"stripe":            buildStripeConfig,
	"adyen":             buildAdyenConfig,
	"adyen_payout":      buildAdyenConfig,
	"braintree":         buildBraintreeConfig,
	"cybersource":       buildCybersourceConfig,
	"authorizedotnet":   buildAuthorizeDotNetConfig,
}

// buildConnectorConfig builds a ConnectorConfig for the given connector.
// Returns nil if the connector is not supported.
func buildConnectorConfig(connectorName string, authConfig map[string]interface{}) *pb.ConnectorConfig {
	builder, ok := configBuilders[connectorName]
	if !ok {
		return nil
	}

	connectorSpecific := builder(authConfig)
	if connectorSpecific == nil {
		return nil
	}

	return &pb.ConnectorConfig{
		ConnectorConfig: connectorSpecific,
		Options: &pb.SdkOptions{
			Environment: pb.Environment_SANDBOX,
		},
	}
}

// buildStripeConfig builds a StripeConfig from auth credentials.
func buildStripeConfig(auth map[string]interface{}) *pb.ConnectorSpecificConfig {
	cfg := &pb.StripeConfig{}
	if v := getSecretString(auth, "api_key"); v != nil {
		cfg.ApiKey = v
	}
	if v := getOptionalString(auth, "base_url"); v != nil {
		cfg.BaseUrl = v
	}
	return &pb.ConnectorSpecificConfig{
		Config: &pb.ConnectorSpecificConfig_Stripe{Stripe: cfg},
	}
}

// buildAdyenConfig builds an AdyenConfig from auth credentials.
func buildAdyenConfig(auth map[string]interface{}) *pb.ConnectorSpecificConfig {
	cfg := &pb.AdyenConfig{}
	if v := getSecretString(auth, "api_key"); v != nil {
		cfg.ApiKey = v
	}
	if v := getSecretString(auth, "merchant_account"); v != nil {
		cfg.MerchantAccount = v
	}
	if v := getSecretString(auth, "review_key"); v != nil {
		cfg.ReviewKey = v
	}
	if v := getOptionalString(auth, "base_url"); v != nil {
		cfg.BaseUrl = v
	}
	return &pb.ConnectorSpecificConfig{
		Config: &pb.ConnectorSpecificConfig_Adyen{Adyen: cfg},
	}
}

// buildBraintreeConfig builds a BraintreeConfig from auth credentials.
func buildBraintreeConfig(auth map[string]interface{}) *pb.ConnectorSpecificConfig {
	cfg := &pb.BraintreeConfig{}
	if v := getSecretString(auth, "public_key"); v != nil {
		cfg.PublicKey = v
	}
	if v := getSecretString(auth, "private_key"); v != nil {
		cfg.PrivateKey = v
	}
	if v := getSecretString(auth, "merchant_account_id"); v != nil {
		cfg.MerchantAccountId = v
	}
	if v := getOptionalString(auth, "base_url"); v != nil {
		cfg.BaseUrl = v
	}
	return &pb.ConnectorSpecificConfig{
		Config: &pb.ConnectorSpecificConfig_Braintree{Braintree: cfg},
	}
}

// buildCybersourceConfig builds a CybersourceConfig from auth credentials.
func buildCybersourceConfig(auth map[string]interface{}) *pb.ConnectorSpecificConfig {
	cfg := &pb.CybersourceConfig{}
	if v := getSecretString(auth, "api_key"); v != nil {
		cfg.ApiKey = v
	}
	if v := getSecretString(auth, "api_secret"); v != nil {
		cfg.ApiSecret = v
	}
	if v := getSecretString(auth, "merchant_account"); v != nil {
		cfg.MerchantAccount = v
	}
	if v := getOptionalString(auth, "base_url"); v != nil {
		cfg.BaseUrl = v
	}
	return &pb.ConnectorSpecificConfig{
		Config: &pb.ConnectorSpecificConfig_Cybersource{Cybersource: cfg},
	}
}

// buildAuthorizeDotNetConfig builds an AuthorizedotnetConfig from auth credentials.
func buildAuthorizeDotNetConfig(auth map[string]interface{}) *pb.ConnectorSpecificConfig {
	cfg := &pb.AuthorizedotnetConfig{}
	if v := getSecretString(auth, "name"); v != nil {
		cfg.Name = v
	}
	if v := getSecretString(auth, "transaction_key"); v != nil {
		cfg.TransactionKey = v
	}
	if v := getOptionalString(auth, "base_url"); v != nil {
		cfg.BaseUrl = v
	}
	return &pb.ConnectorSpecificConfig{
		Config: &pb.ConnectorSpecificConfig_Authorizedotnet{Authorizedotnet: cfg},
	}
}
