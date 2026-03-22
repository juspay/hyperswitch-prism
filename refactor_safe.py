import sys
import re

file_path = 'backend/domain_types/src/types.rs'
with open(file_path, 'r') as f:
    content = f.read()

# 1. missing_required_field -> MissingRequiredField
def fix_mrf(m):
    return f'report!(ConnectorRequestError::MissingRequiredField {{ field_name: "{m.group(1)}" }})'
content = re.sub(r'ApplicationErrorResponse::missing_required_field\(\s*"(.*?)"\s*\)', fix_mrf, content)

# 2. BadRequest -> InvalidDataFormat (generic for now)
def fix_bad_request(m):
    return 'report!(ConnectorRequestError::InvalidDataFormat { field_name: "unknown" })'
content = re.sub(r'ApplicationErrorResponse::BadRequest\(ApiError \{.*?\}\)', fix_bad_request, content, flags=re.DOTALL)

# 3. Type Aliases
content = content.replace('type Error = ApplicationErrorResponse;', 'type Error = ConnectorRequestError;')

# 4. Result types
content = content.replace('Result<Self, error_stack::Report<Self::Error>>', 'Result<Self, error_stack::Report<ConnectorRequestError>>')
content = content.replace('error_stack::Report<ApplicationErrorResponse>', 'error_stack::Report<ConnectorRequestError>')

# 5. Correct Step E function signatures (return ConnectorResponseError)
def replace_response_sig(match):
    return match.group(0).replace("ConnectorRequestError", "ConnectorResponseError")

content = re.sub(r"pub fn generate_payment_.*?Result<.*?, error_stack::Report<ConnectorRequestError>>", 
                 replace_response_sig, content, flags=re.DOTALL)

# 6. Final cleanup of any potential report!(report!(...))
content = content.replace('report!(report!(', 'report!(')

with open(file_path, 'w') as f:
    f.write(content)
print("Successfully performed safe bulk refactor.")
