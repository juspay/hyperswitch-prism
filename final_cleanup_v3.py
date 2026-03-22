import sys
import re

file_path = 'backend/domain_types/src/types.rs'
with open(file_path, 'r') as f:
    content = f.read()

# 1. Fix Err(report!(...)) missing one )
# Match: Err(report!(...)) followed by newline and then }
content = re.sub(r'Err\((report!\(ConnectorRequestError::.*?\s*\{.*?\}\))(?!\))\s*\n\s*\}', 
                 r'Err(\1)\n            }', content, flags=re.DOTALL)

# 2. Fix standalone return Err(report!(...)) missing )
content = re.sub(r'return Err\((report!\(ConnectorRequestError::.*?\s*\{.*?\}\))(?!\))\s*;',
                 r'return Err(\1);', content, flags=re.DOTALL)

# 3. Handle the match arms _ => Err(report!(...)),
content = re.sub(r'_ => Err\((report!\(ConnectorRequestError::.*?\s*\{.*?\}\))(?!\)),',
                 r'_ => Err(\1),', content, flags=re.DOTALL)

# 4. Correct the ApplePay/GPay None arms
content = re.sub(r'None => Err\((report!\(ConnectorRequestError::InvalidDataFormat\s*\{\s*field_name:\s*"unknown"\s*\}\))(?!\))\s*\n\s*\}',
                 r'None => Err(\1)\n        }', content, flags=re.DOTALL)

# 5. Fix specific nested structures that broke
content = content.replace('})))));', '}));')
content = content.replace('}))));', '}));')
content = content.replace('})))));', '}));')
content = content.replace('})) );', '}));')
content = content.replace('}))),', '})),')

with open(file_path, 'w') as f:
    f.write(content)
