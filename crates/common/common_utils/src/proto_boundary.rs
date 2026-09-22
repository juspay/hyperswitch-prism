//! Proto/domain boundary access for monetary types.
//!
//! **Connector code must NEVER import this module.**
//!
//! This trait provides raw i64 construction and extraction on [`MinorUnit`]
//! and [`Money`]. These operations are only needed at the proto ↔ domain
//! boundary (in `domain_types` and `grpc-server`).
//!
//! A CI lint enforces that `crates/integrations/connector-integration/`
//! never contains `use ...::proto_boundary`.

use common_enums::enums;

use crate::types::{MinorUnit, Money};

/// Raw i64 access for [`MinorUnit`].
///
/// Import this trait **only** in proto/domain boundary code.
/// Connector code must use [`AmountConvertor`] instead.
pub trait MinorUnitProtoAccess {
    /// Construct from a raw i64 at the proto boundary.
    fn new(value: i64) -> Self;

    /// Extract the raw i64 at the proto boundary.
    fn get_amount_as_i64(self) -> i64;
}

impl MinorUnitProtoAccess for MinorUnit {
    fn new(value: i64) -> Self {
        Self::from_i64(value)
    }

    fn get_amount_as_i64(self) -> i64 {
        self.as_i64()
    }
}

/// Raw field access for [`Money`].
///
/// Import this trait **only** in proto/domain boundary code.
pub trait MoneyProtoAccess {
    /// Construct from a [`MinorUnit`] and currency at the proto boundary.
    fn new(amount: MinorUnit, currency: enums::Currency) -> Self;

    /// Decompose into (MinorUnit, Currency).
    fn into_parts(self) -> (MinorUnit, enums::Currency);

    /// Access the inner [`MinorUnit`].
    fn amount(&self) -> MinorUnit;
}

impl MoneyProtoAccess for Money {
    fn new(amount: MinorUnit, currency: enums::Currency) -> Self {
        Self::from_minor_unit(amount, currency)
    }

    fn into_parts(self) -> (MinorUnit, enums::Currency) {
        (self.amount, self.currency)
    }

    fn amount(&self) -> MinorUnit {
        self.amount
    }
}
