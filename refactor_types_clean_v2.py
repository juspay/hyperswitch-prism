import re
import os

file_path = 'backend/domain_types/src/types.rs'

# Start from fresh state
import subprocess
subprocess.run(['git', 'checkout', file_path])

with open(file_path, 'r') as f:
    content = f.read()

# 1. Update imports
content = content.replace(
    'use crate::{', 
    'use crate::errors::{ConnectorRequestError, ConnectorResponseError};\nuse crate:{'
)

# 2. Update type aliases
content = content.replace('type Error = ApplicationErrorResponse;', 'type Error = ConnectorRequestError;')
content = content.replace('Result<Self, error_stack::Report<Self::Error>>', 'Result<Self, error_stack::Report<ConnectorRequestError>>')

# 3. Fix Step E signatures
def fix_resp_sig(match):
    return match.group(0).replace('ConnectorRequestError', 'ConnectorResponseError')

content = re.sub(
    r'pub fn generate_payment_.*?Result<.*?, error_stack::Report<ConnectorRequestError>>', 
    fix_resp_sig, 
    content, 
    flags=re.DOTALL
)

# 4. Correct all ApplicationErrorResponse usages
# Map missing_required_field to the struct variant
content = re.sub(
    r'ApplicationErrorResponse::missing_required_field\(\s*"(.*?)"\s*\)',
    r'report!(ConnectorRequestError::MissingRequiredField { field_name: "\1" })', 
    content
)

# Map BadRequest blocks
# Since some blocks are complex, we replace the entire call with a report!
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

# 5. Fix InternalServerError and catch-all BadRequest
# In response generation, these should be unit variants
content = re.sub(
    r'ApplicationErrorResponse::(?:InternalServerError|BadRequest)\(ApiError \{.*?\}\)',
    r'report!(ConnectorResponseError::ResponseHandlingFailed)', 
    content, flags=re.DOTALL
)

# 6. Final cleanup of any remaining ApplicationErrorResponse
content = content.replace('ApplicationErrorResponse', 'ConnectorRequestError')

# 7. Correct the double report! and other debris
content = content.replace('report!(report!(', 'report!(')

# 8. Fix common type mismatches by adding .change_context() where needed
# (This is speculative based on the previous error messages)
content = content.replace('Option::foreign_try_from(resource_id)?', 
                          'Option::foreign_try_from(resource_id).change_context(ConnectorResponseError::ResponseHandlingFailed)?')

with open(file_path, 'w') as f:
    f.write(content)
