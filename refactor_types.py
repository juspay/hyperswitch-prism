import re
import os

file_path = 'backend/domain_types/src/types.rs'

if not os.path.exists(file_path):
    print(f"File {file_path} not found")
    exit(1)

with open(file_path, 'r') as f:
    content = f.read()

# 1. Update imports
content = content.replace(
    'use crate::{', 
    'use crate::errors::{ConnectorRequestError, ConnectorResponseError};\nuse crate::{'
)

# 2. Update type aliases and Result types
content = content.replace('type Error = ApplicationErrorResponse;', 'type Error = ConnectorRequestError;')
content = content.replace('Result<Self, error_stack::Report<Self::Error>>', 'Result<Self, error_stack::Report<ConnectorRequestError>>')

# 3. Fix Step E (generate_payment_*_response) return types
def fix_resp_sig(match):
    return match.group(0).replace('ConnectorRequestError', 'ConnectorResponseError')

content = re.sub(
    r'pub fn generate_payment_.*?Result<.*?, error_stack::Report<ConnectorRequestError>>', 
    fix_resp_sig, 
    content, 
    flags=re.DOTALL
)

# 4. Surgical replacement of ApplicationErrorResponse::missing_required_field
content = re.sub(
    r'ApplicationErrorResponse::missing_required_field\(\s*"(.*?)"\s*\)',
    r'report!(ConnectorRequestError::MissingRequiredField { field_name: "\1" })', 
    content
)

# 5. Fix common BadRequest blocks
content = re.sub(
    r'ApplicationErrorResponse::BadRequest\(ApiError \{.*?sub_code: "MISSING_AMOUNT".*?\}\)',
    r'report!(ConnectorRequestError::MissingRequiredField { field_name: "amount" })',
    content, flags=re.DOTALL
)

content = re.sub(
    r'ApplicationErrorResponse::BadRequest\(ApiError \{.*?sub_code: "INVALID_EMAIL_FORMAT".*?\}\)',
    r'report!(ConnectorRequestError::InvalidDataFormat { field_name: "email" })',
    content, flags=re.DOTALL
)

# 6. Map internal server errors in response generation
content = content.replace('ApplicationErrorResponse::InternalServerError', 'ConnectorResponseError::ResponseHandlingFailed')

# 7. Final cleanup of any remaining ApplicationErrorResponse
content = content.replace('ApplicationErrorResponse', 'ConnectorRequestError')

# 8. Fix common parenthetical mismatches that occurred in previous attempts
content = content.replace('report!(report!(', 'report!(')

with open(file_path, 'w') as f:
    f.write(content)
