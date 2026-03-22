import re

file_path = 'backend/domain_types/src/types.rs'
with open(file_path, 'r') as f:
    content = f.read()

# 1. Fix double report! caused by previous nested replaces
content = content.replace('report!(report!(', 'report!(')

# 2. Fix the specific broken block at 768 (SDK Session Token)
# Find the implementation block and fix its closing
pattern_768 = r'impl\s+ForeignTryFrom\s*<\(\s*MerchantAuthenticationServiceCreateSdkSessionTokenRequest.*?\)\s*>\s*for\s*PaymentFlowData\s*\{.*?type Error = ConnectorRequestError;.*?fn foreign_try_from\(.*?\) -> Result<Self, error_stack::Report<ConnectorRequestError>> \{.*?Ok\(Self \{.*?\}\);'
replacement_768 = lambda m: m.group(0).replace('});', '})\n    }')
content = re.sub(pattern_768, replacement_768, content, flags=re.DOTALL)

# 3. Fix the Email map_err broken blocks
# Patterns like: .map_err(|_| {\n                    error_stack::Report::new(report!(...))
content = re.sub(r'\.map_err\(\|_\|\s*\{\s*error_stack::Report::new\(report!\(ConnectorRequestError::MissingRequiredField\s*\{\s*field_name:\s*"amount"\s*\}\)\)\s*\}',
                 r'.map_err(|_| { report!(ConnectorRequestError::InvalidDataFormat { field_name: "email" }) }',
                 content)

# 4. Fix the mismatched ) in generate_repeat_payment_response or similar match blocks
# Pattern: _ => Err(report!(ConnectorResponseError::ResponseHandlingFailed))\n        }\n    }\n}
# (The previous fix might have been slightly off)

# 5. Fix the mismatched braces in ForeignTryFrom<PaymentMethodServiceTokenizeRequest>
pattern_tokenize = r'impl.*?for\s+PaymentMethodTokenizationData<T>\s*\{.*?Ok\(Self\s*\{.*?\}\),'
replacement_tokenize = lambda m: m.group(0).replace('}),', '})\n    }')
content = re.sub(pattern_tokenize, replacement_tokenize, content, flags=re.DOTALL)

# 6. General cleanup of trailing )) that should be )
content = content.replace('}))\n        }?;', '})\n        }?;')
content = content.replace(' }))\n        }?;', ' })\n        }?;')

with open(file_path, 'w') as f:
    f.write(content)
