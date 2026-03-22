import sys
import re

file_path = 'backend/domain_types/src/types.rs'
with open(file_path, 'r') as f:
    content = f.read()

# Identify the broken block after PazeDecryptedData
# It starts after the first successful closing of the ForeignTryFrom impl
# and ends before the next impl.

# Look for the successful end:
#         Ok(Self {
#             token,
#             ...
#             email_address,
#         })
#     }
# }

# The pattern to remove starts right after that and ends at the next impl
orphaned_regex = re.compile(r'email_address,\s*\}\)\s*\}\s*\}\s*let sync_type =.*?setup_future_usage,\s*\}\s*\}\s*(\n\s*impl)', re.DOTALL)
content = orphaned_regex.sub('email_address,\n        })\n    }\n}\n\n\\1', content)

with open(file_path, 'w') as f:
    f.write(content)
