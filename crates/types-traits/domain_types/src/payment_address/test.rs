use common_enums::CountryAlpha2;
use hyperswitch_masking::{PeekInterface, Secret};

use super::{AddressDetails, State};
use crate::utils::ForeignTryFrom;

fn grpc_address(
    legacy_state: Option<&str>,
    state_details: Option<grpc_api_types::payments::address::StateDetails>,
    country: grpc_api_types::payments::CountryAlpha2,
) -> grpc_api_types::payments::Address {
    #[allow(deprecated)]
    grpc_api_types::payments::Address {
        state: legacy_state.map(|state| Secret::new(state.to_string())),
        state_details,
        country_alpha2_code: Some(country as i32),
        ..Default::default()
    }
}

#[test]
fn canonical_state_code_is_normalized_to_subdivision_component() {
    let address = grpc_address(
        None,
        Some(
            grpc_api_types::payments::address::StateDetails::StateCodeIso(Secret::new(
                "US-CA".to_string(),
            )),
        ),
        grpc_api_types::payments::CountryAlpha2::Us,
    );

    let address = AddressDetails::foreign_try_from(address).expect("valid ISO state code");

    assert_eq!(address.country, Some(CountryAlpha2::US));
    assert!(matches!(
        address.state.as_ref(),
        Some(State::StateCode(state)) if state.peek() == "CA"
    ));
    assert_eq!(
        address
            .to_state_code()
            .expect("state code should be available")
            .peek(),
        "CA"
    );
}

#[test]
fn state_details_takes_precedence_over_legacy_state() {
    let address = grpc_address(
        Some("California"),
        Some(
            grpc_api_types::payments::address::StateDetails::StateCodeIso(Secret::new(
                "CA".to_string(),
            )),
        ),
        grpc_api_types::payments::CountryAlpha2::Us,
    );

    let address = AddressDetails::foreign_try_from(address).expect("valid address");

    assert!(matches!(
        address.state.as_ref(),
        Some(State::StateCode(state)) if state.peek() == "CA"
    ));
}

#[test]
fn canonical_state_code_country_prefix_must_match_address_country() {
    let address = grpc_address(
        None,
        Some(
            grpc_api_types::payments::address::StateDetails::StateCodeIso(Secret::new(
                "CA-ON".to_string(),
            )),
        ),
        grpc_api_types::payments::CountryAlpha2::Us,
    );

    assert!(AddressDetails::foreign_try_from(address).is_err());
}
