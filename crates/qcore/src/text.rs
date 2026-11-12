//! Quake text uses byte indices into conchars, not UTF-8. In string-based
//! parsers/APIs carry each byte as the same-valued Unicode scalar (Latin-1).
//! Encode it back before handing it to the VM, network, font or save file.

pub fn decode(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| char::from(b)).collect()
}

pub fn glyphs(text: &str) -> impl Iterator<Item = u8> + '_ {
    text.chars().map(|c| u8::try_from(c as u32).unwrap_or(b'?'))
}

pub fn encode(text: &str) -> Vec<u8> {
    glyphs(text).collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_font_indices_roundtrip_even_when_the_bytes_form_valid_utf8() {
        let bytes: Vec<u8> = (0..=255).collect();
        assert_eq!(super::encode(&super::decode(&bytes)), bytes);
        assert_eq!(super::decode(&[0xc3, 0xa9]).chars().count(), 2);
    }
}
