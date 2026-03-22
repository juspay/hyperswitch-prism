import sys
import re

file_path = 'backend/domain_types/src/types.rs'
# Start from fresh state
import subprocess
subprocess.run(['git', 'checkout', file_path])

with open(file_path, 'r') as f:
    content = f.read()

# 1. Update imports
content = content.replace('use crate::{', 'use crate::errors::{ConnectorRequestError, ConnectorResponseError};\nuse crate::{')

# 2. Update type aliases
content = content.replace('type Error = ApplicationErrorResponse;', 'type Error = ConnectorRequestError;')
content = content.replace('Result<Self, error_stack::Report<Self::Error>>', 'Result<Self, error_stack::Report<ConnectorRequestError>>')

# 3. Fix Step E (generate_payment_*_response) -> ConnectorResponseError in signatures
content = re.sub(r'pub fn generate_payment_.*?Result<.*?, error_stack::Report<ApplicationErrorResponse>>', 
                 lambda m: m.group(0).replace('ApplicationErrorResponse', 'ConnectorResponseError'), content, flags=re.DOTALL)

# 4. Correct all ApplicationErrorResponse::missing_required_field usages
# We will use a regex that handles both one-line and multi-line calls correctly
content = re.sub(r'ApplicationErrorResponse::missing_required_field\(\s*"(.*?)"\s*\)',
                 r'report!(ConnectorRequestError::MissingRequiredField { field_name: "\1" })',
                 content, flags=re.DOTALL)

# 5. Fix common BadRequest blocks
#MISSING_AMOUNT
content = re.sub(r'ApplicationErrorResponse::BadRequest\(ApiError \{.*?sub_code: "MISSING_AMOUNT".*?\}\)',
                 r'report!(ConnectorRequestError::MissingRequiredField { field_name: "amount" })',
                 content, flags=re.DOTALL)
#INVALID_EMAIL_FORMAT
content = re.sub(r'ApplicationErrorResponse::BadRequest\(ApiError \{.*?sub_code: "INVALID_EMAIL_FORMAT".*?\}\)',
                 r'report!(ConnectorRequestError::InvalidDataFormat { field_name: "email" })',
                 content, flags=re.DOTALL)
#INVALID_URL
content = re.sub(r'ApplicationErrorResponse::BadRequest\(ApiError \{.*?sub_code: "INVALID_URL".*?\}\)',
                 r'report!(ConnectorRequestError::InvalidDataFormat { field_name: "url" })',
                 content, flags=re.DOTALL)

# 6. Final cleanup of any remaining ApplicationErrorResponse
content = content.replace('ApplicationErrorResponse', 'ConnectorRequestError')

with open(file_path, 'w') as f:
    f.write(content)
