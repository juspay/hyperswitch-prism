#![allow(unused_imports)]
#![cfg_attr(
    not(feature = "domain_boundary"),
    forbid(clippy::disallowed_methods, clippy::disallowed_types)
)]
#![forbid(unfulfilled_lint_expectations)]

use common_enums::Currency;
#[cfg(feature = "domain_boundary")]
use common_utils::types::MinorUnit;
use common_utils::types::{AmountConvertor, ConnectorMinorUnitForConnector, Money};

#[cfg(feature = "float_major_read")]
pub fn float_major_read(amount: common_utils::FloatMajorUnit) {
    let _ = amount.0;
}

#[cfg(feature = "float_major_write")]
pub fn float_major_write(amount: &mut common_utils::FloatMajorUnit) {
    amount.0 = 1.23;
}

#[cfg(feature = "float_major_constructor")]
pub fn float_major_constructor() {
    let _ = common_utils::FloatMajorUnit(1.23);
}

#[cfg(feature = "two_decimal_float_pass")]
pub fn two_decimal_float_pass(money: &Money) {
    let converter = common_utils::FloatMajorUnitWithTwoDecimalsForConnector;
    let amount: common_utils::FloatMajorUnitWithTwoDecimals = converter.convert(money).unwrap();
    let _ = amount == Default::default();
    let _ = converter.convert_back(amount, money.currency()).unwrap();
}

#[cfg(feature = "two_decimal_float_read")]
pub fn two_decimal_float_read(amount: common_utils::FloatMajorUnitWithTwoDecimals) {
    let _ = amount.0;
}

#[cfg(feature = "two_decimal_float_write")]
pub fn two_decimal_float_write(amount: &mut common_utils::FloatMajorUnitWithTwoDecimals) {
    amount.0 = 1.23;
}

#[cfg(feature = "two_decimal_float_constructor")]
pub fn two_decimal_float_constructor() {
    let _ = common_utils::FloatMajorUnitWithTwoDecimals(1.23);
}

#[cfg(feature = "two_decimal_float_add")]
pub fn two_decimal_float_add(amount: common_utils::FloatMajorUnitWithTwoDecimals) {
    let _ = amount + amount;
}

#[cfg(feature = "two_decimal_float_sub")]
pub fn two_decimal_float_sub(amount: common_utils::FloatMajorUnitWithTwoDecimals) {
    let _ = amount - amount;
}

#[cfg(feature = "two_decimal_float_mul")]
pub fn two_decimal_float_mul(amount: common_utils::FloatMajorUnitWithTwoDecimals) {
    let _ = amount * 2_u16;
}

#[cfg(feature = "two_decimal_float_sum")]
pub fn two_decimal_float_sum(amount: common_utils::FloatMajorUnitWithTwoDecimals) {
    let _: common_utils::FloatMajorUnitWithTwoDecimals = std::iter::once(amount).sum();
}

#[cfg(feature = "minor_unit_type_alias")]
pub type DomainAmount = common_utils::MinorUnit;

#[cfg(feature = "money_amount_read")]
pub fn money_amount_read(money: &Money) {
    let _ = money.amount;
}

#[cfg(feature = "money_currency_read")]
pub fn money_currency_read(money: &Money) {
    let _ = money.currency;
}

#[cfg(feature = "money_amount_write")]
pub fn money_amount_write(money: &mut Money) {
    money.amount = Default::default();
}

#[cfg(feature = "money_currency_write")]
pub fn money_currency_write(money: &mut Money) {
    money.currency = Currency::USD;
}

#[cfg(feature = "connector_minor_add")]
pub fn connector_minor_add(amount: common_utils::ConnectorMinorUnit) {
    let _ = amount + amount;
}

#[cfg(feature = "connector_minor_sub")]
pub fn connector_minor_sub(amount: common_utils::ConnectorMinorUnit) {
    let _ = amount - amount;
}

#[cfg(feature = "connector_minor_mul")]
pub fn connector_minor_mul(amount: common_utils::ConnectorMinorUnit) {
    let _ = amount * 2_u16;
}

#[cfg(feature = "connector_minor_sum")]
pub fn connector_minor_sum(amount: common_utils::ConnectorMinorUnit) {
    let _: common_utils::ConnectorMinorUnit = std::iter::once(amount).sum();
}

#[cfg(feature = "connector_minor_checked_mul")]
pub fn connector_minor_checked_mul(amount: common_utils::ConnectorMinorUnit) {
    let _ = amount.checked_mul(2);
}

#[cfg(feature = "connector_minor_trait_arithmetic")]
pub fn connector_minor_trait_arithmetic(amount: common_utils::ConnectorMinorUnit) {
    let _ = std::ops::Add::add(amount, amount);
    let _ = std::ops::Sub::sub(amount, amount);
    let _ = std::ops::Mul::mul(amount, 2_u16);
}

#[cfg(feature = "float_major_add")]
pub fn float_major_add(amount: common_utils::FloatMajorUnit) {
    let _ = amount + amount;
}

#[cfg(feature = "float_major_mul")]
pub fn float_major_mul(amount: common_utils::FloatMajorUnit) {
    let _ = amount * 2_u16;
}

#[cfg(feature = "float_major_trait_arithmetic")]
pub fn float_major_trait_arithmetic(amount: common_utils::FloatMajorUnit) {
    let _ = std::ops::Add::add(amount, amount);
    let _ = std::ops::Mul::mul(amount, 2_u16);
}

#[cfg(feature = "connector_integer_getter")]
pub fn connector_integer_getter(amount: common_utils::types::ConnectorMinorUnit) -> i64 {
    amount.get_amount_as_i64()
}

#[cfg(feature = "connector_integer_getter_pointer")]
pub fn connector_integer_getter_pointer(amount: common_utils::types::ConnectorMinorUnit) -> i64 {
    let extract = common_utils::types::ConnectorMinorUnit::get_amount_as_i64;
    extract(amount)
}

#[cfg(feature = "domain_proto_money_mapper")]
pub fn domain_proto_money_mapper(money: &Money) {
    let _ = domain_types::money::to_proto_money(money);
}

#[cfg(feature = "domain_order_money_preparation")]
pub fn domain_order_money_preparation() {
    let _ = domain_types::types::map_order_details(Default::default(), Currency::USD);
}

#[cfg(feature = "prepared_order_money_pass")]
pub fn prepared_order_money_pass(item: &domain_types::payment_address::OrderDetailsWithAmount) {
    let _ = ConnectorMinorUnitForConnector.convert(&item.money);
    let _ = item
        .line_total_money
        .as_ref()
        .map(|money| ConnectorMinorUnitForConnector.convert(money));
    let _ = item
        .line_discount_money
        .as_ref()
        .map(|money| ConnectorMinorUnitForConnector.convert(money));
}

#[cfg(feature = "legacy_order_amount_field")]
pub fn legacy_order_amount_field(item: &domain_types::payment_address::OrderDetailsWithAmount) {
    let _ = item.amount;
}

#[cfg(feature = "currencyless_order_mapper")]
pub fn currencyless_order_mapper(proto: grpc_api_types::payments::OrderDetailsWithAmount) {
    use domain_types::utils::ForeignTryFrom;
    let _ = domain_types::payment_address::OrderDetailsWithAmount::foreign_try_from(proto);
}

#[cfg(feature = "percentage_ratio")]
pub fn percentage_ratio() -> Option<common_utils::types::Percentage> {
    common_utils::types::Percentage::from_minor_unit_ratio(Default::default(), Default::default())
}

#[cfg(feature = "domain_boundary")]
pub fn domain_boundary(amount: MinorUnit, currency: Currency) -> Money {
    let _created = MinorUnit::new(1);
    common_utils::types::Money::from_minor_unit(amount, currency)
}

#[cfg(feature = "connector_pass")]
pub fn connector_pass(money: &Money) {
    let float_converter = common_utils::FloatMajorUnitForConnector;
    let float_amount = float_converter.convert(money).unwrap();
    let _ = float_amount.is_positive();
    let _ = float_converter.convert_back(float_amount, money.currency()).unwrap();
    let _zero = Money::zero(money.currency());
    let _one_minor_unit = Money::one_minor_unit(money.currency());
    let amount: common_utils::types::ConnectorMinorUnit =
        ConnectorMinorUnitForConnector.convert(money);
    let _fallible_amount =
        AmountConvertor::convert(&ConnectorMinorUnitForConnector, money).unwrap();
    let _restored = ConnectorMinorUnitForConnector
        .convert_back(amount, money.currency())
        .unwrap();
    let _major_wire = common_utils::types::StringMajorUnitForConnector
        .convert(money)
        .unwrap();
    let _two_decimal_wire = common_utils::types::StringTwoDecimalUnitForConnector
        .convert(money)
        .unwrap();
}

#[cfg(feature = "connector_minor_to_string")]
pub fn connector_minor_to_string(amount: common_utils::types::ConnectorMinorUnit) -> String {
    amount.to_string()
}

#[cfg(feature = "connector_minor_display")]
pub fn connector_minor_display(amount: common_utils::types::ConnectorMinorUnit) -> String {
    format!("{amount}")
}

#[cfg(feature = "string_minor_to_string")]
pub fn string_minor_to_string(amount: common_utils::types::StringMinorUnit) -> String {
    amount.to_string()
}

#[cfg(feature = "string_minor_display")]
pub fn string_minor_display(amount: common_utils::types::StringMinorUnit) -> String {
    format!("{amount}")
}

#[cfg(feature = "string_major_to_string")]
pub fn string_major_to_string(amount: common_utils::types::StringMajorUnit) -> String {
    amount.to_string()
}

#[cfg(feature = "string_major_display")]
pub fn string_major_display(amount: common_utils::types::StringMajorUnit) -> String {
    format!("{amount}")
}

#[cfg(feature = "string_two_decimal_to_string")]
pub fn string_two_decimal_to_string(amount: common_utils::types::StringTwoDecimalUnit) -> String {
    amount.to_string()
}

#[cfg(feature = "string_two_decimal_display")]
pub fn string_two_decimal_display(amount: common_utils::types::StringTwoDecimalUnit) -> String {
    format!("{amount}")
}

#[cfg(feature = "string_major_getter")]
pub fn string_major_getter(amount: common_utils::types::StringMajorUnit) -> String {
    amount.get_amount_as_string()
}

#[cfg(feature = "string_two_decimal_getter")]
pub fn string_two_decimal_getter(amount: &common_utils::types::StringTwoDecimalUnit) -> &str {
    amount.get_amount_as_string()
}

#[cfg(feature = "boundary_expect")]
pub fn boundary_expect(currency: Currency) -> Money {
    #[expect(
        clippy::disallowed_methods,
        reason = "Attempted exception must fail under the L3 forbid policy"
    )]
    let money = common_utils::types::Money::from_minor_unit(Default::default(), currency);
    money
}

#[cfg(feature = "boundary_expect_scope")]
pub fn boundary_expect_scope(currency: Currency) -> (Money, Money) {
    #[expect(
        clippy::disallowed_methods,
        reason = "Attempted exception must fail under the L3 forbid policy"
    )]
    let legacy_money =
        common_utils::types::Money::from_minor_unit(Default::default(), currency);
    let unapproved_money =
        common_utils::types::Money::from_minor_unit(Default::default(), currency);
    (legacy_money, unapproved_money)
}

#[cfg(feature = "obsolete_boundary_expect")]
pub fn obsolete_boundary_expect(currency: Currency) -> Money {
    #[expect(
        clippy::disallowed_methods,
        reason = "An obsolete compatibility exception must fail"
    )]
    let money = Money::zero(currency);
    money
}

#[cfg(feature = "boundary_allow")]
pub fn boundary_allow(currency: Currency) -> Money {
    #[allow(clippy::disallowed_methods, clippy::disallowed_types)]
    let money = common_utils::types::Money::from_minor_unit(Default::default(), currency);
    money
}

#[cfg(all(test, feature = "boundary_in_tests"))]
pub fn boundary_in_tests(currency: Currency) -> Money {
    common_utils::types::Money::from_minor_unit(Default::default(), currency)
}

#[cfg(feature = "private_constructor")]
pub fn private_constructor(source: &Money, currency: Currency) -> Money {
    Money::new(source.amount(), currency)
}

#[cfg(feature = "private_fields")]
pub fn private_fields(currency: Currency) -> Money {
    Money {
        amount: Default::default(),
        currency,
    }
}

#[cfg(feature = "boundary_direct")]
pub fn boundary_direct(currency: Currency) -> Money {
    common_utils::money::Money::from_minor_unit(Default::default(), currency)
}

#[cfg(feature = "boundary_alias")]
pub fn boundary_alias(currency: Currency) -> Money {
    use common_utils::Money as Factory;
    Factory::from_minor_unit(Default::default(), currency)
}

#[cfg(feature = "boundary_pointer")]
pub fn boundary_pointer(currency: Currency) -> Money {
    let create = common_utils::types::Money::from_minor_unit;
    create(Default::default(), currency)
}

#[cfg(feature = "boundary_type_alias")]
pub fn boundary_type_alias(currency: Currency) -> Money {
    type Factory = common_utils::types::Money;
    Factory::from_minor_unit(Default::default(), currency)
}

#[cfg(feature = "boundary_reexport")]
pub fn boundary_reexport(currency: Currency) -> Money {
    mod facade {
        pub use common_utils::types::Money as Factory;
    }
    facade::Factory::from_minor_unit(Default::default(), currency)
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

#[cfg(feature = "minor_unit_constructor")]
pub fn minor_unit_constructor() {
    let _ = common_utils::types::MinorUnit::new(1);
}

#[cfg(feature = "minor_unit_constructor_pointer")]
pub fn minor_unit_constructor_pointer() {
    let create = common_utils::types::MinorUnit::new;
    let _ = create(1);
}

#[cfg(feature = "domain_minor_converter")]
pub fn domain_minor_converter(money: &Money) {
    let _ = common_utils::types::MinorUnitForConnector.convert(money);
}

#[cfg(feature = "inferred_domain_minor_unit")]
pub fn inferred_domain_minor_unit(
    request: &domain_types::payouts::payouts_types::PayoutStageRequest,
) {
    let _ = request.amount.get_amount_as_i64();
}

#[cfg(feature = "money_amount")]
pub fn money_amount(money: &Money) {
    let _ = money.amount();
}

#[cfg(feature = "inferred_minor_field_read")]
pub fn inferred_minor_field_read(
    item: &domain_types::payment_address::OrderDetailsWithAmount,
) -> i64 {
    item.get_total_amount().0
}

#[cfg(feature = "inferred_minor_field_write")]
pub fn inferred_minor_field_write(
    item: &mut domain_types::payment_address::OrderDetailsWithAmount,
) {
    if let Some(amount) = item.total_tax_amount.as_mut() {
        amount.0 = 1;
    }
}

#[cfg(feature = "inferred_minor_to_string")]
pub fn inferred_minor_to_string(
    item: &domain_types::payment_address::OrderDetailsWithAmount,
) -> String {
    item.get_total_amount().to_string()
}

#[cfg(feature = "inferred_minor_display")]
pub fn inferred_minor_display(
    item: &domain_types::payment_address::OrderDetailsWithAmount,
) -> String {
    format!("{}", item.get_total_amount())
}

#[cfg(feature = "inferred_minor_add")]
pub fn inferred_minor_add(item: &mut domain_types::payment_address::OrderDetailsWithAmount) {
    item.total_amount = Some(item.get_total_amount() + item.get_total_amount());
}

#[cfg(feature = "inferred_minor_mul")]
pub fn inferred_minor_mul(item: &mut domain_types::payment_address::OrderDetailsWithAmount) {
    item.total_amount = Some(item.get_total_amount() * 2_u16);
}

#[cfg(feature = "inferred_minor_sub")]
pub fn inferred_minor_sub(
    item: &mut domain_types::payment_address::OrderDetailsWithAmount,
    other: &domain_types::payment_address::OrderDetailsWithAmount,
) {
    item.total_amount = Some(item.get_total_amount() - other.get_total_amount());
}

#[cfg(feature = "inferred_minor_sum")]
pub fn inferred_minor_sum(item: &mut domain_types::payment_address::OrderDetailsWithAmount) {
    item.total_amount = Some(std::iter::once(item.get_total_amount()).sum());
}
