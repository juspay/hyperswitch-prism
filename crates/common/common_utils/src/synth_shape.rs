//! Deterministic values for id and time seams that miss on replay.

/// `length` characters drawn from `alphabet`, derived from `miss`.
pub fn over(miss: &deja::SubstituteMiss, alphabet: &[char], length: usize) -> String {
    let span = match u64::try_from(alphabet.len()) {
        Ok(span) if span > 0 => span,
        _ => return String::new(),
    };
    let seed = deja::synth::u64(miss);
    (0..length)
        .filter_map(|index| {
            let index = u64::try_from(index).ok()?;
            let pick = usize::try_from(mix(seed, index) % span).ok()?;
            alphabet.get(pick).copied()
        })
        .collect()
}

/// `length` deterministic bytes derived from `miss`.
pub fn byte_vec(miss: &deja::SubstituteMiss, length: usize) -> Vec<u8> {
    let seed = deja::synth::u64(miss);
    (0..length)
        .filter_map(|position| {
            let position = u64::try_from(position).ok()?;
            u8::try_from(mix(seed, position) & 0xff).ok()
        })
        .collect()
}

/// A deterministic version 8 UUID, which no live generator emits.
pub fn uuid(miss: &deja::SubstituteMiss) -> String {
    deja::synth::uuid_v8(miss)
}

/// Step between two synthesized instants at one call site: one millisecond.
pub const CLOCK_STEP_NS: i64 = 1_000_000;

/// A deterministic instant near the Unix epoch, advancing with the occurrence.
pub fn instant(miss: &deja::SubstituteMiss) -> time::OffsetDateTime {
    let nanos = deja::synth::monotonic(miss, 0, CLOCK_STEP_NS);
    time::OffsetDateTime::from_unix_timestamp_nanos(i128::from(nanos))
        .unwrap_or(time::OffsetDateTime::UNIX_EPOCH)
}

/// splitmix64 finalizer over the seed and a position.
fn mix(seed: u64, index: u64) -> u64 {
    let mut state = seed ^ index.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    state = (state ^ (state >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    state = (state ^ (state >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    state ^ (state >> 31)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    const HEX: [char; 16] = [
        '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f',
    ];

    fn miss(args: serde_json::Value) -> deja::SubstituteMiss {
        deja::SubstituteMiss::new("id", "test", "generate", args)
    }

    #[test]
    fn the_same_miss_gives_the_same_value() {
        let first = over(&miss(serde_json::json!({"n": 12})), &HEX, 12);
        let second = over(&miss(serde_json::json!({"n": 12})), &HEX, 12);
        assert_eq!(first, second);
        assert!(!first.is_empty(), "and it must not be vacuously equal");
        assert_eq!(
            uuid(&miss(serde_json::json!({}))),
            uuid(&miss(serde_json::json!({})))
        );
        assert_eq!(
            instant(&miss(serde_json::json!({}))),
            instant(&miss(serde_json::json!({})))
        );
    }

    #[test]
    fn a_different_miss_gives_a_different_value() {
        let a = over(&miss(serde_json::json!({"n": 12})), &HEX, 12);
        let b = over(&miss(serde_json::json!({"n": 13})), &HEX, 12);
        assert_ne!(a, b);
    }

    #[test]
    fn the_value_honours_the_alphabet_and_the_length() {
        for length in [1_usize, 8, 20, 64] {
            let value = over(&miss(serde_json::json!({"n": length})), &HEX, length);
            assert_eq!(value.chars().count(), length);
            assert!(value.chars().all(|c| HEX.contains(&c)), "{value}");
            assert_eq!(
                byte_vec(&miss(serde_json::json!({"n": length})), length).len(),
                length
            );
        }
    }

    #[test]
    fn positions_do_not_all_collapse_to_one_character() {
        let value = over(&miss(serde_json::json!({})), &HEX, 32);
        let distinct: std::collections::BTreeSet<char> = value.chars().collect();
        assert!(distinct.len() > 4, "{value}");
    }

    #[test]
    fn a_synthesized_uuid_is_version_8() {
        let parsed = ::uuid::Uuid::parse_str(&uuid(&miss(serde_json::json!({})))).unwrap();
        assert_eq!(parsed.get_version_num(), 8);
    }

    #[test]
    fn an_empty_alphabet_yields_an_empty_string_rather_than_a_panic() {
        assert_eq!(over(&miss(serde_json::json!({})), &[], 8), "");
    }
}
