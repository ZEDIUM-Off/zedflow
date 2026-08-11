use std::fs;

use zedflow_coding_agent::{
    resource_loader::DefaultResourceLoader,
    system_prompt::{BuildSystemPromptOptions, build_system_prompt},
};

#[test]
fn sdk_discovers_default_skills_and_injects_them_into_read_prompts() {
    let root = std::env::temp_dir().join(format!("zedflow-sdk-skills-{}", std::process::id()));
    let skill = root.join("skills/test-skill/SKILL.md");
    fs::create_dir_all(skill.parent().unwrap()).unwrap();
    fs::write(
        &skill,
        "---\nname: test-skill\ndescription: SDK test skill.\n---\n# Test Skill\n",
    )
    .unwrap();

    let mut loader = DefaultResourceLoader::new(&root, &root);
    loader.reload();
    let skills = loader.get_skills().skills.clone();
    assert!(skills.iter().any(|skill| skill.name == "test-skill"));

    let prompt = build_system_prompt(&BuildSystemPromptOptions {
        selected_tools: Some(vec!["read".into()]),
        skills,
        ..Default::default()
    });
    assert!(prompt.contains("<name>test-skill</name>"));
    assert!(prompt.contains(&skill.display().to_string()));
    let _ = fs::remove_dir_all(root);
}
