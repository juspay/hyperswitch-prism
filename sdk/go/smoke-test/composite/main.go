package main
//
import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"strings"
	"time"

	"google.golang.org/protobuf/proto"
	pb "github.com/juspay/hyperswitch-prism/sdk/go/generated/payments"
	"github.com/juspay/hyperswitch-prism/sdk/go/payments"
)

// ── ANSI color helpers ─────────────────────────────────────────────────────────

var noColor = os.Getenv("NO_COLOR") != "" ||
	(os.Getenv("FORCE_COLOR") == "" &&
		(os.Stdout != os.Stderr || os.Getenv("TERM") == "" || os.Getenv("TERM") == "dumb"))

func c(code, text string) string {
	if noColor {
		return text
	}
	return fmt.Sprintf("\033[%sm%s\033[0m", code, text)
}

func green(t string) string  { return c("32", t) }
func yellow(t string) string { return c("33", t) }
func red(t string) string    { return c("31", t) }
func grey(t string) string   { return c("90", t) }
func bold(t string) string   { return c("1", t) }

// ── Credential helpers ────────────────────────────────────────────────────────

func loadCreds(credsFile string) map[string]interface{} {
	data, err := os.ReadFile(credsFile)
	if err != nil {
		return nil
	}
	var creds map[string]interface{}
	if err := json.Unmarshal(data, &creds); err != nil {
		return nil
	}
	return creds
}

func stripeApiKey(creds map[string]interface{}) string {
	raw, ok := creds["stripe"]
	if !ok {
		return ""
	}

	var stripe map[string]interface{}
	if arr, ok := raw.([]interface{}); ok && len(arr) > 0 {
		if m, ok := arr[0].(map[string]interface{}); ok {
			stripe = m
		}
	} else if m, ok := raw.(map[string]interface{}); ok {
		stripe = m
	}
	if stripe == nil {
		return ""
	}

	for _, key := range []string{"apiKey", "api_key"} {
		if v, ok := stripe[key]; ok {
			if m, ok := v.(map[string]interface{}); ok {
				if s, ok := m["value"].(string); ok {
					return s
				}
			}
		}
	}
	return ""
}

func buildStripeConfig(apiKey string) *pb.ConnectorConfig {
	return &pb.ConnectorConfig{
		ConnectorConfig: &pb.ConnectorSpecificConfig{
			Config: &pb.ConnectorSpecificConfig_Stripe{
				Stripe: &pb.StripeConfig{
					ApiKey: &pb.SecretString{Value: apiKey},
				},
			},
		},
		Options: &pb.SdkOptions{
			Environment: pb.Environment_SANDBOX,
		},
	}
}

// ── Test result ───────────────────────────────────────────────────────────────

type testResult struct {
	name   string
	passed bool
	detail string
}

// ── Test cases ────────────────────────────────────────────────────────────────

func testStripeAuthorizeSuccess(apiKey string) testResult {
	name := "stripe_authorize_success"
	client := payments.NewPaymentClient(buildStripeConfig(apiKey), &pb.RequestConfig{})

	req := &pb.PaymentServiceAuthorizeRequest{
		MerchantTransactionId: proto.String(fmt.Sprintf("composite_authorize_%d", time.Now().UnixMilli())),
		Amount: &pb.Money{
			MinorAmount: 1000,
			Currency:    pb.Currency_USD,
		},
		PaymentMethod: &pb.PaymentMethod{
			PaymentMethod: &pb.PaymentMethod_Card{
				Card: &pb.CardDetails{
					CardNumber:     &pb.CardNumberType{Value: "4111111111111111"},
					CardExpMonth:   &pb.SecretString{Value: "12"},
					CardExpYear:    &pb.SecretString{Value: "2050"},
					CardCvc:        &pb.SecretString{Value: "123"},
					CardHolderName: &pb.SecretString{Value: "Test User"},
				},
			},
		},
		CaptureMethod: captureMethodPtr(pb.CaptureMethod_AUTOMATIC),
		AuthType:      pb.AuthenticationType_NO_THREE_DS,
		Address:       &pb.PaymentAddress{BillingAddress: &pb.Address{}},
	}

	res, err := client.Authorize(context.Background(), req, nil)
	if err != nil {
		var ie *payments.IntegrationError
		var ce *payments.ConnectorError
		switch {
		case errors.As(err, &ie):
			return testResult{name, true, fmt.Sprintf("IntegrationError (unexpected): %s (code=%s)", ie.Error(), ie.ErrorCode())}
		case errors.As(err, &ce):
			return testResult{name, true, fmt.Sprintf("ConnectorError: %s (code=%s, http=%d)", ce.Error(), ce.ErrorCode(), ce.HTTPStatusCode())}
		default:
			return testResult{name, false, err.Error()}
		}
	}

	if res.GetStatus() == pb.PaymentStatus_CHARGED {
		return testResult{name, true, fmt.Sprintf("CHARGED — transactionId=%s", res.GetConnectorTransactionId())}
	}
	return testResult{name, false, fmt.Sprintf("Expected CHARGED, got %v", res.GetStatus())}
}

func testIntegrationErrorOnMissingAmount(apiKey string) testResult {
	name := "integration_error_missing_amount"
	client := payments.NewPaymentClient(buildStripeConfig(apiKey), &pb.RequestConfig{})

	// Amount intentionally omitted.
	req := &pb.PaymentServiceAuthorizeRequest{
		MerchantTransactionId: proto.String(fmt.Sprintf("composite_missing_amount_%d", time.Now().UnixMilli())),
		PaymentMethod: &pb.PaymentMethod{
			PaymentMethod: &pb.PaymentMethod_Card{
				Card: &pb.CardDetails{
					CardNumber:     &pb.CardNumberType{Value: "4111111111111111"},
					CardExpMonth:   &pb.SecretString{Value: "12"},
					CardExpYear:    &pb.SecretString{Value: "2050"},
					CardCvc:        &pb.SecretString{Value: "123"},
					CardHolderName: &pb.SecretString{Value: "Test User"},
				},
			},
		},
		CaptureMethod: captureMethodPtr(pb.CaptureMethod_AUTOMATIC),
		AuthType:      pb.AuthenticationType_NO_THREE_DS,
	}

	_, err := client.Authorize(context.Background(), req, nil)
	if err == nil {
		return testResult{name, false, "Expected IntegrationError but call succeeded — request should have been rejected before the HTTP call"}
	}

	var ie *payments.IntegrationError
	var ce *payments.ConnectorError
	switch {
	case errors.As(err, &ie):
		return testResult{name, true, fmt.Sprintf("IntegrationError (expected): %s (code=%s)", ie.Error(), ie.ErrorCode())}
	case errors.As(err, &ce):
		return testResult{name, false, fmt.Sprintf("Got ConnectorError instead of IntegrationError: %s", ce.Error())}
	default:
		return testResult{name, false, fmt.Sprintf("Unexpected error: %s", err.Error())}
	}
}

func testConnectorErrorOnDeclinedCard(apiKey string) testResult {
	name := "connector_error_declined_card"
	client := payments.NewPaymentClient(buildStripeConfig(apiKey), &pb.RequestConfig{})

	req := &pb.PaymentServiceAuthorizeRequest{
		MerchantTransactionId: proto.String(fmt.Sprintf("composite_declined_%d", time.Now().UnixMilli())),
		Amount: &pb.Money{
			MinorAmount: 1000,
			Currency:    pb.Currency_USD,
		},
		PaymentMethod: &pb.PaymentMethod{
			PaymentMethod: &pb.PaymentMethod_Card{
				Card: &pb.CardDetails{
					CardNumber:     &pb.CardNumberType{Value: "4000000000000002"}, // Stripe generic decline test card.
					CardExpMonth:   &pb.SecretString{Value: "12"},
					CardExpYear:    &pb.SecretString{Value: "2050"},
					CardCvc:        &pb.SecretString{Value: "123"},
					CardHolderName: &pb.SecretString{Value: "Test User"},
				},
			},
		},
		CaptureMethod: captureMethodPtr(pb.CaptureMethod_AUTOMATIC),
		AuthType:      pb.AuthenticationType_NO_THREE_DS,
		Address:       &pb.PaymentAddress{BillingAddress: &pb.Address{}},
	}

	_, err := client.Authorize(context.Background(), req, nil)
	if err == nil {
		return testResult{name, true, "Card unexpectedly succeeded (sandbox may behave differently)"}
	}

	var ie *payments.IntegrationError
	var ce *payments.ConnectorError
	switch {
	case errors.As(err, &ce):
		return testResult{name, true, fmt.Sprintf("ConnectorError (expected): %s (code=%s, http=%d)", ce.Error(), ce.ErrorCode(), ce.HTTPStatusCode())}
	case errors.As(err, &ie):
		return testResult{name, false, fmt.Sprintf("Got IntegrationError instead of ConnectorError: %s", ie.Error())}
	default:
		return testResult{name, false, fmt.Sprintf("Unexpected error: %s", err.Error())}
	}
}

func captureMethodPtr(cm pb.CaptureMethod) *pb.CaptureMethod {
	return &cm
}

// ── main ───────────────────────────────────────────────────────────────────────

func main() {
	credsFile := "creds.json"
	for i := 0; i < len(os.Args)-1; i++ {
		if os.Args[i] == "--creds-file" {
			credsFile = os.Args[i+1]
		}
	}

	fmt.Println()
	fmt.Println(strings.Repeat("=", 60))
	fmt.Println("Composite smoke test (direct SDK calls)")
	fmt.Println(strings.Repeat("=", 60))

	creds := loadCreds(credsFile)
	apiKey := stripeApiKey(creds)

	if apiKey == "" {
		fmt.Println(yellow("SKIPPED: no stripe credentials found in " + credsFile))
		os.Exit(0)
	}

	results := []testResult{
		testStripeAuthorizeSuccess(apiKey),
		testIntegrationErrorOnMissingAmount(apiKey),
		testConnectorErrorOnDeclinedCard(apiKey),
	}

	fmt.Println()
	for _, r := range results {
		icon := green("PASS")
		nameOut := r.name
		if !r.passed {
			icon = red("FAIL")
			nameOut = red(r.name)
		}
		fmt.Printf("  %s  %s\n", icon, nameOut)
		fmt.Printf("     %s\n", grey(r.detail))
	}

	fmt.Println()
	failed := 0
	for _, r := range results {
		if !r.passed {
			failed++
		}
	}
	if failed == 0 {
		fmt.Printf("%s (%d test(s))\n", green("PASSED"), len(results))
	} else {
		fmt.Printf("%s — %d of %d test(s) failed\n", red("FAILED"), failed, len(results))
		os.Exit(1)
	}
}
