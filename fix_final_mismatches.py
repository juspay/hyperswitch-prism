import sys
import re

file_path = 'backend/domain_types/src/types.rs'
with open(file_path, 'r') as f:
    content = f.read()

# 1. Correct the common broken patterns where extra parentheses were added by regex
# Match: report!(ConnectorRequestError::MissingRequiredField { ... }))),
# Should be: report!(ConnectorRequestError::MissingRequiredField { ... })
content = re.sub(r'report!\(ConnectorRequestError::MissingRequiredField \{ field_name: "(.*?)" \}\)\)\)',
                 r'report!(ConnectorRequestError::MissingRequiredField { field_name: "\1" })', content)

# 2. Fix nested report!
content = content.replace('report!(report!(', 'report!(')

# 3. Fix the specific Email parsing block corruption
# Change: .map_err(|_| { error_stack::Report::new(report!(...)) })))
# To: .map_err(|_| { report!(...) })
content = re.sub(r'\.map_err\(\|_\|\s*\{\s*error_stack::Report::new\(report!\(ConnectorRequestError::(?:MissingRequiredField|InvalidDataFormat)\s*\{\s*field_name:\s*"(.*?)"\s*\}\s*\)\s*\)\s*\}\s*\)\s*\)\s*\)',
                 r'.map_err(|_| { report!(ConnectorRequestError::InvalidDataFormat { field_name: "\1" }) })', content, flags=re.DOTALL)

# 4. Cleanup trailing debris from previous turns
content = content.replace('})) )?', '))?')
content = content.replace('})) )', '))')
content = content.replace('})) ) );', '}));')
content = content.replace('})) );', '}));')

# 5. Correct broken match arms at the end of functions
content = re.sub(r'_ => Err\(report!\(ConnectorResponseError::ResponseHandlingFailed,.*?\}\s*\)\)\),', 
                 r'_ => Err(report!(ConnectorResponseError::ResponseHandlingFailed))', 
                 content, flags=re.DOTALL)

with open(file_path, 'w') as f:
    f.write(content)
