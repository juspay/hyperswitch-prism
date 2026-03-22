import sys
import re

file_path = 'backend/domain_types/src/types.rs'
with open(file_path, 'r') as f:
    content = f.read()

# 1. Correct the common broken patterns where extra parentheses were added
# Match: report!(ConnectorRequestError::MissingRequiredField { ... }))),
# Should be: report!(ConnectorRequestError::MissingRequiredField { ... })),
content = re.sub(r'report!\(ConnectorRequestError::MissingRequiredField \{ field_name: "(.*?)" \}\)\)\)',
                 r'report!(ConnectorRequestError::MissingRequiredField { field_name: "\1" })', content)

# 2. Fix nested report! caused by previous script
content = content.replace('report!(report!(', 'report!(')

# 3. Fix the specific Email parsing block corruption
# Match: .map_err(|_| { error_stack::Report::new(report!(...)) }),
# Should be: .map_err(|_| { report!(...) })
content = re.sub(r'\.map_err\(\|_\|\s*\{\s*error_stack::Report::new\(report!\(ConnectorRequestError::MissingRequiredField \{ field_name: "(.*?)" \}\)\)\s*\}',
                 r'.map_err(|_| { report!(ConnectorRequestError::MissingRequiredField { field_name: "\1" }) }', content)

# 4. Clean up any trailing )) )? or )) );
content = content.replace('})) )?', '}))?')
content = content.replace('})) );', '}));')
content = content.replace('})) )', '}))')

# 5. Fix the date parsing block
content = re.sub(r'report!\(ConnectorRequestError::MissingRequiredField \{ field_name: "amount" \}\)\)\)',
                 r'report!(ConnectorRequestError::MissingRequiredField { field_name: "amount" })', content)

with open(file_path, 'w') as f:
    f.write(content)
