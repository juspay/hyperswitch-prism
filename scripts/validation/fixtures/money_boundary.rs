#![allow(unused_imports)]
#![cfg_attr(
    not(feature = "domain_boundary"),
    deny(clippy::disallowed_methods, clippy::disallowed_types)
)]
// Exercise the future policy separately from the temporary production deny policy.
#![cfg_attr(
    feature = "boundary_allow",
    forbid(clippy::disallowed_methods, clippy::disallowed_types)
)]
#![forbid(unfulfilled_lint_expectations)]

use common_enums::Currency;
#[cfg(feature = "domain_boundary")]
use common_utils::types::MinorUnit;
use common_utils::types::{AmountConvertor, ConnectorMinorUnitForConnector, Money};

#[cfg(feature = "domain_boundary")]
pub fn domain_boundary(amount: MinorUnit, currency: Currency) -> Money {
    common_utils::types::MoneyBoundary::from_minor_unit(amount, currency)
}

#[cfg(feature = "connector_pass")]
pub fn connector_pass(money: &Money) {
    let _zero = Money::zero(money.currency());
    let _one_minor_unit = Money::one_minor_unit(money.currency());
    let amount = ConnectorMinorUnitForConnector.convert(money).unwrap();
    let _restored = ConnectorMinorUnitForConnector
        .convert_back(amount, money.currency())
        .unwrap();
}

#[cfg(feature = "boundary_expect")]
pub fn boundary_expect(source: &Money, currency: Currency) -> Money {
    #[expect(
        clippy::disallowed_methods,
        clippy::disallowed_types,
        reason = "Temporary statement-scoped compatibility exception"
    )]
    let money = common_utils::types::MoneyBoundary::from_minor_unit(source.amount(), currency);
    money
}

#[cfg(feature = "boundary_expect_scope")]
pub fn boundary_expect_scope(source: &Money, currency: Currency) -> (Money, Money) {
    #[expect(
        clippy::disallowed_methods,
        clippy::disallowed_types,
        reason = "Temporary statement-scoped compatibility exception"
    )]
    let legacy_money =
        common_utils::types::MoneyBoundary::from_minor_unit(source.amount(), currency);
    let unapproved_money =
        common_utils::types::MoneyBoundary::from_minor_unit(source.amount(), currency);
    (legacy_money, unapproved_money)
}

#[cfg(feature = "obsolete_boundary_expect")]
pub fn obsolete_boundary_expect(currency: Currency) -> Money {
    #[expect(
        clippy::disallowed_methods,
        clippy::disallowed_types,
        reason = "An obsolete compatibility exception must fail"
    )]
    let money = Money::zero(currency);
    money
}

#[cfg(feature = "boundary_allow")]
pub fn boundary_allow(source: &Money, currency: Currency) -> Money {
    #[allow(clippy::disallowed_methods, clippy::disallowed_types)]
    let money = common_utils::types::MoneyBoundary::from_minor_unit(source.amount(), currency);
    money
}

#[cfg(all(test, feature = "boundary_in_tests"))]
pub fn boundary_in_tests(source: &Money, currency: Currency) -> Money {
    common_utils::types::MoneyBoundary::from_minor_unit(source.amount(), currency)
}

#[cfg(feature = "private_constructor")]
pub fn private_constructor(source: &Money, currency: Currency) -> Money {
    Money::new(source.amount(), currency)
}

#[cfg(feature = "private_fields")]
pub fn private_fields(source: &Money, currency: Currency) -> Money {
    Money {
        amount: source.amount(),
        currency,
    }
}

#[cfg(feature = "boundary_direct")]
pub fn boundary_direct(source: &Money, currency: Currency) -> Money {
    common_utils::types::MoneyBoundary::from_minor_unit(source.amount(), currency)
}

#[cfg(feature = "boundary_alias")]
pub fn boundary_alias(source: &Money, currency: Currency) -> Money {
    use common_utils::types::MoneyBoundary as Factory;
    Factory::from_minor_unit(source.amount(), currency)
}

#[cfg(feature = "boundary_pointer")]
pub fn boundary_pointer(source: &Money, currency: Currency) -> Money {
    let create = common_utils::types::MoneyBoundary::from_minor_unit;
    create(source.amount(), currency)
}

#[cfg(feature = "boundary_type_alias")]
pub fn boundary_type_alias(source: &Money, currency: Currency) -> Money {
    type Factory = common_utils::types::MoneyBoundary;
    Factory::from_minor_unit(source.amount(), currency)
}

#[cfg(feature = "boundary_reexport")]
pub fn boundary_reexport(source: &Money, currency: Currency) -> Money {
    mod facade {
        pub use common_utils::types::MoneyBoundary as Factory;
    }
    facade::Factory::from_minor_unit(source.amount(), currency)
}

#[cfg(feature = "proto_trait")]
pub fn proto_trait(proto: grpc_api_types::payments::Money) {
    use domain_types::utils::ForeignTryFrom;
    let _ = Money::foreign_try_from(proto);
}

#[cfg(feature = "domain_mapper")]
pub fn domain_mapper(proto: grpc_api_types::payments::Money) {
    let _ = domain_types::money::from_proto_money(proto);
}

#[cfg(feature = "domain_minor_unit")]
pub struct DomainMinorUnitPayload {
    pub amount: common_utils::types::MinorUnit,
}

#[cfg(feature = "domain_minor_converter")]
pub fn domain_minor_converter(money: &Money) {
    let _ = common_utils::types::MinorUnitForConnector.convert(money);
}

#[cfg(feature = "money_amount")]
pub fn money_amount(money: &Money) {
    let _ = money.amount();
}
