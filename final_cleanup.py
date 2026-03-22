import sys
import re

file_path = 'backend/domain_types/src/types.rs'
with open(file_path, 'r') as f:
    content = f.read()

# 1. Correct Err(report!(...))) .into()) -> Err(report!(...))
content = re.sub(r'Err\(report!\(ConnectorRequestError::(.*?)\s*\{(.*?)\}\)\)\)\s*\.into\(\)\)', 
                 r'Err(report!(ConnectorRequestError::\1 { \2 }))', content, flags=re.DOTALL)

# 2. Correct return Err(report!(...))) .into()); -> return Err(report!(...));
content = re.sub(r'return Err\(report!\(ConnectorRequestError::(.*?)\s*\{(.*?)\}\)\)\)\s*\.into\(\)\);', 
                 r'return Err(report!(ConnectorRequestError::\1 { \2 }));', content, flags=re.DOTALL)

# 3. Handle return Err(report!(...))) followed by )? (happens in or_else blocks)
content = re.sub(r'Err\(report!\(ConnectorRequestError::(.*?)\s*\{(.*?)\}\)\)\)\s*\.into\(\)\)\?', 
                 r'Err(report!(ConnectorRequestError::\1 { \2 }))?', content, flags=re.DOTALL)

with open(file_path, 'w') as f:
    f.write(content)
print("Successfully performed final cleanup of delimiters.")
