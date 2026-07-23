use zedflow_coding_agent::core::{
    source_info::SourceOrigin, source_info::SourceScope, source_info::create_synthetic_source_info,
};
use zedflow_coding_agent::{
    export_html::ansi_to_html,
    skills::{Skill, format_skills_for_prompt},
};

#[test]
fn resource_prompt_and_ansi_conversion_keep_trust_boundaries() {
    let skill = Skill {
        name: "safe-skill".into(),
        description: "Use <only> when needed".into(),
        file_path: "/tmp/SKILL.md".into(),
        base_dir: "/tmp".into(),
        source_info: create_synthetic_source_info(
            "/tmp/SKILL.md",
            "test",
            Some(SourceScope::Temporary),
            Some(SourceOrigin::TopLevel),
            None,
        ),
        disable_model_invocation: false,
    };
    let prompt = format_skills_for_prompt(&[skill]);
    assert!(prompt.contains("Use &lt;only&gt; when needed"));
    assert_eq!(
        ansi_to_html("\x1b[31mred\x1b[0m"),
        "<span style=\"color:#800000\">red</span>"
    );
}
