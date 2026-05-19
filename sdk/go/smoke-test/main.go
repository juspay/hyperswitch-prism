package main

import (
	"encoding/json"
	"flag"
	"fmt"
	"os"
	"path/filepath"
	"strings"
)

func main() {
	var (
		credsFile   = flag.String("creds-file", "", "Path to connector credentials JSON file")
		connectors  = flag.String("connectors", "", "Comma-separated list of connectors to test")
		all         = flag.Bool("all", false, "Test all connectors found in credentials file")
		dryRun      = flag.Bool("dry-run", false, "Skip HTTP calls, just validate configs")
		mock        = flag.Bool("mock", false, "Intercept HTTP and return empty responses (req_transformer verification)")
		jsonOutput  = flag.Bool("json", false, "Output results as JSON")
		sdkRootFlag = flag.String("sdk-root", "", "Override SDK root directory (default: auto-detect)")
	)
	flag.Parse()

	if *credsFile == "" {
		fmt.Fprintln(os.Stderr, red("ERROR: --creds-file is required"))
		fmt.Fprintln(os.Stderr, "\nUsage:")
		fmt.Fprintln(os.Stderr, "  go run . --creds-file creds.json --all")
		fmt.Fprintln(os.Stderr, "  go run . --creds-file creds.json --connectors stripe,adyen")
		fmt.Fprintln(os.Stderr, "  go run . --creds-file creds.json --all --mock")
		fmt.Fprintln(os.Stderr, "  go run . --creds-file creds.json --all --dry-run")
		os.Exit(1)
	}

	// Auto-detect SDK root (parent of smoke-test directory).
	sdkRoot := *sdkRootFlag
	if sdkRoot == "" {
		exe, err := os.Executable()
		if err == nil {
			sdkRoot = filepath.Dir(exe)
		} else {
			// Fallback: assume we're running from sdk/go/smoke-test/
			if wd, err := os.Getwd(); err == nil {
				sdkRoot = filepath.Join(wd, "..")
			}
		}
	}

	// Load credentials.
	credentials, err := loadCredentials(*credsFile)
	if err != nil {
		fmt.Fprintf(os.Stderr, red("ERROR: %v\n"), err)
		os.Exit(1)
	}

	// Determine which connectors to test.
	var testConnectors []string
	if *all {
		for name := range credentials {
			testConnectors = append(testConnectors, name)
		}
	} else if *connectors != "" {
		for _, name := range strings.Split(*connectors, ",") {
			name = strings.TrimSpace(name)
			if name != "" {
				testConnectors = append(testConnectors, name)
			}
		}
	} else {
		fmt.Fprintln(os.Stderr, red("ERROR: Specify --all or --connectors"))
		os.Exit(1)
	}

	if len(testConnectors) == 0 {
		fmt.Fprintln(os.Stderr, yellow("WARNING: No connectors to test"))
		os.Exit(0)
	}

	// Print header.
	fmt.Fprintln(os.Stderr)
	fmt.Fprintln(os.Stderr, strings.Repeat("=", 60))
	fmt.Fprintf(os.Stderr, "Running smoke tests for %d connector(s)\n", len(testConnectors))
	if *mock {
		fmt.Fprintln(os.Stderr, "Mode: MOCK (HTTP intercepted, req_transformer verification)")
	}
	fmt.Fprintf(os.Stderr, "SDK root: %s\n", sdkRoot)
	fmt.Fprintln(os.Stderr, strings.Repeat("=", 60))

	// Run tests for each connector.
	var results []*ConnectorResult
	for _, connectorName := range testConnectors {
		authConfig, ok := credentials[connectorName]
		if !ok {
			results = append(results, &ConnectorResult{
				Connector: connectorName,
				Status:    "skipped",
				Scenarios: map[string]*ScenarioResult{
					"skipped": {Status: "skipped", Reason: "no_credentials"},
				},
			})
			continue
		}

		if !hasValidCredentials(authConfig) {
			fmt.Fprintf(os.Stderr, "\n%s %s\n", yellow("SKIP"), bold(connectorName))
			fmt.Fprintf(os.Stderr, "  %s: All credential values are placeholders\n", yellow("REASON"))
			results = append(results, &ConnectorResult{
				Connector: connectorName,
				Status:    "skipped",
				Scenarios: map[string]*ScenarioResult{
					"skipped": {Status: "skipped", Reason: "placeholder_credentials"},
				},
			})
			continue
		}

		fmt.Printf("--- Testing %s ---\n", connectorName)
		fmt.Fprintf(os.Stderr, "\n%s %s\n", bold("TEST"), bold(connectorName))

		config := buildConnectorConfig(connectorName, authConfig)
		if config == nil {
			fmt.Fprintf(os.Stderr, "  %s: Connector config builder not implemented\n", yellow("SKIP"))
			results = append(results, &ConnectorResult{
				Connector: connectorName,
				Status:    "skipped",
				Scenarios: map[string]*ScenarioResult{
					"skipped": {Status: "skipped", Reason: "unsupported_connector"},
				},
			})
			continue
		}

		result := runConnectorScenarios(connectorName, config, sdkRoot, *dryRun, *mock)
		results = append(results, result)
	}

	// Output results.
	if *jsonOutput {
		data, err := json.MarshalIndent(results, "", "  ")
		if err != nil {
			fmt.Fprintf(os.Stderr, red("ERROR: Failed to marshal JSON: %v\n"), err)
			os.Exit(1)
		}
		fmt.Println(string(data))
	} else {
		printConnectorResults(results)
	}

	// Exit with non-zero if any connector failed.
	for _, r := range results {
		if r.Status == "failed" {
			os.Exit(1)
		}
	}
}
