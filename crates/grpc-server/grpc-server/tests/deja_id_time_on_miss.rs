#![cfg(feature = "deja")]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use common_utils::{date_time, fp_utils};

fn install_empty_replay_hook() {
    let table = deja::LookupTable {
        recording_id: "id-time-on-miss-test".to_string(),
        policy_version: deja::POLICY_VERSION,
        event_schema_version: Some(deja::CURRENT_EVENT_SCHEMA_VERSION),
        entries: vec![],
        identity_entries: vec![],
    };
    let path =
        std::env::temp_dir().join(format!("deja-id-time-on-miss-{}.json", std::process::id()));
    std::fs::write(&path, serde_json::to_vec(&table).unwrap()).unwrap();
    let hook = deja::LookupTableHook::from_source(
        deja::LocalFileLookupSource::new(path),
        deja::InMemoryObservedSink::new(),
    )
    .expect("lookup hook");
    deja::set_global_runtime_hook(Some(deja::RuntimeHook::LookupReplay(hook)))
        .expect("install runtime hook");
}

const ALPHANUMERIC: &str = "0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";

#[test]
fn every_id_and_time_seam_answers_a_miss_instead_of_stopping() {
    install_empty_replay_hook();

    for uuid in [fp_utils::generate_uuid_v4(), fp_utils::generate_uuid_v7()] {
        let parsed = uuid::Uuid::parse_str(&uuid).expect("a synthesized uuid must parse");
        assert_eq!(parsed.get_version_num(), 8);
    }

    let id = fp_utils::generate_id(12, "pay");
    let (prefix, body) = id.split_once('_').expect("prefix_body");
    assert_eq!(prefix, "pay");
    assert_eq!(body.chars().count(), 12, "{id}");
    assert!(body.chars().all(|c| ALPHANUMERIC.contains(c)), "{id}");

    let default_len = fp_utils::generate_id_with_default_len("ref");
    assert!(default_len.starts_with("ref_"), "{default_len}");
    assert_eq!(
        default_len.trim_start_matches("ref_").chars().count(),
        common_utils::consts::ID_LENGTH
    );

    let ordered = common_utils::generate_time_ordered_id("evt");
    assert!(ordered.starts_with("evt_"), "{ordered}");
    assert!(
        !ordered.contains('-'),
        "a time-ordered id is hyphen-free: {ordered}"
    );

    {
        use common_utils::crypto::{DecodeMessage, EncodeMessage, GcmAes256};
        let key = [7_u8; 32];
        let sealed = GcmAes256
            .encode_message(&key, b"on-miss nonce")
            .expect("encryption must run on a synthesized nonce");
        let opened = GcmAes256
            .decode_message(&key, hyperswitch_masking::Secret::new(sealed))
            .expect("and what it seals must open again");
        assert_eq!(opened, b"on-miss nonce");
    }

    assert_eq!(domain_types::utils::generate_random_bytes(24).len(), 24);
    let secret = common_utils::crypto::generate_cryptographically_secure_random_string(32);
    assert_eq!(secret.chars().count(), 32);
    assert!(secret.chars().all(|c| ALPHANUMERIC.contains(c)));

    let now = date_time::now();
    assert_eq!(now.year(), 1970, "{now}");
    let unix = date_time::now_unix_timestamp();
    assert!((0..60).contains(&unix), "{unix}");
}
