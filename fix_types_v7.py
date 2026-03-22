import sys
import re

file_path = 'backend/domain_types/src/types.rs'
with open(file_path, 'r') as f:
    content = f.read()

# The file is clearly corrupted with duplicated and partially replaced blocks.
# Let's try to restore the core structure.

# Fix the broken Ok(Self) blocks that were duplicated or improperly closed
# 1. SDK Session Token block around 768
# (Previous fix attempts might have left debris)

# 2. Authenticate flow blocks
# These are the most broken ones according to cargo check

# Let's use a very aggressive approach to remove the known duplicated segments.
# The orphaned code usually contains things like "field_name: \"router_return_url\"," or "redirect_response,"
# outside of any valid structure.

# Fix common broken method calls one more time
content = content.replace('ConnectorRequestError::MissingRequiredField { field_name: "amount" })),', 
                          'report!(ConnectorRequestError::MissingRequiredField { field_name: "amount" }))')

# Correct the double report!
content = content.replace('report!(report!(', 'report!(')

# Fix the specific broken mandate reference block that keeps coming back
content = content.replace('                       ))\n                        }\n                    }),',
                          '                        })),\n                        }\n                    }),')

# Final attempt at structural correction for the most problematic areas
content = content.replace('})\n    }\n}\n                        field_name: "router_return_url",', '})\n    }\n}\n')

with open(file_path, 'w') as f:
    f.write(content)
