//! Lean-generated vectors for `AmountConvertor` (`convert` / `convert_back`).
//!
//! The vectors and their expected amounts come from the Lean spec in
//! `tests/lean/PrismSpec/Amount/` (regenerate: `cd tests/lean && lake build && lake exe gen`).
//! Each vector runs through the real convertors in `common_utils::types`.
//!
//! Vectors UCS gets wrong today are listed in `KNOWN_GAPS`. The test fails if a gap closes
//! without being removed from the list, so the list cannot go stale.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::{collections::BTreeMap, str::FromStr};

use common_enums::Currency;
use common_utils::types::{
    AmountConvertor, FloatMajorUnit, FloatMajorUnitForConnector, MinorUnit, StringMajorUnit,
    StringMajorUnitForConnector, StringMinorUnit, StringMinorUnitForConnector,
};
use serde::Deserialize;

const VECTORS: &str = include_str!("../../vectors/amount_conversion.json");

/// Vectors UCS gets wrong today. Remove an entry once UCS matches the spec for it.
const KNOWN_GAPS: &[&str] = &[
    // Problem 3: `to_major_unit_as_f64` divides CLF by 10^4, but the `to_minor_unit_as_i64`
    // impls only know exponents 0 and 3 and multiply everything else by 100
    "send_clf_1",
    "send_clf_5",
    "send_clf_100",
    "send_clf_12345",
    "send_clf_999999999",
    "receive_clf_1.2345_ok",
    "receive_clf_1.23_ok",
    "receive_clf_0.0001_ok",
    // Problem 2: `Decimal::to_i64` truncates the digits past the currency's exponent
    // instead of rejecting the amount
    "receive_usd_10.999_reject",
    "receive_usd_10.009_reject",
    "receive_usd_0.001_reject",
    "receive_jpy_10.5_reject",
    "receive_kwd_1.2345_reject",
    "receive_clf_1.23456_reject",
    "receive_minor_12.7_reject",
];

#[derive(Deserialize)]
struct Document {
    vectors: Vec<Vector>,
}

#[derive(Deserialize)]
struct Vector {
    id: String,
    kind: String,
    currency: String,
    wire: String,
    minor: Option<i64>,
}

fn string_major(wire: &str) -> StringMajorUnit {
    serde_json::from_value(serde_json::Value::String(wire.to_owned())).unwrap()
}

fn string_minor(wire: &str) -> StringMinorUnit {
    serde_json::from_value(serde_json::Value::String(wire.to_owned())).unwrap()
}

fn float_major(wire: &str) -> FloatMajorUnit {
    FloatMajorUnit(wire.parse().unwrap())
}

fn back<T>(r: Result<MinorUnit, T>) -> Option<i64> {
    r.ok().map(|m| m.get_amount_as_i64())
}

/// Every way UCS disagrees with the spec on one vector, as `convertor: detail`.
fn mismatches(v: &Vector) -> Vec<String> {
    let currency = Currency::from_str(&v.currency).unwrap();
    let mut out = Vec::new();
    let check = |out: &mut Vec<String>, name: &str, got: Option<i64>| {
        if got != v.minor {
            out.push(format!("{name}: got {got:?}, spec {:?}", v.minor));
        }
    };
    match v.kind.as_str() {
        "send" => {
            let minor = MinorUnit::new(v.minor.unwrap());
            let (s, f) = (StringMajorUnitForConnector, FloatMajorUnitForConnector);

            let sent = s.convert(minor, currency).unwrap();
            if sent.get_amount_as_string() != v.wire {
                out.push(format!(
                    "StringMajor convert: got {:?}, spec {:?}",
                    sent.get_amount_as_string(),
                    v.wire
                ));
            }
            let got = back(s.convert_back(sent, currency));
            check(&mut out, "StringMajor round trip", got);

            let sent = f.convert(minor, currency).unwrap();
            if sent.0 != float_major(&v.wire).0 {
                out.push(format!(
                    "FloatMajor convert: got {}, spec {}",
                    sent.0, v.wire
                ));
            }
            let got = back(f.convert_back(sent, currency));
            check(&mut out, "FloatMajor round trip", got);
        }
        "receive_major" => {
            let got =
                back(StringMajorUnitForConnector.convert_back(string_major(&v.wire), currency));
            check(&mut out, "StringMajor convert_back", got);
            let got = back(FloatMajorUnitForConnector.convert_back(float_major(&v.wire), currency));
            check(&mut out, "FloatMajor convert_back", got);
        }
        "receive_minor" => {
            let got =
                back(StringMinorUnitForConnector.convert_back(string_minor(&v.wire), currency));
            check(&mut out, "StringMinor convert_back", got);
        }
        other => panic!("unknown vector kind {other}"),
    }
    out
}

#[test]
fn ucs_amount_conversion_matches_spec() {
    let doc: Document = serde_json::from_str(VECTORS).unwrap();
    let failing: BTreeMap<&str, Vec<String>> = doc
        .vectors
        .iter()
        .map(|v| (v.id.as_str(), mismatches(v)))
        .filter(|(_, m)| !m.is_empty())
        .collect();

    let unexpected: Vec<_> = failing
        .iter()
        .filter(|(id, _)| !KNOWN_GAPS.contains(id))
        .collect();
    let closed: Vec<_> = KNOWN_GAPS
        .iter()
        .filter(|id| !failing.contains_key(*id))
        .collect();

    assert!(
        unexpected.is_empty(),
        "UCS disagrees with the spec on vectors not in KNOWN_GAPS:\n{unexpected:#?}"
    );
    assert!(
        closed.is_empty(),
        "these KNOWN_GAPS now match the spec; remove them from the list: {closed:?}"
    );
}
