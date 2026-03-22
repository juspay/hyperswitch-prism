import sys
import re

file_path = 'backend/domain_types/src/types.rs'
with open(file_path, 'r') as f:
    content = f.read()

# Helper to replace type Error and function signature in a block
def refactor_block(content, start_marker, end_marker):
    start_idx = content.find(start_marker)
    if start_idx == -1:
        return content, False
    
    end_idx = content.find(end_marker, start_idx)
    if end_idx == -1:
        return content, False
    
    end_idx += len(end_marker)
    block = content[start_idx:end_idx]
    
    # Replace Error type
    block = block.replace('type Error = ApplicationErrorResponse;', 'type Error = ConnectorRequestError;')
    
    # Replace Result types
    block = block.replace('Result<Self, error_stack::Report<Self::Error>>', 'Result<Self, error_stack::Report<ConnectorRequestError>>')
    block = block.replace('Result<String, error_stack::Report<ApplicationErrorResponse>>', 'Result<String, error_stack::Report<ConnectorRequestError>>')
    block = block.replace('Result<_, error_stack::Report<ApplicationErrorResponse>>', 'Result<_, error_stack::Report<ConnectorRequestError>>')

    # Replace missing_required_field
    def fix_mrf(m):
        return f'report!(ConnectorRequestError::MissingRequiredField {{ field_name: "{m.group(1)}" }})'
    block = re.sub(r'ApplicationErrorResponse::missing_required_field\(\s*"(.*?)"\s*\)', fix_mrf, block)

    # Replace BadRequest with generic InvalidDataFormat for now
    def fix_bad_request(m):
        return 'report!(ConnectorRequestError::InvalidDataFormat { field_name: "unknown" })'
    block = re.sub(r'ApplicationErrorResponse::BadRequest\(ApiError \{.*?\}\)', fix_bad_request, block, flags=re.DOTALL)

    new_content = content[:start_idx] + block + content[end_idx:]
    return new_content, True

# List of blocks to refactor
blocks = [
    ('impl ForeignTryFrom<grpc_api_types::payments::CaptureMethod>', 'Ok(Self::Automatic),\n        }\n    }\n}'),
    ('impl ForeignTryFrom<grpc_api_types::payments::ThreeDsCompletionIndicator>', 'Ok(Self::NotAvailable),\n        }\n    }\n}'),
    ('impl ForeignTryFrom<grpc_api_types::payments::CardNetwork>', '.into())\n            }\n        }\n    }\n}'),
    ('impl ForeignTryFrom<grpc_api_types::payments::Tokenization>', '.into())\n            }\n        }\n    }\n}'),
    ('fn validate_last_four_digits(', 'Ok(trimmed.to_string())\n}'),
    ('impl ForeignTryFrom<grpc_api_types::payments::samsung_wallet::PaymentCredential>', 'token_data: payment_method_data::SamsungPayTokenData {\n                three_ds_type: token_data.r#type.clone(),\n                version: token_data.version.clone(),\n                data: raw_token,\n            },\n        })\n    }\n}'),
]

for start, end in blocks:
    content, success = refactor_block(content, start, end)
    if not success:
        print(f"Failed to find block: {start}")

with open(file_path, 'w') as f:
    f.write(content)
print("Successfully refactored multiple blocks.")
