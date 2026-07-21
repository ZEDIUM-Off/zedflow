use serde_yaml::{Mapping, Value};

/// Parsed YAML frontmatter and the remaining body.
#[derive(Debug, PartialEq)]
pub struct ParsedFrontmatter {
    pub frontmatter: Mapping,
    pub body: String,
}

/// Parses leading `---` YAML frontmatter, normalizing newlines like Pi.
pub fn parse_frontmatter(content: &str) -> Result<ParsedFrontmatter, serde_yaml::Error> {
    let normalized = content.replace("\r\n", "\n").replace('\r', "\n");
    let Some(rest) = normalized.strip_prefix("---") else {
        return Ok(ParsedFrontmatter {
            frontmatter: Mapping::new(),
            body: normalized,
        });
    };
    let Some(end) = rest.find("\n---") else {
        return Ok(ParsedFrontmatter {
            frontmatter: Mapping::new(),
            body: normalized,
        });
    };
    let yaml = format!("{}\n", &rest[1..end]);
    let value: Value = serde_yaml::from_str(&yaml)?;
    let frontmatter = match value {
        Value::Null => Mapping::new(),
        Value::Mapping(mapping) => mapping,
        other => serde_yaml::from_value(other)?,
    };
    Ok(ParsedFrontmatter {
        frontmatter,
        body: rest[end + 4..].trim().to_owned(),
    })
}

/// Removes leading YAML frontmatter when present.
pub fn strip_frontmatter(content: &str) -> Result<String, serde_yaml::Error> {
    Ok(parse_frontmatter(content)?.body)
}
