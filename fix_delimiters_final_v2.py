import sys
import re

file_path = 'backend/domain_types/src/types.rs'
with open(file_path, 'r') as f:
    content = f.read()

# 1. Correct Err(report!(...)) structures where a parenthesis was missing
# Pattern: Err(report!(ConnectorRequestError::(Variant) { (Fields) })
# Should be: Err(report!(ConnectorRequestError::(Variant) { (Fields) }))
def fix_err_report(m):
    return f"Err({m.group(1)}))"

content = re.sub(r"Err\((report!\(ConnectorRequestError::.*?\s*\{.*?\}\))(?!\))", fix_err_report, content, flags=re.DOTALL)

# 2. Correct return Err(report!(...)) structures
def fix_return_err_report(m):
    return f"return Err({m.group(1)}));"

content = re.sub(r"return Err\((report!\(ConnectorRequestError::.*?\s*\{.*?\}\))(?!\))", fix_return_err_report, content, flags=re.DOTALL)

# 3. Handle the match arms ending with a comma: _ => Err(report!(...)),
# If the previous step didn't catch it because of the comma
def fix_comma_err_report(m):
    return f"Err({m.group(1)})),"

content = re.sub(r"Err\((report!\(ConnectorRequestError::.*?\s*\{.*?\}\))(?!\)),", fix_comma_err_report, content, flags=re.DOTALL)

with open(file_path, 'w') as f:
    f.write(content)
