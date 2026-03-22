import sys
import re

file_path = 'backend/domain_types/src/types.rs'
with open(file_path, 'r') as f:
    content = f.read()

# Pattern: report!(...)))
# We need to replace it with report!(...)
# Match: report!(ConnectorRequestError::(Variant) { (Fields) }))
# Replace with: report!(ConnectorRequestError::(Variant) { (Fields) })

def fix_double_paren(m):
    # m.group(1) is the variant name, m.group(2) is the fields
    return f"report!(ConnectorRequestError::{m.group(1)} {{ {m.group(2)} }})"

# Match report!(ConnectorRequestError::Variant { fields })) followed by )
content = re.sub(r'report!\(ConnectorRequestError::(.*?)\s*\{(.*?)\}\)\)\)', 
                 fix_double_paren, content, flags=re.DOTALL)

# Also handle return Err(report!(...)))
def fix_return_err(m):
    return f"return Err(report!(ConnectorRequestError::{m.group(1)} {{ {m.group(2)} }}))"

content = re.sub(r'return Err\(report!\(ConnectorRequestError::(.*?)\s*\{(.*?)\}\)\)\)', 
                 fix_return_err, content, flags=re.DOTALL)

# Handle cases where it is report!(...));)
content = content.replace('})) );', '}));')

with open(file_path, 'w') as f:
    f.write(content)
print("Successfully fixed double parentheses.")
