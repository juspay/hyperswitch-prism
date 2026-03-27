//! Common credential loading utilities for test files
//!
//! This module provides a generic way to load connector credentials from
//! the JSON configuration file (.github/test/creds.json)

#![allow(dead_code)]

use common_enums::enums::Currency;
use common_utils::pii::SecretSerdeValue;
use domain_types::router_data::ConnectorAuthType;
use hyperswitch_masking::Secret;
use std::{collections::HashMap, fs};

// Path to the credentials file - use environment variable if set (for CI), otherwise use relative path (for local)
fn get_creds_file_path() -> String {
    std::env::var("CONNECTOR_AUTH_FILE_PATH")
        .unwrap_or_else(|_| "../../.github/test/creds.json".to_string())
}

/// Generic credential structure that can deserialize any connector's credentials
#[derive(serde::Deserialize, Debug, Clone)]
pub struct ConnectorAccountDetails {
    pub auth_type: String,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub key1: Option<String>,
    #[serde(default)]
    pub api_secret: Option<String>,
    #[serde(default)]
    pub key2: Option<String>,
    #[serde(default)]
    pub certificate: Option<String>,
    #[serde(default)]
    pub private_key: Option<String>,
    #[serde(default)]
    pub auth_key_map: Option<HashMap<Currency, SecretSerdeValue>>,
}

#[derive(serde::Deserialize, Debug, Clone)]
pub struct ConnectorCredentials {
    pub connector_account_details: ConnectorAccountDetails,
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
}

/// All connector credentials stored in the JSON file
pub type AllCredentials = HashMap<String, ConnectorCredentials>;

/// Error type for credential loading operations
#[derive(Debug, thiserror::Error)]
pub enum CredentialError {
    #[error("Failed to read credentials file: {0}")]
    FileReadError(#[from] std::io::Error),
    #[error("Failed to parse credentials JSON: {0}")]
    ParseError(#[from] serde_json::Error),
    #[error("Connector '{0}' not found in credentials")]
    ConnectorNotFound(String),
    #[error("Invalid auth type '{0}' for connector '{1}'")]
    InvalidAuthType(String, String),
    #[error("Missing required field '{0}' for auth type '{1}'")]
    MissingField(String, String),
    #[error("Invalid structure for connector '{0}': {1}")]
    InvalidStructure(String, String),
}

/// Load credentials for a specific connector from the JSON configuration file
///
/// # Arguments
/// * `connector_name` - Name of the connector (e.g., "aci", "authorizedotnet")
///
/// # Returns
/// * `ConnectorAuthType` - The loaded and converted credentials
///
/// # Examples
/// ```
/// // Load Authorize.Net credentials
/// let auth = load_connector_auth("authorizedotnet").unwrap();
/// ```
pub fn load_connector_auth(connector_name: &str) -> Result<ConnectorAuthType, CredentialError> {
    load_from_json(connector_name)
}

/// Load metadata for a specific connector from the JSON configuration file
///
/// # Arguments
/// * `connector_name` - Name of the connector (e.g., "nexinets", "fiserv")
///
/// # Returns
/// * `HashMap<String, String>` - The metadata key-value pairs, or empty map if no metadata
///
/// # Examples
/// ```
/// // Load connector metadata (e.g., terminal_id, shop_name)
/// let metadata = load_connector_metadata("fiserv").unwrap();
/// let terminal_id = metadata.get("terminal_id");
/// ```
pub fn load_connector_metadata(
    connector_name: &str,
) -> Result<HashMap<String, String>, CredentialError> {
    let creds_file_path = get_creds_file_path();
    let creds_content = fs::read_to_string(&creds_file_path)?;

    let json_value: serde_json::Value = serde_json::from_str(&creds_content)?;

    let all_credentials = match load_credentials_individually(&json_value) {
        Ok(creds) => creds,
        Err(_e) => {
            // Try standard parsing as fallback
            serde_json::from_value(json_value)?
        }
    };

    let connector_creds = all_credentials
        .get(connector_name)
        .ok_or_else(|| CredentialError::ConnectorNotFound(connector_name.to_string()))?;

    match &connector_creds.metadata {
        Some(serde_json::Value::Object(map)) => {
            let mut result = HashMap::new();
            for (key, value) in map {
                if let Some(string_val) = value.as_str() {
                    result.insert(key.clone(), string_val.to_string());
                }
            }
            Ok(result)
        }
        _ => Ok(HashMap::new()),
    }
}

/// Load credentials from JSON file
fn load_from_json(connector_name: &str) -> Result<ConnectorAuthType, CredentialError> {
    let creds_file_path = get_creds_file_path();
    let creds_content = fs::read_to_string(&creds_file_path)?;

    let json_value: serde_json::Value = serde_json::from_str(&creds_content)?;

    let all_credentials = match load_credentials_individually(&json_value) {
        Ok(creds) => creds,
        Err(_e) => {
            // Try standard parsing as fallback
            serde_json::from_value(json_value)?
        }
    };

    let connector_creds = all_credentials
        .get(connector_name)
        .ok_or_else(|| CredentialError::ConnectorNotFound(connector_name.to_string()))?;

    convert_to_auth_type(&connector_creds.connector_account_details, connector_name)
}

/// Load credentials by parsing each connector individually
fn load_credentials_individually(
    json_value: &serde_json::Value,
) -> Result<AllCredentials, CredentialError> {
    let mut all_credentials = HashMap::new();

    let root_object = json_value.as_object().ok_or_else(|| {
        CredentialError::InvalidStructure(
            "root".to_string(),
            "Expected JSON object at root".to_string(),
        )
    })?;

    for (connector_name, connector_value) in root_object {
        match parse_single_connector(connector_name, connector_value) {
            Ok(creds) => {
                all_credentials.insert(connector_name.clone(), creds);
            }
            Err(_e) => {
                // Continue loading other connectors instead of failing completely
            }
        }
    }

    if all_credentials.is_empty() {
        return Err(CredentialError::InvalidStructure(
            "root".to_string(),
            "No valid connectors found".to_string(),
        ));
    }

    Ok(all_credentials)
}

/// Parse a single connector's credentials
fn parse_single_connector(
    connector_name: &str,
    connector_value: &serde_json::Value,
) -> Result<ConnectorCredentials, CredentialError> {
    let connector_obj = connector_value.as_object().ok_or_else(|| {
        CredentialError::InvalidStructure(
            connector_name.to_string(),
            "Expected JSON object".to_string(),
        )
    })?;

    // Check if this is a flat structure (has connector_account_details directly)
    if connector_obj.contains_key("connector_account_details") {
        // Flat structure: connector_name -> { connector_account_details: {...} }
        return parse_connector_credentials(connector_name, connector_value);
    }

    // Nested structure: connector_name -> { connector_1: {...}, connector_2: {...} } eg. stripe
    for (_sub_name, sub_value) in connector_obj.iter() {
        if let Some(sub_obj) = sub_value.as_object() {
            if sub_obj.contains_key("connector_account_details") {
                return parse_connector_credentials(connector_name, sub_value);
            }
        }
    }

    // If we get here, no valid connector_account_details was found
    Err(CredentialError::InvalidStructure(
        connector_name.to_string(),
        "No connector_account_details found in flat or nested structure".to_string(),
    ))
}

/// Parse connector credentials from JSON value
fn parse_connector_credentials(
    connector_name: &str,
    connector_value: &serde_json::Value,
) -> Result<ConnectorCredentials, CredentialError> {
    let connector_obj = connector_value.as_object().ok_or_else(|| {
        CredentialError::InvalidStructure(
            connector_name.to_string(),
            "Expected JSON object".to_string(),
        )
    })?;

    let account_details_value =
        connector_obj
            .get("connector_account_details")
            .ok_or_else(|| {
                CredentialError::InvalidStructure(
                    connector_name.to_string(),
                    "Missing connector_account_details".to_string(),
                )
            })?;

    let account_details = parse_connector_account_details(connector_name, account_details_value)?;

    // Parse metadata if present
    let metadata = connector_obj
        .get("metadata")
        .map(|v| serde_json::from_value(v.clone()))
        .transpose()?;

    Ok(ConnectorCredentials {
        connector_account_details: account_details,
        metadata,
    })
}

/// Parse connector account details
fn parse_connector_account_details(
    connector_name: &str,
    value: &serde_json::Value,
) -> Result<ConnectorAccountDetails, CredentialError> {
    let obj = value.as_object().ok_or_else(|| {
        CredentialError::InvalidStructure(
            connector_name.to_string(),
            "connector_account_details must be an object".to_string(),
        )
    })?;

    // Extract auth_type first
    let auth_type = obj
        .get("auth_type")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            CredentialError::InvalidStructure(
                connector_name.to_string(),
                "Missing or invalid auth_type".to_string(),
            )
        })?
        .to_string();

    // Handle different auth types with specific parsing logic
    match auth_type.as_str() {
        "CurrencyAuthKey" => {
            // Special handling for CurrencyAuthKey which has complex nested structure
            parse_currency_auth_key_details(connector_name, obj)
        }
        _ => {
            // For other auth types, use standard serde parsing
            serde_json::from_value(value.clone()).map_err(CredentialError::ParseError)
        }
    }
}

/// Special parsing logic for CurrencyAuthKey auth type
fn parse_currency_auth_key_details(
    connector_name: &str,
    obj: &serde_json::Map<String, serde_json::Value>,
) -> Result<ConnectorAccountDetails, CredentialError> {
    let auth_key_map_value = obj.get("auth_key_map").ok_or_else(|| {
        CredentialError::InvalidStructure(
            connector_name.to_string(),
            "Missing auth_key_map for CurrencyAuthKey".to_string(),
        )
    })?;

    let auth_key_map_obj = auth_key_map_value.as_object().ok_or_else(|| {
        CredentialError::InvalidStructure(
            connector_name.to_string(),
            "auth_key_map must be an object".to_string(),
        )
    })?;

    let mut auth_key_map = HashMap::new();

    for (currency_str, secret_value) in auth_key_map_obj {
        let currency = currency_str.parse::<Currency>().map_err(|_| {
            CredentialError::InvalidStructure(
                connector_name.to_string(),
                format!("Invalid currency: {currency_str}"),
            )
        })?;

        let secret_serde_value = SecretSerdeValue::new(secret_value.clone());
        auth_key_map.insert(currency, secret_serde_value);
    }

    Ok(ConnectorAccountDetails {
        auth_type: "CurrencyAuthKey".to_string(),
        api_key: None,
        key1: None,
        api_secret: None,
        key2: None,
        certificate: None,
        private_key: None,
        auth_key_map: Some(auth_key_map),
    })
}

/// Convert generic credential details to specific ConnectorAuthType
fn convert_to_auth_type(
    details: &ConnectorAccountDetails,
    connector_name: &str,
) -> Result<ConnectorAuthType, CredentialError> {
    match details.auth_type.as_str() {
        "HeaderKey" => {
            let api_key = details.api_key.as_ref().ok_or_else(|| {
                CredentialError::MissingField("api_key".to_string(), "HeaderKey".to_string())
            })?;

            Ok(ConnectorAuthType::HeaderKey {
                api_key: Secret::new(api_key.clone()),
            })
        }
        "BodyKey" => {
            let api_key = details.api_key.as_ref().ok_or_else(|| {
                CredentialError::MissingField("api_key".to_string(), "BodyKey".to_string())
            })?;
            let key1 = details.key1.as_ref().ok_or_else(|| {
                CredentialError::MissingField("key1".to_string(), "BodyKey".to_string())
            })?;

            Ok(ConnectorAuthType::BodyKey {
                api_key: Secret::new(api_key.clone()),
                key1: Secret::new(key1.clone()),
            })
        }
        "SignatureKey" => {
            let api_key = details.api_key.as_ref().ok_or_else(|| {
                CredentialError::MissingField("api_key".to_string(), "SignatureKey".to_string())
            })?;
            let key1 = details.key1.as_ref().ok_or_else(|| {
                CredentialError::MissingField("key1".to_string(), "SignatureKey".to_string())
            })?;
            let api_secret = details.api_secret.as_ref().ok_or_else(|| {
                CredentialError::MissingField("api_secret".to_string(), "SignatureKey".to_string())
            })?;

            Ok(ConnectorAuthType::SignatureKey {
                api_key: Secret::new(api_key.clone()),
                key1: Secret::new(key1.clone()),
                api_secret: Secret::new(api_secret.clone()),
            })
        }
        "MultiAuthKey" => {
            let api_key = details.api_key.as_ref().ok_or_else(|| {
                CredentialError::MissingField("api_key".to_string(), "MultiAuthKey".to_string())
            })?;
            let key1 = details.key1.as_ref().ok_or_else(|| {
                CredentialError::MissingField("key1".to_string(), "MultiAuthKey".to_string())
            })?;
            let api_secret = details.api_secret.as_ref().ok_or_else(|| {
                CredentialError::MissingField("api_secret".to_string(), "MultiAuthKey".to_string())
            })?;
            let key2 = details.key2.as_ref().ok_or_else(|| {
                CredentialError::MissingField("key2".to_string(), "MultiAuthKey".to_string())
            })?;

            Ok(ConnectorAuthType::MultiAuthKey {
                api_key: Secret::new(api_key.clone()),
                key1: Secret::new(key1.clone()),
                api_secret: Secret::new(api_secret.clone()),
                key2: Secret::new(key2.clone()),
            })
        }
        "CurrencyAuthKey" => {
            // For CurrencyAuthKey, we expect the auth_key_map field to contain the mapping
            let auth_key_map = details.auth_key_map.as_ref().ok_or_else(|| {
                CredentialError::MissingField(
                    "auth_key_map".to_string(),
                    "CurrencyAuthKey".to_string(),
                )
            })?;

            Ok(ConnectorAuthType::CurrencyAuthKey {
                auth_key_map: auth_key_map.clone(),
            })
        }
        "CertificateAuth" => {
            let certificate = details.certificate.as_ref().ok_or_else(|| {
                CredentialError::MissingField(
                    "certificate".to_string(),
                    "CertificateAuth".to_string(),
                )
            })?;
            let private_key = details.private_key.as_ref().ok_or_else(|| {
                CredentialError::MissingField(
                    "private_key".to_string(),
                    "CertificateAuth".to_string(),
                )
            })?;

            Ok(ConnectorAuthType::CertificateAuth {
                certificate: Secret::new(certificate.clone()),
                private_key: Secret::new(private_key.clone()),
            })
        }
        "NoKey" => Ok(ConnectorAuthType::NoKey),
        "TemporaryAuth" => Ok(ConnectorAuthType::TemporaryAuth),
        _ => Err(CredentialError::InvalidAuthType(
            details.auth_type.clone(),
            connector_name.to_string(),
        )),
    }
}
