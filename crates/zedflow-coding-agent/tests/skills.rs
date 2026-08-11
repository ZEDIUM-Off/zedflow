use std::fs;
use zedflow_coding_agent::skills::{
    LoadSkillsFromDirOptions, format_skills_for_prompt, load_skills_from_dir,
};
#[test]
fn loads_valid_skill_and_escapes_prompt_xml() {
    let root = std::env::temp_dir().join(format!("zedflow-skill-{}", std::process::id()));
    let skill = root.join("demo/SKILL.md");
    fs::create_dir_all(skill.parent().unwrap()).unwrap();
    fs::write(&skill, "---\nname: demo\ndescription: x & y\n---\nbody").unwrap();
    let result = load_skills_from_dir(LoadSkillsFromDirOptions {
        dir: root.display().to_string(),
        source: "user".into(),
    });
    assert_eq!(result.skills.len(), 1);
    assert!(format_skills_for_prompt(&result.skills).contains("x &amp; y"));
    let _ = fs::remove_dir_all(root);
}
