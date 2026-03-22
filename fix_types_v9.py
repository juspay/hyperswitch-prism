import sys
import re

file_path = 'backend/domain_types/src/types.rs'
with open(file_path, 'r') as f:
    content = f.read()

# 1. Surgical replacement of ApplicationErrorResponse::missing_required_field
# These usually look like .ok_or(ApplicationErrorResponse::missing_required_field("..."))?
content = re.sub(r'ApplicationErrorResponse::missing_required_field\(\s*"(.*?)"\s*\)', 
                 r'report!(ConnectorRequestError::MissingRequiredField { field_name: "\1" })', 
                 content)

# 2. Fix the BadRequest blocks
# Map them to ConnectorRequestError or ConnectorResponseError based on context.
# Since we already did many, let's target the ones reported in errors.
content = re.sub(r'ApplicationErrorResponse::BadRequest\(ApiError \{.*?sub_code: "MISSING_AMOUNT".*?\}\)',
                 r'report!(ConnectorRequestError::MissingRequiredField { field_name: "amount" })',
                 content, flags=re.DOTALL)

content = re.sub(r'ApplicationErrorResponse::BadRequest\(ApiError \{.*?sub_code: "INVALID_EMAIL_FORMAT".*?\}\)',
                 r'report!(ConnectorRequestError::InvalidDataFormat { field_name: "email" })',
                 content, flags=re.DOTALL)

content = re.sub(r'ApplicationErrorResponse::BadRequest\(ApiError \{.*?sub_code: "INVALID_URL".*?\}\)',
                 r'report!(ConnectorRequestError::InvalidDataFormat { field_name: "url" })',
                 content, flags=re.DOTALL)

# 3. Handle InternalServerError
# In generate_payment_ functions, these should be ConnectorResponseError::ResponseHandlingFailed
content = re.sub(r'ApplicationErrorResponse::InternalServerError\(ApiError \{.*?sub_code: "(.*?)".*?\}\)',
                 r'report!(ConnectorResponseError::ResponseHandlingFailed)',
                 content, flags=re.DOTALL)

# 4. Handle remaining ApplicationErrorResponse instances
# Map them to a generic but appropriate type
content = content.replace('ApplicationErrorResponse', 'ConnectorRequestError')

# 5. Fix common broken closures from cargo check
# .map_err(|_| { error_stack::Report::new(...) })
content = re.sub(r'\.map_err\(\|_\|\s*\{\s*error_stack::Report::new\((.*?)\)\s*\}',
                 r'.map_err(|_| { \1 }',
                 content)

# 6. Correct double report!
content = content.replace('report!(report!(', 'report!(')

# 7. Specific fix for generate_payment_ void/sync etc signature errors
# (They were changed to ConnectorRequestError but should be ConnectorResponseError)
# Match functions that take RouterDataV2 and return Result
func_pattern = re.compile(r'pub fn generate_payment_.*?Result<.*?, error_stack::Report<ConnectorRequestError>>', re.DOTALL)
content = func_pattern.sub(lambda m: m.group(0).replace('ConnectorRequestError', 'ConnectorResponseError'), content)

with open(file_path, 'w') as f:
    f.write(content)
