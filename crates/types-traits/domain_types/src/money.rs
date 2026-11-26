use common_utils::types::{MinorUnit, Money};

use crate::{utils::ForeignTryFrom, IntegrationError};

/// Keep the existing unspecified-currency fallback for flows with optional payment money.
pub(crate) fn order_currency(
    payment: Option<grpc_api_types::payments::Money>,
) -> Result<common_enums::Currency, error_stack::Report<IntegrationError>> {
    common_enums::Currency::foreign_try_from(
        payment
            .map(|payment| payment.currency())
            .unwrap_or_default(),
    )
}

/// Proto mapping stays in this crate: common_utils cannot depend on grpc-api-types.
/// Callers with strict currency validation must check the raw enum integer first;
/// the generated currency accessor maps unknown values to Unspecified.
pub(crate) fn from_proto_money(
    value: grpc_api_types::payments::Money,
) -> Result<Money, error_stack::Report<IntegrationError>> {
    Ok(Money::from_minor_unit(
        MinorUnit::new(value.minor_amount),
        common_enums::Currency::foreign_try_from(value.currency())?,
    ))
}

/// Whole-money response mapping stays in L2, never in connector transformers.
pub(crate) fn to_proto_money(
    value: &Money,
) -> Result<grpc_api_types::payments::Money, error_stack::Report<crate::errors::ConnectorError>> {
    Ok(grpc_api_types::payments::Money {
        minor_amount: value.amount().get_amount_as_i64(),
        currency: grpc_api_types::payments::Currency::foreign_try_from(value.currency())?.into(),
    })
}
