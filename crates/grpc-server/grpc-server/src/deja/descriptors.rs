//! Decode captured gRPC wire messages to proto3-JSON using the server's own emitted
//! descriptor set (`grpc_api_types::FILE_DESCRIPTOR_SET`). Because UCS owns its protos,
//! ingress events are recorded decoded — no descriptor-set build step is needed here.
//!
//! A decode miss (unknown rpc, non-unary body, or malformed framing) returns `None`;
//! callers fall back to raw-bytes identity. Ported from hyperswitch's client-side gRPC
//! boundary (`external_services::grpc_client::semantic_boundary`), specialised to the
//! single flat descriptor set UCS emits.

use std::sync::OnceLock;

use prost_reflect::{DescriptorPool, DynamicMessage, MessageDescriptor};

static POOL: OnceLock<Option<DescriptorPool>> = OnceLock::new();

/// The descriptor pool, built once from the emitted `FILE_DESCRIPTOR_SET`.
/// `None` if the set fails to decode (should never happen; the boundary degrades to
/// raw-bytes identity rather than failing a request).
pub fn pool() -> Option<&'static DescriptorPool> {
    POOL.get_or_init(|| DescriptorPool::decode(grpc_api_types::FILE_DESCRIPTOR_SET).ok())
        .as_ref()
}

/// Strip gRPC length-prefix framing — repeated `[compressed: u8][len: u32 BE][payload]`.
/// Returns each message payload, or `None` on a set compression flag or malformed framing
/// (UCS gRPC never negotiates compression, so a compressed frame is treated as opaque).
pub fn unframe_messages(body: &[u8]) -> Option<Vec<&[u8]>> {
    let mut out = Vec::new();
    let mut rest = body;
    while !rest.is_empty() {
        let (head, tail) = rest.split_at_checked(5)?;
        if *head.first()? != 0 {
            return None;
        }
        let len_bytes: [u8; 4] = head.get(1..5)?.try_into().ok()?;
        let len = usize::try_from(u32::from_be_bytes(len_bytes)).ok()?;
        let (message, remaining) = tail.split_at_checked(len)?;
        out.push(message);
        rest = remaining;
    }
    Some(out)
}

/// `(input, output)` message descriptors for an rpc path `/package.Service/Method`.
pub fn method_descriptors(rpc: &str) -> Option<(MessageDescriptor, MessageDescriptor)> {
    let (service, method) = rpc.strip_prefix('/')?.split_once('/')?;
    let pool = pool()?;
    let service = pool.services().find(|candidate| candidate.full_name() == service)?;
    let method = service.methods().find(|candidate| candidate.name() == method)?;
    Some((method.input(), method.output()))
}

/// Proto3-JSON projection of one wire message.
pub fn decode_to_json(descriptor: &MessageDescriptor, message: &[u8]) -> Option<serde_json::Value> {
    let decoded = DynamicMessage::decode(descriptor.clone(), message).ok()?;
    serde_json::to_value(&decoded).ok()
}

/// Decode a single-message (unary) request body into proto3-JSON via the rpc's input
/// descriptor. `None` = unknown schema, non-unary body, or malformed framing.
pub fn decode_unary_request(rpc: &str, request_body: &[u8]) -> Option<serde_json::Value> {
    let (input, _) = method_descriptors(rpc)?;
    let messages = unframe_messages(request_body)?;
    let (message, rest) = messages.split_first()?;
    if !rest.is_empty() {
        return None;
    }
    decode_to_json(&input, message)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_pool_loads_and_indexes_a_method() {
        // Proves the decode path is wired to the real emitted protos: the descriptor set
        // decodes, and a path built from the pool resolves back to input/output descriptors.
        // This is where a prost-reflect / prost version mismatch would surface.
        let pool = pool().expect("FILE_DESCRIPTOR_SET decodes into a DescriptorPool");
        let service = pool
            .services()
            .next()
            .expect("at least one gRPC service in the descriptor set");
        let method = service
            .methods()
            .next()
            .expect("the service has at least one method");
        let rpc = format!("/{}/{}", service.full_name(), method.name());
        let (input, output) =
            method_descriptors(&rpc).expect("method_descriptors resolves a pool-derived path");
        assert_eq!(input.full_name(), method.input().full_name());
        assert_eq!(output.full_name(), method.output().full_name());
    }

    #[test]
    fn unframe_handles_framing_and_rejects_compression() {
        // Compressed flag => opaque (None).
        assert!(unframe_messages(&[1, 0, 0, 0, 0]).is_none());
        // One empty message: [flag=0][len=0].
        assert_eq!(unframe_messages(&[0, 0, 0, 0, 0]), Some(vec![&[][..]]));
        // One 3-byte message.
        assert_eq!(
            unframe_messages(&[0, 0, 0, 0, 3, 9, 8, 7]),
            Some(vec![&[9u8, 8, 7][..]])
        );
        // Truncated length => None (malformed).
        assert!(unframe_messages(&[0, 0, 0, 0, 5, 1, 2]).is_none());
    }
}
