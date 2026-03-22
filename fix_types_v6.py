import sys
import re

file_path = 'backend/domain_types/src/types.rs'
with open(file_path, 'r') as f:
    content = f.read()

# Identify the broken block around line 2336 (Authenticate flow)
# It has a duplicated struct closure and then starts another Ok(Self)

broken_auth_pattern = re.compile(r'router_return_url: return_url\s*\.map\(.*?field_name: "url" \)\s*,\s*\}\s*?;.*?let return_url = value\.return_url;.*?redirect_response,\s*\}\s*\}', re.DOTALL)
content = broken_auth_pattern.sub('router_return_url: return_url.clone().map(|url_str| { url::Url::parse(&url_str).change_context(ConnectorRequestError::InvalidDataFormat { field_name: "router_return_url" }) }).transpose()?, enrolled_for_3ds: false, redirect_response, capture_method: None, })', content)

# General cleanup of common debris from failed scripts
content = content.replace('report!(report!(', 'report!(')
content = content.replace(' })),', '))')
content = content.replace(' }));', '))')

with open(file_path, 'w') as f:
    f.write(content)
