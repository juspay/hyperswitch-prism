import sys
import re
import subprocess

file_path = 'backend/domain_types/src/types.rs'
subprocess.run(['git', 'checkout', file_path])

with open(file_path, 'r') as f:
    content = f.read()

# 1. Update imports cleanly
content = content.replace(
    'errors::{ApiError, ApplicationErrorResponse},', 
    'errors::{ApiError, ConnectorRequestError, ConnectorResponseError},'
)

# 2. Update type aliases and Result types
content = content.replace('type Error = ApplicationErrorResponse;', 'type Error = ConnectorRequestError;')
content = content.replace('Result<Self, error_stack::Report<Self::Error>>', 'Result<Self, error_stack::Report<ConnectorRequestError>>')
content = content.replace('error_stack::Report<ApplicationErrorResponse>', 'error_stack::Report<ConnectorRequestError>')

# 3. Correct Step E function signatures (return ConnectorResponseError)
def replace_response_sig(match):
    return match.group(0).replace("ConnectorRequestError", "ConnectorResponseError")

content = re.sub(r"pub fn generate_payment_.*?Result<.*?, error_stack::Report<ConnectorRequestError>>", 
                 replace_response_sig, content, flags=re.DOTALL)

# 4. SURGICAL REPLACEMENT OF ERROR CALLS
# We must use proper Rust struct syntax: report!(ConnectorRequestError::MissingRequiredField { field_name: "..." })

# A. missing_required_field
def fix_mrf(match):
    field = match.group(1)
    return f'report!(ConnectorRequestError::MissingRequiredField {{ field_name: "{field}" }})'

content = re.sub(r'ApplicationErrorResponse::missing_required_field\(\s*"(.*?)"\s*\)', fix_mrf, content)
content = re.sub(r'ConnectorRequestError::missing_required_field\(\s*"(.*?)"\s*\)', fix_mrf, content)

# B. BadRequest MISSING_AMOUNT -> MissingRequiredField
content = re.sub(r'ApplicationErrorResponse::BadRequest\(ApiError \{.*?sub_code: "MISSING_AMOUNT".*?\}\)',
                 r'report!(ConnectorRequestError::MissingRequiredField { field_name: "amount" })', content, flags=re.DOTALL)

# C. BadRequest INVALID_EMAIL_FORMAT -> InvalidDataFormat
content = re.sub(r'ApplicationErrorResponse::BadRequest\(ApiError \{.*?sub_code: "INVALID_EMAIL_FORMAT".*?\}\)',
                 r'report!(ConnectorRequestError::InvalidDataFormat { field_name: "email" })', content, flags=re.DOTALL)

# D. Generic BadRequest -> InvalidDataFormat("unknown") or specific if possible
# Note: We use a simplified regex to avoid complex nesting issues that broke previous attempts
def fix_generic_bad_request(match):
    return 'report!(ConnectorRequestError::InvalidDataFormat { field_name: "generic" })'

content = re.sub(r'ApplicationErrorResponse::BadRequest\(ApiError \{.*?\}\)', fix_generic_bad_request, content, flags=re.DOTALL)
content = re.sub(r'ConnectorRequestError::BadRequest\(ApiError \{.*?\}\)', fix_generic_bad_request, content, flags=re.DOTALL)

# E. Response generation errors (Step E)
# These should be mapped to ConnectorResponseError::ResponseHandlingFailed
def fix_step_e_err(match):
    return 'report!(ConnectorResponseError::ResponseHandlingFailed)'

# Target match arms in generate_payment_* functions
content = re.sub(r'_ => Err\(report!\(ConnectorResponseError::ResponseHandlingFailed\)\(.*?\}\)\)\),', 
                 r'_ => Err(report!(ConnectorResponseError::ResponseHandlingFailed))', content, flags=re.DOTALL)

# 5. Final cleanup of common regex side-effects
content = content.replace('report!(report!(', 'report!(')
content = content.replace('}) )?', '}))?')
content = content.replace('}) )', '}))')
content = content.replace('}) )', '}))') # double for good measure

# 6. Manual fixes for known troublesome blocks (Paze, SDK Session)
# We will do these after the script runs via surgical replace if needed, 
# but let's try to make the script smarter.

with open(file_path, 'w') as f:
    f.write(content)
