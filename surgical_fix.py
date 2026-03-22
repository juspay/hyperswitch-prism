import sys
import re

file_path = 'backend/domain_types/src/types.rs'

with open(file_path, 'r') as f:
    content = f.read()

# 1. Global cleanup of common broken parenthetical patterns introduced by previous attempts
content = content.replace('report!(report!(', 'report!(')
# Fix the specific broken ok_or/map_err patterns found in cargo check
content = re.sub(r'None => Err\(report!\(ConnectorRequestError::MissingRequiredField \{ field_name: "amount" \} \),\s*\}\?;',
                 r'None => Err(report!(ConnectorRequestError::MissingRequiredField { field_name: "amount" })),\n        }?;', content)

# 2. Fix the Email map_err corruption
# Pattern: .map_err(|_| { report!(...) })
email_regex = re.compile(r'\.map_err\(\|_\|\s*\{.*?report!\(ConnectorRequestError::(?:MissingRequiredField|InvalidDataFormat).*?\}\s*\}\s*\)', re.DOTALL)
content = email_regex.sub(r'.map_err(|_| { report!(ConnectorRequestError::InvalidDataFormat { field_name: "email" }) })', content)

# 3. Fix the generate_response tail corruption
# Functions like generate_payment_void_response were ending with duplicated/orphaned match arms
content = re.sub(r'_ => Err\(report!\(ConnectorResponseError::ResponseHandlingFailed,.*?\}\s*\)\)\),', 
                 r'_ => Err(report!(ConnectorResponseError::ResponseHandlingFailed))', 
                 content, flags=re.DOTALL)

# 4. Correct the Result type aliases in signatures
content = content.replace('error_stack::Report<Self::Error>', 'error_stack::Report<ConnectorRequestError>')

# 5. Fix structural closure of match blocks that were left open
content = content.replace('})) )?', '))?')
content = content.replace('})) )', '))')

with open(file_path, 'w') as f:
    f.write(content)
