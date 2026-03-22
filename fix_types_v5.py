import sys
import re

file_path = 'backend/domain_types/src/types.rs'
with open(file_path, 'r') as f:
    content = f.read()

# 1. Fix the ok_or syntax error
# It was ok_or(ConnectorRequestError::MissingRequiredField { ... }))
# Should be ok_or(report!(ConnectorRequestError::MissingRequiredField { ... }))
content = re.sub(r'\.ok_or\(ConnectorRequestError::MissingRequiredField \{ field_name:\s*(.*?)\s*\}\)',
                 r'.ok_or(report!(ConnectorRequestError::MissingRequiredField { field_name: \1 }))',
                 content)

# 2. Identify the orphaned block after the PazeDecryptedData impl
# The impl ends with:
#         Ok(Self {
#             token,
#             ...
#             email_address,
#         })
#     }
# }
# [STRAY CODE]
# impl ForeignTryFrom<grpc_api_types::payments::PazeBillingAddress>

# The stray code starts with "                    error_stack::Report::new(ConnectorRequestError::MissingRequiredField"
# and ends before the next impl

orphaned_pattern = re.compile(r'\}\s*\}\s*error_stack::Report::new\(ConnectorRequestError::MissingRequiredField.*?transpose\(\)\?;\s*(\n\s*impl)', re.DOTALL)
content = orphaned_pattern.sub('}\n}\n\\1', content)

with open(file_path, 'w') as f:
    f.write(content)
