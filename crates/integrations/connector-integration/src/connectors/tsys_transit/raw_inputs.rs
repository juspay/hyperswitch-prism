//! Sanitised merchant-intent DTO.
//!
//! Holds everything the assembler needs that is *not* a profile dimension:
//! card data, amounts, addresses, merchant metadata, 3DS values, mandate
//! references. Every value here is already cleaned (e.g. `external_reference_id`
//! has its underscores stripped) so rules can pass them through unconditionally.
//!
//! Empty for now; the rules-extraction PR will populate it.
