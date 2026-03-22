import sys
import re

file_path = 'backend/domain_types/src/types.rs'
with open(file_path, 'r') as f:
    content = f.read()

# 1. Ensure traits are in scope
# Check if they are already there, if not add them
if 'use crate::utils::{ForeignFrom, ForeignTryFrom};' not in content:
    content = content.replace('use crate::{', 'use crate::utils::{ForeignFrom, ForeignTryFrom};\nuse crate::{')

# 2. Fix the most persistent errors: ForeignTryFrom vs try_from
# The error E0599 says "no variant or associated item named foreign_try_from found for enum ucs_common_enums::Currency"
# This usually happens if the trait is not in scope or if it should be try_from.
# Actually, looking at the error message, it says "items from traits can only be used if the trait is implemented and in scope"
# So let's make sure they are imported correctly.

# 3. Fix structural issues causing E0308 (mismatched arms)
# The pre-authenticate response was incorrectly returning AuthenticateResponse
content = content.replace('PaymentMethodAuthenticationServiceAuthenticateResponse {', 'PaymentMethodAuthenticationServicePreAuthenticateResponse {')

# 4. Fix specific broken method calls that should be direct trait calls or try_from
# based on the help messages in cargo check
content = content.replace('common_enums::Currency::foreign_try_from', 'common_enums::Currency::try_from')
content = content.replace('PaymentAddress::foreign_try_from', 'PaymentAddress::try_from')
content = content.replace('PaymentMethod::foreign_try_from', 'PaymentMethod::try_from')
content = content.replace('BrowserInformation::foreign_try_from', 'BrowserInformation::try_from')
content = content.replace('AccessTokenResponseData::foreign_try_from', 'AccessTokenResponseData::try_from')
content = content.replace('Option::foreign_try_from', 'Option::try_from')
content = content.replace('grpc_api_types::payments::Currency::foreign_try_from', 'grpc_api_types::payments::Currency::try_from')
content = content.replace('grpc_api_types::payments::PaymentStatus::foreign_from', 'grpc_api_types::payments::PaymentStatus::try_from')
content = content.replace('grpc_api_types::payments::HttpMethod::foreign_from', 'grpc_api_types::payments::HttpMethod::try_from')

# 5. Fix the AuthenticationData mapping
content = content.replace('authentication_data.map(ForeignFrom::foreign_from)', 
                          'authentication_data.map(|ad| grpc_api_types::payments::AuthenticationData::try_from(ad).unwrap_or_default())')

with open(file_path, 'w') as f:
    f.write(content)
