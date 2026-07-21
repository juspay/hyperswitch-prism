pub mod effects;
pub mod flush;
pub mod replay;
pub mod runtime;
pub mod schema;

#[cfg(test)]
mod schema_tests {
    use std::collections::HashMap;

    use serde_json::json;

    use crate::schema::{
        CallApiEntry, CsvRecording, Either, ErrorPayload, HttpRequestEntry, HttpResponseEntry,
        IncomingApiEntry, IncomingApiRequestEntry, IncomingApiResponseEntry, MetadataEntry,
        RandomBytesEntry, RecordingEntry, TimestampEntry,
    };

    #[test]
    fn metadata_entry_serializes_with_eulerhs_tag_contents_shape() {
        let entry = RecordingEntry::Metadata(MetadataEntry::new(
            "PRISM_ART_CONTEXT",
            json!({
                "request_id": "req_123",
                "merchant_id": "merchant_123",
                "reference_id": "order_123"
            }),
        ));

        assert_eq!(
            serde_json::to_value(entry).expect("entry should serialize"),
            json!({
                "tag": "MetadataEntryT",
                "contents": {
                    "tag": "PRISM_ART_CONTEXT",
                    "metadata": {
                        "request_id": "req_123",
                        "merchant_id": "merchant_123",
                        "reference_id": "order_123"
                    }
                }
            })
        );
    }

    #[test]
    fn timestamp_entry_serializes_with_eulerhs_tag_contents_shape() {
        let entry = RecordingEntry::Timestamp(TimestampEntry {
            function_name: "getCurrentTime".to_string(),
            timestamp: json!("2026-07-07T13:00:00.000Z"),
            tag: "authorize".to_string(),
        });

        assert_eq!(
            serde_json::to_value(entry).expect("entry should serialize"),
            json!({
                "tag": "TimeStampEntryT",
                "contents": {
                    "functionName": "getCurrentTime",
                    "timestamp": "2026-07-07T13:00:00.000Z",
                    "tag": "authorize"
                }
            })
        );
    }

    #[test]
    fn call_api_entry_serializes_eulerhs_http_field_names() {
        let mut request_headers = HashMap::new();
        request_headers.insert("content-type".to_string(), "application/json".to_string());

        let mut response_headers = HashMap::new();
        response_headers.insert("x-request-id".to_string(), "req_123".to_string());

        let entry = RecordingEntry::CallApi(CallApiEntry {
            json_request: HttpRequestEntry {
                get_request_method: "Post".to_string(),
                get_request_headers: request_headers,
                get_request_body: Some("eyJhbW91bnQiOjEwMH0=".to_string()),
                get_request_url: "https://connector.example/payments".to_string(),
                get_request_timeout: Some(30_000_000),
                get_request_redirects: Some(5),
            },
            json_result: Either::Right(json!(HttpResponseEntry {
                get_response_body: "eyJzdGF0dXMiOiJPSyJ9".to_string(),
                get_response_code: 200,
                get_response_headers: response_headers,
                get_response_status: "OK".to_string(),
            })),
            api_tag: "AUTHORIZE".to_string(),
        });

        assert_eq!(
            serde_json::to_value(entry).expect("entry should serialize"),
            json!({
                "tag": "CallAPIEntryT",
                "contents": {
                    "jsonRequest": {
                        "getRequestMethod": "Post",
                        "getRequestHeaders": { "content-type": "application/json" },
                        "getRequestBody": "eyJhbW91bnQiOjEwMH0=",
                        "getRequestURL": "https://connector.example/payments",
                        "getRequestTimeout": 30000000,
                        "getRequestRedirects": 5
                    },
                    "jsonResult": {
                        "Right": {
                            "getResponseBody": "eyJzdGF0dXMiOiJPSyJ9",
                            "getResponseCode": 200,
                            "getResponseHeaders": { "x-request-id": "req_123" },
                            "getResponseStatus": "OK"
                        }
                    },
                    "apiTag": "AUTHORIZE"
                }
            })
        );
    }

    #[test]
    fn call_api_entry_deserializes_legacy_pii_tag() {
        let entry = serde_json::from_value::<RecordingEntry>(json!({
            "tag": "CallAPIEntryPII",
            "contents": {
                "jsonRequest": {
                    "getRequestMethod": "Get",
                    "getRequestHeaders": {},
                    "getRequestBody": null,
                    "getRequestURL": "https://connector.example/status",
                    "getRequestTimeout": null,
                    "getRequestRedirects": null
                },
                "jsonResult": {
                    "Right": {
                        "getResponseBody": "e30=",
                        "getResponseCode": 200,
                        "getResponseHeaders": {},
                        "getResponseStatus": "OK"
                    }
                },
                "apiTag": "PSYNC"
            }
        }))
        .expect("legacy CallAPIEntryPII should deserialize");

        assert!(matches!(entry, RecordingEntry::CallApiPii(_)));
    }

    #[test]
    fn call_api_entry_serializes_error_payload_as_left() {
        let entry = CallApiEntry {
            json_request: HttpRequestEntry {
                get_request_method: "Get".to_string(),
                get_request_headers: HashMap::new(),
                get_request_body: None,
                get_request_url: "https://connector.example/status".to_string(),
                get_request_timeout: None,
                get_request_redirects: None,
            },
            json_result: Either::Left(ErrorPayload {
                is_error: true,
                error_message: "connector timeout".to_string(),
                user_message: "Connector timed out".to_string(),
            }),
            api_tag: "PSYNC".to_string(),
        };

        assert_eq!(
            serde_json::to_value(entry).expect("entry should serialize"),
            json!({
                "jsonRequest": {
                    "getRequestMethod": "Get",
                    "getRequestHeaders": {},
                    "getRequestBody": null,
                    "getRequestURL": "https://connector.example/status",
                    "getRequestTimeout": null,
                    "getRequestRedirects": null
                },
                "jsonResult": {
                    "Left": {
                        "isError": true,
                        "errorMessage": "connector timeout",
                        "userMessage": "Connector timed out"
                    }
                },
                "apiTag": "PSYNC"
            })
        );
    }

    #[test]
    fn incoming_api_entry_serializes_eulerhs_field_names() {
        let entry = RecordingEntry::IncomingApi(IncomingApiEntry {
            api_request: IncomingApiRequestEntry {
                api_req_body: json!({"amount": 100}),
                api_req_url: "/payments".to_string(),
                api_req_method: "POST".to_string(),
                api_req_headers: HashMap::from([(
                    "x-request-id".to_string(),
                    "req_123".to_string(),
                )]),
                api_req_query_params: HashMap::new(),
                api_req_route_params: HashMap::new(),
            },
            api_response: IncomingApiResponseEntry {
                api_res_body: json!({"status": "succeeded"}),
                api_res_headers: HashMap::from([(
                    "content-type".to_string(),
                    "application/json".to_string(),
                )]),
                api_res_code: 200,
            },
            api_tag: "AUTHORIZE".to_string(),
            hostname: "connector-service".to_string(),
            start_time: json!("2026-07-07T13:00:00.000Z"),
            end_time: json!("2026-07-07T13:00:01.000Z"),
        });

        assert_eq!(
            serde_json::to_value(entry).expect("entry should serialize"),
            json!({
                "tag": "IncomingApiEntryT",
                "contents": {
                    "apiRequest": {
                        "apiReqBody": { "amount": 100 },
                        "apiReqUrl": "/payments",
                        "apiReqMethod": "POST",
                        "apiReqHeaders": { "x-request-id": "req_123" },
                        "apiReqQueryParams": {},
                        "apiReqRouteParams": {}
                    },
                    "apiResponse": {
                        "apiResBody": { "status": "succeeded" },
                        "apiResHeaders": { "content-type": "application/json" },
                        "apiResCode": 200
                    },
                    "apiTag": "AUTHORIZE",
                    "hostname": "connector-service",
                    "startTime": "2026-07-07T13:00:00.000Z",
                    "endTime": "2026-07-07T13:00:01.000Z"
                }
            })
        );
    }

    #[test]
    fn random_bytes_entry_encodes_value_as_base64() {
        let entry = RandomBytesEntry::from_bytes(vec![1, 2, 3, 4], "encryption_nonce");

        assert_eq!(
            serde_json::to_value(entry).expect("entry should serialize"),
            json!({
                "functionName": "getRandomBytes",
                "input": 4,
                "value": "AQIDBA==",
                "tag": "encryption_nonce"
            })
        );
    }

    #[test]
    fn csv_recording_serializes_art_upload_field_names() {
        let row = CsvRecording {
            sess_id: "req_123".to_string(),
            merch_id: "merchant_123".to_string(),
            ord_id: "order_123".to_string(),
            counter: 7,
            val_type: "TIMESTAMP".to_string(),
            rec_entry: "{\"tag\":\"TimeStampEntryT\"}".to_string(),
        };

        assert_eq!(
            serde_json::to_value(row).expect("row should serialize"),
            json!({
                "sessId": "req_123",
                "merchId": "merchant_123",
                "ordId": "order_123",
                "counter": 7,
                "valType": "TIMESTAMP",
                "recEntry": "{\"tag\":\"TimeStampEntryT\"}"
            })
        );
    }
}

#[cfg(test)]
mod effects_tests {
    use std::{collections::HashMap, ops::RangeInclusive};

    use serde_json::json;

    use crate::{
        effects,
        runtime::{ArtRuntime, ArtRuntimeSettings, SessionContext},
        schema::{
            CallApiEntry, Either, HttpRequestEntry, HttpResponseEntry, IncomingApiEntry,
            IncomingApiRequestEntry, IncomingApiResponseEntry, RecordingEntry,
        },
    };

    fn session_context() -> SessionContext {
        SessionContext {
            request_id: "req_123".to_string(),
            merchant_id: "merchant_123".to_string(),
            connector: "stripe".to_string(),
            flow: "authorize".to_string(),
            hostname: "connector-service".to_string(),
        }
    }

    #[test]
    fn disabled_runtime_returns_live_values_without_recording() {
        let mut runtime = ArtRuntime::disabled();

        let uuid = effects::uuid_v4_with_runtime(&mut runtime, "payment_id")
            .expect("disabled uuid generation should succeed");
        let bytes = effects::random_bytes_with_runtime(&mut runtime, 4, "nonce")
            .expect("disabled random bytes should succeed");

        assert!(!uuid.is_empty());
        assert_eq!(bytes.len(), 4);
        assert!(runtime.recorded_entries().is_empty());
    }

    #[test]
    fn record_runtime_records_time_uuid_random_and_bytes_effects() {
        let mut runtime = ArtRuntime::recording(session_context(), Some(10));

        let timestamp = effects::now_with_runtime(&mut runtime, "timestamp")
            .expect("record timestamp should succeed");
        let uuid = effects::uuid_v4_with_runtime(&mut runtime, "payment_id")
            .expect("record uuid should succeed");
        let random_value = effects::random_i64_range_with_runtime(
            &mut runtime,
            RangeInclusive::new(10, 20),
            "amount",
        )
        .expect("record random value should succeed");
        let bytes = effects::random_bytes_with_runtime(&mut runtime, 4, "nonce")
            .expect("record bytes should succeed");

        assert_eq!(runtime.recorded_entries().len(), 4);
        assert_eq!(
            runtime.recorded_entries()[0],
            RecordingEntry::Timestamp(crate::schema::TimestampEntry {
                function_name: "getCurrentTime".to_string(),
                timestamp,
                tag: "timestamp".to_string(),
            })
        );
        assert_eq!(
            runtime.recorded_entries()[1],
            RecordingEntry::Uuid(crate::schema::UuidEntry {
                function_name: "uuidV4".to_string(),
                uuid,
                tag: None,
            })
        );
        assert_eq!(
            runtime.recorded_entries()[2],
            RecordingEntry::RandomRio(crate::schema::RandomRioEntry {
                function_name: "randomRIO".to_string(),
                range: json!([10, 20]),
                value: json!(random_value),
                tag: "amount".to_string(),
            })
        );
        assert_eq!(
            runtime.recorded_entries()[3],
            RecordingEntry::RandomBytes(crate::schema::RandomBytesEntry::from_bytes(
                bytes, "nonce"
            ))
        );
    }

    #[test]
    fn record_runtime_records_outgoing_and_incoming_http_entries() {
        let mut runtime = ArtRuntime::recording(session_context(), Some(10));
        let call_api_entry = CallApiEntry {
            json_request: HttpRequestEntry {
                get_request_method: "Post".to_string(),
                get_request_headers: HashMap::new(),
                get_request_body: Some("e30=".to_string()),
                get_request_url: "https://connector.example/payments".to_string(),
                get_request_timeout: None,
                get_request_redirects: None,
            },
            json_result: Either::Right(json!(HttpResponseEntry {
                get_response_body: "e30=".to_string(),
                get_response_code: 200,
                get_response_headers: HashMap::new(),
                get_response_status: "OK".to_string(),
            })),
            api_tag: "AUTHORIZE".to_string(),
        };
        let incoming_api_entry = IncomingApiEntry {
            api_request: IncomingApiRequestEntry {
                api_req_body: json!({}),
                api_req_url: "/payments".to_string(),
                api_req_method: "POST".to_string(),
                api_req_headers: HashMap::new(),
                api_req_query_params: HashMap::new(),
                api_req_route_params: HashMap::new(),
            },
            api_response: IncomingApiResponseEntry {
                api_res_body: json!({}),
                api_res_headers: HashMap::new(),
                api_res_code: 200,
            },
            api_tag: "AUTHORIZE".to_string(),
            hostname: "connector-service".to_string(),
            start_time: json!("2026-07-07T13:00:00.000Z"),
            end_time: json!("2026-07-07T13:00:01.000Z"),
        };

        effects::record_outgoing_http_with_runtime(&mut runtime, call_api_entry.clone())
            .expect("record outgoing HTTP should succeed");
        effects::record_incoming_api_with_runtime(&mut runtime, incoming_api_entry.clone())
            .expect("record incoming API should succeed");

        assert_eq!(
            runtime.recorded_entries(),
            &[
                RecordingEntry::CallApi(call_api_entry),
                RecordingEntry::IncomingApi(incoming_api_entry),
            ]
        );
    }

    #[test]
    fn record_runtime_enforces_max_entries() {
        let mut runtime = ArtRuntime::recording(session_context(), Some(1));

        effects::uuid_v4_with_runtime(&mut runtime, "first")
            .expect("first entry should be accepted");
        let error = effects::uuid_v4_with_runtime(&mut runtime, "second")
            .expect_err("second entry should exceed max entries");

        assert_eq!(
            error.to_string(),
            "ART recorder reached max entries per session: 1"
        );
    }

    #[test]
    fn recording_settings_can_skip_outgoing_http_entries() {
        let mut runtime = ArtRuntime::recording_with_settings(
            session_context(),
            Some(10),
            ArtRuntimeSettings {
                record_outgoing_http: false,
                ..ArtRuntimeSettings::default()
            },
        );
        let entry = CallApiEntry {
            json_request: HttpRequestEntry {
                get_request_method: "Get".to_string(),
                get_request_headers: HashMap::new(),
                get_request_body: None,
                get_request_url: "https://connector.example/status".to_string(),
                get_request_timeout: None,
                get_request_redirects: None,
            },
            json_result: Either::Right(json!(HttpResponseEntry {
                get_response_body: "e30=".to_string(),
                get_response_code: 200,
                get_response_headers: HashMap::new(),
                get_response_status: "OK".to_string(),
            })),
            api_tag: "PSYNC".to_string(),
        };

        effects::record_outgoing_http_with_runtime(&mut runtime, entry)
            .expect("skipped outgoing HTTP recording should be a no-op");

        assert!(runtime.recorded_entries().is_empty());
    }
}

#[cfg(test)]
mod flush_tests {
    use base64::{engine::general_purpose::STANDARD as BASE64_ENGINE, Engine};
    use serde_json::json;

    use crate::{
        flush::{recording_rows_from_runtime, RecEntryTransform},
        runtime::{ArtRuntime, SessionContext},
        schema::{RecordingEntry, TimestampEntry, UuidEntry},
    };

    fn session_context() -> SessionContext {
        SessionContext {
            request_id: "req_123".to_string(),
            merchant_id: "merchant_123".to_string(),
            connector: "stripe".to_string(),
            flow: "authorize".to_string(),
            hostname: "connector-service".to_string(),
        }
    }

    #[test]
    fn recording_rows_from_runtime_preserves_session_metadata_and_entry_order() {
        let mut runtime = ArtRuntime::recording(session_context(), Some(10));
        runtime
            .record_entry(RecordingEntry::Timestamp(TimestampEntry::new(
                json!("2026-07-08T10:00:00Z"),
                "start_time",
            )))
            .expect("timestamp entry should record");
        runtime
            .record_entry(RecordingEntry::Uuid(UuidEntry::new(
                "uuidV4",
                "uuid-recorded",
                "payment_id",
            )))
            .expect("uuid entry should record");

        let rows =
            recording_rows_from_runtime(&runtime, Some("order_123"), RecEntryTransform::Plain)
                .expect("runtime rows should be built");

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].sess_id, "req_123");
        assert_eq!(rows[0].merch_id, "merchant_123");
        assert_eq!(rows[0].ord_id, "order_123");
        assert_eq!(rows[0].counter, 1);
        assert_eq!(rows[0].val_type, "TIMESTAMP");
        let decoded_rec_entry = BASE64_ENGINE
            .decode(&rows[0].rec_entry)
            .expect("recEntry should be base64");
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&decoded_rec_entry)
                .expect("recEntry should be serialized JSON"),
            json!({
                "tag": "TimeStampEntryT",
                "contents": {
                    "functionName": "getCurrentTime",
                    "timestamp": "2026-07-08T10:00:00Z",
                    "tag": "start_time"
                }
            })
        );

        assert_eq!(rows[1].counter, 2);
        assert_eq!(rows[1].val_type, "UUID");
        assert_eq!(rows[1].ord_id, "order_123");
    }

    #[test]
    fn recording_rows_from_runtime_uses_empty_order_id_when_reference_id_is_missing() {
        let mut runtime = ArtRuntime::recording(session_context(), Some(10));
        runtime
            .record_entry(RecordingEntry::Uuid(UuidEntry::new(
                "uuidV4",
                "uuid-recorded",
                "payment_id",
            )))
            .expect("uuid entry should record");

        let rows = recording_rows_from_runtime(&runtime, None, RecEntryTransform::Plain)
            .expect("runtime rows should be built");

        assert_eq!(rows[0].ord_id, "");
    }

    #[test]
    fn recording_rows_can_encrypt_rec_entry_with_aes_256_cbc() {
        let mut runtime = ArtRuntime::recording(session_context(), Some(10));
        runtime
            .record_entry(RecordingEntry::Uuid(UuidEntry::new(
                "uuidV4",
                "uuid-recorded",
                "payment_id",
            )))
            .expect("uuid entry should record");

        let rows = recording_rows_from_runtime(
            &runtime,
            Some("order_123"),
            RecEntryTransform::Aes256Cbc {
                key: "0123456789abcdef0123456789abcdef",
                iv: "abcdef9876543210",
            },
        )
        .expect("runtime rows should be built");

        assert_ne!(rows[0].rec_entry, "");
        assert_ne!(
            rows[0].rec_entry,
            serde_json::to_string(runtime.recorded_entries().first().expect("entry exists"))
                .expect("entry should serialize")
        );
        assert!(
            rows[0].rec_entry.starts_with("aes256cbc:"),
            "encrypted recEntry should identify its cipher"
        );
    }
}

#[cfg(test)]
mod replay_tests {
    use std::ops::RangeInclusive;

    use serde_json::json;

    use crate::{
        effects,
        replay::ReplayClient,
        runtime::ArtRuntime,
        schema::{RandomBytesEntry, RandomRioEntry, RecordingEntry, TimestampEntry, UuidEntry},
    };

    fn session_context() -> crate::runtime::SessionContext {
        crate::runtime::SessionContext {
            request_id: "req_replay_123".to_string(),
            merchant_id: "merchant_123".to_string(),
            connector: "stripe".to_string(),
            flow: "authorize".to_string(),
            hostname: "connector-service".to_string(),
        }
    }

    #[test]
    fn replay_runtime_returns_recorded_values_without_appending_entries() {
        let mut runtime = ArtRuntime::replay(
            session_context(),
            vec![
                RecordingEntry::Timestamp(TimestampEntry::new(
                    json!("2026-07-07T13:00:00.000Z"),
                    "timestamp",
                )),
                RecordingEntry::Uuid(UuidEntry::new("uuidV4", "recorded-uuid", "payment_id")),
                RecordingEntry::RandomRio(RandomRioEntry::new(
                    json!([10, 20]),
                    json!(17),
                    "amount",
                )),
                RecordingEntry::RandomBytes(RandomBytesEntry::from_bytes(
                    vec![1, 2, 3, 4],
                    "nonce",
                )),
            ],
        );

        assert_eq!(
            effects::now_with_runtime(&mut runtime, "timestamp").expect("timestamp should replay"),
            json!("2026-07-07T13:00:00.000Z")
        );
        assert_eq!(
            effects::uuid_v4_with_runtime(&mut runtime, "payment_id").expect("uuid should replay"),
            "recorded-uuid"
        );
        assert_eq!(
            effects::random_i64_range_with_runtime(
                &mut runtime,
                RangeInclusive::new(10, 20),
                "amount",
            )
            .expect("random value should replay"),
            17
        );
        assert_eq!(
            effects::random_bytes_with_runtime(&mut runtime, 4, "nonce")
                .expect("bytes should replay"),
            vec![1, 2, 3, 4]
        );
        assert!(runtime.recorded_entries().is_empty());
    }

    #[test]
    fn replay_runtime_errors_on_mismatched_entry_type() {
        let mut runtime = ArtRuntime::replay(
            session_context(),
            vec![RecordingEntry::Timestamp(TimestampEntry::new(
                json!("2026-07-07T13:00:00.000Z"),
                "timestamp",
            ))],
        );

        let error = effects::uuid_v4_with_runtime(&mut runtime, "payment_id")
            .expect_err("uuid replay should reject timestamp entry");

        assert_eq!(
            error.to_string(),
            "ART replay expected UuidEntryT entry but found TimeStampEntryT"
        );
    }

    #[test]
    fn replay_client_builds_art_upload_mock_redis_lookup_url() {
        let client = ReplayClient::new("http://localhost:3000");

        let url = client
            .lookup_url("req_replay_123", 4)
            .expect("lookup URL should be valid");

        assert_eq!(
            url.as_str(),
            "http://localhost:3000/mockRedis?guuid=req_replay_123&counter=4"
        );
    }
}

#[cfg(test)]
mod task_local_runtime_tests {
    use crate::{
        effects,
        runtime::{self, ArtMode, ArtRuntime, SessionContext},
        schema::RecordingEntry,
    };

    fn session_context() -> SessionContext {
        SessionContext {
            request_id: "req_task_local".to_string(),
            merchant_id: "merchant_123".to_string(),
            connector: "stripe".to_string(),
            flow: "authorize".to_string(),
            hostname: "connector-service".to_string(),
        }
    }

    #[tokio::test]
    async fn scoped_runtime_records_effects_without_explicit_runtime_argument() {
        let runtime = ArtRuntime::recording(session_context(), Some(10));

        let (uuid, runtime) = runtime::scope(runtime, async {
            effects::uuid_v4("payment_id").expect("scoped uuid generation should succeed")
        })
        .await;

        assert!(!uuid.is_empty());
        assert_eq!(runtime.mode(), ArtMode::Record);
        assert_eq!(
            runtime.session().map(SessionContext::session_id),
            Some("req_task_local")
        );
        assert!(matches!(
            runtime.recorded_entries(),
            [RecordingEntry::Uuid(entry)] if entry.function_name == "uuidV4"
                && entry.uuid == uuid
                && entry.tag.is_none()
        ));
    }

    #[test]
    fn effect_helpers_fall_back_to_disabled_mode_without_scope() {
        let uuid = effects::uuid_v4("payment_id")
            .expect("unscoped uuid generation should still work in disabled mode");

        assert!(!uuid.is_empty());
    }
}
