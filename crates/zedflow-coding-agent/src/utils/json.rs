/// Strips `//` comments and trailing commas while preserving JSON strings.
#[must_use]
pub fn strip_json_comments(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut output = Vec::with_capacity(input.len());
    let mut index = 0;
    let mut in_string = false;
    while index < bytes.len() {
        if in_string {
            output.push(bytes[index]);
            if bytes[index] == b'\\' && index + 1 < bytes.len() {
                index += 1;
                output.push(bytes[index]);
            } else if bytes[index] == b'"' {
                in_string = false;
            }
        } else if bytes[index] == b'"' {
            in_string = true;
            output.push(b'"');
        } else if bytes[index..].starts_with(b"//") {
            index += 2;
            while index < bytes.len() && bytes[index] != b'\n' {
                index += 1;
            }
            continue;
        } else if bytes[index] == b',' {
            let mut next = index + 1;
            while next < bytes.len() && bytes[next].is_ascii_whitespace() {
                next += 1;
            }
            if matches!(bytes.get(next), Some(b'}' | b']')) {
                index += 1;
                continue;
            }
            output.push(b',');
        } else {
            output.push(bytes[index]);
        }
        index += 1;
    }
    String::from_utf8(output).expect("stripping JSON preserves UTF-8")
}
