use qformats::{Bsp, bsp, parse_entities};

#[test]
fn bsp_entity_text_preserves_font_bytes() {
    let entities = b"{\n\"classname\" \"worldspawn\"\n\"message\" \"\xa0\xd0\xe5\xee\xe4\xf2\xe1\xe7\xef\xee\xa0\\n\x9e\x9d\x9d\xc3\xa9\"\n}\0";
    let header = 4 + bsp::HEADER_LUMPS * 8;
    let mut data = 29i32.to_le_bytes().to_vec();
    for i in 0..bsp::HEADER_LUMPS {
        data.extend_from_slice(&(header as i32).to_le_bytes());
        let len = if i == bsp::LUMP_ENTITIES { entities.len() as i32 } else { 0 };
        data.extend_from_slice(&len.to_le_bytes());
    }
    data.extend_from_slice(entities);
    let bsp = Bsp::parse(&data).unwrap();
    assert_eq!(qcore::text::encode(&bsp.entities), &entities[..entities.len() - 1]);
    let parsed = parse_entities(&bsp.entities).unwrap();
    assert_eq!(qcore::text::encode(parsed[0].get("message").unwrap()),
        b"\xa0\xd0\xe5\xee\xe4\xf2\xe1\xe7\xef\xee\xa0\\n\x9e\x9d\x9d\xc3\xa9");
}
