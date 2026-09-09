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
- **Signature:** `fn handle_json_response_deserialization_failure(res: Response, connector: &'static str) -> CustomResult<ErrorResponse, ConnectorError>`
- **Note:** the error type is `ConnectorError` (response phase), **not** `IntegrationError`. The
  `connector` argument is currently ignored by the implementation but is still required.
- **Description:** Fallback handler when JSON deserialization fails; checks if response is HTML/text.
- **Example:**
```rust
serde_json::from_str::<ErrorResponse>(&response_data)
    .change_context(errors::ConnectorError::ResponseDeserializationFailed { context: Default::default() })
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
// NotImplemented is a TUPLE variant of TWO fields:
//   NotImplemented(String, IntegrationErrorContext)
// The helper itself takes exactly ONE argument.
PaymentMethodData::Wallet(_) => Err(errors::IntegrationError::NotImplemented(
    get_unimplemented_payment_method_error_message("connector_name"),
    Default::default(),
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

There are **five**, not four:

| Convertor | Output | Example | Connectors at HEAD |
|-----------|--------|---------|--------------------|
| `StringMajorUnitForConnector` | String major units | `"10.00"` | 25 |
| `FloatMajorUnitForConnector` | Float major units | `10.00` | 22 |
| `MinorUnitForConnector` | MinorUnit passthrough | `1000` | 11 |
| `StringMinorUnitForConnector` | String minor units | `"1000"` | 10 |
| `StringTwoDecimalUnitForConnector` | String, always 2 dp even for zero-decimal currencies | `"10.00"` | 0 |

**Pick the one that matches the vendor spec's wire format.** There is no safe default:
`StringMinorUnit` accounts for only 10 of the 67 converter-declaring connectors, so treating it as
the fallback is wrong about 85% of the time. A decimal point in the vendor's sample payload means a
*major* unit; quotes mean a *String* variant.

### `convert_back_amount_to_minor_units`
- **Location:** `domain_types::utils`
- **Signature:** `fn convert_back_amount_to_minor_units<T>(amount_convertor: &dyn AmountConvertor<Output = T>, amount: T, currency: Currency) -> Result<MinorUnit, Error>`
- **Description:** Converts connector format back to minor units for responses.

### `to_currency_base_unit`
- **Location:** `domain_types::utils::to_currency_base_unit`
- **Signature:** `fn to_currency_base_unit(amount: MinorUnit, currency: Currency) -> Result<String, Report<IntegrationError>>`
- **Description:** Converts minor unit amount to base unit string (e.g., 1000 cents -> "10.00").
  Takes a `MinorUnit`, not a bare `i64`. (`to_currency_base_unit_with_zero_decimal_check` *does*
  take `amount: i64` -- the two differ.)

### `to_currency_base_unit_with_zero_decimal_check`
- **Location:** `domain_types::utils`
- **Description:** Same as above but with special handling for zero-decimal currencies (JPY, etc.).

---

## Data Transformation Utilities

### `to_connector_meta_from_secret`
- **Location:** `connector_integration::utils::to_connector_meta_from_secret`
- **Signature:** `pub(crate) fn to_connector_meta_from_secret<T: DeserializeOwned>(connector_meta: Option<Secret<Value>>) -> Result<T, Error>`
- **Note:** `pub(crate)` -- reachable from connector modules via `crate::utils::` or `utils::`,
  but not from outside the `connector-integration` crate. Handles both a JSON object and a JSON
  string containing JSON.
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

### Country alpha-2 to alpha-3
- **Location:** `common_enums::transformers` -- an associated function on `CountryAlpha2`, **not** a
  free function in `domain_types::utils`. There is no `convert_country_alpha2_to_alpha3`.
- **Signature:** `const fn CountryAlpha2::from_alpha2_to_alpha3(code: CountryAlpha2) -> CountryAlpha3`
- **Description:** Total mapping over all ISO 3166-1 codes; infallible, so no `?`.
- **Example:**
```rust
use common_enums::{CountryAlpha2, CountryAlpha3};
let alpha3: CountryAlpha3 = CountryAlpha2::from_alpha2_to_alpha3(billing_country);
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
- **Location:** a **method** on the card types in
  `domain_types::payment_method_data` -- `Card<T>` (`payment_method_data.rs:253`) and the
  tokenised card type (`payment_method_data.rs:1586`). It is *not* a free function in
  `domain_types::utils`.
- **Signature:** `fn get_card_expiry_month_year_2_digit_with_delimiter(&self, delimiter: String) -> Result<Secret<String>, IntegrationError>`
- **Description:** Formats card expiry as `MM<delimiter>YY`; pass `"".to_string()` for `MMYY`.
  Takes the delimiter by value as a `String`, and only that -- month and year come from `self`.
- **Example** (real call sites: `connectors/mollie/transformers.rs:936`, `connectors/nmi/transformers.rs:627`):
```rust
let expiry = card_data.get_card_expiry_month_year_2_digit_with_delimiter("/".to_string())?;
```

### `is_mandate_supported`
- **Location:** `domain_types::utils`
- **Signature:** `fn is_mandate_supported<T>(selected_pmd: PaymentMethodData<T>, payment_method_type: Option<PaymentMethodType>, mandate_implemented_pmds: HashSet<PaymentMethodDataType>, connector: &'static str) -> Result<(), Error>`
- **Description:** Validates if a payment method supports mandate/recurring payments.

---

## Address / Phone Accessors -- read the semantics before choosing

These live on the flow request data (`domain_types::connector_types`) and on `PhoneDetails`
(`domain_types::payment_address`). Two of them look interchangeable and are not. Picking the wrong
one sends a malformed phone number to the gateway, which usually surfaces as a vague validation
rejection rather than a compile error.

| Accessor | Returns | Value |
|----------|---------|-------|
| `req.get_billing_phone_number()` | `Result<Secret<String>, Error>` | **Country code + number, concatenated** -- e.g. `"+14155550123"` |
| `req.get_billing_phone()?.get_number()?` | `Secret<String>` | **Bare national number only** -- e.g. `"4155550123"` |

`get_billing_phone_number()` is `get_number_with_country_code()` under the hood:

```rust
pub fn get_number_with_country_code(&self) -> Result<Secret<String>, Error> {
    let number = self.get_number()?;
    let country_code = self.get_country_code()?;
    Ok(Secret::new(format!("{}{}", country_code, number.peek())))
}
```

`PhoneDetails::country_code` is stored **with its leading `+`** -- which is why
`extract_country_code()` and `get_number_with_hash_country_code()` both call
`trim_start_matches('+')`. So `get_billing_phone_number()` yields an E.164-style string beginning
with `+`, and it errors if *either* the number or the country code is missing.

The full `PhoneDetails` set (`payment_address.rs:365-395`):

| Method | Returns | Value |
|--------|---------|-------|
| `get_number()` | `Result<Secret<String>, Error>` | bare national number |
| `get_country_code()` | `Result<String, Error>` | country code **with** `+`, e.g. `"+1"` |
| `extract_country_code()` | `Result<String, Error>` | country code **without** `+`, e.g. `"1"` |
| `get_number_with_country_code()` | `Result<Secret<String>, Error>` | `"+14155550123"` |
| `get_number_with_hash_country_code()` | `Result<Secret<String>, Error>` | `"1#4155550123"` (no `+`) |

Choose by what the vendor spec asks for:
- Spec says "E.164" or shows `+1...` → `get_billing_phone_number()`
- Spec has **separate** `country_code` and `phone` fields → `extract_country_code()` +
  `get_billing_phone()?.get_number()?` (most specs want the code without `+` in its own field)
- Spec shows a `#`-delimited pair → `get_number_with_hash_country_code()`

Every one of these returns `Err(MissingRequiredField)` rather than a default, so a missing phone
surfaces as a proper error -- do not wrap them in `unwrap_or_default()`.

---

## Failure and Fallback Helpers

### `is_payment_failure` / `is_refund_failure`
- **Location:** `domain_types::utils::is_payment_failure` (`utils.rs:231`),
  `connector_integration::utils::is_refund_failure` (`utils.rs:301`)
- **Signature:** `fn is_payment_failure(status: AttemptStatus) -> bool`,
  `fn is_refund_failure(status: RefundStatus) -> bool`
- **Description:** The framework's definition of "this is a failure". `is_payment_failure` is true
  for `AuthenticationFailed`, `AuthorizationFailed`, `CaptureFailed`, `VoidFailed`, `Expired` and
  `Failure`; `is_refund_failure` is true for `Failure` and `TransactionFailure`. Both are
  exhaustive matches, so they stay correct when a status variant is added.
- **Use them to gate in-band 2xx failures.** A gateway that returns HTTP 200 with a declined body
  must produce `Err(ErrorResponse{..})`, not `Ok(..)` carrying a failure status:

```rust
let status = AttemptStatus::from(item.response.status);
let response = if utils::is_payment_failure(status) {
    Err(ErrorResponse {
        code: item.response.error_code.clone()
            .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
        message: item.response.error_message.clone()
            .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
        reason: item.response.error_message.clone(),
        status_code: item.http_code,
        attempt_status: Some(FlowStatus::Payment(status)),
        connector_transaction_id: Some(item.response.id.clone()),
        ..Default::default()
    })
} else {
    Ok(PaymentsResponseData::TransactionResponse { /* all 11 fields */ })
};
```

Do not re-derive which statuses count as failure with a hand-written match or a string comparison.

### `NO_ERROR_CODE` / `NO_ERROR_MESSAGE`
- **Location:** `common_utils::consts` (`crates/common/common_utils/src/consts.rs:154`, `:156`)
- **Values:** `"No error code"`, `"No error message"`
- **Description:** The standard stand-ins when the connector's error body omits a code or message.
  `NO_ERROR_CODE` alone appears 247 times across real connectors.
- **Never** `.unwrap_or_default()` an error code or message: the resulting `""` is
  indistinguishable downstream from a connector that genuinely sent an empty string, and it
  silently breaks error-code reporting.

```rust
use common_utils::consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE};

// Wrong
code: response.error_code.unwrap_or_default(),

// Correct
code: response.error_code.unwrap_or_else(|| NO_ERROR_CODE.to_string()),
```

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
  (`crates/integrations/connector-integration/src/utils/xml_utils.rs:14`)
- **Signature:** `fn preprocess_xml_response_bytes(xml_data: Bytes, http_status: u16) -> Result<Bytes, ConnectorError>`
- **Description:** Converts XML response to JSON bytes for deserialization into Rust structs.
  Takes **two** arguments -- the HTTP status is used to build the error context -- and fails with
  `ConnectorError`, not `IntegrationError`.
- **Example** (real call site: `connectors/elavon.rs:260`):
```rust
let json_bytes = preprocess_xml_response_bytes(res.response, res.status_code)?;
let response: ConnectorResponse = serde_json::from_slice(&json_bytes)
    .change_context(errors::ConnectorError::ResponseDeserializationFailed { context: Default::default() })?;
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
| Parse XML response | `preprocess_xml_response_bytes(bytes, status_code)` | `connector_integration::utils::xml_utils` |
| Serialize to XML | `serialize_to_xml_string_with_root("root", &data)` | `connector_integration::utils` |
| Card network from BIN | `get_card_issuer(card_number)` | `domain_types::utils` |
| Card expiry formatting | `card.get_card_expiry_month_year_2_digit_with_delimiter("/".to_string())` | method on `Card<T>` (`domain_types::payment_method_data`) |
| US state to code | `convert_us_state_to_code("California")` | `domain_types::utils` |
| Current timestamp (s) | `now_unix_timestamp()` | `common_utils::date_time` |
| Current timestamp (ms) | `get_timestamp_in_milliseconds(&now())` | `domain_types::utils` |
| Format date | `format_date(date, DateFormat::YYYYMMDDHHmmss)` | `common_utils::date_time` |
| Extract HTTP header | `get_http_header("X-Header", headers)` | `domain_types::utils` |
| Extract card data | `get_card_details(pmd, "connector")` | `domain_types::utils` |
| Parse connector meta | `to_connector_meta_from_secret(meta)` | `connector_integration::utils` |
| Unimplemented PM error | `get_unimplemented_payment_method_error_message(conn)` | `domain_types::utils` |
| Phone, E.164 (`"+1415..."`) | `req.get_billing_phone_number()` | `domain_types::connector_types` |
| Phone, bare national number | `req.get_billing_phone()?.get_number()?` | `domain_types::payment_address` |
| Country code without `+` | `req.get_billing_phone()?.extract_country_code()?` | `domain_types::payment_address` |
| Is this status a failure? | `is_payment_failure(status)` / `is_refund_failure(status)` | `domain_types::utils` / `connector_integration::utils` |
| Missing connector error code | `NO_ERROR_CODE` / `NO_ERROR_MESSAGE` | `common_utils::consts` |
| Country alpha2 to alpha3 | `CountryAlpha2::from_alpha2_to_alpha3(c)` | `common_enums::transformers` |
