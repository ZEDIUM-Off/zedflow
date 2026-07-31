use std::fs;
use zedflow_coding_agent::skills::{LoadSkillsFromDirOptions, load_skills_from_dir};

#[test]
fn skill_loader_reads_frontmatter_from_a_skill_file() {
    let root = std::env::temp_dir().join(format!("zedflow-suite-skill-{}", std::process::id()));
    let skill = root.join("demo/SKILL.md");
    fs::create_dir_all(skill.parent().unwrap()).unwrap();
    fs::write(&skill, "---\nname: demo\ndescription: test\n---\nbody").unwrap();
    let result = load_skills_from_dir(LoadSkillsFromDirOptions {
        dir: root.display().to_string(),
        source: "user".into(),
    });
    assert_eq!(result.skills[0].name, "demo");
    fs::remove_dir_all(root).unwrap();
}
