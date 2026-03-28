#!/usr/bin/env python3
"""
Scenario Documentation Generator

Generates scenario-centric documentation from field-probe manifest.

Usage:
    python3 scripts/generators/docs/generate_scenarios.py
    python3 scripts/generators/docs/generate_scenarios.py --scenario checkout_card

Output:
    docs-generated/scenarios/*.md
"""

import argparse
import json
from datetime import datetime
from pathlib import Path
from typing import Optional

from jinja2 import Environment, FileSystemLoader

import sys
sys.path.insert(0, str(Path(__file__).parent.parent))
from snippets.extract import extract_snippet, extract_all_snippets, LANGUAGE_EXTENSIONS


# ─── Paths ───────────────────────────────────────────────────────────────────

REPO_ROOT = Path(__file__).parent.parent.parent.parent
TEMPLATES_DIR = Path(__file__).parent / "templates"
OUTPUT_DIR = REPO_ROOT / "docs-generated" / "scenarios"
EXAMPLES_DIR = REPO_ROOT / "examples" / "scenarios"
PROBE_DATA_DIR = REPO_ROOT / "data" / "field_probe"
MANIFEST_PATH = PROBE_DATA_DIR / "manifest.json"


# ─── Data Loading ─────────────────────────────────────────────────────────────

def load_manifest() -> dict:
    """Load the field probe manifest."""
    with open(MANIFEST_PATH) as f:
        return json.load(f)


def get_scenario_groups(manifest: dict) -> list[dict]:
    """Extract scenario groups from manifest."""
    return manifest.get("scenario_groups", [])


def get_flow_metadata(manifest: dict) -> dict[str, dict]:
    """Get flow metadata keyed by flow_key."""
    return {m["flow_key"]: m for m in manifest.get("flow_metadata", [])}


# ─── Connector Support Detection ───────────────────────────────────────────────

def get_connectors_supporting_scenario(scenario: dict) -> list[str]:
    """
    Find all connectors that support a given scenario.
    
    Checks each connector's probe data for the required flows.
    """
    connectors = []
    required_flows = scenario.get("required_flows", [])
    
    for probe_file in PROBE_DATA_DIR.glob("*.json"):
        if probe_file.name == "manifest.json":
            continue
        
        connector_name = probe_file.stem
        
        try:
            with open(probe_file) as f:
                probe_data = json.load(f)
            
            # Check if all required flows are supported
            supported = True
            flows = probe_data.get("flows", {})
            
            for req in required_flows:
                flow_key = req.get("flow_key")
                pm_key = req.get("pm_key")
                pm_variants = req.get("pm_key_variants", [])
                
                flow_data = flows.get(flow_key, {})
                
                if pm_variants:
                    # Check if any variant is supported
                    variant_supported = any(
                        flow_data.get(pm, {}).get("status") == "supported"
                        for pm in pm_variants
                    )
                    if not variant_supported:
                        supported = False
                        break
                elif pm_key:
                    if flow_data.get(pm_key, {}).get("status") != "supported":
                        supported = False
                        break
                else:
                    # Just check if any PM supports this flow
                    if not any(
                        v.get("status") == "supported" 
                        for v in flow_data.values()
                    ):
                        supported = False
                        break
            
            if supported:
                connectors.append(connector_name)
        
        except Exception as e:
            print(f"Warning: Failed to load probe data for {connector_name}: {e}")
    
    return sorted(connectors)


# ─── Snippet Collection ─────────────────────────────────────────────────────

def collect_snippets_for_scenario(scenario_key: str, connectors: list[str]) -> dict:
    """
    Collect all code snippets for a scenario.
    
    Returns a nested dict with quickstart and per-connector snippets.
    """
    snippets = {
        "python_quickstart": "",
        "python_config": {},
        "python_full": {},
        "rust_quickstart": "",
        "rust_config": {},
        "rust_full": {},
        "javascript_quickstart": "",
        "javascript_config": {},
        "javascript_full": {},
        "kotlin_quickstart": "",
        "kotlin_config": {},
        "kotlin_full": {},
    }
    
    # Quick start snippets (generic process function)
    for lang in ["python", "rust", "javascript", "kotlin"]:
        ext = LANGUAGE_EXTENSIONS.get(lang, lang)
        example_file = EXAMPLES_DIR / f"{scenario_key}.{ext}"
        
        if example_file.exists():
            # Extract process function as quickstart
            quickstart = extract_snippet(example_file, f"process_{scenario_key}")
            if quickstart:
                snippets[f"{lang}_quickstart"] = quickstart
            
            # Extract all available connector configs from the example file
            all_snippets = extract_all_snippets(example_file)
            
            for connector in connectors:
                # Look for config function (e.g., stripe_config, get_stripe_config)
                config_snippet = None
                for marker in [f"{connector}_config", f"get_{connector}_config"]:
                    if marker in all_snippets:
                        config_snippet = all_snippets[marker]
                        break
                
                if config_snippet:
                    snippets[f"{lang}_config"][connector] = config_snippet
                
                # For full example, use the process function (it's parameterized)
                if quickstart:
                    snippets[f"{lang}_full"][connector] = quickstart
    
    return snippets


# ─── Status Handling ─────────────────────────────────────────────────────────

def get_status_handling(scenario_key: str) -> dict:
    """
    Get status handling tables for a scenario.
    
    These are hardcoded based on scenario type, matching existing behavior.
    """
    status_handling = {}
    
    if scenario_key in ["checkout_card"]:
        status_handling["authorize"] = {
            "AUTHORIZED": "Funds reserved — proceed to Capture to settle",
            "PENDING": "Awaiting async confirmation — wait for webhook before capturing",
            "FAILED": "Payment declined — surface error to customer, do not retry without new details",
        }
        status_handling["capture"] = {
            "CAPTURED": "Funds settled successfully — payment complete",
            "PENDING": "Settlement processing — await webhook confirmation",
            "FAILED": "Capture failed — check error details",
        }
    
    elif scenario_key in ["checkout_autocapture", "checkout_wallet", "checkout_bank"]:
        status_handling["authorize"] = {
            "AUTHORIZED": "Payment authorized and captured — funds will be settled automatically",
            "PENDING": "Payment processing — await webhook for final status before fulfilling",
            "FAILED": "Payment declined — surface error to customer, do not retry without new details",
        }
    
    elif scenario_key == "recurring":
        status_handling["setup_recurring"] = {
            "PENDING": "Mandate stored — save connector_transaction_id for future RecurringPaymentService.Charge calls",
            "FAILED": "Setup failed — customer must re-enter payment details",
        }
    
    return status_handling


# ─── Scenario Rendering ──────────────────────────────────────────────────────

def get_scenario_use_case(scenario_key: str) -> str:
    """Get human-readable use case description for a scenario."""
    use_cases = {
        "checkout_card": "Physical goods or delayed fulfillment where you need to reserve funds before shipping",
        "checkout_autocapture": "Digital goods or immediate fulfillment where funds should be captured immediately",
        "checkout_wallet": "Accepting Apple Pay, Google Pay, or other digital wallets",
        "checkout_bank": "Direct bank debits via SEPA, ACH, or BACS",
        "refund": "Returning funds to customers after successful payment",
        "recurring": "Setting up subscription payments or stored mandates",
        "void_payment": "Canceling an authorization before capture",
        "get_payment": "Checking payment status for reconciliation",
        "create_customer": "Creating customer records for future payments",
        "tokenize": "Storing payment methods securely for future use",
        "tokenized_checkout": "Charging using stored payment method tokens",
        "tokenized_recurring": "Setting up subscriptions with stored tokens",
        "proxy_checkout": "Processing payments without touching card data (PCI-compliant)",
        "proxy_3ds_checkout": "3D Secure authentication with vault tokens",
    }
    return use_cases.get(scenario_key, "See scenario description")


def render_scenario(scenario: dict, manifest: dict) -> str:
    """Render a single scenario documentation file."""
    
    # Setup Jinja2
    env = Environment(loader=FileSystemLoader(TEMPLATES_DIR))
    template = env.get_template("scenario.md.j2")
    
    # Get connectors supporting this scenario
    connectors = get_connectors_supporting_scenario(scenario)
    
    # Collect snippets
    snippets = collect_snippets_for_scenario(scenario["key"], connectors)
    
    # Get status handling
    status_handling = get_status_handling(scenario["key"])
    
    # Get flow metadata
    flow_metadata = get_flow_metadata(manifest)
    
    # Build template context
    context = {
        "key": scenario["key"],
        "title": scenario["title"],
        "description": scenario.get("description", ""),
        "use_case": get_scenario_use_case(scenario["key"]),
        "flows": scenario.get("flows", []),
        "pm_key": scenario.get("pm_key"),
        "pm_variants": scenario.get("pm_variants", []),
        "connectors": connectors,
        "snippets": snippets,
        "status_handling": status_handling,
        "flow_metadata": flow_metadata,
        "related_scenarios": [],  # TODO: Link related scenarios
        "generation_date": datetime.now().strftime("%Y-%m-%d"),
    }
    
    return template.render(**context)


# ─── Main ────────────────────────────────────────────────────────────────────

def main():
    parser = argparse.ArgumentParser(description="Generate scenario documentation")
    parser.add_argument(
        "--scenario",
        help="Generate only this scenario (e.g., checkout_card)"
    )
    parser.add_argument(
        "--all",
        action="store_true",
        help="Generate all scenarios"
    )
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=OUTPUT_DIR,
        help="Output directory for generated docs"
    )
    
    args = parser.parse_args()
    
    # Ensure output directory exists
    args.output_dir.mkdir(parents=True, exist_ok=True)
    
    # Load manifest
    manifest = load_manifest()
    scenarios = get_scenario_groups(manifest)
    
    if args.scenario:
        # Generate single scenario
        scenario = next((s for s in scenarios if s["key"] == args.scenario), None)
        if not scenario:
            print(f"Error: Scenario '{args.scenario}' not found in manifest")
            return 1
        
        print(f"Generating scenario: {scenario['key']}")
        content = render_scenario(scenario, manifest)
        
        output_file = args.output_dir / f"{scenario['key']}.md"
        with open(output_file, 'w') as f:
            f.write(content)
        
        print(f"  → {output_file}")
    
    elif args.all:
        # Generate all scenarios
        print(f"Generating {len(scenarios)} scenarios...")
        
        for scenario in scenarios:
            print(f"  Generating: {scenario['key']}")
            try:
                content = render_scenario(scenario, manifest)
                
                output_file = args.output_dir / f"{scenario['key']}.md"
                with open(output_file, 'w') as f:
                    f.write(content)
                
                print(f"    → {output_file}")
            except Exception as e:
                print(f"    ✗ Error: {e}")
        
        print("Done!")
    
    else:
        print("Error: Specify --scenario <name> or --all")
        return 1
    
    return 0


if __name__ == "__main__":
    exit(main())
