# Documentation Generator

Generates consistent SDK examples across 4 languages (Python, JavaScript, Kotlin, Rust) for 80+ payment connectors.

## Quick Start

```bash
# Generate docs for Stripe
make docs CONNECTORS=stripe

# Generate docs for all connectors
make docs CONNECTORS=all

# Run tests
make test-docs
```

## Architecture

```
scripts/generators/docs/
├── config/
│   ├── connectors.yaml         # Display names
│   ├── payment_methods.yaml    # Payment method categories
│   ├── currency_overrides.yaml # Currency mappings
│   └── status_mapping.yaml     # Per-language status values
├── specs/
│   └── scenarios.yaml          # Behavioral specifications
├── core/
│   ├── models.py               # Pydantic models
│   ├── hydrator.py             # Scenario hydration
│   └── validator.py            # Structural validation
├── renderers/
│   ├── base.py                 # Base renderer
│   ├── python.py               # Python renderer
│   ├── javascript.py           # JavaScript renderer
│   ├── kotlin.py               # Kotlin renderer
│   └── rust.py                 # Rust renderer
└── tests/                      # Test suite
```

## Design Principles

1. **Behavioral Consistency**: All languages implement identical status checks, error handling, and return shapes
2. **Single Source of Truth**: `scenarios.yaml` defines behavior; renderers translate mechanically
3. **Flow Graph**: Per-connector flow dependencies with field-level linking
4. **Pure Probe Data**: Probe JSON remains request payloads only

## Adding Scenarios

Edit `specs/scenarios.yaml` to add new payment scenarios:

```yaml
- key: "my_scenario"
  name: "My Scenario"
  description: "What this scenario does"
  flows:
    - name: "flow_name"
      required: true
      pm_type_variants: ["Card", "GooglePay"]
      status_handling:
        - status: ["FAILED"]
          action: "error"
          message: "Failed: {error}"
  return_fields:
    status: "response.status"
```

## Configuration

- **connectors.yaml**: Add new connector display names
- **payment_methods.yaml**: Map probe payment methods to categories
- **status_mapping.yaml**: Language-specific status values

## Testing

```bash
# Run all tests
python -m pytest tests/ -v

# Run specific test file
python -m pytest tests/test_hydrator.py -v

# Run with coverage
python -m pytest tests/ --cov=.
```

## How It Works

1. **Field Probe** (Rust): Generates flow graphs for each connector
2. **Hydrator**: Combines probe data + scenarios → hydrated scenarios
3. **Renderers**: Generate language-specific code from hydrated scenarios
4. **Output**: Markdown docs with code examples for all languages

## StatusRule Actions

- `error`: Raise/throw exception with message
- `return_early`: Return immediately with specified fields
- `continue`: Proceed to next step (default)

## Field Links

The `use_from_previous` field in scenarios tells the renderer which fields to extract from previous flow responses and pass to the current flow.
