/// A decoded HTML entity and its consumed byte length.
#[derive(Debug, PartialEq, Eq)]
pub struct DecodedHtmlEntity {
    pub text: String,
    pub length: usize,
}

/// Decodes Pi's supported named and numeric HTML entities.
#[must_use]
pub fn decode_html_entity(entity: &str) -> Option<String> {
    let named = match entity {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        _ => None,
    };
    named
        .or_else(|| {
            let (digits, radix) = if let Some(digits) = entity
                .strip_prefix("#x")
                .or_else(|| entity.strip_prefix("#X"))
            {
                (digits, 16)
            } else {
                (entity.strip_prefix('#')?, 10)
            };
            char::from_u32(u32::from_str_radix(digits, radix).ok()?)
        })
        .map(|character| character.to_string())
}

/// Decodes an entity beginning at the byte index of `&`.
#[must_use]
pub fn decode_html_entity_at(html: &str, index: usize) -> Option<DecodedHtmlEntity> {
    if html.as_bytes().get(index) != Some(&b'&') {
        return None;
    }
    let relative_end = html.get(index + 1..)?.find(';')?;
    let end = index + 1 + relative_end;
    if end - index > 16 {
        return None;
    }
    Some(DecodedHtmlEntity {
        text: decode_html_entity(&html[index + 1..end])?,
        length: end - index + 1,
    })
}
