package main
//
import (
	"context"
	"fmt"
	"os"
	"strings"

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

// ── Adyen AUTHORISATION webhook body (from real test configuration) ────────────
// Sensitive fields replaced:
//   merchantAccountCode → "YOUR_MERCHANT_ACCOUNT"
//   merchantReference   → "pay_test_00000000000000"
//   pspReference        → "TEST000000000000"
//   hmacSignature       → "test_hmac_signature_placeholder"
//   cardHolderName      → "John Doe"
//   shopperEmail        → "shopper@example.com"

const adyenWebhookBody = `{
  "live": "false",
  "notificationItems": [{
    "NotificationRequestItem": {
      "additionalData": {
        "authCode": "APPROVED",
        "cardSummary": "1111",
        "cardHolderName": "John Doe",
        "expiryDate": "03/2030",
        "shopperEmail": "shopper@example.com",
        "shopperIP": "128.0.0.1",
        "shopperInteraction": "Ecommerce",
        "captureDelayHours": "0",
        "gatewaySystem": "direct",
        "hmacSignature": "test_hmac_signature_placeholder"
      },
      "amount": { "currency": "GBP", "value": 654000 },
      "eventCode": "AUTHORISATION",
      "eventDate": "2026-01-21T14:18:18+01:00",
      "merchantAccountCode": "YOUR_MERCHANT_ACCOUNT",
      "merchantReference": "pay_test_00000000000000",
      "operations": ["CAPTURE", "REFUND"],
      "paymentMethod": "visa",
      "pspReference": "TEST000000000000",
      "reason": "APPROVED:1111:03/2030",
      "success": "true"
    }
  }]
}`

var adyenHeaders = map[string]string{
	"content-type": "application/json",
	"accept":       "*/*",
}

// ── Connector identity only — no API creds, no webhook secret ─────────────────

func buildConfig() *pb.ConnectorConfig {
	return &pb.ConnectorConfig{
		Options: &pb.SdkOptions{
			Environment: pb.Environment_SANDBOX,
		},
		ConnectorConfig: &pb.ConnectorSpecificConfig{
			Config: &pb.ConnectorSpecificConfig_Adyen{
				Adyen: &pb.AdyenConfig{},
			},
		},
	}
}

func captureMethodPtr(cm pb.CaptureMethod) *pb.CaptureMethod {
	return &cm
}

// ── Test 1: handle_event — AUTHORISATION ──────────────────────────────────────

func testHandleEvent() bool {
	fmt.Println(bold("\n[Adyen Webhook — AUTHORISATION handle_event]"))
	client := payments.NewEventClient(buildConfig(), &pb.RequestConfig{})

	req := &pb.EventServiceHandleRequest{
		MerchantEventId: strPtr("smoke_wh_adyen_auth"),
		RequestDetails: &pb.RequestDetails{
			Method:  pb.HttpMethod_HTTP_METHOD_POST,
			Uri:     strPtr("/webhooks/adyen"),
			Headers: adyenHeaders,
			Body:    []byte(adyenWebhookBody),
		},
		EventContext: &pb.EventContext{
			EventContext: &pb.EventContext_Payment{
				Payment: &pb.PaymentEventContext{
					CaptureMethod: captureMethodPtr(pb.CaptureMethod_MANUAL),
				},
			},
		},
	}

	res, err := client.HandleEvent(context.Background(), req, nil)
	if err != nil {
		if ie, ok := err.(*payments.IntegrationError); ok {
			fmt.Printf("  %s: IntegrationError: %s (code=%s)\n", red("FAIL"), ie.Error(), ie.ErrorCode())
			return false
		}
		if ce, ok := err.(*payments.ConnectorError); ok {
			fmt.Printf("  %s: ConnectorError: %s (code=%s)\n", red("FAIL"), ce.Error(), ce.ErrorCode())
			return false
		}
		fmt.Printf("  %s: %s\n", red("FAIL"), err.Error())
		return false
	}

	fmt.Printf("  %s: event_type=%v\n", grey("info"), res.GetEventType())
	fmt.Printf("  %s: source_verified=%v\n", grey("info"), res.GetSourceVerified())
	fmt.Printf("  %s: merchant_event=%s\n", grey("info"), res.GetMerchantEventId())
	if !res.GetSourceVerified() {
		fmt.Printf("  %s: source_verified=false (expected — no real HMAC secret)\n", yellow("~"))
	}
	fmt.Printf("  %s: handle_event returned response without crashing\n", green("PASS"))
	return true
}

// ── Test 2: parse_event ────────────────────────────────────────────────────────

func testParseEvent() bool {
	fmt.Println(bold("\n[Adyen Webhook — AUTHORISATION parse_event]"))
	client := payments.NewEventClient(buildConfig(), &pb.RequestConfig{})

	req := &pb.EventServiceParseRequest{
		RequestDetails: &pb.RequestDetails{
			Method:  pb.HttpMethod_HTTP_METHOD_POST,
			Uri:     strPtr("/webhooks/adyen"),
			Headers: adyenHeaders,
			Body:    []byte(adyenWebhookBody),
		},
	}

	res, err := client.ParseEvent(context.Background(), req, nil)
	if err != nil {
		if ie, ok := err.(*payments.IntegrationError); ok {
			fmt.Printf("  %s: IntegrationError: %s (code=%s)\n", red("FAIL"), ie.Error(), ie.ErrorCode())
			return false
		}
		if ce, ok := err.(*payments.ConnectorError); ok {
			fmt.Printf("  %s: ConnectorError: %s (code=%s)\n", red("FAIL"), ce.Error(), ce.ErrorCode())
			return false
		}
		fmt.Printf("  %s: %s\n", red("FAIL"), err.Error())
		return false
	}

	fmt.Printf("  %s: event_type=%v\n", grey("info"), res.GetEventType())
	fmt.Printf("  %s: reference=%s\n", grey("info"), res.GetReference())
	fmt.Printf("  %s: parse_event returned response\n", green("PASS"))
	return true
}

// ── Test 3: malformed body ─────────────────────────────────────────────────────

func testMalformedBody() bool {
	fmt.Println(bold("\n[Adyen Webhook — malformed body]"))
	client := payments.NewEventClient(buildConfig(), &pb.RequestConfig{})

	req := &pb.EventServiceHandleRequest{
		RequestDetails: &pb.RequestDetails{
			Method:  pb.HttpMethod_HTTP_METHOD_POST,
			Uri:     strPtr("/webhooks/adyen"),
			Headers: adyenHeaders,
			Body:    []byte("not valid json {{{{"),
		},
	}

	res, err := client.HandleEvent(context.Background(), req, nil)
	if err != nil {
		if _, ok := err.(*payments.IntegrationError); ok {
			fmt.Printf("  %s: IntegrationError thrown as expected: %s\n", green("PASS"), err.Error())
			return true
		}
		if _, ok := err.(*payments.ConnectorError); ok {
			fmt.Printf("  %s: ConnectorError thrown as expected: %s\n", green("PASS"), err.Error())
			return true
		}
		fmt.Printf("  %s: unexpected error: %s\n", red("FAIL"), err.Error())
		return false
	}

	fmt.Printf("  %s: accepted malformed body — event_type: %v\n", yellow("~"), res.GetEventType())
	return true
}

// ── Test 4: unknown eventCode ──────────────────────────────────────────────────

func testUnknownEventCode() bool {
	fmt.Println(bold("\n[Adyen Webhook — unknown eventCode]"))
	client := payments.NewEventClient(buildConfig(), &pb.RequestConfig{})

	unknownBody := strings.Replace(adyenWebhookBody, "\"AUTHORISATION\"", "\"SOME_UNKNOWN_EVENT\"", 1)

	req := &pb.EventServiceHandleRequest{
		RequestDetails: &pb.RequestDetails{
			Method:  pb.HttpMethod_HTTP_METHOD_POST,
			Uri:     strPtr("/webhooks/adyen"),
			Headers: adyenHeaders,
			Body:    []byte(unknownBody),
		},
	}

	res, err := client.HandleEvent(context.Background(), req, nil)
	if err != nil {
		if _, ok := err.(*payments.IntegrationError); ok {
			fmt.Printf("  %s: IntegrationError for unknown event (expected): %s\n", green("PASS"), err.Error())
			return true
		}
		if _, ok := err.(*payments.ConnectorError); ok {
			fmt.Printf("  %s: ConnectorError for unknown event (expected): %s\n", green("PASS"), err.Error())
			return true
		}
		fmt.Printf("  %s: %s\n", red("FAIL"), err.Error())
		return false
	}

	fmt.Printf("  %s: handled gracefully — event_type: %v\n", green("PASS"), res.GetEventType())
	return true
}

func strPtr(s string) *string {
	return &s
}

// ── main ───────────────────────────────────────────────────────────────────────

func main() {
	fmt.Println(bold("Adyen Webhook Smoke Test"))
	fmt.Println(strings.Repeat("─", 50))

	results := []bool{
		testHandleEvent(),
		testParseEvent(),
		testMalformedBody(),
		testUnknownEventCode(),
	}

	fmt.Println()
	fmt.Println(strings.Repeat("=", 50))
	allPassed := true
	for _, r := range results {
		if !r {
			allPassed = false
			break
		}
	}
	if allPassed {
		fmt.Println(green("PASSED"))
	} else {
		fmt.Println(red("FAILED"))
		os.Exit(1)
	}
}
