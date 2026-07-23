/// Returns the EXIF orientation value (1 when absent or malformed).
pub fn get_exif_orientation(bytes: &[u8]) -> u16 {
    if bytes.len() < 2 || bytes[0] != 0xff || bytes[1] != 0xd8 {
        return 1;
    }
    let mut p = 2;
    while p + 4 <= bytes.len() {
        if bytes[p] != 0xff {
            break;
        }
        let marker = bytes[p + 1];
        let len = u16::from_be_bytes([bytes[p + 2], bytes[p + 3]]) as usize;
        if marker == 0xe1 && p + 10 <= bytes.len() && &bytes[p + 4..p + 10] == b"Exif\0\0" {
            return tiff_orientation(&bytes[p + 10..p + 4 + len]);
        }
        if len < 2 {
            break;
        }
        p += 2 + len;
    }
    1
}
fn tiff_orientation(b: &[u8]) -> u16 {
    if b.len() < 10 {
        return 1;
    };
    let le = &b[..2] == b"II";
    let u16r = |x: &[u8]| {
        if le {
            u16::from_le_bytes([x[0], x[1]])
        } else {
            u16::from_be_bytes([x[0], x[1]])
        }
    };
    let u32r = |x: &[u8]| {
        if le {
            u32::from_le_bytes(x.try_into().unwrap())
        } else {
            u32::from_be_bytes(x.try_into().unwrap())
        }
    };
    let off = u32r(&b[4..8]) as usize;
    if off + 2 > b.len() {
        return 1;
    };
    let n = u16r(&b[off..off + 2]) as usize;
    for i in 0..n {
        let p = off + 2 + i * 12;
        if p + 12 > b.len() {
            break;
        }
        if u16r(&b[p..p + 2]) == 0x112 {
            let v = u16r(&b[p + 8..p + 10]);
            return if (1..=8).contains(&v) { v } else { 1 };
        }
    }
    1
}
