use zedflow_coding_agent::prompt_templates::expand_prompt_template;
#[test]
fn extension_style_prompt_expands_arguments() {
    let template = zedflow_coding_agent::prompt_templates::PromptTemplate {
        name: "plan".into(),
        description: String::new(),
        argument_hint: None,
        content: "Plan $1".into(),
        source_info: zedflow_coding_agent::source_info::create_synthetic_source_info(
            "", "", None, None, None,
        ),
        file_path: String::new(),
    };
    assert_eq!(
        expand_prompt_template("/plan task", &[template]),
        "Plan task"
    );
}
