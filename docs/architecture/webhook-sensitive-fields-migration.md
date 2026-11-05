# Webhook and redirect sensitive-field migration

Deploy the updated UCS server before migrating callers. Existing protobuf field
numbers, scalar types, names and HTTP JSON representations remain accepted.
Deprecated fields are masked in audits during migration, including when nested
inside composite requests. Debug formatting also hides their values.

| Existing field | Replacement |
| --- | --- |
| `WebhookSecrets.secret` | `signing_secret` |
| `WebhookSecrets.additional_secret` | `additional_signing_secret` |
| `RedirectResponseSecrets.secret` | `verification_secret` |
| `RedirectResponseSecrets.additional_secret` | `additional_verification_secret` |
| `RequestDetails.body` | `sensitive_body` |
| `RequestDetails.headers` | `sensitive_headers.values` |
| `RequestDetails.uri` | `sensitive_uri` |
| `RequestDetails.query_params` | `sensitive_query_params` |

New fields take precedence by presence, even if empty. Callers should populate
only the new fields after migration. The header message distinguishes omitted
headers from an explicitly empty map. No field-name scanning or JSON rewriting
is performed by the audit logger.

The g2h HTTP endpoint uses Rust Serde, whose Secret wrappers accept the original
inner JSON value. A migrated HTTP fragment looks like:

```json
{
  "webhook_secrets": {"signing_secret": "synthetic-signing-secret"},
  "request_details": {
    "method": "HTTP_METHOD_POST",
    "sensitive_body": [0, 255, 128],
    "sensitive_headers": {"values": {"x-signature": "synthetic-signature"}},
    "sensitive_uri": "/callback",
    "sensitive_query_params": ""
  }
}
```

Standard protobuf JSON (including grpcurl) represents secret wrapper messages
as objects: `"signingSecret": {"value": "synthetic-signing-secret"}` and
`"sensitiveBody": {"value": "AP+A"}` (base64). Header values likewise use
`{"value": "..."}`. Generated binary clients use each wrapper's `value` field.
These are distinct JSON conventions; the existing legacy JSON remains valid
in both interfaces.

Raw body bytes, URI spelling/escaping and header/query values reach connector
verification unchanged. Audits retain the method and header names but mask
values, bodies and raw URIs/query strings. This includes webhook acknowledgement
body/header audit serialization; actual acknowledgement data is unchanged.

Rollback order matters: an old server ignores the new binary fields and cannot
use them. Revert clients to the legacy fields before rolling back UCS. Do not
remove legacy fields until every caller and supported rollback version has
migrated. Removal must be a separate PR reserving the retired field numbers and
names.
