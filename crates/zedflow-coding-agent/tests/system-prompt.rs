#[test]
fn system_prompt_includes_custom_tool_and_guideline_once() {
    let mut snippets = std::collections::HashMap::new();
    snippets.insert("read".into(), "Read files".into());
    let prompt = zedflow_coding_agent::system_prompt::build_system_prompt(
        &zedflow_coding_agent::system_prompt::BuildSystemPromptOptions {
            selected_tools: Some(vec!["read".into()]),
            tool_snippets: Some(snippets),
            prompt_guidelines: vec!["Be safe".into(), " Be safe ".into()],
            ..Default::default()
        },
    );
    assert!(prompt.contains("- read: Read files"));
    assert_eq!(prompt.matches("- Be safe").count(), 1);
}
