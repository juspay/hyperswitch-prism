import sys
import re
import os
import subprocess

file_path = 'backend/domain_types/src/types.rs'
subprocess.run(['git', 'checkout', file_path])

with open(file_path, 'r') as f:
    content = f.read()

# 1. Update imports
content = content.replace(
    'use crate::errors::ApiError;',
    'use crate::errors::{ApiError, ConnectorRequestError, ConnectorResponseError};'
)
content = content.replace('ApplicationErrorResponse, ', '')
content = content.replace(', ApplicationErrorResponse', '')

# 2. Update type aliases
content = content.replace('type Error = ApplicationErrorResponse;', 'type Error = ConnectorRequestError;')
content = content.replace('error_stack::Report<ApplicationErrorResponse>', 'error_stack::Report<ConnectorRequestError>')
content = content.replace('error_stack::Report<Self::Error>', 'error_stack::Report<ConnectorRequestError>')

# 3. Correct all ApplicationErrorResponse::missing_required_field usages
# Using regex with re.DOTALL to handle multiline calls
content = re.sub(
    r'ApplicationErrorResponse::missing_required_field\(\s*"(.*?)"\s*\)',
    r'report!(ConnectorRequestError::MissingRequiredField { field_name: "\1" })', 
    content,
    flags=re.DOTALL
)

# 4. Correct Step E (generate_payment_*_response) return types
def fix_resp_sig(match):
    return match.group(0).replace('ConnectorRequestError', 'ConnectorResponseError')

content = re.sub(
    r'pub fn generate_payment_.*?Result<.*?, error_stack::Report<ConnectorRequestError>>', 
    fix_resp_sig, 
    content, 
    flags=re.DOTALL
)

# 5. Correct all BadRequest and InternalServerError blocks
# Map known codes to specific variants, others to a generic one
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

content = re.sub(
    r'ApplicationErrorResponse::BadRequest\(ApiError \{.*?sub_code: "INVALID_URL".*?\}\)',
    r'report!(ConnectorRequestError::InvalidDataFormat { field_name: "url" })',
    content, flags=re.DOTALL
)

# Generic BadRequest in response functions
content = re.sub(
    r'ApplicationErrorResponse::BadRequest\(ApiError \{.*?sub_code: "INVALID_RESPONSE".*?\}\)',
    r'report!(ConnectorResponseError::ResponseHandlingFailed)',
    content, flags=re.DOTALL
)

# InternalServerError mapping
content = content.replace('ApplicationErrorResponse::InternalServerError', 'report!(ConnectorResponseError::ResponseHandlingFailed)')

# 6. Final cleanup of any remaining ApplicationErrorResponse
content = content.replace('ApplicationErrorResponse', 'ConnectorRequestError')

# 7. Remove any double report!
content = content.replace('report!(report!(', 'report!(')

with open(file_path, 'w') as f:
    f.write(content)
