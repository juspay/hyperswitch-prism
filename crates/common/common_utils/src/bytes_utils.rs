//! Small helpers over raw byte slices.

const UTF8_BOM: &[u8] = &[0xEF, 0xBB, 0xBF];

/// Strip any leading UTF-8 BOMs from `bytes`.
///
/// Several gateways (Authorize.Net among them) prefix responses with a BOM, which every body
/// parser rejects. Takes bytes rather than a decoded string so callers can strip before deciding
/// whether the body is even UTF-8. Repeated BOMs are all removed.
pub fn strip_utf8_bom(bytes: &[u8]) -> &[u8] {
    let mut rest = bytes;
    while let Some(stripped) = rest.strip_prefix(UTF8_BOM) {
        rest = stripped;
    }
    rest
}
