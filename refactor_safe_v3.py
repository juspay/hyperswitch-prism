import sys
import re
import subprocess

file_path = 'backend/domain_types/src/types.rs'
subprocess.run(['git', 'checkout', file_path])

with open(file_path, 'r') as f:
    content = f.read()

# 1. ADD new types but KEEP old one for matching
content = content.replace(
    'errors::{ApiError, ApplicationErrorResponse},', 
    'errors::{ApiError, ApplicationErrorResponse, ConnectorRequestError, ConnectorResponseError},'
)

# 2. Update type aliases
content = content.replace('type Error = ApplicationErrorResponse;', 'type Error = ConnectorRequestError;')
content = content.replace('Result<Self, error_stack::Report<Self::Error>>', 'Result<Self, error_stack::Report<ConnectorRequestError>>')
content = content.replace('error_stack::Report<ApplicationErrorResponse>', 'error_stack::Report<ConnectorRequestError>')

# 3. Correct Step E signatures (generate_payment_*_response)
def replace_response_sig(match):
    return match.group(0).replace("ConnectorRequestError", "ConnectorResponseError")

content = re.sub(r"pub fn generate_payment_.*?Result<.*?, error_stack::Report<ConnectorRequestError>>", 
                 replace_response_sig, content, flags=re.DOTALL)

# 4. Correct all ApplicationErrorResponse calls manually without regex backreferences to be safe
# missing_required_field -> MissingRequiredField { field_name: "..." }
def replace_missing_field(match):
    field_name = match.group(1)
    return f'report!(ConnectorRequestError::MissingRequiredField {{ field_name: "{field_name}" }})'

content = re.sub(
    r'ApplicationErrorResponse::missing_required_field\(\s*"(.*?)"\s*\)',
    replace_missing_field, 
    content,
    flags=re.DOTALL
)

# 5. Correct all BadRequest blocks
# Map known codes to specific variants, others to a generic one
def replace_missing_amount(match):
    return 'report!(ConnectorRequestError::MissingRequiredField { field_name: "amount" })'

content = re.sub(
    r'ApplicationErrorResponse::BadRequest\(ApiError \{.*?sub_code: "MISSING_AMOUNT".*?\}\)',
    replace_missing_amount,
    content, flags=re.DOTALL
)

def replace_invalid_email(match):
    return 'report!(ConnectorRequestError::InvalidDataFormat { field_name: "email" })'

content = re.sub(
    r'ApplicationErrorResponse::BadRequest\(ApiError \{.*?sub_code: "INVALID_EMAIL_FORMAT".*?\}\)',
    replace_invalid_email,
    content, flags=re.DOTALL
)

def replace_invalid_url(match):
    return 'report!(ConnectorRequestError::InvalidDataFormat { field_name: "url" })'

content = re.sub(
    r'ApplicationErrorResponse::BadRequest\(ApiError \{.*?sub_code: "INVALID_URL".*?\}\)',
    replace_invalid_url,
    content, flags=re.DOTALL
)

# Generic BadRequest/InternalServerError in response functions -> ResponseHandlingFailed
def replace_resp_err(match):
    return 'report!(ConnectorResponseError::ResponseHandlingFailed)'

content = re.sub(
    r'ApplicationErrorResponse::(?:InternalServerError|BadRequest)\(ApiError \{.*?\}\)',
    replace_resp_err,
    content, flags=re.DOTALL
)

# 6. Final pass for any remaining ApplicationErrorResponse (mostly in Step A validations)
# Map these to ConnectorRequestError for now
content = content.replace('ApplicationErrorResponse', 'ConnectorRequestError')

# 7. Remove the now-unused import
content = content.replace('ApplicationErrorResponse, ', '')

# 8. Cleanup debris
content = content.replace('report!(report!(', 'report!(')
content = content.replace('}) )?', '}))?')
content = content.replace('}) )', '}))')

with open(file_path, 'w') as f:
    f.write(content)
