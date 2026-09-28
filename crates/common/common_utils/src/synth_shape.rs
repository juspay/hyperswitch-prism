//! Deterministic stand-ins for a generator's output when a déjà replay finds no
//! recorded value at an id or time seam.
//!
//! Without an `on_miss` arm, a `Substitute` seam that misses fail-stops: the
//! request ends at that line and nothing after it is compared. A candidate that
//! adds one id or clock read — which is what a change often is — then loses the
//! whole correlation to a call that says nothing about behaviour. These arms keep
//! the request alive. The miss is still scored: the lookup records the call as
//! novel before the arm runs, and the scorer makes a correlation that continued
//! on a synthesized value inconclusive, never passed.
//!
//! [`deja::synth::id`] is the generic answer, and wrong here: its marker contains
//! `-` and its length is fixed, while `consts::ALPHABETS` forbids `-` and the
//! callers promise a length. A value a connector would reject is worse than a
//! stop, because the rejection is attributed to the candidate. So these derive
//! over the CALLER's alphabet and length.
//!
//! Every value is a function of the miss alone — boundary, method, args,
//! occurrence and correlation — so one replay gets the same answer every run, and
//! two candidates replayed on one tape stay comparable past the edge of the tape.

/// `length` characters drawn from `alphabet`, derived from `miss`.
///
/// Returns an empty string for an empty alphabet rather than panicking: this
/// runs on the miss path, where a panic would end the correlation the arm exists
/// to keep alive.
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

/// `length` deterministic bytes. Not [`deja::synth::bytes`], whose length is a
/// const generic: these callers choose a length at runtime.
pub fn byte_vec(miss: &deja::SubstituteMiss, length: usize) -> Vec<u8> {
    let seed = deja::synth::u64(miss);
    (0..length)
        .filter_map(|position| {
            let position = u64::try_from(position).ok()?;
            u8::try_from(mix(seed, position) & 0xff).ok()
        })
        .collect()
}

/// A deterministic UUID in the **version 8** space, hyphenated.
///
/// Version 8 is RFC 9562's custom space, so a synthesized uuid is structurally
/// disjoint from the v4 and v7 values real code produces: a synthesized id can
/// never satisfy a lookup keyed on a recorded one.
pub fn uuid(miss: &deja::SubstituteMiss) -> String {
    deja::synth::uuid_v8(miss)
}

/// How far a synthesized clock advances between two misses at one call site:
/// one millisecond, in nanoseconds.
pub const CLOCK_STEP_NS: i64 = 1_000_000;

/// A deterministic UTC instant for this miss, advancing with the occurrence at
/// its call site.
///
/// The base is the Unix epoch, not a correlation's time origin: no such origin
/// is available at a miss, and inventing one would reintroduce the ambient
/// dependency these arms exist to remove. It also reads as obviously synthetic.
/// Total: falls back to the epoch rather than panicking.
pub fn instant(miss: &deja::SubstituteMiss) -> time::OffsetDateTime {
    let nanos = deja::synth::monotonic(miss, 0, CLOCK_STEP_NS);
    time::OffsetDateTime::from_unix_timestamp_nanos(i128::from(nanos))
        .unwrap_or(time::OffsetDateTime::UNIX_EPOCH)
}

/// One digest per position, so two positions never correlate and extending the
/// length never rewrites the characters already produced. splitmix64's
/// finalizer; nothing here is secret — everything is derivable from the query,
/// which is the point.
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

    /// The property replay depends on: one query, one answer, every run.
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

    /// Two different queries must not collide, or a downstream lookup keyed on
    /// one synthesized id would resolve another's.
    #[test]
    fn a_different_miss_gives_a_different_value() {
        let a = over(&miss(serde_json::json!({"n": 12})), &HEX, 12);
        let b = over(&miss(serde_json::json!({"n": 13})), &HEX, 12);
        assert_ne!(a, b);
    }

    /// The reason this exists rather than `deja::synth::id`: the value honours
    /// the alphabet and length its caller promised.
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

    /// Positions must differ, not merely be in the alphabet: an index-blind `mix`
    /// passes every other test here with a value carrying four bits.
    #[test]
    fn positions_do_not_all_collapse_to_one_character() {
        let value = over(&miss(serde_json::json!({})), &HEX, 32);
        let distinct: std::collections::BTreeSet<char> = value.chars().collect();
        assert!(distinct.len() > 4, "{value}");
    }

    /// A synthesized uuid must never be a v4 or v7, so it cannot resolve a
    /// lookup keyed on a recorded one.
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
