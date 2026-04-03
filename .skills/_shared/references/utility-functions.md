# Utility Functions Reference

> **RULE: Always check utility functions before implementing custom ones.**
> Many common operations (country codes, date formatting, card handling, amount conversion, state codes, XML/JSON, errors) already have utilities. Using them ensures consistency, reduces duplication, and prevents bugs.

---

## Error Handling Utilities

### `missing_field_err`
- **Location:** `domain_types::utils::missing_field_err`
- **Signature:** `fn missing_field_err(message: &'static str) -> Box<dyn Fn() -> Report<IntegrationError> + 'static>`
- **Description:** Creates a closure that generates a `MissingRequiredField` error.
- **Example:**
```rust
let return_url = data.router_return_url
    .clone()
    .ok_or_else(missing_field_err("return_url"))?;
```

### `handle_json_response_deserialization_failure`
- **Location:** `domain_types::utils::handle_json_response_deserialization_failure`
- **Signature:** `fn handle_json_response_deserialization_failure(res: Response, connector: &'static str) -> CustomResult<ErrorResponse, IntegrationError>`
- **Description:** Fallback handler when JSON deserialization fails; checks if response is HTML/text.
- **Example:**
```rust
serde_json::from_str::<ErrorResponse>(&response_data)
    .change_context(errors::ConnectorResponseTransformationError::ResponseDeserializationFailed { context: Default::default() })
    .or_else(|_| handle_json_response_deserialization_failure(res, "connector_name"))
```

### `construct_not_supported_error_report`
- **Location:** `domain_types::utils`
- **Signature:** `fn construct_not_supported_error_report(capture_method: CaptureMethod, connector_name: &'static str) -> Report<IntegrationError>`
- **Description:** Standardized error for unsupported capture methods/features.

### `get_unimplemented_payment_method_error_message`
- **Location:** `domain_types::utils`
- **Signature:** `fn get_unimplemented_payment_method_error_message(connector: &str) -> String`
- **Example:**
```rust
PaymentMethodData::Wallet(_) => Err(errors::IntegrationError::NotImplemented(
    get_unimplemented_payment_method_error_message("connector_name", Default::default())
))?,
```

---

## Amount Conversion Utilities

### `convert_amount`
- **Location:** `domain_types::utils::convert_amount`
- **Signature:** `fn convert_amount<T>(amount_convertor: &dyn AmountConvertor<Output = T>, amount: MinorUnit, currency: Currency) -> Result<T, Error>`
- **Description:** Converts amount from minor units to connector's required format. Handles currency-specific decimal places (JPY, KWD, etc.).
- **Example:**
```rust
use common_utils::types::StringMajorUnitForConnector;
let amount_str = convert_amount(&StringMajorUnitForConnector, item.amount, item.currency)?;
```

### Available Amount Convertors (`common_utils::types`)
| Convertor | Output | Example |
|-----------|--------|---------|
| `StringMajorUnitForConnector` | String major units | `"10.00"` |
| `StringMinorUnitForConnector` | String minor units | `"1000"` |
| `FloatMajorUnitForConnector` | Float major units | `10.00` |
| `MinorUnitForConnector` | MinorUnit passthrough | `1000` |

### `convert_back_amount_to_minor_units`
- **Location:** `domain_types::utils`
- **Signature:** `fn convert_back_amount_to_minor_units<T>(amount_convertor: &dyn AmountConvertor<Output = T>, amount: T, currency: Currency) -> Result<MinorUnit, Error>`
- **Description:** Converts connector format back to minor units for responses.

### `to_currency_base_unit`
- **Location:** `domain_types::utils::to_currency_base_unit`
- **Signature:** `fn to_currency_base_unit(amount: i64, currency: Currency) -> Result<String, Error>`
- **Description:** Converts minor unit amount to base unit string (e.g., 1000 cents -> "10.00").

### `to_currency_base_unit_with_zero_decimal_check`
- **Location:** `domain_types::utils`
- **Description:** Same as above but with special handling for zero-decimal currencies (JPY, etc.).

---

## Data Transformation Utilities

### `to_connector_meta_from_secret`
- **Location:** `connector_integration::utils::to_connector_meta_from_secret`
- **Signature:** `fn to_connector_meta_from_secret<T>(connector_meta: Option<Secret<Value>>) -> Result<T, Error>`
- **Description:** Deserializes connector metadata from secret JSON to a typed struct.
- **Example:**
```rust
let meta: ConnectorMeta = to_connector_meta_from_secret(item.connector_meta_data.clone())?;
```

### `convert_uppercase`
- **Location:** `connector_integration::utils::convert_uppercase`
- **Signature:** `fn convert_uppercase<D, T>(v: D) -> Result<T, D::Error>`
- **Description:** Serde deserializer that converts strings to uppercase during deserialization.
- **Example:**
```rust
#[derive(Deserialize)]
struct Response {
    #[serde(deserialize_with = "convert_uppercase")]
    status: StatusEnum,
}
```

### `convert_country_alpha2_to_alpha3`
- **Location:** `domain_types::utils::convert_country_alpha2_to_alpha3`
- **Description:** Converts ISO country code from 2-letter to 3-letter format. Handles all 249 codes.
- **Example:**
```rust
let alpha3 = convert_country_alpha2_to_alpha3(&billing_address.country)?;
```

### `convert_us_state_to_code`
- **Location:** `domain_types::utils::convert_us_state_to_code`
- **Signature:** `fn convert_us_state_to_code(state: &str) -> String`
- **Description:** Converts US state full names to 2-letter codes ("California" -> "CA"). Covers all 50 states + territories.

### `deserialize_zero_minor_amount_as_none`
- **Location:** `connector_integration::utils`
- **Description:** Deserializes zero amounts as `None` instead of `Some(0)`.
- **Example:**
```rust
#[serde(deserialize_with = "deserialize_zero_minor_amount_as_none")]
refunded_amount: Option<MinorUnit>,
```

---

## Card Processing Utilities

### `get_card_details`
- **Location:** `domain_types::utils::get_card_details`
- **Signature:** `fn get_card_details<T>(payment_method_data: PaymentMethodData<T>, connector_name: &'static str) -> Result<Card<T>, IntegrationError>`
- **Description:** Extracts card details from payment method data; errors if not a card payment.
- **Example:**
```rust
let card = get_card_details(item.payment_method_data, "connector_name")?;
```

### `get_card_issuer`
- **Location:** `domain_types::utils::get_card_issuer`
- **Signature:** `fn get_card_issuer(card_number: &str) -> Result<CardIssuer, Error>`
- **Description:** Identifies card network from card number using BIN patterns (Visa, Mastercard, Amex, etc.).
- **Example:**
```rust
let issuer = get_card_issuer(&card.card_number.peek())?;
```

### `get_card_expiry_month_year_2_digit_with_delimiter`
- **Location:** `domain_types::utils`
- **Description:** Formats card expiry as "MM/YY" (or custom delimiter). Handles validation and padding.
- **Example:**
```rust
let expiry = get_card_expiry_month_year_2_digit_with_delimiter(&card.expiry_month, &card.expiry_year, "/")?;
```

### `is_mandate_supported`
- **Location:** `domain_types::utils`
- **Signature:** `fn is_mandate_supported<T>(selected_pmd: PaymentMethodData<T>, payment_method_type: Option<PaymentMethodType>, mandate_implemented_pmds: HashSet<PaymentMethodDataType>, connector: &'static str) -> Result<(), Error>`
- **Description:** Validates if a payment method supports mandate/recurring payments.

---

## Date/Time Utilities

### `now` / `now_unix_timestamp`
- **Location:** `common_utils::date_time`
- `fn now() -> PrimitiveDateTime` -- current UTC time
- `fn now_unix_timestamp() -> i64` -- current UNIX timestamp (seconds)

### `get_timestamp_in_milliseconds`
- **Location:** `domain_types::utils`
- **Signature:** `fn get_timestamp_in_milliseconds(datetime: &PrimitiveDateTime) -> i64`
- **Example:**
```rust
let ts_ms = get_timestamp_in_milliseconds(&item.created_at);
```

### `format_date`
- **Location:** `common_utils::date_time::format_date`
- **Signature:** `fn format_date(date: PrimitiveDateTime, format: DateFormat) -> Result<String, time::error::Format>`
- **Supported formats:** `YYYYMMDDHHmmss`, `YYYYMMDD`, `YYYYMMDDHHmm`, `DDMMYYYYHHmmss`
- **Example:**
```rust
use common_utils::date_time::{format_date, DateFormat, now};
let formatted = format_date(now(), DateFormat::YYYYMMDDHHmmss)?; // "20250117153045"
```

### `date_as_yyyymmddthhmmssmmmz`
- **Location:** `common_utils::date_time`
- **Description:** Returns current date in ISO8601 with milliseconds (`"2025-01-17T15:30:45.123Z"`).

---

## XML/JSON Utilities

### `preprocess_xml_response_bytes`
- **Location:** `connector_integration::utils::xml_utils::preprocess_xml_response_bytes`
- **Signature:** `fn preprocess_xml_response_bytes(xml_data: Bytes) -> Result<Bytes, IntegrationError>`
- **Description:** Converts XML response to JSON bytes for deserialization into Rust structs.
- **Example:**
```rust
let json_bytes = preprocess_xml_response_bytes(res.response)?;
let response: ConnectorResponse = serde_json::from_slice(&json_bytes)
    .change_context(errors::ConnectorResponseTransformationError::ResponseDeserializationFailed { context: Default::default() })?;
```

### `serialize_to_xml_string_with_root`
- **Location:** `connector_integration::utils`
- **Signature:** `fn serialize_to_xml_string_with_root<T: Serialize>(root_name: &str, data: &T) -> Result<String, Error>`
- **Description:** Serializes struct to XML with declaration and custom root element.
- **Example:**
```rust
let xml_body = serialize_to_xml_string_with_root("transaction", &request)?;
// <?xml version="1.0" encoding="UTF-8"?><transaction>...</transaction>
```

---

## Additional Utilities

### Header / ID / Crypto
| Function | Location | Description |
|----------|----------|-------------|
| `get_http_header(key, headers)` | `domain_types::utils` | Extract header value from HeaderMap |
| `generate_random_bytes(len)` | `domain_types::utils` | Cryptographically secure random bytes |
| `base64_decode(data)` | `domain_types::utils` | Decode base64 string to bytes |
| `generate_id(length, prefix)` | `common_utils::fp_utils` | Unique ID with custom length/prefix |
| `generate_id_with_default_len(prefix)` | `common_utils::fp_utils` | Unique ID with default length |
| `generate_time_ordered_id(prefix)` | `common_utils` | UUIDv7 time-sortable ID |

### Validation
| Function | Location | Description |
|----------|----------|-------------|
| `is_payment_failure(status)` | `domain_types::utils` | Check if AttemptStatus is a failure |
| `is_refund_failure(status)` | `connector_integration::utils` | Check if RefundStatus is a failure |

### Traits
- **`ValueExt`** (`domain_types::utils`): `json_value.parse_value::<T>("TypeName")?` -- parse JSON Value to typed struct
- **`Encode`** (`domain_types::utils`): `struct.encode_to_value()?` -- convert struct to serde_json::Value
- **`PaymentsAuthorizeRequestData`** (`connector_integration::utils`): `item.get_router_return_url()?` -- safely extract return URL

### Logging Macros
- `with_response_body!(event_builder, success_response)` -- log success responses
- `with_error_response_body!(event_builder, error_response)` -- log error responses

---

## Quick Reference

| Use Case | Function | Module |
|----------|----------|--------|
| Missing field error | `missing_field_err("field")` | `domain_types::utils` |
| Amount to major string | `convert_amount(&StringMajorUnitForConnector, ..)` | `domain_types::utils` |
| Amount to minor string | `convert_amount(&StringMinorUnitForConnector, ..)` | `domain_types::utils` |
| Parse XML response | `preprocess_xml_response_bytes(bytes)` | `connector_integration::utils` |
| Serialize to XML | `serialize_to_xml_string_with_root("root", &data)` | `connector_integration::utils` |
| Card network from BIN | `get_card_issuer(card_number)` | `domain_types::utils` |
| Card expiry formatting | `get_card_expiry_month_year_2_digit_with_delimiter(..)` | `domain_types::utils` |
| Country alpha2 to alpha3 | `convert_country_alpha2_to_alpha3(&country)` | `domain_types::utils` |
| US state to code | `convert_us_state_to_code("California")` | `domain_types::utils` |
| Current timestamp (s) | `now_unix_timestamp()` | `common_utils::date_time` |
| Current timestamp (ms) | `get_timestamp_in_milliseconds(&now())` | `domain_types::utils` |
| Format date | `format_date(date, DateFormat::YYYYMMDDHHmmss)` | `common_utils::date_time` |
| Extract HTTP header | `get_http_header("X-Header", headers)` | `domain_types::utils` |
| Extract card data | `get_card_details(pmd, "connector")` | `domain_types::utils` |
| Parse connector meta | `to_connector_meta_from_secret(meta)` | `connector_integration::utils` |
| Unimplemented PM error | `get_unimplemented_payment_method_error_message(..)` | `domain_types::utils` |
