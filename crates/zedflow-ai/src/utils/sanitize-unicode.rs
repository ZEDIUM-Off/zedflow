//! Unicode sanitization helpers ported from Pi's `packages/ai/src/utils/sanitize-unicode.ts`.

const HIGH_SURROGATE_START: u16 = 0xd800;
const HIGH_SURROGATE_END: u16 = 0xdbff;
const LOW_SURROGATE_START: u16 = 0xdc00;
const LOW_SURROGATE_END: u16 = 0xdfff;

/// Removes unpaired Unicode surrogate code units from text.
///
/// Pi operates on JavaScript UTF-16 strings, where lone surrogate code units can
/// exist. Rust `&str` values are valid Unicode scalar values, so callers cannot
/// pass lone surrogates directly; this function still round-trips through UTF-16
/// code units to preserve Pi's behavior for all representable text.
#[must_use]
pub fn sanitize_surrogates(text: &str) -> String {
    let code_units = text.encode_utf16().collect::<Vec<_>>();
    sanitize_utf16_code_units(&code_units)
}

fn sanitize_utf16_code_units(code_units: &[u16]) -> String {
    let mut sanitized = String::with_capacity(code_units.len());
    let mut index = 0;

    while let Some(&unit) = code_units.get(index) {
        if (HIGH_SURROGATE_START..=HIGH_SURROGATE_END).contains(&unit) {
            let Some(&next_unit) = code_units.get(index + 1) else {
                index += 1;
                continue;
            };

            if (LOW_SURROGATE_START..=LOW_SURROGATE_END).contains(&next_unit) {
                let high = u32::from(unit - HIGH_SURROGATE_START);
                let low = u32::from(next_unit - LOW_SURROGATE_START);
                let scalar = 0x1_0000 + ((high << 10) | low);
                if let Some(ch) = char::from_u32(scalar) {
                    sanitized.push(ch);
                }
                index += 2;
                continue;
            }

            index += 1;
            continue;
        }

        if (LOW_SURROGATE_START..=LOW_SURROGATE_END).contains(&unit) {
            index += 1;
            continue;
        }

        if let Some(ch) = char::from_u32(u32::from(unit)) {
            sanitized.push(ch);
        }
        index += 1;
    }

    sanitized
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_valid_unicode() {
        assert_eq!(sanitize_surrogates("Hello 🙈 World"), "Hello 🙈 World");
    }

    #[test]
    fn removes_unpaired_surrogates_from_utf16_units() {
        let high = 0xd83d;
        let monkey = [0xd83d, 0xde48];
        let low = 0xde48;

        assert_eq!(
            sanitize_utf16_code_units(&[b'T'.into(), high, b'!'.into()]),
            "T!"
        );
        assert_eq!(
            sanitize_utf16_code_units(&[b'T'.into(), low, b'!'.into()]),
            "T!"
        );
        assert_eq!(
            sanitize_utf16_code_units(&[high, monkey[0], monkey[1], low]),
            "🙈"
        );
    }
}
