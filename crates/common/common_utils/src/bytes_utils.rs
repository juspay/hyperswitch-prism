const UTF8_BOM: &[u8] = &[0xEF, 0xBB, 0xBF];

pub fn strip_utf8_bom(bytes: &[u8]) -> &[u8] {
    let mut rest = bytes;
    while let Some(stripped) = rest.strip_prefix(UTF8_BOM) {
        rest = stripped;
    }
    rest
}
