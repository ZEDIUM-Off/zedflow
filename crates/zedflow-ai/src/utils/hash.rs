//! Hash helpers ported from Pi's `packages/ai/src/utils/hash.ts`.

/// Returns Pi's fast deterministic short hash for a string.
///
/// The algorithm matches JavaScript's `Math.imul` and UTF-16 `charCodeAt`
/// behavior so hashes stay stable across the TypeScript and Rust ports.
pub fn short_hash(value: &str) -> String {
    let mut h1 = 0xdead_beefu32;
    let mut h2 = 0x41c6_ce57u32;

    for ch in value.encode_utf16() {
        let ch = u32::from(ch);
        h1 = (h1 ^ ch).wrapping_mul(2_654_435_761);
        h2 = (h2 ^ ch).wrapping_mul(1_597_334_677);
    }

    h1 = (h1 ^ (h1 >> 16)).wrapping_mul(2_246_822_507)
        ^ (h2 ^ (h2 >> 13)).wrapping_mul(3_266_489_909);
    h2 = (h2 ^ (h2 >> 16)).wrapping_mul(2_246_822_507)
        ^ (h1 ^ (h1 >> 13)).wrapping_mul(3_266_489_909);

    format!("{}{}", to_base36(h2), to_base36(h1))
}

fn to_base36(mut value: u32) -> String {
    if value == 0 {
        return "0".to_owned();
    }

    let mut chars = Vec::new();
    while value > 0 {
        let digit = (value % 36) as u8;
        let ch = if digit < 10 {
            b'0' + digit
        } else {
            b'a' + (digit - 10)
        };
        chars.push(char::from(ch));
        value /= 36;
    }
    chars.iter().rev().collect()
}

#[cfg(test)]
mod tests {
    use super::short_hash;

    #[test]
    fn matches_pi_fixtures() {
        assert_eq!(short_hash(""), "k4n83c7h0j2b");
        assert_eq!(short_hash("hello"), "1h6qa0qrowduu");
        assert_eq!(short_hash("hello world"), "n7rb4n1m39uz8");
        assert_eq!(short_hash("é"), "1ohi2dgmq6lzp");
        assert_eq!(short_hash("💩"), "k8rlr19znth3");
    }
}
