# UCS Connector Utility Functions Reference

This document provides a comprehensive mapping of all utility functions available in the UCS connector-service codebase. Use these functions during connector integration to avoid code duplication and maintain consistency.

---

## **CHECK THIS REFERENCE BEFORE IMPLEMENTING CUSTOM LOGIC**

**IMPORTANT**: Many common operations already have utility functions in the codebase. Before writing custom logic for:
- Country code conversion
- Date/time formatting
- Card number handling
- Amount conversion
- State code conversion
- XML/JSON processing
- Error handling

**→ Search this document first!** Using existing utilities ensures consistency, reduces code duplication, and prevents bugs.

### Common Operations - Use These Utilities!

#### **Country Code Conversion**
❌ **WRONG** - Custom implementation:
```rust
fn alpha2_to_alpha3(code: &str) -> String {
    match code {
        "US" => "USA".to_string(),
        "GB" => "GBR".to_string(),
        "CA" => "CAN".to_string(),
        // ... hundreds of lines ...
    }
}
```

✅ **RIGHT** - Use existing utility:
```rust
use domain_types::utils::convert_country_alpha2_to_alpha3;

let alpha3_code = convert_country_alpha2_to_alpha3(&billing_address.country)?;
// Handles all 249 ISO country codes automatically
```

#### **Card Expiry Date Formatting**
❌ **WRONG** - Manual string manipulation:
```rust
let month = card.expiry_month.clone();
let year = card.expiry_year.clone();
let expiry = format!("{}/{}", month, &year[2..]);
```

✅ **RIGHT** - Use existing utility:
```rust
use domain_types::utils::get_card_expiry_month_year_2_digit_with_delimiter;

let expiry = get_card_expiry_month_year_2_digit_with_delimiter(
    &card.expiry_month,
    &card.expiry_year,
    "/"
)?;
// Handles validation, padding, and formatting automatically
```

#### **US State Code Conversion**
❌ **WRONG** - Manual state mapping:
```rust
let state_code = match state_name {
    "California" => "CA",
    "New York" => "NY",
    // ... 50 states ...
    _ => state_name,
};
```

✅ **RIGHT** - Use existing utility:
```rust
use domain_types::utils::convert_us_state_to_code;

let state_code = convert_us_state_to_code(&billing_address.state);
// Handles all 50 US states + territories
```

#### **Amount Conversion**
❌ **WRONG** - Manual division/formatting:
```rust
let amount_str = format!("{:.2}", item.amount as f64 / 100.0);
```

✅ **RIGHT** - Use amount convertors:
```rust
use common_utils::types::StringMajorUnitForConnector;
use domain_types::utils::convert_amount;

let amount_str = convert_amount(
    &StringMajorUnitForConnector,
    item.amount,
    item.currency
)?;
// Handles currency-specific decimal places (JPY, KWD, etc.)
```

#### **Card Network Detection**
❌ **WRONG** - Partial BIN regex:
```rust
let network = if card_number.starts_with("4") {
    "visa"
} else if card_number.starts_with("5") {
    "mastercard"
} // Missing many networks...
```

✅ **RIGHT** - Use card issuer utility:
```rust
use domain_types::utils::get_card_issuer;

let issuer = get_card_issuer(&card.card_number.peek())?;
match issuer {
    CardIssuer::Visa => "visa",
    CardIssuer::Mastercard => "mastercard",
    CardIssuer::AmericanExpress => "amex",
    // Complete BIN database coverage
}
```

#### **Date Formatting**
❌ **WRONG** - Manual string formatting:
```rust
let formatted = format!("{}{:02}{:02}{:02}{:02}{:02}",
    now.year(), now.month(), now.day(),
    now.hour(), now.minute(), now.second());
```

✅ **RIGHT** - Use date formatting utility:
```rust
use common_utils::date_time::{format_date, DateFormat, now};

let formatted = format_date(now(), DateFormat::YYYYMMDDHHmmss)?;
// Supports: YYYYMMDDHHmmss, YYYYMMDD, YYYYMMDDHHmm, DDMMYYYYHHmmss
```

### When You Find Yourself Writing...

| **If you're writing...** | **Use this instead** |
|-------------------------|---------------------|
| Custom country code mappings | `convert_country_alpha2_to_alpha3` |
| Card expiry date string manipulation | `get_card_expiry_month_year_2_digit_with_delimiter` |
| State name to code conversion | `convert_us_state_to_code` |
| Amount to string conversion | `convert_amount` with appropriate convertor |
| BIN/card network detection | `get_card_issuer` |
| Date formatting with format strings | `format_date` with `DateFormat` enum |
| XML response parsing | `preprocess_xml_response_bytes` |
| Missing field errors | `missing_field_err("field_name")` |
| Payment method not implemented errors | `get_unimplemented_payment_method_error_message` |
| Current timestamp generation | `now_unix_timestamp()` or `get_timestamp_in_milliseconds` |

**Remember**: If the operation feels "common", it probably has a utility function. Search this document before implementing!

---

## Table of Contents
- [Error Handling Utilities](#error-handling-utilities)
- [Amount Conversion Utilities](#amount-conversion-utilities)
- [Data Transformation Utilities](#data-transformation-utilities)
- [XML/JSON Utilities](#xmljson-utilities)
- [Card Processing Utilities](#card-processing-utilities)
- [Date/Time Utilities](#datetime-utilities)
- [Validation Utilities](#validation-utilities)
- [Helper Macros](#helper-macros)

---

## Error Handling Utilities

### `missing_field_err`
**Location:** `domain_types::utils::missing_field_err`
**Signature:** `fn missing_field_err(message: &'static str) -> Box<dyn Fn() -> Report<ConnectorError> + 'static>`
**Description:** Creates a closure that generates a `MissingRequiredField` error for the specified field name.
**Use Case:** Use when a required field is missing from the request or response.
**Example:**
```rust
let return_url = data.router_return_url
    .clone()
    .ok_or_else(missing_field_err("return_url"))?;
```

### `handle_json_response_deserialization_failure`
**Location:** `domain_types::utils::handle_json_response_deserialization_failure`
**Signature:** `fn handle_json_response_deserialization_failure(res: Response, connector: &'static str) -> CustomResult<ErrorResponse, ConnectorError>`
**Description:** Handles cases where JSON deserialization fails by checking if response is valid JSON or HTML/text.
**Use Case:** Use in `build_error_response` when deserialization of expected error response format fails.
**Example:**
```rust
fn build_error_response(&self, res: Response, event_builder: Option<&mut ConnectorEvent>)
    -> CustomResult<ErrorResponse, errors::ConnectorError> {
    let response_data = String::from_utf8(res.response.to_vec())
        .change_context(errors::ConnectorError::ResponseDeserializationFailed)?;

    serde_json::from_str::<ErrorResponse>(&response_data)
        .change_context(errors::ConnectorError::ResponseDeserializationFailed)
        .or_else(|_| handle_json_response_deserialization_failure(res, "connector_name"))
}
```

### `construct_not_supported_error_report`
**Location:** `domain_types::utils::construct_not_supported_error_report`
**Signature:** `fn construct_not_supported_error_report(capture_method: CaptureMethod, connector_name: &'static str) -> Report<ConnectorError>`
**Description:** Creates a standardized error report for unsupported features.
**Use Case:** When a specific capture method or feature is not supported by the connector.

### `get_unimplemented_payment_method_error_message`
**Location:** `domain_types::utils::get_unimplemented_payment_method_error_message`
**Signature:** `fn get_unimplemented_payment_method_error_message(connector: &str) -> String`
**Description:** Generates a standardized error message for unimplemented payment methods.
**Use Case:** When a payment method is not yet implemented for the connector.
**Example:**
```rust
PaymentMethodData::Wallet(_) => Err(errors::ConnectorError::NotImplemented(
    get_unimplemented_payment_method_error_message("connector_name")
))?,
```

---

## Amount Conversion Utilities

### `convert_amount`
**Location:** `domain_types::utils::convert_amount`
**Signature:** `fn convert_amount<T>(amount_convertor: &dyn AmountConvertor<Output = T>, amount: MinorUnit, currency: Currency) -> Result<T, Error>`
**Description:** Converts amount from minor units to the connector's required format using the specified amount convertor.
**Use Case:** Converting amount to connector-specific format (string, float, major/minor units).
**Example:**
```rust
use common_utils::{types::StringMajorUnitForConnector, AmountConvertor};

let amount_str = convert_amount(
    &StringMajorUnitForConnector,
    item.amount,
    item.currency
)?;
```

### `convert_back_amount_to_minor_units`
**Location:** `domain_types::utils::convert_back_amount_to_minor_units`
**Signature:** `fn convert_back_amount_to_minor_units<T>(amount_convertor: &dyn AmountConvertor<Output = T>, amount: T, currency: Currency) -> Result<MinorUnit, Error>`
**Description:** Converts amount from connector format back to minor units.
**Use Case:** Converting response amounts back to core format.

### `get_amount_as_string`
**Location:** `domain_types::utils::get_amount_as_string`
**Signature:** `fn get_amount_as_string(currency_unit: &CurrencyUnit, amount: i64, currency: Currency) -> Result<String, Error>`
**Description:** Converts amount to string based on currency unit (Minor or Base).
**Use Case:** When you need amount as a string with proper currency formatting.

### `to_currency_base_unit`
**Location:** `domain_types::utils::to_currency_base_unit`
**Signature:** `fn to_currency_base_unit(amount: i64, currency: Currency) -> Result<String, Error>`
**Description:** Converts minor unit amount to currency base unit as string.
**Use Case:** Converting cents to dollars (e.g., 1000 cents -> "10.00" dollars).

### `to_currency_base_unit_with_zero_decimal_check`
**Location:** `domain_types::utils::to_currency_base_unit_with_zero_decimal_check`
**Signature:** `fn to_currency_base_unit_with_zero_decimal_check(amount: i64, currency: Currency) -> Result<String, Error>`
**Description:** Converts to base unit with special handling for zero-decimal currencies (like JPY).
**Use Case:** When handling currencies with different decimal places.

### Available Amount Convertors
**Location:** `common_utils::types`
- `StringMajorUnitForConnector` - Converts to string in major units (e.g., "10.00")
- `StringMinorUnitForConnector` - Converts to string in minor units (e.g., "1000")
- `FloatMajorUnitForConnector` - Converts to float in major units (e.g., 10.00)
- `MinorUnitForConnector` - Keeps as MinorUnit (passthrough)

---

## Data Transformation Utilities

### `to_connector_meta_from_secret`
**Location:** `connector_integration::utils::to_connector_meta_from_secret`
**Signature:** `fn to_connector_meta_from_secret<T>(connector_meta: Option<Secret<Value>>) -> Result<T, Error>`
**Description:** Deserializes connector metadata from a secret JSON value to a typed struct.
**Use Case:** When you need to extract connector-specific metadata from merchant_connector_account.
**Example:**
```rust
#[derive(Deserialize)]
struct ConnectorMeta {
    merchant_id: String,
    api_version: String,
}

let meta: ConnectorMeta = to_connector_meta_from_secret(
    item.connector_meta_data.clone()
)?;
```

### `convert_uppercase`
**Location:** `connector_integration::utils::convert_uppercase`
**Signature:** `fn convert_uppercase<D, T>(v: D) -> Result<T, D::Error>`
**Description:** Serde deserializer that converts string to uppercase during deserialization.
**Use Case:** When connector returns values in lowercase but you need uppercase enum variants.
**Example:**
```rust
#[derive(Deserialize)]
struct Response {
    #[serde(deserialize_with = "convert_uppercase")]
    status: StatusEnum,
}
```

### `deserialize_zero_minor_amount_as_none`
**Location:** `connector_integration::utils::deserialize_zero_minor_amount_as_none`
**Signature:** `fn deserialize_zero_minor_amount_as_none<'de, D>(deserializer: D) -> Result<Option<MinorUnit>, D::Error>`
**Description:** Deserializes zero amounts as None instead of Some(0).
**Use Case:** When connector returns 0 for optional amounts and you want to treat them as missing.
**Example:**
```rust
#[derive(Deserialize)]
struct Response {
    #[serde(deserialize_with = "deserialize_zero_minor_amount_as_none")]
    refunded_amount: Option<MinorUnit>,
}
```

### `convert_us_state_to_code`
**Location:** `domain_types::utils::convert_us_state_to_code`
**Signature:** `fn convert_us_state_to_code(state: &str) -> String`
**Description:** Converts US state full names to 2-letter abbreviations (e.g., "California" -> "CA").
**Use Case:** When connector requires state codes but you have full state names.
**Example:**
```rust
let state_code = convert_us_state_to_code("New York"); // Returns "NY"
```

---

## XML/JSON Utilities

### `preprocess_xml_response_bytes`
**Location:** `connector_integration::utils::xml_utils::preprocess_xml_response_bytes`
**Signature:** `fn preprocess_xml_response_bytes(xml_data: Bytes) -> Result<Bytes, ConnectorError>`
**Description:** Converts XML response to properly structured JSON by parsing XML, removing declarations, flattening nested structures.
**Use Case:** When connector returns XML and you need to deserialize it into Rust structs.
**Example:**
```rust
use connector_integration::utils::preprocess_xml_response_bytes;

let json_bytes = preprocess_xml_response_bytes(res.response)?;
let response: ConnectorResponse = serde_json::from_slice(&json_bytes)
    .change_context(errors::ConnectorError::ResponseDeserializationFailed)?;
```

### `serialize_to_xml_string_with_root`
**Location:** `connector_integration::utils::serialize_to_xml_string_with_root`
**Signature:** `fn serialize_to_xml_string_with_root<T: Serialize>(root_name: &str, data: &T) -> Result<String, Error>`
**Description:** Serializes a struct to XML string with XML declaration and custom root element name.
**Use Case:** When connector requires XML request body.
**Example:**
```rust
let xml_body = serialize_to_xml_string_with_root("transaction", &request)?;
// Output: <?xml version="1.0" encoding="UTF-8"?><transaction>...</transaction>
```

---

## Card Processing Utilities

### `get_card_details`
**Location:** `domain_types::utils::get_card_details`
**Signature:** `fn get_card_details<T>(payment_method_data: PaymentMethodData<T>, connector_name: &'static str) -> Result<Card<T>, ConnectorError>`
**Description:** Extracts card details from payment method data, returning error if not a card payment.
**Use Case:** When you need to ensure payment method is a card and extract card data.
**Example:**
```rust
let card = get_card_details(item.payment_method_data, "connector_name")?;
let card_number = card.card_number;
```

### `get_card_issuer`
**Location:** `domain_types::utils::get_card_issuer`
**Signature:** `fn get_card_issuer(card_number: &str) -> Result<CardIssuer, Error>`
**Description:** Identifies card issuer/network from card number using BIN regex patterns.
**Use Case:** When connector requires card network/issuer information.
**Example:**
```rust
let issuer = get_card_issuer(&card.card_number.peek())?;
match issuer {
    CardIssuer::Visa => "visa",
    CardIssuer::Mastercard => "mastercard",
    // ...
}
```

### `is_mandate_supported`
**Location:** `domain_types::utils::is_mandate_supported`
**Signature:** `fn is_mandate_supported<T>(selected_pmd: PaymentMethodData<T>, payment_method_type: Option<PaymentMethodType>, mandate_implemented_pmds: HashSet<PaymentMethodDataType>, connector: &'static str) -> Result<(), Error>`
**Description:** Validates if the selected payment method supports mandate payments.
**Use Case:** In SetupMandate flow to validate payment method support.

---

## Date/Time Utilities

### `now`
**Location:** `common_utils::date_time::now`
**Signature:** `fn now() -> PrimitiveDateTime`
**Description:** Returns current date and time in UTC as PrimitiveDateTime.
**Use Case:** When you need current timestamp for requests.

### `now_unix_timestamp`
**Location:** `common_utils::date_time::now_unix_timestamp`
**Signature:** `fn now_unix_timestamp() -> i64`
**Description:** Returns current UNIX timestamp in seconds.
**Use Case:** When connector requires UNIX timestamp.

### `get_timestamp_in_milliseconds`
**Location:** `domain_types::utils::get_timestamp_in_milliseconds`
**Signature:** `fn get_timestamp_in_milliseconds(datetime: &PrimitiveDateTime) -> i64`
**Description:** Converts PrimitiveDateTime to UNIX timestamp in milliseconds.
**Use Case:** When connector requires timestamp in milliseconds.
**Example:**
```rust
let timestamp_ms = get_timestamp_in_milliseconds(&item.created_at);
```

### `format_date`
**Location:** `common_utils::date_time::format_date`
**Signature:** `fn format_date(date: PrimitiveDateTime, format: DateFormat) -> Result<String, time::error::Format>`
**Description:** Formats date with custom format (YYYYMMDDHHmmss, YYYYMMDD, YYYYMMDDHHmm, DDMMYYYYHHmmss).
**Use Case:** When connector requires specific date format.
**Example:**
```rust
use common_utils::date_time::{format_date, DateFormat};

let formatted = format_date(common_utils::date_time::now(), DateFormat::YYYYMMDDHHmmss)?;
// Output: "20250117153045"
```

### `date_as_yyyymmddthhmmssmmmz`
**Location:** `common_utils::date_time::date_as_yyyymmddthhmmssmmmz`
**Signature:** `fn date_as_yyyymmddthhmmssmmmz() -> Result<String, time::error::Format>`
**Description:** Returns current date in ISO8601 format with milliseconds.
**Use Case:** When connector requires ISO8601 timestamp.
**Example:**
```rust
let iso_date = common_utils::date_time::date_as_yyyymmddthhmmssmmmz()?;
// Output: "2025-01-17T15:30:45.123Z"
```

---

## Validation Utilities

### `is_payment_failure`
**Location:** `domain_types::utils::is_payment_failure`
**Signature:** `fn is_payment_failure(status: AttemptStatus) -> bool`
**Description:** Checks if payment attempt status represents a failure.
**Use Case:** When mapping connector status to core status and need to determine failure.

### `is_refund_failure`
**Location:** `connector_integration::utils::is_refund_failure`
**Signature:** `fn is_refund_failure(status: RefundStatus) -> bool`
**Description:** Checks if refund status represents a failure.
**Use Case:** When mapping connector refund status to determine failure.

---

## Helper Macros

### `with_error_response_body!`
**Location:** `connector_integration::utils`
**Description:** Sets connector response in event builder for error responses.
**Use Case:** Logging error responses in connector events.
**Example:**
```rust
with_error_response_body!(event_builder, error_response);
```

### `with_response_body!`
**Location:** `connector_integration::utils`
**Description:** Sets connector response in event builder for success responses.
**Use Case:** Logging success responses in connector events.
**Example:**
```rust
with_response_body!(event_builder, success_response);
```

---

## Additional Utilities

### `generate_random_bytes`
**Location:** `domain_types::utils::generate_random_bytes`
**Signature:** `fn generate_random_bytes(length: usize) -> Vec<u8>`
**Description:** Generates cryptographically secure random bytes of specified length.
**Use Case:** When connector requires random nonce or ID generation.

### `base64_decode`
**Location:** `domain_types::utils::base64_decode`
**Signature:** `fn base64_decode(data: String) -> Result<Vec<u8>, Error>`
**Description:** Decodes base64 string to bytes.
**Use Case:** When connector returns base64-encoded data.

### `get_http_header`
**Location:** `domain_types::utils::get_http_header`
**Signature:** `fn get_http_header<'a>(key: &str, headers: &'a http::HeaderMap) -> CustomResult<&'a str, ConnectorError>`
**Description:** Extracts header value from HTTP header map.
**Use Case:** In webhook verification when extracting signature headers.
**Example:**
```rust
let signature = get_http_header("X-Signature", headers)?;
```

### `extract_connector_request_reference_id`
**Location:** `domain_types::utils::extract_connector_request_reference_id`
**Signature:** `fn extract_connector_request_reference_id(identifier: &Option<grpc_api_types::payments::Identifier>) -> String`
**Description:** Extracts connector transaction ID from identifier (returns empty string if not found).
**Use Case:** When extracting connector transaction ID from response identifier.

### `extract_merchant_id_from_metadata`
**Location:** `domain_types::utils::extract_merchant_id_from_metadata`
**Signature:** `fn extract_merchant_id_from_metadata(metadata: &MaskedMetadata) -> Result<MerchantId, ApplicationErrorResponse>`
**Description:** Extracts merchant ID from request metadata.
**Use Case:** In webhook handlers to identify merchant from metadata.

### `generate_id_with_default_len`
**Location:** `common_utils::fp_utils::generate_id_with_default_len`
**Signature:** `fn generate_id_with_default_len(prefix: &str) -> String`
**Description:** Generates unique ID with default length and specified prefix.
**Use Case:** When generating internal reference IDs.

### `generate_id`
**Location:** `common_utils::fp_utils::generate_id`
**Signature:** `fn generate_id(length: usize, prefix: &str) -> String`
**Description:** Generates unique ID with custom length and prefix.
**Use Case:** When generating reference IDs with specific length requirements.

### `generate_time_ordered_id`
**Location:** `common_utils::generate_time_ordered_id`
**Signature:** `fn generate_time_ordered_id(prefix: &str) -> String`
**Description:** Generates time-sortable unique identifier using UUIDv7.
**Use Case:** When you need IDs that maintain temporal ordering.

---

## Traits

### `PaymentsAuthorizeRequestData`
**Location:** `connector_integration::utils::PaymentsAuthorizeRequestData`
**Methods:**
- `get_router_return_url(&self) -> Result<String, Error>` - Safely extracts return URL from authorize data

**Use Case:** Getting return URL with proper error handling.
**Example:**
```rust
let return_url = item.get_router_return_url()?;
```

### `SplitPaymentData`
**Location:** `connector_integration::utils::SplitPaymentData`
**Methods:**
- `get_split_payment_data(&self) -> Option<SplitPaymentsRequest>` - Extracts split payment information if available

**Use Case:** When connector supports split payments/marketplace payments.

### `ValueExt`
**Location:** `domain_types::utils::ValueExt`
**Methods:**
- `parse_value<T>(self, type_name: &'static str) -> Result<T, ParsingError>` - Parse JSON value to typed struct

**Use Case:** When parsing dynamic JSON values to specific types.
**Example:**
```rust
use domain_types::utils::ValueExt;

let parsed: MyStruct = json_value.parse_value("MyStruct")?;
```

### `Encode`
**Location:** `domain_types::utils::Encode`
**Methods:**
- `encode_to_value(&self) -> Result<serde_json::Value, ParsingError>` - Convert struct to JSON value

**Use Case:** When you need to convert struct to Value for dynamic manipulation.

---

## Best Practices

1. **Import utilities from correct locations**: Use `domain_types::utils` for domain utilities, `connector_integration::utils` for connector-specific utilities
2. **Prefer utility functions over manual implementation**: Reduces code duplication and ensures consistency
3. **Use amount convertors consistently**: Always use the appropriate convertor for the connector's amount format
4. **Handle errors properly**: All utility functions return proper error types - don't unwrap
5. **Use macros for logging**: `with_error_response_body!` and `with_response_body!` for consistent logging

---

## Quick Reference by Use Case

| **Use Case** | **Function** | **Location** |
|-------------|-------------|-------------|
| Missing field error | `missing_field_err("field_name")` | `domain_types::utils` |
| Convert to major units (string) | `convert_amount(&StringMajorUnitForConnector, amount, currency)` | `domain_types::utils` |
| Convert to minor units (string) | `convert_amount(&StringMinorUnitForConnector, amount, currency)` | `domain_types::utils` |
| Parse XML response | `preprocess_xml_response_bytes(bytes)` | `connector_integration::utils::xml_utils` |
| Serialize to XML | `serialize_to_xml_string_with_root("root", &data)` | `connector_integration::utils` |
| Get card network | `get_card_issuer(card_number)` | `domain_types::utils` |
| Current timestamp (seconds) | `now_unix_timestamp()` | `common_utils::date_time` |
| Current timestamp (milliseconds) | `get_timestamp_in_milliseconds(&now())` | `domain_types::utils` |
| Format date | `format_date(date, DateFormat::YYYYMMDDHHmmss)` | `common_utils::date_time` |
| Extract header | `get_http_header("X-Header", headers)` | `domain_types::utils` |
| State to code | `convert_us_state_to_code("California")` | `domain_types::utils` |
| Check payment failure | `is_payment_failure(status)` | `domain_types::utils` |
| Extract card data | `get_card_details(payment_method_data, "connector")` | `domain_types::utils` |
| Parse connector meta | `to_connector_meta_from_secret(meta)` | `connector_integration::utils` |
