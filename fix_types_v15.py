import sys
import re

file_path = 'backend/domain_types/src/types.rs'
with open(file_path, 'r') as f:
    content = f.read()

# The file is a mess of mismatched delimiters. 
# Let is fix the common patterns that caused the issues one last time, very carefully.

# 1. Fix the Email parsing map_err blocks - ensuring they end with })?)
# Pattern: .map_err(|_| { report!(...) }) )?
# or .map_err(|_| { report!(...) })))?
content = re.sub(r'map_err\(\|_\s*\|.*?report!\(ConnectorRequestError::MissingRequiredField\s*\{\s*field_name:\s*"amount"\s*\}\s*\)\s*\}\s*\)\s*\)\s*\)?',
                 r'map_err(|_| { report!(ConnectorRequestError::MissingRequiredField { field_name: "amount" }) })?)',
                 content, flags=re.DOTALL)

# 2. Fix ok_or_else blocks - ensuring they end with })?
content = re.sub(r'ok_or_else\(\|\|\s*\{\s*report!\(ConnectorRequestError::MissingRequiredField\s*\{\s*field_name:\s*"amount"\s*\}\s*\)\s*\}\s*\)\s*\)\s*\)?',
                 r'ok_or_else(|| { report!(ConnectorRequestError::MissingRequiredField { field_name: "amount" }) })?)',
                 content, flags=re.DOTALL)

# 3. Fix date parsing map_err - ensuring it ends with })?
content = re.sub(r'map_err\(\|err\|\s*\{\s*tracing::error!.*?report!\(ConnectorRequestError::MissingRequiredField\s*\{\s*field_name:\s*"amount"\s*\}\s*\)\s*\}\s*\)\s*\)\s*\)?',
                 r'map_err(|err| { tracing::error!("Failed to parse date string: {}", err); report!(ConnectorRequestError::MissingRequiredField { field_name: "amount" }) })?)',
                 content, flags=re.DOTALL)

# 4. Handle Redirection payload - specifically the triple )))
content = re.sub(r'Some\(Secret::new\(serde_json::Value::Object\(.*?\)\s*\)\s*\)\s*\}\s*\)\s*\)?',
                 lambda m: m.group(0).split('}')[0] + '})) })', content, flags=re.DOTALL)

# 5. Correct the double report!
content = content.replace('report!(report!(', 'report!(')

# 6. Final cleanup of any trailing )))
content = content.replace(' })))?;', ' })?;')
content = content.replace(' })))', ' })')

with open(file_path, 'w') as f:
    f.write(content)
