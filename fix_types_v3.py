import sys

file_path = 'backend/domain_types/src/types.rs'
with open(file_path, 'r') as f:
    content = f.read()

# Locate the end of the post-authenticate block and the start of pre-authenticate response
target_pattern = '''            enrolled_for_3ds: false,
            redirect_response,
            capture_method: None,
        })
    }
}
                        field_name: "router_return_url",'''

# The orphaned block starts after the first "    }\n}" which closes the post-authenticate impl
# And ends before "pub fn generate_payment_pre_authenticate_response"

import re
# Look for the duplicated part
orphaned_regex = re.compile(r'\}\s*\}\s*field_name: "router_return_url".*?redirect_response,.*?\s*\}\s*\}', re.DOTALL)
content = orphaned_regex.sub('}\n}', content)

with open(file_path, 'w') as f:
    f.write(content)
