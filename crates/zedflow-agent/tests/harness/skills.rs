use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use futures::executor::block_on;
use zedflow_agent::harness::env::nodejs::NodeExecutionEnv;
use zedflow_agent::harness::skills::{SkillDiagnosticCode, load_skills, load_sourced_skills};
use zedflow_agent::harness::types::Skill;

#[derive(Debug)]
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock before unix epoch")
            .as_nanos();
        path.push(format!(
            "zedflow-agent-skills-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create temp dir");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    fn join(&self, path: &str) -> PathBuf {
        self.0.join(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Source {
    kind: &'static str,
}

fn env(root: &TempDir) -> NodeExecutionEnv {
    NodeExecutionEnv::with_cwd(root.path().to_string_lossy().into_owned())
}

fn skill(path: PathBuf, name: &str, description: &str, content: &str, disabled: bool) -> Skill {
    Skill {
        name: name.to_string(),
        description: description.to_string(),
        content: content.to_string(),
        file_path: path.to_string_lossy().into_owned(),
        disable_model_invocation: Some(disabled),
    }
}

#[test]
fn loads_skill_md_files_through_the_execution_environment() {
    let root = TempDir::new();
    fs::create_dir_all(root.join(".agents/skills/example")).expect("create skill dir");
    fs::write(
        root.join(".agents/skills/example/SKILL.md"),
        "---\nname: example\ndescription: Example skill\ndisable-model-invocation: true\n---\nUse this skill.\n",
    )
    .expect("write skill");

    let result = block_on(load_skills(&env(&root), [".agents/skills"]));

    assert_eq!(result.diagnostics, []);
    assert_eq!(
        result.skills,
        vec![skill(
            root.join(".agents/skills/example/SKILL.md"),
            "example",
            "Example skill",
            "Use this skill.",
            true,
        )]
    );
}

#[cfg(unix)]
#[test]
fn loads_skills_through_symlinked_directories() {
    use std::os::unix::fs::symlink;

    let root = TempDir::new();
    fs::create_dir_all(root.join("actual/example")).expect("create actual/example");
    fs::write(
        root.join("actual/example/SKILL.md"),
        "---\nname: example\ndescription: Example skill\n---\nUse this skill.",
    )
    .expect("write skill");
    symlink(root.join("actual"), root.join("skills-link")).expect("symlink skills dir");

    let result = block_on(load_skills(&env(&root), ["skills-link"]));

    assert_eq!(
        result
            .skills
            .iter()
            .map(|skill| skill.name.as_str())
            .collect::<Vec<_>>(),
        vec!["example"]
    );
    assert_eq!(
        result.skills[0].file_path,
        root.join("skills-link/example/SKILL.md").to_string_lossy()
    );
}

#[test]
fn preserves_source_info_for_sourced_skills() {
    let root = TempDir::new();
    fs::create_dir_all(root.join("user/example")).expect("create user/example");
    fs::write(
        root.join("user/example/SKILL.md"),
        "---\nname: example\ndescription: Example skill\n---\nUse this skill.",
    )
    .expect("write skill");

    let result = block_on(load_sourced_skills::<
        Source,
        Skill,
        fn(Skill, &Source) -> Skill,
    >(
        &env(&root),
        [("user".to_string(), Source { kind: "user" })],
        None,
    ));

    assert_eq!(result.diagnostics, []);
    assert_eq!(result.skills.len(), 1);
    assert_eq!(result.skills[0].source, Source { kind: "user" });
    assert_eq!(
        result.skills[0].skill,
        skill(
            root.join("user/example/SKILL.md"),
            "example",
            "Example skill",
            "Use this skill.",
            false,
        )
    );
}

#[test]
fn attaches_source_info_to_skill_diagnostics() {
    let root = TempDir::new();
    fs::create_dir_all(root.join("user/broken")).expect("create broken skill dir");
    fs::write(
        root.join("user/broken/SKILL.md"),
        "---\nname: broken\n---\nMissing description.",
    )
    .expect("write broken skill");

    let result = block_on(load_sourced_skills::<
        Source,
        Skill,
        fn(Skill, &Source) -> Skill,
    >(
        &env(&root),
        [("user".to_string(), Source { kind: "user" })],
        None,
    ));

    assert_eq!(result.skills, []);
    assert_eq!(result.diagnostics.len(), 1);
    let diagnostic = &result.diagnostics[0];
    assert_eq!(diagnostic.source, Source { kind: "user" });
    assert_eq!(diagnostic.diagnostic.diagnostic_type, "warning");
    assert_eq!(
        diagnostic.diagnostic.code,
        SkillDiagnosticCode::InvalidMetadata
    );
    assert_eq!(diagnostic.diagnostic.message, "description is required");
    assert_eq!(
        diagnostic.diagnostic.path,
        root.join("user/broken/SKILL.md").to_string_lossy()
    );
}

#[test]
fn loads_direct_markdown_children_only_from_the_root_directory() {
    let root = TempDir::new();
    fs::create_dir_all(root.join("skills/nested")).expect("create nested");
    fs::write(
        root.join("skills/root.md"),
        "---\ndescription: Root skill\n---\nRoot content",
    )
    .expect("write root skill");
    fs::write(
        root.join("skills/nested/ignored.md"),
        "---\ndescription: Ignored\n---\nIgnored content",
    )
    .expect("write ignored skill");

    let result = block_on(load_skills(&env(&root), ["skills"]));

    assert_eq!(
        result
            .skills
            .iter()
            .map(|skill| skill.name.as_str())
            .collect::<Vec<_>>(),
        vec!["skills"]
    );
    assert_eq!(result.skills[0].content, "Root content");
}

#[test]
fn honors_gitignore_ignore_fdignore_hidden_and_node_modules_filters() {
    let root = TempDir::new();
    for dir in [
        "skills/keep",
        "skills/by-gitignore",
        "skills/by-fdignore",
        "skills/nested/by-ignore",
        "skills/.hidden",
        "skills/node_modules/pkg",
    ] {
        fs::create_dir_all(root.join(dir)).expect("create skill dir");
    }
    for (dir, name) in [
        ("skills/keep", "keep"),
        ("skills/by-gitignore", "by-gitignore"),
        ("skills/by-fdignore", "by-fdignore"),
        ("skills/nested/by-ignore", "by-ignore"),
        ("skills/.hidden", "hidden"),
        ("skills/node_modules/pkg", "pkg"),
    ] {
        fs::write(
            root.join(&format!("{dir}/SKILL.md")),
            format!("---\nname: {name}\ndescription: {name}\n---\n{name}"),
        )
        .expect("write skill");
    }
    fs::write(root.join("skills/.gitignore"), "by-gitignore/\n").expect("write gitignore");
    fs::write(root.join("skills/.fdignore"), "by-fdignore/\n").expect("write fdignore");
    fs::write(root.join("skills/nested/.ignore"), "by-ignore/\n").expect("write ignore");

    let result = block_on(load_skills(&env(&root), ["skills"]));

    assert_eq!(result.diagnostics, []);
    assert_eq!(
        result
            .skills
            .iter()
            .map(|skill| skill.name.as_str())
            .collect::<Vec<_>>(),
        vec!["keep"]
    );
}
