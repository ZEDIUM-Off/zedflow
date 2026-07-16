use zedflow_agent::harness::system_prompt::format_skills_for_system_prompt;
use zedflow_agent::harness::types::Skill;

fn skill(name: &str, description: &str, content: &str, file_path: &str, disabled: bool) -> Skill {
    Skill {
        name: name.to_string(),
        description: description.to_string(),
        content: content.to_string(),
        file_path: file_path.to_string(),
        disable_model_invocation: Some(disabled),
    }
}

#[test]
fn formats_visible_skills_in_order_and_skips_model_disabled_skills() {
    let visible = skill(
        "visible",
        "Use <this> & that",
        "visible content",
        "/skills/visible/SKILL.md",
        false,
    );
    let second = skill(
        "second",
        "Second skill",
        "second content",
        "/skills/second/SKILL.md",
        false,
    );
    let disabled = skill(
        "hidden",
        "Hidden",
        "hidden content",
        "/skills/hidden/SKILL.md",
        true,
    );

    assert_eq!(
        format_skills_for_system_prompt(&[visible, disabled, second]),
        "The following skills provide specialized instructions for specific tasks.\nRead the full skill file when the task matches its description.\nWhen a skill file references a relative path, resolve it against the skill directory (parent of SKILL.md / dirname of the path) and use that absolute path in tool commands.\n\n<available_skills>\n  <skill>\n    <name>visible</name>\n    <description>Use &lt;this&gt; &amp; that</description>\n    <location>/skills/visible/SKILL.md</location>\n  </skill>\n  <skill>\n    <name>second</name>\n    <description>Second skill</description>\n    <location>/skills/second/SKILL.md</location>\n  </skill>\n</available_skills>"
    );
}

#[test]
fn returns_empty_string_when_no_skills_are_model_visible() {
    let disabled = skill(
        "hidden",
        "Hidden",
        "hidden content",
        "/skills/hidden/SKILL.md",
        true,
    );

    assert_eq!(format_skills_for_system_prompt(&[disabled]), "");
}

#[test]
fn escapes_xml_in_all_model_visible_skill_fields() {
    let escaped = format_skills_for_system_prompt(&[skill(
        "a&b",
        "Quote \"double\" and 'single'",
        "content",
        "/skills/<bad>&\"quote\"/SKILL.md",
        false,
    )]);

    assert!(escaped.contains(
        "<name>a&amp;b</name>\n    <description>Quote &quot;double&quot; and &apos;single&apos;</description>\n    <location>/skills/&lt;bad&gt;&amp;&quot;quote&quot;/SKILL.md</location>"
    ));
}
