#![allow(clippy::unwrap_used)]

use domain_types::payment_method_data::{DefaultPCIHolder, PaymentMethodData, VoucherData};
use grpc_api_types::payments::{payment_method::PaymentMethod as ProtoPaymentMethod, Boleto};

fn boleto(expiration_date: Option<&str>) -> grpc_api_types::payments::PaymentMethod {
    grpc_api_types::payments::PaymentMethod {
        payment_method: Some(ProtoPaymentMethod::Boleto(Boleto {
            social_security_number: None,
            expiration_date: expiration_date.map(str::to_string),
        })),
    }
}

#[test]
fn malformed_boleto_due_date_is_rejected() {
    let result =
        PaymentMethodData::<DefaultPCIHolder>::convert_to_domain_model_for_non_card_payment_methods(
            boleto(Some("next tuesday")),
        );
    assert!(result.is_err());
}

#[test]
fn valid_boleto_due_date_is_kept() {
    let data =
        PaymentMethodData::<DefaultPCIHolder>::convert_to_domain_model_for_non_card_payment_methods(
            boleto(Some("2026-12-01T10:00:00")),
        )
        .unwrap();
    match data {
        PaymentMethodData::Voucher(VoucherData::Boleto(boleto)) => {
            assert!(boleto.expiration_date.is_some())
        }
        other => panic!("expected a boleto voucher, got {other:?}"),
    }
}

#[test]
fn absent_boleto_due_date_is_still_none() {
    let data =
        PaymentMethodData::<DefaultPCIHolder>::convert_to_domain_model_for_non_card_payment_methods(
            boleto(None),
        )
        .unwrap();
    match data {
        PaymentMethodData::Voucher(VoucherData::Boleto(boleto)) => {
            assert!(boleto.expiration_date.is_none())
        }
        other => panic!("expected a boleto voucher, got {other:?}"),
    }
}
