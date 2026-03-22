import sys
import re

file_path = 'backend/domain_types/src/types.rs'
with open(file_path, 'r') as f:
    content = f.read()

# 1. Clean up the massive duplicated/orphaned block in PreAuthenticate
orphaned_pattern = re.compile(r'None => \{.*?return Err\(ConnectorResponseError::ResponseHandlingFailed\).*?Ok\(response\)\;\s*\}', re.DOTALL)
content = orphaned_pattern.sub('None => { PaymentAddress::new(None, None, None, None) }', content)

# 2. Fix the broken ok_or_else/match closures
content = content.replace('ConnectorRequestError::MissingRequiredField { field_name: "amount" })),', 
                          'report!(ConnectorRequestError::MissingRequiredField { field_name: "amount" }))')

# 3. Clean up any remaining double report!
content = content.replace('report!(report!(', 'report!(')

# 4. Remove the stray "};" at the end if it's there
content = content.replace('    };\n    Ok(response)\n}', '}\n}')

with open(file_path, 'w') as f:
    f.write(content)
