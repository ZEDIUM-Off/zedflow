use zedflow_agent::harness::prompt_templates::format_prompt_template_invocation;
use zedflow_agent::harness::skills::format_skill_invocation;
use zedflow_agent::harness::types::{PromptTemplate, Skill};

#[test]
fn formats_skill_invocations_with_additional_instructions() {
    let skill = Skill {
        name: "inspect".to_string(),
        description: "Inspect things".to_string(),
        content: "Use inspection tools.".to_string(),
        file_path: "/project/.pi/skills/inspect/SKILL.md".to_string(),
        disable_model_invocation: None,
    };

    assert_eq!(
        format_skill_invocation(&skill, Some("Check errors.")),
        "<skill name=\"inspect\" location=\"/project/.pi/skills/inspect/SKILL.md\">\nReferences are relative to /project/.pi/skills/inspect.\n\nUse inspection tools.\n</skill>\n\nCheck errors."
    );
}

#[test]
fn formats_prompt_template_invocations_with_positional_arguments() {
    let template = PromptTemplate {
        name: "review".to_string(),
        description: None,
        content: "Review $1 with $ARGUMENTS".to_string(),
    };

    assert_eq!(
        format_prompt_template_invocation(&template, &["a.ts".to_string(), "care".to_string()]),
        "Review a.ts with a.ts care"
    );
}
