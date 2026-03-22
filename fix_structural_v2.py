import sys
import re

file_path = 'backend/domain_types/src/types.rs'
with open(file_path, 'r') as f:
    content = f.read()

# Pattern: Some(StructName { fields }))
# Note: we need to be careful not to match report!(Variant { fields })) which IS correct.
# Struct names start with uppercase. Variants also start with uppercase.
# But Some( is very specific.

def fix_some_struct(m):
    # m.group(1) is the inner part: StructName { ... }
    return f"Some({m.group(1)})"

# Match: Some( followed by any characters until a balanced { } block, followed by ))
# This is hard with regex. I will use a simpler approach:
# Just replace any "}))," with "})," and "}))" with "})"
# but ONLY if it is not preceded by report! (which is handled separately)

# Let s try a more targeted one for the ErrorInfo blocks
content = content.replace('})),', '}),')
content = content.replace('}))\n', '})\n')

# But wait, report!(Variant { ... }) needs ))
# So we restore those.
content = content.replace('report!(ConnectorRequestError::MissingRequiredField { field_name: "unknown" }),', 
                          'report!(ConnectorRequestError::MissingRequiredField { field_name: "unknown" })),')
# and so on... this is dangerous.

# Better: use a specific regex for the ones we saw in sed.
# connector_details: Some(grpc_api_types::payments::ConnectorErrorDetails { ... })),
content = re.sub(r'(connector_details: Some\(grpc_api_types::payments::ConnectorErrorDetails \{.*?)\)\),', 
                 r'\1}),', content, flags=re.DOTALL)

content = re.sub(r'(network_details: Some\(grpc_api_types::payments::NetworkErrorDetails \{.*?)\)\),', 
                 r'\1}),', content, flags=re.DOTALL)

with open(file_path, 'w') as f:
    f.write(content)
