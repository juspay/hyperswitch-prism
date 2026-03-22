import sys

file_path = 'backend/domain_types/src/types.rs'
with open(file_path, 'r') as f:
    content = f.read()

# Markers for the block
start_marker = 'impl ForeignTryFrom<grpc_api_types::payments::PazeDecryptedData>'
end_marker = 'impl ForeignTryFrom<grpc_api_types::payments::PazeBillingAddress>'

start_idx = content.find(start_marker)
end_idx = content.find(end_marker)

if start_idx != -1 and end_idx != -1:
    new_block = """impl ForeignTryFrom<grpc_api_types::payments::PazeDecryptedData>
    for router_data::PazeDecryptedData
{
    type Error = ConnectorRequestError;

    fn foreign_try_from(
        value: grpc_api_types::payments::PazeDecryptedData,
    ) -> Result<Self, error_stack::Report<ConnectorRequestError>> {
        let token = value
            .token
            .ok_or(report!(ConnectorRequestError::MissingRequiredField {
                field_name: "payment_method.paze.decrypted_data.token",
            }))?;
        let billing_address =
            value
                .billing_address
                .ok_or(report!(ConnectorRequestError::MissingRequiredField {
                    field_name: "payment_method.paze.decrypted_data.billing_address",
                }))?;
        let consumer = value
            .consumer
            .ok_or(report!(ConnectorRequestError::MissingRequiredField {
                field_name: "payment_method.paze.decrypted_data.consumer",
            }))?;

        let consumer_country_code = convert_optional_country_alpha2(consumer.country_code())?;

        let email_address = Email::try_from(
            consumer
                .email_address
                .ok_or(report!(ConnectorRequestError::MissingRequiredField {
                    field_name: "payment_method.paze.decrypted_data.consumer.email_address",
                }))?
                .expose(),
        )
        .change_context(ConnectorRequestError::InvalidDataFormat {
            field_name: "payment_method.paze.decrypted_data.consumer.email_address",
        })?;

        let mobile_number = consumer
            .mobile_number
            .map(
                |mobile_number| -> Result<_, error_stack::Report<ConnectorRequestError>> {
                    Ok(router_data::PazePhoneNumber {
                        country_code: mobile_number
                            .country_code
                            .ok_or(report!(ConnectorRequestError::MissingRequiredField {
                                field_name:
                                    "payment_method.paze.decrypted_data.consumer.mobile_number.country_code",
                            }))?,
                        phone_number: mobile_number
                            .phone_number
                            .ok_or(report!(ConnectorRequestError::MissingRequiredField {
                                field_name:
                                    "payment_method.paze.decrypted_data.consumer.mobile_number.phone_number",
                            }))?,
                    })
                },
            )
            .transpose()?;

        Ok(Self {
            token,
            billing_address: router_data::PazeBillingAddress::foreign_try_from(billing_address)?,
            consumer_name: consumer.name,
            consumer_phone_number: mobile_number,
            consumer_country_code,
            email_address,
        })
    }
}

"""
    new_content = content[:start_idx] + new_block + content[end_idx:]
    with open(file_path, 'w') as f:
        f.write(new_content)
    print("Successfully refactored PazeDecryptedData block.")
else:
    print(f"Could not find block markers: start={start_idx}, end={end_idx}")
    sys.exit(1)
