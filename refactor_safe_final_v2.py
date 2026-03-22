import sys
import re
import subprocess

file_path = 'backend/domain_types/src/types.rs'
subprocess.run(['git', 'checkout', file_path])

with open(file_path, 'r') as f:
    content = f.read()

# 1. Update imports
content = content.replace(
    'errors::{ApiError, ApplicationErrorResponse},', 
    'errors::{ApiError, ConnectorRequestError, ConnectorResponseError},'
)

# 2. Update type aliases
content = content.replace('type Error = ApplicationErrorResponse;', 'type Error = ConnectorRequestError;')
content = content.replace('Result<Self, error_stack::Report<Self::Error>>', 'Result<Self, error_stack::Report<ConnectorRequestError>>')
content = content.replace('error_stack::Report<ApplicationErrorResponse>', 'error_stack::Report<ConnectorRequestError>')

# 3. Correct response function signatures
content = re.sub(r"pub fn generate_payment_.*?Result<.*?, error_stack::Report<ConnectorRequestError>>", 
                 lambda m: m.group(0).replace("ConnectorRequestError", "ConnectorResponseError"), content, flags=re.DOTALL)

# 4. SURGICAL REPLACEMENT OF ERROR CALLS
# We replace the entire function call ApplicationErrorResponse::missing_required_field("...")
# with report!(ConnectorRequestError::MissingRequiredField { field_name: "..." })
def replace_mrf(match):
    field = match.group(1)
    return f"report!(ConnectorRequestError::MissingRequiredField {{ field_name: \"{field}\" }})"

content = re.sub(r"ApplicationErrorResponse::missing_required_field\(\s*\"(.*?)\"\s*\)", replace_mrf, content)

# 5. Correct common BadRequest blocks
# MISSING_AMOUNT
content = re.sub(r"ApplicationErrorResponse::BadRequest\(ApiError \{.*?sub_code: \"MISSING_AMOUNT\",.*?\}\)",
                 r"report!(ConnectorRequestError::MissingRequiredField { field_name: \"amount\" })", content, flags=re.DOTALL)
# INVALID_EMAIL_FORMAT
content = re.sub(r"ApplicationErrorResponse::BadRequest\(ApiError \{ sub_code: \"INVALID_EMAIL_FORMAT\",.*?\}\)",
                 r"report!(ConnectorRequestError::InvalidDataFormat { field_name: \"email\" })", content, flags=re.DOTALL)
# INVALID_URL
content = re.sub(r"ApplicationErrorResponse::BadRequest\(ApiError \{ sub_code: \"INVALID_URL\",.*?\}\)",
                 r"report!(ConnectorRequestError::InvalidDataFormat { field_name: \"url\" })", content, flags=re.DOTALL)

# 6. Correct response function generic errors (Step E)
# These should always be ConnectorResponseError::ResponseHandlingFailed
def replace_step_e_err(match):
    return "report!(ConnectorResponseError::ResponseHandlingFailed)"

# Use a very specific pattern for the match arms in generate_payment functions
content = re.sub(r"_ => Err\(report!\(ConnectorResponseError::ResponseHandlingFailed\)\(.*?\}\s*\)\)\),", 
                 r"_ => Err(report!(ConnectorResponseError::ResponseHandlingFailed))", content, flags=re.DOTALL)

content = content.replace("ApplicationErrorResponse::InternalServerError", "report!(ConnectorResponseError::ResponseHandlingFailed)")

# 7. Final catch-all for remaining ApplicationErrorResponse
content = content.replace("ApplicationErrorResponse", "ConnectorRequestError")

# 8. POST-PROCESS delimiter balance
content = content.replace("report!(report!(", "report!(")
content = content.replace("}) )?", "}))?")
content = content.replace("}) )", "}))")

# 9. Special fix for the orphaned blocks we saw earlier
# If the script accidentally created them again, we clean them
# (This is defensive)

with open(file_path, 'w') as f:
    f.write(content)
