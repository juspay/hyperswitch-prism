import sys
import re

file_path = 'backend/domain_types/src/types.rs'
with open(file_path, 'r') as f:
    content = f.read()

# 1. Correct the broken closures that are missing proper closing })
# The pattern usually is ... { report!(...) })
# and it should be ... { report!(...) } )

# Specifically fix the Email parsing blocks
content = re.sub(r'Some\(Email::try_from\(email_str\.clone\(\)\.expose\(\)\)\.map_err\(\|_\s*\|.*?report!\(ConnectorRequestError::MissingRequiredField\s*\{\s*field_name:\s*"amount"\s*\}\s*\)\s*\)',
                 r'Some(Email::try_from(email_str.clone().expose()).map_err(|_| { report!(ConnectorRequestError::MissingRequiredField { field_name: "amount" }) }))',
                 content, flags=re.DOTALL)

# 2. Fix the ok_or_else blocks
content = re.sub(r'value\.payment_method\.clone\(\)\.ok_or_else\(\|\|\s*\{\s*report!\(ConnectorRequestError::MissingRequiredField\s*\{\s*field_name:\s*"amount"\s*\}\s*\)\s*\)',
                 r'value.payment_method.clone().ok_or_else(|| { report!(ConnectorRequestError::MissingRequiredField { field_name: "amount" }) })',
                 content, flags=re.DOTALL)

# 3. Fix date parsing map_err
content = re.sub(r'\.map_err\(\|err\|\s*\{\s*tracing::error!.*?report!\(ConnectorRequestError::MissingRequiredField\s*\{\s*field_name:\s*"amount"\s*\}\s*\)\s*\)',
                 r'.map_err(|err| { tracing::error!("Failed to parse date string: {}", err); report!(ConnectorRequestError::MissingRequiredField { field_name: "amount" }) })',
                 content, flags=re.DOTALL)

# 4. Handle InternalServerError properly
content = re.sub(r'Err\(report!\(ConnectorRequestError::InternalServerError\(.*?\)\)\)',
                 r'Err(report!(ConnectorResponseError::ResponseHandlingFailed))',
                 content, flags=re.DOTALL)

# 5. Correct nested report!
content = content.replace('report!(report!(', 'report!(')

# 6. Global cleanup of common messed up endings
content = content.replace(' }))\n        }?;', ' }))?;')
content = content.replace(' }) )?', ' }))?')

with open(file_path, 'w') as f:
    f.write(content)
