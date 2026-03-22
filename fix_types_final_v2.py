import sys
import re

file_path = 'backend/domain_types/src/types.rs'
with open(file_path, 'r') as f:
    content = f.read()

# The corrupted part starts after the first successful closing of PazeDecryptedData impl
# and ends before the next impl.

# Identifying the start point
start_marker = 'email_address,\n        })\n    }\n}'
# Identifying the next impl start
end_marker = 'impl ForeignTryFrom<grpc_api_types::payments::PazeBillingAddress>'

start_idx = content.find(start_marker)
if start_idx != -1:
    end_idx = content.find(end_marker, start_idx + len(start_marker))
    if end_idx != -1:
        # Remove everything between the markers
        corrupted_block = content[start_idx + len(start_marker) : end_idx]
        content = content.replace(corrupted_block, '\n\n')

with open(file_path, 'w') as f:
    f.write(content)
