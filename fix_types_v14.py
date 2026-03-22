import sys
import re

file_path = 'backend/domain_types/src/types.rs'
with open(file_path, 'r') as f:
    content = f.read()

# The file is a mess of mismatched delimiters. Let's fix the specific ones identified by cargo check.

# 1. Fix broken struct closures missing proper closing })
# Pattern: ok_or_else(|| { report!(...) })
content = re.sub(r'ok_or_else\(\|\|\s*\{\s*report!\(ConnectorRequestError::MissingRequiredField\s*\{\s*field_name:\s*"amount"\s*\}\s*\)\s*\}',
                 r'ok_or_else(|| { report!(ConnectorRequestError::MissingRequiredField { field_name: "amount" }) })',
                 content, flags=re.DOTALL)

# 2. Fix the broken Email parsing map_err blocks (missing proper closing })
content = re.sub(r'Some\(Email::try_from\(email_str\.clone\(\)\.expose\(\)\)\.map_err\(\|_\s*\|.*?report!\(ConnectorRequestError::InvalidDataFormat\s*\{\s*field_name:\s*"email"\s*\}\s*\)\s*\}',
                 r'Some(Email::try_from(email_str.clone().expose()).map_err(|_| { report!(ConnectorRequestError::InvalidDataFormat { field_name: "email" }) })',
                 content, flags=re.DOTALL)

# 3. Fix the broken redirection payload blocks (missing proper closing })
content = re.sub(r'payload:\s*Some\(Secret::new\(serde_json::Value::Object\(.*?\)\s*\)\s*\)\s*\}',
                 lambda m: m.group(0).replace(')', ')))'), content, flags=re.DOTALL)

# 4. Correct common corrupted endings of ForeignTryFrom
content = content.replace('}))\n        }?;', '}))?;')
content = content.replace('}))\n    }', '})));\n    }')

# 5. Correct nested report!
content = content.replace('report!(report!(', 'report!(')

# 6. Global fix for specific broken endings found in cargo check
content = content.replace('report!(ConnectorRequestError::MissingRequiredField { field_name: "amount" }) }',
                          'report!(ConnectorRequestError::MissingRequiredField { field_name: "amount" }) })')

with open(file_path, 'w') as f:
    f.write(content)
