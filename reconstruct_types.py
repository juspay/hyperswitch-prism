import sys

file_path = 'backend/domain_types/src/types.rs'
with open(file_path, 'r') as f:
    content = f.read()

# Using exact markers found from previous read_file
start_marker = 'impl ForeignTryFrom<grpc_api_types::payments::PazeDecryptedData>'
end_marker = '// For decoding connector feature data'

start_idx = content.find(start_marker)
end_idx = content.find(end_marker)

if start_idx == -1 or end_idx == -1:
    print(f"Markers not found: start={start_idx}, end={end_idx}")
    sys.exit(1)

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

        let grpc_payment_card_network =
            grpc_api_types::payments::CardNetwork::try_from(value.payment_card_network)
                .change_context(ConnectorRequestError::InvalidDataFormat {
                    field_name: "payment_method.paze.payment_card_network",
                })?;

        let payment_card_network = CardNetwork::foreign_try_from(grpc_payment_card_network)
            .change_context(ConnectorRequestError::InvalidDataFormat {
                field_name: "payment_method.paze.payment_card_network",
            })?;

        let dynamic_data = value
            .dynamic_data
            .into_iter()
            .map(|dynamic_data| router_data::PazeDynamicData {
                dynamic_data_value: dynamic_data.dynamic_data_value,
                dynamic_data_type: dynamic_data.dynamic_data_type,
                dynamic_data_expiration: dynamic_data.dynamic_data_expiration,
            })
            .collect();

        let billing_country_code = convert_optional_country_alpha2(billing_address.country_code())?;

        Ok(Self {
            client_id: value
                .client_id
                .ok_or(report!(ConnectorRequestError::MissingRequiredField {
                    field_name: "payment_method.paze.decrypted_data.client_id",
                }))?,
            profile_id: value.profile_id,
            token: router_data::PazeToken {
                payment_token: token.payment_token.ok_or(
                    report!(ConnectorRequestError::MissingRequiredField {
                        field_name: "payment_method.paze.decrypted_data.token.payment_token",
                    }),
                )?,
                token_expiration_month: token.token_expiration_month.ok_or(
                    report!(ConnectorRequestError::MissingRequiredField {
                        field_name: "payment_method.paze.decrypted_data.token.token_expiration_month",
                    }),
                )?,
                token_expiration_year: token.token_expiration_year.ok_or(
                    report!(ConnectorRequestError::MissingRequiredField {
                        field_name: "payment_method.paze.decrypted_data.token.token_expiration_year",
                    }),
                )?,
                payment_account_reference: token.payment_account_reference.ok_or(
                    report!(ConnectorRequestError::MissingRequiredField {
                        field_name: "payment_method.paze.decrypted_data.token.payment_account_reference",
                    }),
                )?,
            },
            payment_card_network,
            dynamic_data,
            billing_address: router_data::PazeAddress {
                name: billing_address.name,
                line1: billing_address.line1,
                line2: billing_address.line2,
                line3: billing_address.line3,
                city: billing_address.city,
                state: billing_address.state,
                zip: billing_address.zip,
                country_code: billing_country_code,
            },
            consumer: router_data::PazeConsumer {
                first_name: consumer.first_name,
                last_name: consumer.last_name,
                full_name: consumer.full_name.ok_or(
                    report!(ConnectorRequestError::MissingRequiredField {
                        field_name: "payment_method.paze.decrypted_data.consumer.full_name",
                    }),
                )?,
                email_address,
                mobile_number,
                country_code: consumer_country_code,
                language_code: consumer.language_code,
            },
            eci: value.eci,
        })
    }
}

impl ForeignTryFrom<(Secret<String>, &'static str)> for SecretSerdeValue {
    type Error = ConnectorRequestError;

    fn foreign_try_from(
        (secret, field_name): (Secret<String>, &'static str),
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let raw = secret.expose();
        serde_json::from_str(&raw).map(Self::new).change_context(
            ConnectorRequestError::InvalidDataFormat {
                field_name: "unknown",
            },
        )
    }
}

"""

new_content = content[:start_idx] + new_block + content[end_idx:]

with open(file_path, 'w') as f:
    f.write(new_content)
print("Successfully reconstructed blocks.")
