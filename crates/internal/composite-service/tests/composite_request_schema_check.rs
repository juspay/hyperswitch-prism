#![allow(clippy::expect_used, clippy::panic, clippy::as_conversions)]

use std::collections::{BTreeMap, BTreeSet};

use prost::Message as _;
use prost_types::{
    field_descriptor_proto::Label, DescriptorProto, FieldDescriptorProto, FileDescriptorSet,
};

// Fields intentionally present in granular requests but excluded from composite request.
const DEFAULT_IGNORE_GRANULAR_ONLY_FIELDS: &[&str] = &["connector"];

// Fields intentionally present only in the composite request.
const DEFAULT_IGNORE_COMPOSITE_ONLY_FIELDS: &[&str] = &[];

// Fields present only in composite requests for flows that don't have payment_method in their granular request
const IGNORE_COMPOSITE_ONLY_FIELDS: &[&str] = &["payment_method"];

struct CompositeFlowSpec {
    name: &'static str,
    composite_request_message: &'static str,
    granular_request_messages: &'static [&'static str],
    ignore_granular_only_fields: &'static [&'static str],
    ignore_composite_only_fields: &'static [&'static str],
}

const COMPOSITE_FLOW_SPECS: &[CompositeFlowSpec] = &[
    CompositeFlowSpec {
        name: "authorize",
        composite_request_message: "CompositeAuthorizeRequest",
        granular_request_messages: &[
            "MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest",
            "CustomerServiceCreateRequest",
            "PaymentServiceAuthorizeRequest",
        ],
        ignore_granular_only_fields: DEFAULT_IGNORE_GRANULAR_ONLY_FIELDS,
        ignore_composite_only_fields: DEFAULT_IGNORE_COMPOSITE_ONLY_FIELDS,
    },
    CompositeFlowSpec {
        name: "get",
        composite_request_message: "CompositeGetRequest",
        granular_request_messages: &[
            "MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest",
            "PaymentServiceGetRequest",
        ],
        ignore_granular_only_fields: DEFAULT_IGNORE_GRANULAR_ONLY_FIELDS,
        ignore_composite_only_fields: IGNORE_COMPOSITE_ONLY_FIELDS,
    },
    CompositeFlowSpec {
        name: "refund",
        composite_request_message: "CompositeRefundRequest",
        granular_request_messages: &[
            "MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest",
            "PaymentServiceRefundRequest",
        ],
        ignore_granular_only_fields: DEFAULT_IGNORE_GRANULAR_ONLY_FIELDS,
        ignore_composite_only_fields: IGNORE_COMPOSITE_ONLY_FIELDS,
    },
    CompositeFlowSpec {
        name: "refund_get",
        composite_request_message: "CompositeRefundGetRequest",
        granular_request_messages: &[
            "MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest",
            "RefundServiceGetRequest",
        ],
        ignore_granular_only_fields: DEFAULT_IGNORE_GRANULAR_ONLY_FIELDS,
        ignore_composite_only_fields: IGNORE_COMPOSITE_ONLY_FIELDS,
    },
];

#[derive(Debug, Clone, PartialEq, Eq)]
struct FieldShape {
    type_code: i32,
    type_name: Option<String>,
    repeated: bool,
}

fn decode_descriptor_set() -> FileDescriptorSet {
    FileDescriptorSet::decode(grpc_api_types::FILE_DESCRIPTOR_SET)
        .expect("failed to decode embedded proto descriptor set")
}

fn find_message<'a>(
    descriptor_set: &'a FileDescriptorSet,
    message_name: &str,
) -> &'a DescriptorProto {
    descriptor_set
        .file
        .iter()
        .flat_map(|file| file.message_type.iter())
        .find(|message| message.name.as_deref() == Some(message_name))
        .unwrap_or_else(|| panic!("message descriptor not found: {message_name}"))
}

fn field_name(field: &FieldDescriptorProto) -> String {
    field
        .name
        .clone()
        .unwrap_or_else(|| "<unnamed_field>".to_string())
}

fn field_shape(field: &FieldDescriptorProto) -> FieldShape {
    let repeated = field.label == Some(Label::Repeated as i32);
    FieldShape {
        type_code: field.r#type.unwrap_or_default(),
        type_name: field.type_name.clone(),
        repeated,
    }
}

fn message_field_map(
    message: &DescriptorProto,
    ignored_fields: &BTreeSet<&str>,
) -> BTreeMap<String, FieldShape> {
    message
        .field
        .iter()
        .filter_map(|field| {
            let name = field_name(field);
            if ignored_fields.contains(name.as_str()) {
                None
            } else {
                Some((name, field_shape(field)))
            }
        })
        .collect()
}

fn merge_into_union(
    union_fields: &mut BTreeMap<String, FieldShape>,
    message: &DescriptorProto,
    ignored_fields: &BTreeSet<&str>,
    flow_name: &str,
) {
    for field in &message.field {
        let name = field_name(field);
        if ignored_fields.contains(name.as_str()) {
            continue;
        }

        let shape = field_shape(field);
        match union_fields.get(&name) {
            Some(existing) => {
                assert_eq!(
                    existing, &shape,
                    "[{flow_name}] field '{name}' has conflicting shapes across granular requests"
                );
            }
            None => {
                union_fields.insert(name, shape);
            }
        }
    }
}

fn validate_composite_flow_schema(spec: &CompositeFlowSpec, descriptor_set: &FileDescriptorSet) {
    let composite_message = find_message(descriptor_set, spec.composite_request_message);
    let ignored_granular_only: BTreeSet<&str> =
        spec.ignore_granular_only_fields.iter().copied().collect();
    let ignored_composite_only: BTreeSet<&str> =
        spec.ignore_composite_only_fields.iter().copied().collect();
    let mut granular_union: BTreeMap<String, FieldShape> = BTreeMap::new();
    for granular_message_name in spec.granular_request_messages {
        let granular_message = find_message(descriptor_set, granular_message_name);
        merge_into_union(
            &mut granular_union,
            granular_message,
            &ignored_granular_only,
            spec.name,
        );
    }

    let composite_fields = message_field_map(composite_message, &ignored_composite_only);

    let missing_in_composite: Vec<String> = granular_union
        .keys()
        .filter(|field| !composite_fields.contains_key(*field))
        .cloned()
        .collect();
    let extra_in_composite: Vec<String> = composite_fields
        .keys()
        .filter(|field| !granular_union.contains_key(*field))
        .cloned()
        .collect();

    assert!(
        missing_in_composite.is_empty(),
        "[{}] composite request is missing granular-union fields: {missing_in_composite:?}",
        spec.name
    );
    assert!(
        extra_in_composite.is_empty(),
        "[{}] composite request has extra fields not in granular-union: {extra_in_composite:?}",
        spec.name
    );

    let mismatched_shapes: Vec<String> = granular_union
        .iter()
        .filter_map(|(name, shape)| {
            let composite_shape = composite_fields.get(name)?;
            if composite_shape == shape {
                None
            } else {
                Some(name.clone())
            }
        })
        .collect();

    assert!(
        mismatched_shapes.is_empty(),
        "[{}] composite request has type/count mismatches for fields: {mismatched_shapes:?}",
        spec.name
    );
}

#[test]
fn composite_request_schemas_match_granular_unions() {
    let descriptor_set = decode_descriptor_set();
    for spec in COMPOSITE_FLOW_SPECS {
        validate_composite_flow_schema(spec, &descriptor_set);
    }
}
