import sys

file_path = 'backend/domain_types/src/types.rs'
with open(file_path, 'r') as f:
    content = f.read()

old_block = """impl ForeignTryFrom<grpc_api_types::payments::PazeDecryptedData>
    for router_data::PazeDecryptedData
{
    type Error = ApplicationErrorResponse;

    fn foreign_try_from(
        value: grpc_api_types::payments::PazeDecryptedData,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let token = value
            .token
            .ok_or(ApplicationErrorResponse::missing_required_field(
                "payment_method.paze.decrypted_data.token",
            ))?;
        let billing_address =
            value
                .billing_address
                .ok_or(ApplicationErrorResponse::missing_required_field(
                    "payment_method.paze.decrypted_data.billing_address",
                ))?;
        let consumer = value
            .consumer
            .ok_or(ApplicationErrorResponse::missing_required_field(
                "payment_method.paze.decrypted_data.consumer",
            ))?;

        let consumer_country_code = convert_optional_country_alpha2(consumer.country_code())?;

        let email_address = Email::try_from(
            consumer
                .email_address
                .ok_or(ApplicationErrorResponse::missing_required_field(
                    "payment_method.paze.decrypted_data.consumer.email_address",
                ))?
                .expose(),
        )
        .change_context(ApplicationErrorResponse::BadRequest(ApiError {
            sub_code: "INVALID_PAZE_CONSUMER_EMAIL".to_owned(),
            error_identifier: 400,
            error_message: "Invalid Paze consumer email in payment_method".to_owned(),
            error_object: None,
        }))?;

        let mobile_number = consumer
            .mobile_number
            .map(
                |mobile_number| -> Result<_, error_stack::Report<ApplicationErrorResponse>> {
                    Ok(router_data::PazePhoneNumber {
                        country_code: mobile_number.country_code.ok_or(
                            ApplicationErrorResponse::missing_required_field(
                                "payment_method.paze.decrypted_data.consumer.mobile_number.country_code",
                            ),
                        )?,
                        phone_number: mobile_number.phone_number.ok_or(
                            ApplicationErrorResponse::missing_required_field(
                                "payment_method.paze.decrypted_data.consumer.mobile_number.phone_number",
                            ),
                        )?,
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
}"""

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
                        country_code: mobile_number.country_code.ok_or(report!(
                            ConnectorRequestError::MissingRequiredField {
                                field_name:
                                    "payment_method.paze.decrypted_data.consumer.mobile_number.country_code",
                            }
                        ))?,
                        phone_number: mobile_number.phone_number.ok_or(report!(
                            ConnectorRequestError::MissingRequiredField {
                                field_name:
                                    "payment_method.paze.decrypted_data.consumer.mobile_number.phone_number",
                            }
                        ))?,
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
}"""

if old_block in content:
    content = content.replace(old_block, new_block)
    with open(file_path, 'w') as f:
        f.write(content)
    print("Successfully replaced.")
else:
    print("Could not find the exact block.")
