macro_rules! create_amount_converter_wrapper {
    (connector_name: $connector_name:ident, amount_type: $amount_type:ty) => {
        paste::paste! {
            #[derive(Default, Debug, Clone, Copy, PartialEq)]
            pub struct [<$connector_name AmountConvertor>];

            impl [<$connector_name AmountConvertor>] {
                pub fn convert(
                    amount: common_utils::types::MinorUnit,
                    currency: common_enums::Currency,
                ) -> Result<
                    common_utils::types::$amount_type,
                    error_stack::Report<domain_types::errors::IntegrationError>,
                > {
                    domain_types::utils::convert_amount(
                        &common_utils::types::[<$amount_type ForConnector>],
                        amount,
                        currency,
                    ).change_context(domain_types::errors::IntegrationError::InvalidDataFormat {
                        field_name: "amount",
                        context: Default::default()
                  })
                }

                /// Convert connector amount back to MinorUnit.
                ///
                /// Returns generic ParsingError - caller should change_context appropriately:
                /// ```
                /// // In response transformation:
                /// let amount = Convertor::convert_back(response.amount, currency)
                ///     .change_context(crate::utils::response_handling_fail_for_connector(http_code, "macros"))?;
                pub fn convert_back(
                    amount: common_utils::types::$amount_type,
                    currency: common_enums::Currency,
                ) -> Result<
                    common_utils::types::MinorUnit,
                    error_stack::Report<common_utils::errors::ParsingError>,
                > {
                    domain_types::utils::convert_back_amount_to_minor_units(
                        &common_utils::types::[<$amount_type ForConnector>],
                        amount,
                        currency,
                    )
                }
            }
        }
    };
}

pub(crate) use create_amount_converter_wrapper;
