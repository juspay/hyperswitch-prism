import sys
import re

file_path = 'backend/domain_types/src/types.rs'
with open(file_path, 'r') as f:
    content = f.read()

# 1. Surgical update of imports
# Add the new error types to crate imports
content = content.replace('use crate::{', 'use crate::errors::{ConnectorRequestError, ConnectorResponseError};\nuse crate::{')

# 2. Fix Step A (ForeignTryFrom) -> ConnectorRequestError
# The bulk perl already did most, but let's fix the ones that are still ApplicationErrorResponse
content = content.replace('ApplicationErrorResponse', 'ConnectorRequestError')

# 3. Fix Step E (generate_payment_*_response) -> ConnectorResponseError
# These functions should return ConnectorResponseError
# Identify functions taking RouterDataV2 and returning Result
func_pattern = re.compile(r'pub fn generate_payment_.*?Result<.*?, error_stack::Report<ConnectorRequestError>>', re.DOTALL)
content = func_pattern.sub(lambda m: m.group(0).replace('ConnectorRequestError', 'ConnectorResponseError'), content)

# 4. Fix specific broken match arms in response functions
# (Re-applying the fix from previous turn because I git checkout-ed)
content = re.sub(r'_ => Err\(report!\(ConnectorResponseError::ResponseHandlingFailed,.*?\}\s*\)\)\),', 
                 r'_ => Err(report!(ConnectorResponseError::ResponseHandlingFailed))', 
                 content, flags=re.DOTALL)

# 5. Fix common broken closures
content = content.replace('ConnectorRequestError::missing_required_field', 'report!(ConnectorRequestError::MissingRequiredField') # Wait missing_required_field is a helper

# Let's fix the helper call itself:
# Original: ApplicationErrorResponse::missing_required_field("name")
# My perl changed it to: ConnectorRequestError::MissingRequiredField { field_name: "name" }
# But it might be missing the report!() wrapping if used in .ok_or()

# 6. Correct double report!
content = content.replace('report!(report!(', 'report!(')

with open(file_path, 'w') as f:
    f.write(content)
