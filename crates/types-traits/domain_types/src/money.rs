use common_utils::types::{MinorUnit, Money, MoneyBoundary};

use crate::{utils::ForeignTryFrom, IntegrationError};

/// Proto mapping stays in this crate: common_utils cannot depend on grpc-api-types.
pub(crate) fn from_proto_money(
    value: grpc_api_types::payments::Money,
) -> Result<Money, error_stack::Report<IntegrationError>> {
    Ok(MoneyBoundary::from_minor_unit(
        MinorUnit::new(value.minor_amount),
        common_enums::Currency::foreign_try_from(value.currency())?,
    ))
}
