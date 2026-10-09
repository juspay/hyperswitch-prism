import copy
import json
from pathlib import Path
import unittest

import generate


class MandateReferenceRenderingTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        root = Path(__file__).resolve().parents[3]
        generate.load_proto_type_map(root / "crates/types-traits/grpc-api-types/proto")

    def setUp(self):
        self.metadata = '{"card_network":"Visa","expiration_date":"3003"}'
        self.reference = {
            "connector_mandate_id": "stored-token",
            "mandate_metadata": self.metadata,
        }
        self.payload = {
            "connector_recurring_payment_id": {
                "mandate_id_type": {"connector_mandate_id": self.reference}
            }
        }

    def test_typescript_preserves_nested_mandate_and_secret_metadata(self):
        lines = generate._annotate_inline_lines(
            self.payload,
            "RecurringPaymentServiceChargeRequest",
            generate._SchemaDB({}),
            1,
            "//",
            camel_keys=True,
            ts_mode=True,
        )
        rendered = json.loads("{\n" + "\n".join(lines) + "\n}")
        reference = rendered["connectorRecurringPaymentId"]["connectorMandateId"]
        self.assertEqual(reference["connectorMandateId"], "stored-token")
        self.assertEqual(reference["mandateMetadata"], {"value": self.metadata})

    def test_rust_uses_mandate_reference_oneof_and_preserves_metadata(self):
        rendered = "\n".join(generate._rust_struct_lines(
            self.payload, "RecurringPaymentServiceChargeRequest", {}, 1
        ))
        self.assertIn(
            "mandate_reference::MandateIdType::ConnectorMandateId(ConnectorMandateReferenceId",
            rendered,
        )
        self.assertIn('connector_mandate_id: Some("stored-token".to_string())', rendered)
        self.assertIn(
            f"mandate_metadata: Some(Secret::new({json.dumps(self.metadata)}.to_string()))",
            rendered,
        )
        flattened = {"connector_recurring_payment_id": {"connector_mandate_id": self.reference}}
        self.assertEqual(rendered, "\n".join(generate._rust_struct_lines(
            flattened, "RecurringPaymentServiceChargeRequest", {}, 1
        )))

    def test_kotlin_flattens_only_the_oneof_wrapper(self):
        original = copy.deepcopy(self.payload)
        processed = generate._preprocess_kt_payload("recurring_charge", self.payload)
        self.assertEqual(self.payload, original)
        self.assertEqual(processed["connector_recurring_payment_id"], {
            "connector_mandate_id": self.reference
        })
        rendered = "\n".join(generate._kotlin_payload_lines(
            processed, "RecurringPaymentServiceChargeRequest", {}, 1
        ))
        self.assertIn('connectorMandateId = "stored-token"', rendered)
        self.assertIn(f"mandateMetadataBuilder.value = {json.dumps(self.metadata)}", rendered)

    def test_python_uses_proto_fields_instead_of_the_oneof_group_name(self):
        rendered = "\n".join(generate._py_direct_lines(
            self.payload, "RecurringPaymentServiceChargeRequest", generate._SchemaDB({}), 1
        ))
        self.assertNotIn("mandate_id_type=", rendered)
        self.assertIn("connector_mandate_id=payment_pb2.ConnectorMandateReferenceId(", rendered)
        self.assertIn('connector_mandate_id="stored-token"', rendered)
        self.assertIn(
            f"mandate_metadata=payment_methods_pb2.SecretString(value={json.dumps(self.metadata)})",
            rendered,
        )

    def test_rust_json_does_not_duplicate_the_oneof_wrapper(self):
        rendered = "\n".join(generate._rust_json_lines(
            self.payload, "RecurringPaymentServiceChargeRequest", {}, 1
        ))
        self.assertEqual(rendered.count('"mandate_id_type"'), 1)
        self.assertIn('"connector_mandate_id": "stored-token"', rendered)

    def test_typescript_still_filters_unknown_fields(self):
        lines = generate._annotate_inline_lines(
            {"unknown_field": "ignored"}, "MandateReference",
            generate._SchemaDB({}), 1, "//", camel_keys=True, ts_mode=True,
        )
        self.assertEqual(lines, [])


if __name__ == "__main__":
    unittest.main()
