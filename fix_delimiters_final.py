import sys
import re

file_path = 'backend/domain_types/src/types.rs'
with open(file_path, 'r') as f:
    content = f.read()

# 1. Correct report!(...))) -> report!(...)
# We match: report!(ConnectorRequestError::(Variant) { (Field) }))),
# We replace with: report!(ConnectorRequestError::(Variant) { (Field) })
content = re.sub(r'report!\(ConnectorRequestError::(.*?)\s*\{(.*?)\}\)\)\)',
                 r'report!(ConnectorRequestError::\1 {\2})', content, flags=re.DOTALL)

# 2. Correct return Err(report!(...))) -> return Err(report!(...))
content = re.sub(r'return Err\(report!\(ConnectorRequestError::(.*?)\s*\{(.*?)\}\)\)\)',
                 r'return Err(report!(ConnectorRequestError::\1 {\2}))', content, flags=re.DOTALL)

# 3. Fix double report!
content = content.replace('report!(report!(', 'report!(')

# 4. Correct generic delimiter leftovers from the script
content = content.replace('})) )?', '}))?')
content = content.replace('})) )', '))')

with open(file_path, 'w') as f:
    f.write(content)
