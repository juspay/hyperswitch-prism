import sys
import re

file_path = 'backend/domain_types/src/types.rs'
with open(file_path, 'r') as f:
    content = f.read()

# 1. Correct BadRequest(ApiError { ... }) patterns
# MISSING_AMOUNT
content = re.sub(r'ApplicationErrorResponse::BadRequest\(ApiError \{.*?sub_code: "MISSING_AMOUNT".*?\}\)',
                 r'report!(ConnectorRequestError::MissingRequiredField { field_name: "amount" })',
                 content, flags=re.DOTALL)

# INVALID_EMAIL_FORMAT
content = re.sub(r'ApplicationErrorResponse::BadRequest\(ApiError \{.*?sub_code: "INVALID_EMAIL_FORMAT".*?\}\)',
                 r'report!(ConnectorRequestError::InvalidDataFormat { field_name: "email" })',
                 content, flags=re.DOTALL)

# INVALID_URL
content = re.sub(r'ApplicationErrorResponse::BadRequest\(ApiError \{.*?sub_code: "INVALID_URL".*?\}\)',
                 r'report!(ConnectorRequestError::InvalidDataFormat { field_name: "url" })',
                 content, flags=re.DOTALL)

# 2. Correct internal server error mapping in response generation
content = content.replace('ApplicationErrorResponse::InternalServerError', 'report!(ConnectorResponseError::ResponseHandlingFailed)')

# 3. Final catch-all for remaining ApplicationErrorResponse in Step A blocks
# These should be mapped to a generic Request error for now
content = content.replace('ApplicationErrorResponse', 'ConnectorRequestError')

# 4. Correct any double report! caused by previous nested replaces
content = content.replace('report!(report!(', 'report!(')

# 5. Correct Step E (generate_payment_*_response) return types to use ConnectorResponseError
def fix_resp_sig(match):
    return match.group(0).replace('ConnectorRequestError', 'ConnectorResponseError')

content = re.sub(
    r'pub fn generate_payment_.*?Result<.*?, error_stack::Report<ConnectorRequestError>>', 
    fix_resp_sig, 
    content, 
    flags=re.DOTALL
)

with open(file_path, 'w') as f:
    f.write(content)
