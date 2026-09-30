//! Rules: L2 / L3 commercial-card field gating.
//!
//! Each function takes the profile (used for `card_family` /
//! `commercial_level`) and the raw value the merchant supplied. The
//! function decides whether the field should land on the wire.
//!
//! Cert csv pushback rows this module addresses:
//!   • "purchaseOrder tag should not be sent on the 1.50 AMEX level II
//!      transaction in step 2 as this is a Visa and Mastercard only tag."
//!      → `purchase_order` returns None on AMEX.
//!   • "customerRefID tag must not be sent on the .52 Mastercard
//!      transaction in step 2 as this is an AMEX only tag."
//!   • "supplierReferenceNumber tag must not be sent on the .52 Mastercard
//!      transaction in step 2 as this is an AMEX only tag."
//!      → `customer_ref_id` / `supplier_reference_number` AMEX-only.
//!   • "shipToZip tag should not be sent on the .52 Mastercard transaction
//!      in step 2 as this is a Level III tag and we are only testing
//!      Level II on this test case."
//!   • "destinationCountryCode tag should not be sent on the .52
//!      Mastercard transaction in step 2 as this is a Level III tag..."
//!      → `ship_to_zip` / `destination_country_code` L3-only on V/MC.
//!   • "shipToZip tag must not be sent on the 1.50 AMEX level II
//!      transaction in step 1 as this is a Visa and Mastercard level III
//!      only tag." (reinforces V/MC-only L3 gating)

use common_utils::types::StringMajorUnit;
use domain_types::errors::{IntegrationError, IntegrationErrorContext};
use error_stack::Report;

use super::super::profile::{CardFamily, CommercialLevel, TxProfile};
use super::super::transformers::{
    TsysTransitAdditionalTaxDetails, TsysTransitProductDetails, TsysTransitTaxCategory,
};

/// Shared shape for the batch of Level-III-only fields below: not sent (and
/// not required) below Level III; mandatory once the profile IS Level III.
fn require_for_l3<T>(
    profile: &TxProfile,
    raw: Option<T>,
    field_name: &'static str,
) -> Result<Option<T>, Report<IntegrationError>> {
    if profile.commercial_level.is_l3() {
        let value = raw.ok_or_else(|| IntegrationError::MissingRequiredField {
            field_name,
            context: IntegrationErrorContext {
                suggested_action: Some(format!("Provide {field_name} in the request")),
                doc_url: None,
                additional_context: Some(format!(
                    "{field_name} is required because commercial_level is LEVEL3 \
                     (card_family={:?})",
                    profile.card_family
                )),
            },
        })?;

        Ok(Some(value))
    } else {
        Ok(None)
    }
}

/// `purchaseOrder` — Visa / Mastercard only, mandatory at both Level II and
/// Level III (see `purchase_order_required`); never sent (and not required)
/// on AMEX or below Level II.
pub fn purchase_order(
    profile: &TxProfile,
    raw: Option<String>,
) -> Result<Option<String>, Report<IntegrationError>> {
    if purchase_order_required(profile) {
        let field_name = "purchaseOrder";
        let purchase_order = raw.ok_or_else(|| IntegrationError::MissingRequiredField {
            field_name,
            context: IntegrationErrorContext {
                suggested_action: Some(format!("Provide {field_name} in the request")),
                doc_url: None,
                additional_context: Some(format!(
                    "{field_name} is required because card_family={:?} and commercial_level={:?}",
                    profile.card_family, profile.commercial_level
                )),
            },
        })?;

        Ok(Some(purchase_order))
    } else {
        Ok(None)
    }
}

/// `customerRefID` — AMEX Level II only; mandatory in that case. Never sent
/// otherwise (Visa/Mastercard at any level, AMEX outside L2, or no
/// commercial level at all).
pub fn customer_ref_id(
    profile: &TxProfile,
    raw: Option<String>,
) -> Result<Option<String>, Report<IntegrationError>> {
    if amex_l2_extras_required(profile) {
        let field_name = "customerRefID";
        let customer_ref_id = raw.ok_or_else(|| IntegrationError::MissingRequiredField {
            field_name,
            context: IntegrationErrorContext {
                suggested_action: Some(format!("Provide {field_name} in the request")),
                doc_url: None,
                additional_context: Some(format!(
                    "{field_name} is required because card_family={:?} and commercial_level={:?}",
                    profile.card_family, profile.commercial_level
                )),
            },
        })?;

        Ok(Some(customer_ref_id))
    } else {
        Ok(None)
    }
}

/// `supplierReferenceNumber` — AMEX Level II only; mandatory in that case.
/// Never sent otherwise (Visa/Mastercard at any level, AMEX outside L2, or
/// no commercial level at all).
pub fn supplier_reference_number(
    profile: &TxProfile,
    raw: Option<String>,
) -> Result<Option<String>, Report<IntegrationError>> {
    if amex_l2_extras_required(profile) {
        let field_name = "supplierReferenceNumber";
        let supplier_reference_number =
            raw.ok_or_else(|| IntegrationError::MissingRequiredField {
                field_name,
                context: IntegrationErrorContext {
                    suggested_action: Some(format!("Provide {field_name} in the request")),
                    doc_url: None,
                    additional_context: Some(format!(
                    "{field_name} is required because card_family={:?} and commercial_level={:?}",
                    profile.card_family, profile.commercial_level
                )),
                },
            })?;

        Ok(Some(supplier_reference_number))
    } else {
        Ok(None)
    }
}

/// `shipToZip` — mandatory for AMEX at Level II, or Visa/Mastercard at Level
/// III. Never sent (and not required) for Visa/Mastercard at Level II, or
/// for any other level/family combination.
/// Source of truth: TSYS_MOTO_V3 row 127 (AMEX Level II) carries
/// `<shipToZip>85284</shipToZip>` (V3 primary); TSYS_MOTO_V2 shows Mastercard
/// L2 (row 126) does NOT carry it. So on L2 it is AMEX-only; Visa/MC only send
/// it at L3. (`destinationCountryCode` stays L3-only.)
pub fn ship_to_zip(
    profile: &TxProfile,
    raw: Option<String>,
) -> Result<Option<String>, Report<IntegrationError>> {
    let amex_l2 = profile.card_family.is_amex() && profile.commercial_level.is_l2();
    let vmc_l3 = profile.commercial_level.is_l3() && profile.card_family.is_visa_or_mastercard();
    if amex_l2 || vmc_l3 {
        let field_name = "shipToZip";
        let ship_to_zip = raw.ok_or_else(|| IntegrationError::MissingRequiredField {
            field_name,
            context: IntegrationErrorContext {
                suggested_action: Some(format!("Provide {field_name} in the request")),
                doc_url: None,
                additional_context: Some(format!(
                    "{field_name} is required because card_family={:?} and commercial_level={:?}",
                    profile.card_family, profile.commercial_level
                )),
            },
        })?;

        Ok(Some(ship_to_zip))
    } else {
        Ok(None)
    }
}

/// `destinationCountryCode` — Visa / Mastercard L3 only; mandatory in that
/// case, not sent (and not required) otherwise.
pub fn destination_country_code(
    profile: &TxProfile,
    raw: Option<String>,
) -> Result<Option<String>, Report<IntegrationError>> {
    if profile.commercial_level.is_l3() && profile.card_family.is_visa_or_mastercard() {
        let field_name = "destinationCountryCode";
        let destination_country_code =
            raw.ok_or_else(|| IntegrationError::MissingRequiredField {
                field_name,
                context: IntegrationErrorContext {
                    suggested_action: Some(format!("Provide {field_name} in the request")),
                    doc_url: None,
                    additional_context: Some(format!(
                    "{field_name} is required because card_family={:?} and commercial_level={:?}",
                    profile.card_family, profile.commercial_level
                )),
                },
            })?;

        Ok(Some(destination_country_code))
    } else {
        Ok(None)
    }
}

/// `commercialCardLevel` — sent only when L2 or L3.
pub fn commercial_card_level(
    profile: &TxProfile,
) -> Option<super::super::transformers::TsysTransitCommercialCardLevel> {
    use super::super::transformers::TsysTransitCommercialCardLevel as L;
    match profile.commercial_level {
        CommercialLevel::None => None,
        CommercialLevel::L2 => Some(L::Level2),
        CommercialLevel::L3 => Some(L::Level3),
    }
}

/// True when `purchaseOrder` is a required field for this profile.
/// Used by the assembler to convert "missing" into a hard error.
pub fn purchase_order_required(profile: &TxProfile) -> bool {
    profile.commercial_level.is_l2_or_l3() && profile.card_family.is_visa_or_mastercard()
}

/// True when `salesTax` is required (any L2 or L3 transaction).
pub fn sales_tax_required(profile: &TxProfile) -> bool {
    profile.commercial_level.is_l2_or_l3()
}

/// True when `customerVATNumber` is required — Level III Visa only.
/// Mastercard Level III carries only `salesTax`, no VAT number.
pub fn customer_vat_number_required(profile: &TxProfile) -> bool {
    profile.commercial_level.is_l3() && profile.card_family.is_visa()
}

/// `salesTax` — mandatory on any Level II or Level III transaction
/// (both card families); not sent otherwise.
pub fn sales_tax(
    profile: &TxProfile,
    raw: Option<StringMajorUnit>,
) -> Result<Option<StringMajorUnit>, Report<IntegrationError>> {
    if sales_tax_required(profile) {
        let field_name = "salesTax";
        let sales_tax = raw.ok_or_else(|| IntegrationError::MissingRequiredField {
            field_name,
            context: IntegrationErrorContext {
                suggested_action: Some(format!("Provide {field_name} in the request")),
                doc_url: None,
                additional_context: Some(format!(
                    "{field_name} is required because card_family={:?} and commercial_level={:?}",
                    profile.card_family, profile.commercial_level
                )),
            },
        })?;

        Ok(Some(sales_tax))
    } else {
        Ok(None)
    }
}

/// `customerVATNumber` — Level III Visa only; mandatory in that case
/// (TSYS requires it alongside `salesTax` for Visa L3). Never sent on
/// Mastercard or below Level III.
pub fn customer_vat_number(
    profile: &TxProfile,
    raw: Option<String>,
) -> Result<Option<String>, Report<IntegrationError>> {
    if customer_vat_number_required(profile) {
        let field_name = "customerVATNumber";
        let customer_vat_number = raw.ok_or_else(|| IntegrationError::MissingRequiredField {
            field_name,
            context: IntegrationErrorContext {
                suggested_action: Some(format!("Provide {field_name} in the request")),
                doc_url: None,
                additional_context: Some(format!(
                    "{field_name} is required because card_family={:?} and commercial_level={:?}",
                    profile.card_family, profile.commercial_level
                )),
            },
        })?;

        Ok(Some(customer_vat_number))
    } else {
        Ok(None)
    }
}

/// True when the AMEX-L2-required quartet (supplierReferenceNumber,
/// customerRefID, chargeDescriptor) is required. shipToZip is NOT in
/// the required set because the cert explicitly excludes it from AMEX L2.
pub fn amex_l2_extras_required(profile: &TxProfile) -> bool {
    matches!(profile.card_family, CardFamily::Amex)
        && matches!(profile.commercial_level, CommercialLevel::L2)
}

/// `chargeDescriptor` — AMEX Level II only; mandatory in that case. Never
/// sent otherwise (including Visa/Mastercard at Level III).
pub fn charge_descriptor(
    profile: &TxProfile,
    raw: Option<String>,
) -> Result<Option<String>, Report<IntegrationError>> {
    if amex_l2_extras_required(profile) {
        let field_name = "chargeDescriptor";
        let charge_descriptor = raw.ok_or_else(|| IntegrationError::MissingRequiredField {
            field_name,
            context: IntegrationErrorContext {
                suggested_action: Some(format!("Provide {field_name} in the request")),
                doc_url: None,
                additional_context: Some(format!(
                    "{field_name} is required because card_family={:?} and commercial_level={:?}",
                    profile.card_family, profile.commercial_level
                )),
            },
        })?;

        Ok(Some(charge_descriptor))
    } else {
        Ok(None)
    }
}

/// True when L3 Visa/MC required fields are required (purchaseOrder,
/// orderDate, summaryCommodityCode, vatInvoice, shipFromZip, shipToZip,
/// destinationCountryCode).
pub fn l3_visa_mc_required(profile: &TxProfile) -> bool {
    profile.commercial_level.is_l3() && profile.card_family.is_visa_or_mastercard()
}

/// `shippingCharges` — Level III only; mandatory in that case.
pub fn shipping_charges(
    profile: &TxProfile,
    raw: Option<StringMajorUnit>,
) -> Result<Option<StringMajorUnit>, Report<IntegrationError>> {
    require_for_l3(
        profile,
        raw,
        "shippingCharges required when commercial_card_level is LEVEL3",
    )
}

/// `dutyCharges` — Level III only; mandatory in that case.
pub fn duty_charges(
    profile: &TxProfile,
    raw: Option<StringMajorUnit>,
) -> Result<Option<StringMajorUnit>, Report<IntegrationError>> {
    require_for_l3(
        profile,
        raw,
        "dutyCharges required when commercial_card_level is LEVEL3",
    )
}

/// `productDetails` — Level III only (order line items); mandatory in that
/// case, empty otherwise.
pub fn product_details(
    profile: &TxProfile,
    raw: Option<Vec<TsysTransitProductDetails>>,
) -> Result<Vec<TsysTransitProductDetails>, Report<IntegrationError>> {
    Ok(require_for_l3(
        profile,
        raw,
        "productDetails required when commercial_card_level is LEVEL3",
    )?
    .unwrap_or_default())
}

/// `orderDate` — Level III only; mandatory in that case.
pub fn order_date(
    profile: &TxProfile,
    raw: Option<String>,
) -> Result<Option<String>, Report<IntegrationError>> {
    require_for_l3(
        profile,
        raw,
        "orderDate required when commercial_card_level is LEVEL3",
    )
}

/// `summaryCommodityCode` — Level III only; mandatory in that case.
pub fn summary_commodity_code(
    profile: &TxProfile,
    raw: Option<String>,
) -> Result<Option<String>, Report<IntegrationError>> {
    require_for_l3(
        profile,
        raw,
        "summaryCommodityCode required when commercial_card_level is LEVEL3",
    )
}

/// `vatInvoice` — Level III only; mandatory in that case.
pub fn vat_invoice(
    profile: &TxProfile,
    raw: Option<String>,
) -> Result<Option<String>, Report<IntegrationError>> {
    require_for_l3(
        profile,
        raw,
        "vatInvoice required when commercial_card_level is LEVEL3",
    )
}

/// `shipFromZip` — Level III only; mandatory in that case.
pub fn ship_from_zip(
    profile: &TxProfile,
    raw: Option<String>,
) -> Result<Option<String>, Report<IntegrationError>> {
    require_for_l3(
        profile,
        raw,
        "shipFromZip required when commercial_card_level is LEVEL3",
    )
}

/// `additionalTaxDetails` — Level III only. Not sent (empty) on any other
/// commercial level; when the profile IS Level III, `taxType`, `taxAmount`,
/// `taxRate` and `taxCategory` are all mandatory and the transaction fails
/// fast if any of them is missing rather than silently omitting the block.
pub fn additional_tax_details(
    profile: &TxProfile,
    tax_type: Option<String>,
    tax_amount: Option<StringMajorUnit>,
    tax_rate: Option<String>,
    tax_category: Option<TsysTransitTaxCategory>,
) -> Result<Vec<TsysTransitAdditionalTaxDetails>, Report<IntegrationError>> {
    if profile.commercial_level.is_l3() {
        let missing_field = |field_name: &'static str| IntegrationError::MissingRequiredField {
            field_name,
            context: IntegrationErrorContext {
                suggested_action: Some(format!("Provide {field_name} in the request")),
                doc_url: None,
                additional_context: Some(format!(
                    "{field_name} is required because card_family={:?} and commercial_level={:?}",
                    profile.card_family, profile.commercial_level
                )),
            },
        };

        let tax_type = tax_type.ok_or_else(|| missing_field("taxType"))?;
        let tax_amount = tax_amount.ok_or_else(|| missing_field("taxAmount"))?;
        let tax_rate = tax_rate.ok_or_else(|| missing_field("taxRate"))?;
        let tax_category = tax_category.ok_or_else(|| missing_field("taxCategory"))?;

        Ok(vec![TsysTransitAdditionalTaxDetails {
            tax_type,
            tax_amount,
            tax_rate: Some(tax_rate),
            tax_category: Some(tax_category),
        }])
    } else {
        Ok(Vec::new())
    }
}
