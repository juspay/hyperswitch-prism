import sys
import re

file_path = 'backend/domain_types/src/types.rs'
with open(file_path, 'r') as f:
    content = f.read()

# 1. Correct Err(report!(...)) missing one )
# Note: we match report!(...) followed by NO ) and then a comma or newline
def fix_err_report(m):
    # m.group(1) is the report!(...) block
    return f"Err({m.group(1)}))"

content = re.sub(r"Err\((report!\(ConnectorRequestError::.*?\s*\{.*?\}\))(?!\))", fix_err_report, content, flags=re.DOTALL)

# 2. Correct return Err(report!(...)) missing )
def fix_return_err_report(m):
    return f"return Err({m.group(1)}));"

content = re.sub(r"return Err\((report!\(ConnectorRequestError::.*?\s*\{.*?\}\))(?!\))\s*;", fix_return_err_report, content, flags=re.DOTALL)

with open(file_path, 'w') as f:
    f.write(content)
print("Successfully fixed Err(report!) delimiters.")
