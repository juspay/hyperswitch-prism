import sys
import re

file_path = 'backend/domain_types/src/types.rs'
with open(file_path, 'r') as f:
    content = f.read()

# 1. Correct report!(...))) -> report!(...)
content = re.sub(r'report!\(ConnectorRequestError::(.*?)\s*\{(.*?)\}\)\)\)',
                 r'report!(ConnectorRequestError::\1 {\2})', content, flags=re.DOTALL)

# 2. Correct report!(...)) -> report!(...) ONLY IF it is followed by } (closing closure)
content = re.sub(r'report!\(ConnectorRequestError::(.*?)\s*\{(.*?)\}\)\)\s*\}',
                 r'report!(ConnectorRequestError::\1 {\2})\n        }', content, flags=re.DOTALL)

# 3. Handle the specific line 564 pattern: report!(...)));
content = content.replace('})) );', '}));')
content = content.replace('})) );', '}));')

# 4. Final check for triple or double reports
content = content.replace('report!(report!(', 'report!(')

with open(file_path, 'w') as f:
    f.write(content)
