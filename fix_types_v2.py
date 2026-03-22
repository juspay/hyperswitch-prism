import sys

file_path = 'backend/domain_types/src/types.rs'
with open(file_path, 'r') as f:
    lines = f.readlines()

# Look for the duplicated blocks around line 1500
# The previous replace calls might have messed up the structure there.

new_lines = []
skip_mode = False
for i, line in enumerate(lines):
    # Detect the weird block that looks like:
    # Ok(Self {
    #     ...
    # })
    # Ok(Self {
    #     ...
    # })
    
    # Actually let's just fix the specific broken email blocks again precisely
    if 'Some(Email::try_from(email_str.clone().expose()).map_err(|_| { report!(ConnectorRequestError::InvalidDataFormat { field_name: "email" }) }?;' in line:
        line = line.replace('Some(Email::try_from(email_str.clone().expose()).map_err(|_| { report!(ConnectorRequestError::InvalidDataFormat { field_name: "email" }) }?;',
                            'Some(Email::try_from(email_str.clone().expose()).map_err(|_| { report!(ConnectorRequestError::InvalidDataFormat { field_name: "email" }) })?)')
    
    new_lines.append(line)

with open(file_path, 'w') as f:
    f.writelines(new_lines)
