#[cfg(test)]
mod tests {
    use hyperswitch_masking::PeekInterface;

    use crate::connectors::tsys_transit::transformers::format_mm_yy;

    #[test]
    fn expiry_is_zero_padded_mm_yy() {
        assert_eq!(format_mm_yy("3", "2025").peek(), "03/25");
        assert_eq!(format_mm_yy("12", "28").peek(), "12/28");
    }
}
