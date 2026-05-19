package main
//
import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"

	"google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/reflect/protoreflect"
	pb "github.com/juspay/hyperswitch-prism/sdk/go/generated/payments"
	"github.com/juspay/hyperswitch-prism/sdk/go/payments"
)

// FlowManifest holds the parsed flows.json.
type FlowManifest struct {
	Flows           []string          `json:"flows"`
	FlowToExampleFn map[string]string `json:"flow_to_example_fn"`
}

// loadFlowManifest reads flows.json from multiple possible locations.
func loadFlowManifest(sdkRoot string) (*FlowManifest, error) {
	locations := []string{}

	if envPath := os.Getenv("FLOWS_JSON_PATH"); envPath != "" {
		locations = append(locations, envPath)
	}

	locations = append(locations,
		filepath.Join(sdkRoot, "generated", "flows.json"),
		filepath.Join(sdkRoot, "..", "..", "generated", "flows.json"),
		"flows.json",
	)

	for _, loc := range locations {
		data, err := os.ReadFile(loc)
		if err != nil {
			continue
		}
		var manifest FlowManifest
		if err := json.Unmarshal(data, &manifest); err != nil {
			continue
		}
		return &manifest, nil
	}

	return nil, fmt.Errorf("flows.json not found. Searched: %v", locations)
}

// setMockBaseURL uses protoreflect to set the base_url field on the
// connector-specific config message (e.g. StripeConfig, AdyenConfig).
func setMockBaseURL(connectorSpecific *pb.ConnectorSpecificConfig, baseURL string) bool {
	msg := connectorSpecific.ProtoReflect()
	desc := msg.Descriptor()

	// Find the "config" oneof.
	oneofDesc := desc.Oneofs().ByName("config")
	if oneofDesc == nil {
		return false
	}

	// Determine which oneof variant is set.
	field := msg.WhichOneof(oneofDesc)
	if field == nil {
		return false
	}

	// Get the nested message and look for a base_url field.
	nestedMsg := msg.Get(field).Message()
	nestedDesc := nestedMsg.Descriptor()

	baseURLField := nestedDesc.Fields().ByName("base_url")
	if baseURLField == nil {
		return false
	}

	nestedMsg.Set(baseURLField, protoreflect.ValueOfString(baseURL))
	return true
}

// captureMethodPtr returns a pointer to a CaptureMethod enum value.
func captureMethodPtr(cm pb.CaptureMethod) *pb.CaptureMethod {
	return &cm
}

// ============================================================================
// Request builders
// ============================================================================

func buildAuthorizeRequest(captureMethod pb.CaptureMethod) *pb.PaymentServiceAuthorizeRequest {
	return &pb.PaymentServiceAuthorizeRequest{
		MerchantTransactionId: proto.String("smoke_test_txn"),
		Amount: &pb.Money{
			MinorAmount: 1000,
			Currency:    pb.Currency_USD,
		},
		PaymentMethod: &pb.PaymentMethod{
			PaymentMethod: &pb.PaymentMethod_Card{
				Card: &pb.CardDetails{
					CardNumber:     &pb.CardNumberType{Value: "4111111111111111"},
					CardExpMonth:   &pb.SecretString{Value: "03"},
					CardExpYear:    &pb.SecretString{Value: "2030"},
					CardCvc:        &pb.SecretString{Value: "737"},
					CardHolderName: &pb.SecretString{Value: "John Doe"},
				},
			},
		},
		CaptureMethod: captureMethodPtr(captureMethod),
		Address: &pb.PaymentAddress{
			BillingAddress: &pb.Address{},
		},
		AuthType:  pb.AuthenticationType_NO_THREE_DS,
		ReturnUrl: proto.String("https://example.com/return"),
	}
}

func buildCaptureRequest(connectorTransactionID string) *pb.PaymentServiceCaptureRequest {
	return &pb.PaymentServiceCaptureRequest{
		MerchantCaptureId:      proto.String("smoke_capture_001"),
		ConnectorTransactionId: connectorTransactionID,
		AmountToCapture: &pb.Money{
			MinorAmount: 1000,
			Currency:    pb.Currency_USD,
		},
	}
}

func buildGetRequest(connectorTransactionID string) *pb.PaymentServiceGetRequest {
	return &pb.PaymentServiceGetRequest{
		MerchantTransactionId:  proto.String("smoke_test_txn"),
		ConnectorTransactionId: connectorTransactionID,
		Amount: &pb.Money{
			MinorAmount: 1000,
			Currency:    pb.Currency_USD,
		},
	}
}

func buildRefundRequest(connectorTransactionID string) *pb.PaymentServiceRefundRequest {
	return &pb.PaymentServiceRefundRequest{
		MerchantRefundId:       proto.String("smoke_refund_001"),
		ConnectorTransactionId: connectorTransactionID,
		PaymentAmount:          1000,
		RefundAmount: &pb.Money{
			MinorAmount: 1000,
			Currency:    pb.Currency_USD,
		},
		Reason: proto.String("customer_request"),
	}
}

func buildVoidRequest(connectorTransactionID string) *pb.PaymentServiceVoidRequest {
	return &pb.PaymentServiceVoidRequest{
		MerchantVoidId:         proto.String("smoke_void_001"),
		ConnectorTransactionId: connectorTransactionID,
	}
}

// ============================================================================
// Scenario runners
// ============================================================================

// ScenarioFunc runs a single scenario against a PaymentClient.
type ScenarioFunc func(ctx context.Context, client *payments.PaymentClient) error

// scenarioRegistry maps example-function names to their implementations.
var scenarioRegistry = map[string]ScenarioFunc{
	"checkout_card":        runCheckoutCard,
	"checkout_autocapture": runCheckoutAutocapture,
	"get_payment":          runGetPayment,
	"refund":               runRefund,
	"void_payment":         runVoidPayment,
}

func runCheckoutCard(ctx context.Context, client *payments.PaymentClient) error {
	req := buildAuthorizeRequest(pb.CaptureMethod_MANUAL)
	_, err := client.Authorize(ctx, req, nil)
	return err
}

func runCheckoutAutocapture(ctx context.Context, client *payments.PaymentClient) error {
	req := buildAuthorizeRequest(pb.CaptureMethod_AUTOMATIC)
	_, err := client.Authorize(ctx, req, nil)
	return err
}

func runGetPayment(ctx context.Context, client *payments.PaymentClient) error {
	req := buildGetRequest("smoke_connector_txn_001")
	_, err := client.Get(ctx, req, nil)
	return err
}

func runRefund(ctx context.Context, client *payments.PaymentClient) error {
	// In a real composite scenario we'd authorize first and use the returned
	// connector_transaction_id.  For the smoke test we use a placeholder.
	req := buildRefundRequest("smoke_connector_txn_001")
	_, err := client.Refund(ctx, req, nil)
	return err
}

func runVoidPayment(ctx context.Context, client *payments.PaymentClient) error {
	req := buildVoidRequest("smoke_connector_txn_001")
	_, err := client.Void(ctx, req, nil)
	return err
}

// ============================================================================
// Connector scenario execution
// ============================================================================

func runConnectorScenarios(
	connectorName string,
	config *pb.ConnectorConfig,
	sdkRoot string,
	dryRun bool,
	mock bool,
) *ConnectorResult {
	result := &ConnectorResult{
		Connector: connectorName,
		Status:    "passed",
		Scenarios: make(map[string]*ScenarioResult),
	}

	if dryRun {
		result.Status = "dry_run"
		return result
	}

	// Load flow manifest.
	manifest, err := loadFlowManifest(sdkRoot)
	if err != nil {
		result.Status = "failed"
		result.Error = fmt.Sprintf("manifest error: %v", err)
		return result
	}

	// Setup mock server if requested.
	var mockServer *httptest.Server
	if mock {
		mockServer = httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			fmt.Fprintf(os.Stderr, grey("      [mock]")+" %s %s\n", r.Method, r.URL.Path)
			w.Header().Set("Content-Type", "application/json")
			w.WriteHeader(http.StatusOK)
			w.Write([]byte("{}"))
		}))
		defer mockServer.Close()

		if config.GetConnectorConfig() != nil {
			if !setMockBaseURL(config.ConnectorConfig, mockServer.URL+"/") {
				fmt.Fprintf(os.Stderr, yellow("  [warn]")+" Could not set mock base URL for %s\n", connectorName)
			}
		}
	}

	client := payments.NewPaymentClient(config, &pb.RequestConfig{})
	anyFailed := false

	for _, flow := range manifest.Flows {
		// Use the flow name as the result key (matches Python behaviour).
		scenarioKey := flow

		// Look up the example function name for this flow.
		lookupKey := manifest.FlowToExampleFn[flow]
		if lookupKey == "" {
			lookupKey = flow
		}

		scenarioFn := scenarioRegistry[lookupKey]
		if scenarioFn == nil {
			result.Scenarios[scenarioKey] = &ScenarioResult{
				Status: "not_implemented",
				Reason: fmt.Sprintf("No scenario for flow '%s'", flow),
			}
			continue
		}

		fmt.Fprintf(os.Stderr, "    [%s] running ...\n", scenarioKey)

		ctx := context.Background()
		err := scenarioFn(ctx, client)

		if err != nil {
			var ie *payments.IntegrationError
			var ce *payments.ConnectorError
			var ne *payments.NetworkError

			switch {
			case errors.As(err, &ie):
				detail := fmt.Sprintf("IntegrationError: %s (code=%s)", ie.Error(), ie.ErrorCode())
				fmt.Fprintf(os.Stderr, red("    [%s] FAILED")+" — %s\n", scenarioKey, detail)
				result.Scenarios[scenarioKey] = &ScenarioResult{
					Status: "failed",
					Error:  detail,
				}
				anyFailed = true

			case errors.As(err, &ce):
				detail := fmt.Sprintf("ConnectorError: %s (code=%s, http=%d)", ce.Error(), ce.ErrorCode(), ce.HTTPStatusCode())
				if mock {
					fmt.Fprintf(os.Stderr, green("    [%s] PASSED")+" — req_transformer OK (mock response)\n", scenarioKey)
					result.Scenarios[scenarioKey] = &ScenarioResult{
						Status: "passed",
						Reason: "mock_verified",
						Detail: detail,
					}
				} else {
					fmt.Fprintf(os.Stderr, yellow("    [%s] SKIPPED")+" (connector error) — %s\n", scenarioKey, detail)
					result.Scenarios[scenarioKey] = &ScenarioResult{
						Status: "skipped",
						Reason: "connector_error",
						Detail: detail,
					}
				}

			case errors.As(err, &ne):
				if mock {
					fmt.Fprintf(os.Stderr, yellow("    [%s] SKIPPED")+" (mock network error) — %s\n", scenarioKey, ne.Error())
					result.Scenarios[scenarioKey] = &ScenarioResult{
						Status: "skipped",
						Reason: "mock_network_error",
						Detail: ne.Error(),
					}
				} else {
					fmt.Fprintf(os.Stderr, yellow("    [%s] SKIPPED")+" (network error) — %s\n", scenarioKey, ne.Error())
					result.Scenarios[scenarioKey] = &ScenarioResult{
						Status: "skipped",
						Reason: "network_error",
						Detail: ne.Error(),
					}
				}

			default:
				fmt.Fprintf(os.Stderr, red("    [%s] FAILED")+" — %T: %s\n", scenarioKey, err, err.Error())
				result.Scenarios[scenarioKey] = &ScenarioResult{
					Status: "failed",
					Error:  fmt.Sprintf("%T: %s", err, err.Error()),
				}
				anyFailed = true
			}
		} else {
			fmt.Fprintf(os.Stderr, green("    [%s] PASSED")+"\n", scenarioKey)
			result.Scenarios[scenarioKey] = &ScenarioResult{
				Status: "passed",
			}
		}
	}

	if anyFailed {
		result.Status = "failed"
	}
	return result
}
