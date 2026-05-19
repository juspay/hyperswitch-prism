package main

import (
	"fmt"
	"os"
	"strings"
)

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

// ScenarioResult is the outcome of a single scenario run.
type ScenarioResult struct {
	Status  string                 `json:"status"`            // "passed" | "skipped" | "failed" | "not_implemented"
	Result  map[string]interface{} `json:"result,omitempty"`
	Reason  string                 `json:"reason,omitempty"`
	Detail  string                 `json:"detail,omitempty"`
	Error   string                 `json:"error,omitempty"`
}

// ConnectorResult aggregates all scenario results for one connector.
type ConnectorResult struct {
	Connector string                     `json:"connector"`
	Status    string                     `json:"status"`
	Scenarios map[string]*ScenarioResult `json:"scenarios"`
	Error     string                     `json:"error,omitempty"`
}

// printConnectorResults prints results in text format.
func printConnectorResults(results []*ConnectorResult) {
	anyFailed := false
	for _, cr := range results {
		fmt.Println()
		switch cr.Status {
		case "passed":
			fmt.Printf("%s %s\n", green("PASS"), bold(cr.Connector))
		case "failed":
			fmt.Printf("%s %s\n", red("FAIL"), bold(cr.Connector))
			anyFailed = true
		case "skipped":
			fmt.Printf("%s %s\n", yellow("SKIP"), bold(cr.Connector))
		case "dry_run":
			fmt.Printf("%s %s\n", grey("DRY"), bold(cr.Connector))
		}

		if cr.Error != "" {
			fmt.Printf("  %s: %s\n", red("ERROR"), cr.Error)
			continue
		}

		for scenarioKey, sr := range cr.Scenarios {
			switch sr.Status {
			case "passed":
				fmt.Printf("  [%s] %s", scenarioKey, green("PASSED"))
				if sr.Detail != "" {
					fmt.Printf(" — %s", sr.Detail)
				}
				fmt.Println()
			case "skipped":
				fmt.Printf("  [%s] %s", scenarioKey, yellow("SKIPPED"))
				if sr.Reason != "" {
					fmt.Printf(" (%s)", sr.Reason)
				}
				if sr.Detail != "" {
					fmt.Printf(" — %s", sr.Detail)
				}
				fmt.Println()
			case "failed":
				fmt.Printf("  [%s] %s", scenarioKey, red("FAILED"))
				if sr.Error != "" {
					fmt.Printf(" — %s", sr.Error)
				}
				fmt.Println()
			case "not_implemented":
				fmt.Printf("  [%s] %s", scenarioKey, grey("NOT IMPLEMENTED"))
				if sr.Reason != "" {
					fmt.Printf(" — %s", sr.Reason)
				}
				fmt.Println()
			}
		}
	}

	fmt.Println()
	fmt.Println(strings.Repeat("=", 60))
	passed := 0
	failed := 0
	skipped := 0
	for _, cr := range results {
		switch cr.Status {
		case "passed":
			passed++
		case "failed":
			failed++
		case "skipped":
			skipped++
		}
	}
	fmt.Printf("Total: %d connectors | %s | %s | %s\n",
		len(results),
		green(fmt.Sprintf("%d passed", passed)),
		red(fmt.Sprintf("%d failed", failed)),
		yellow(fmt.Sprintf("%d skipped", skipped)),
	)

	if anyFailed {
		fmt.Println(red("\nSome tests FAILED"))
	} else {
		fmt.Println(green("\nAll tests PASSED"))
	}
}
