import sys
import re

file_path = 'backend/domain_types/src/types.rs'
with open(file_path, 'r') as f:
    content = f.read()

# The file has many broken blocks like:
# .map_err(|_| { report!(...) })
# where the closing }) was messed up by previous bulk replaces.

# 1. Fix the Email parsing map_err blocks (there are several)
content = re.sub(r'Some\(Email::try_from\(email_str\.clone\(\)\.expose\(\)\)\.map_err\(\|_\s*\|.*?report!\(ConnectorRequestError::MissingRequiredField\s*\{\s*field_name:\s*"amount"\s*\}\s*\)\s*\)',
                 r'Some(Email::try_from(email_str.clone().expose()).map_err(|_| { report!(ConnectorRequestError::InvalidDataFormat { field_name: "email" }) })',
                 content, flags=re.DOTALL)

# 2. Fix ok_or_else blocks
content = re.sub(r'value\.payment_method\.clone\(\)\.ok_or_else\(\|\|\s*\{\s*report!\(ConnectorRequestError::MissingRequiredField\s*\{\s*field_name:\s*"amount"\s*\}\s*\)\s*\)',
                 r'value.payment_method.clone().ok_or_else(|| { report!(ConnectorRequestError::MissingRequiredField { field_name: "amount" }) })',
                 content, flags=re.DOTALL)

# 3. Fix date parsing map_err
content = re.sub(r'\.map_err\(\|err\|\s*\{\s*tracing::error!.*?report!\(ConnectorRequestError::MissingRequiredField\s*\{\s*field_name:\s*"amount"\s*\}\s*\)\s*\)',
                 r'.map_err(|err| { tracing::error!("Failed to parse date string: {}", err); report!(ConnectorRequestError::InvalidDataFormat { field_name: "date" }) })',
                 content, flags=re.DOTALL)

# 4. Correct common corrupted endings of ForeignTryFrom
content = content.replace('}))\n        }?;', '})\n        }?;')

# 5. Fix duplicated struct field assignments
content = content.replace('status_code: status_code as u32,\n                    raw_connector_response,\n                    response_headers: router_data_v2\n                        .resource_common_data\n                        .get_connector_response_headers_as_map(),\n                    raw_connector_request,\n                    state,\n                    mandate_reference: mandate_reference_grpc,\n                    incremental_authorization_allowed,\n                    connector_feature_data: convert_connector_metadata_to_secret_string(\n                        connector_metadata,\n                    ),\n                })\n            }\n            _ => Err(report!(ConnectorResponseError::ResponseHandlingFailed))\n        },',
                          'status_code: status_code as u32,\n                    raw_connector_response,\n                    response_headers,\n                    raw_connector_request,\n                    state,\n                    mandate_reference: mandate_reference_grpc,\n                    incremental_authorization_allowed,\n                    connector_feature_data: convert_connector_metadata_to_secret_string(\n                        connector_metadata,\n                    ),\n                })\n            }\n            _ => Err(report!(ConnectorResponseError::ResponseHandlingFailed))\n        },')

with open(file_path, 'w') as f:
    f.write(content)
