import re
import os

file_path = 'backend/domain_types/src/types.rs'

with open(file_path, 'r') as f:
    content = f.read()

# 1. Surgical replacement of ApplicationErrorResponse::missing_required_field
# .ok_or(ApplicationErrorResponse::missing_required_field("..."))?
content = re.sub(r'ApplicationErrorResponse::missing_required_field\(\s*"(.*?)"\s*\)',
                 r'report!(ConnectorRequestError::MissingRequiredField { field_name: "\1" })', content)

# 2. Fix the BadRequest blocks with ApiError
# These are the ones that caused so much trouble. 
# We will match the entire block and replace it with a report! call.
content = re.sub(r'ApplicationErrorResponse::BadRequest\(ApiError \{.*?\}\)',
                 r'ConnectorRequestError::InvalidDataFormat { field_name: "unknown" }', content, flags=re.DOTALL)

# Let's be more specific for common ones if we can, but safety first.
# Actually, the most common variants are MissingRequiredField and InvalidDataFormat.

# 3. Fix the signatures
content = content.replace('error_stack::Report<ApplicationErrorResponse>', 'error_stack::Report<ConnectorRequestError>')
content = content.replace('type Error = ApplicationErrorResponse;', 'type Error = ConnectorRequestError;')
content = content.replace('error_stack::Report<Self::Error>', 'error_stack::Report<ConnectorRequestError>')

# 4. Handle generate_payment_*_response functions
def fix_resp_sig(m):
    return m.group(0).replace('ConnectorRequestError', 'ConnectorResponseError')

content = re.sub(r'pub fn generate_payment_.*?Result<.*?, error_stack::Report<ConnectorRequestError>>', 
                 fix_resp_sig, content, flags=re.DOTALL)

# 5. Fix internal server error
content = content.replace('ApplicationErrorResponse::InternalServerError', 'ConnectorResponseError::ResponseHandlingFailed')

# 6. Final cleanup
content = content.replace('ApplicationErrorResponse', 'ConnectorRequestError')

with open(file_path, 'w') as f:
    f.write(content)
