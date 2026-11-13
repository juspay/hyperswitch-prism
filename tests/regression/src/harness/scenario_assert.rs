use std::collections::BTreeMap;

use serde_json::Value;

use crate::harness::scenario_types::{FieldAssert, ScenarioError};

/// Applies all assertion rules for one request/response pair.
pub fn do_assertion(
    assert_rules: &BTreeMap<String, FieldAssert>,
    response_json: &Value,
    request_json: &Value,
) -> Result<(), ScenarioError> {
    for (field, rule) in assert_rules {
        apply_rule(field, rule, response_json, request_json)?;
    }
    Ok(())
}

/// Evaluates a single assertion rule against a response field.
fn apply_rule(
    field: &str,
    rule: &FieldAssert,
    response_json: &Value,
    request_json: &Value,
) -> Result<(), ScenarioError> {
    // All rule variants begin by resolving the response field path.
    let actual = lookup_json_path(response_json, field);
    match rule {
        FieldAssert::MustExist { must_exist } => {
            if !*must_exist {
                return Err(ScenarioError::InvalidAssertionRule {
                    field: field.to_string(),
                    message: "'must_exist' must be true".to_string(),
                });
            }
            if actual.is_none_or(Value::is_null) {
                return Err(ScenarioError::AssertionFailed {
                    field: field.to_string(),
                    message: "expected field to exist".to_string(),
                });
            }
            Ok(())
        }
        FieldAssert::MustNotExist { must_not_exist } => {
            if !*must_not_exist {
                return Err(ScenarioError::InvalidAssertionRule {
                    field: field.to_string(),
                    message: "'must_not_exist' must be true".to_string(),
                });
            }
            if actual.is_some_and(|value| !value.is_null()) {
                return Err(ScenarioError::AssertionFailed {
                    field: field.to_string(),
                    message: format!(
                        "expected field to be absent or null, got {}",
                        render_json(actual.expect("checked is_some"))
                    ),
                });
            }
            Ok(())
        }
        FieldAssert::Equals { equals } => {
            let Some(actual_value) = actual else {
                return Err(ScenarioError::AssertionFailed {
                    field: field.to_string(),
                    message: format!("expected {}, got missing", render_json(equals)),
                });
            };

            if actual_value != equals {
                return Err(ScenarioError::AssertionFailed {
                    field: field.to_string(),
                    message: format!(
                        "expected {}, got {}",
                        render_json(equals),
                        render_json(actual_value)
                    ),
                });
            }
            Ok(())
        }
        FieldAssert::OneOf { one_of } => {
            let Some(actual_value) = actual else {
                return Err(ScenarioError::AssertionFailed {
                    field: field.to_string(),
                    message: "expected one_of value but field is missing".to_string(),
                });
            };

            if one_of.iter().all(|expected| expected != actual_value) {
                return Err(ScenarioError::AssertionFailed {
                    field: field.to_string(),
                    message: format!(
                        "expected one of [{}], got {}",
                        one_of
                            .iter()
                            .map(render_json)
                            .collect::<Vec<_>>()
                            .join(", "),
                        render_json(actual_value)
                    ),
                });
            }
            Ok(())
        }
        FieldAssert::Contains { contains } => {
            let Some(actual_value) = actual else {
                return Err(ScenarioError::AssertionFailed {
                    field: field.to_string(),
                    message: "expected string containing value but field is missing".to_string(),
                });
            };
            let Some(actual_text) = actual_value.as_str() else {
                return Err(ScenarioError::AssertionFailed {
                    field: field.to_string(),
                    message: format!(
                        "expected string containing '{}', got {}",
                        contains,
                        render_json(actual_value)
                    ),
                });
            };

            if !actual_text
                .to_ascii_lowercase()
                .contains(&contains.to_ascii_lowercase())
            {
                return Err(ScenarioError::AssertionFailed {
                    field: field.to_string(),
                    message: format!("expected '{}' to contain '{}'", actual_text, contains),
                });
            }
            Ok(())
        }
        FieldAssert::Echo { echo } => {
            let expected = lookup_json_path(request_json, echo).ok_or_else(|| {
                ScenarioError::AssertionFailed {
                    field: field.to_string(),
                    message: format!("request path '{}' for echo is missing", echo),
                }
            })?;

            let Some(actual_value) = actual else {
                return Err(ScenarioError::AssertionFailed {
                    field: field.to_string(),
                    message: format!("expected echo from '{}', response field is missing", echo),
                });
            };

            if actual_value != expected {
                return Err(ScenarioError::AssertionFailed {
                    field: field.to_string(),
                    message: format!(
                        "echo mismatch expected {} got {}",
                        render_json(expected),
                        render_json(actual_value)
                    ),
                });
            }
            Ok(())
        }
    }
}

/// Looks up a dot-separated JSON path with array-index support.
///
/// Example paths:
/// - `status`
/// - `mandate_reference.connector_mandate_id.connector_mandate_id`
/// - `errors.0.message`
pub fn lookup_json_path<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    if path.is_empty() {
        return Some(value);
    }

    let mut current = value;
    for segment in path.split('.') {
        if segment.is_empty() {
            return None;
        }

        current = if let Ok(index) = segment.parse::<usize>() {
            current.get(index)?
        } else {
            lookup_object_segment(current, segment)?
        };
    }

    Some(current)
}

/// Resolves one object segment and tolerates snake_case/camelCase mismatches.
fn lookup_object_segment<'a>(current: &'a Value, segment: &str) -> Option<&'a Value> {
    if let Some(value) = current.get(segment) {
        return Some(value);
    }

    let camel = snake_to_camel_case(segment);
    if camel != segment {
        if let Some(value) = current.get(&camel) {
            return Some(value);
        }
    }

    let snake = camel_to_snake_case(segment);
    if snake != segment {
        if let Some(value) = current.get(&snake) {
            return Some(value);
        }
    }

    None
}

/// Converts snake_case to camelCase for path fallback resolution.
fn snake_to_camel_case(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut uppercase_next = false;
    for ch in input.chars() {
        if ch == '_' {
            uppercase_next = true;
            continue;
        }

        if uppercase_next {
            out.push(ch.to_ascii_uppercase());
            uppercase_next = false;
        } else {
            out.push(ch);
        }
    }
    out
}

/// Converts camelCase to snake_case for path fallback resolution.
fn camel_to_snake_case(input: &str) -> String {
    let mut out = String::with_capacity(input.len() + 4);
    for (idx, ch) in input.chars().enumerate() {
        if ch.is_ascii_uppercase() && idx > 0 {
            out.push('_');
        }
        out.push(ch.to_ascii_lowercase());
    }
    out
}

/// Renders JSON values safely for assertion error messages.
fn render_json(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "<json-render-error>".to_string())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;

    use crate::harness::scenario_types::FieldAssert;

    use super::do_assertion;

    #[test]
    fn checks_core_rules() {
        let response = json!({
            "status": "CHARGED",
            "connectorTransactionId": { "id": "txn_123" },
            "error": null,
            "captured_amount": 6000,
            "details": { "message": "declined by issuer" }
        });
        let request = json!({
            "amount": { "minor_amount": 6000 }
        });

        let mut rules = BTreeMap::new();
        rules.insert(
            "status".to_string(),
            FieldAssert::OneOf {
                one_of: vec![json!("CHARGED"), json!("AUTHORIZED")],
            },
        );
        rules.insert(
            "connector_transaction_id".to_string(),
            FieldAssert::MustExist { must_exist: true },
        );
        rules.insert(
            "error".to_string(),
            FieldAssert::MustNotExist {
                must_not_exist: true,
            },
        );
        rules.insert(
            "captured_amount".to_string(),
            FieldAssert::Echo {
                echo: "amount.minor_amount".to_string(),
            },
        );
        rules.insert(
            "details.message".to_string(),
            FieldAssert::Contains {
                contains: "declin".to_string(),
            },
        );

        do_assertion(&rules, &response, &request).expect("assertions should pass");
    }
}
