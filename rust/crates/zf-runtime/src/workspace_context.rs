//! Source-backed workspace instructions and progressive skill discovery.
//!
//! Discovery and explicit invocation follow the useful Pi 0.85.1 conventions;
//! ADK parses the normalized skill frontmatter. Pi packages are not executed.

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ContextSnapshot {
    #[serde(default)]
    pub cwd: PathBuf,
    #[serde(default)]
    pub instructions: Vec<Instruction>,
    #[serde(default)]
    pub skills: Vec<Skill>,
    #[serde(default)]
    pub diagnostics: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub loaded_skills: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Instruction {
    pub path: PathBuf,
    pub content: String,
    pub hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub path: PathBuf,
    #[serde(default, skip_serializing)]
    pub body: Option<String>,
    pub hash: String,
    #[serde(default)]
    pub manual_only: bool,
}

impl ContextSnapshot {
    /// `extra_dirs` are explicit skill paths (directories or Markdown files),
    /// resolved relative to cwd and preceding automatically discovered skills.
    pub async fn load(cwd: &Path, extra_dirs: &[PathBuf]) -> Result<Self> {
        Self::load_with_home(cwd, extra_dirs, None).await
    }

    /// Isolate only instruction/skill sources for another daemon instance. This
    /// does not change HOME, credentials, model configuration or tool permissions.
    pub async fn load_with_home(
        cwd: &Path,
        extra_dirs: &[PathBuf],
        source_home: Option<&Path>,
    ) -> Result<Self> {
        let cwd = cwd.to_path_buf();
        let extra_dirs = extra_dirs.to_vec();
        let home = source_home
            .map(Path::to_path_buf)
            .or_else(|| std::env::var_os("HOME").map(PathBuf::from));
        tokio::task::spawn_blocking(move || load_snapshot(&cwd, &extra_dirs, home.as_deref()))
            .await
            .context("workspace context loader failed")?
    }

    pub fn system_prompt(&self) -> String {
        self.system_prompt_for_tools(&["read".into(), "write".into(), "edit".into(), "exec".into()])
    }

    pub fn system_prompt_for_tools(&self, tools: &[String]) -> String {
        let mut prompt = String::from(
            "You are a coding assistant operating in Zedflow. Use only the tools actually available.\n",
        );
        for (name, guideline) in [
            ("read", "Use read to examine UTF-8 text files."),
            ("exec", "Use exec for commands, listing and searching."),
            ("write", "Use write for new files or complete rewrites."),
            (
                "edit",
                "Use edit for precise exact-text replacements after reading the file in this run; old_string must be unique unless replace_all is explicitly intended.",
            ),
        ] {
            if tools.iter().any(|tool| tool == name) {
                prompt.push_str(guideline);
                prompt.push('\n');
            }
        }
        prompt.push_str("Use concise responses and show the paths and results needed to verify your work. Tool failures are observations to reason about, not proof that a requested action succeeded.\n");
        if !self.instructions.is_empty() {
            prompt.push_str("\n<project_context>\n");
            for instruction in &self.instructions {
                prompt.push_str(&format!(
                    "<project_instructions path=\"{}\" hash=\"{}\">\n{}\n</project_instructions>\n",
                    xml(&instruction.path.to_string_lossy()),
                    instruction.hash,
                    instruction.content
                ));
            }
            prompt.push_str("</project_context>\n");
        }
        let reader = ["read", "exec"]
            .into_iter()
            .find(|name| tools.iter().any(|tool| tool == name));
        if let Some(reader) = reader
            && self.skills.iter().any(|skill| !skill.manual_only)
        {
            prompt.push_str(&format!("\nThe following skills provide specialized instructions. Use {reader} to load the complete file when its description matches the task. Resolve every relative reference against the skill file's directory and use that absolute path.\n<available_skills>\n"));
            for skill in self.skills.iter().filter(|skill| !skill.manual_only) {
                prompt.push_str(&format!("<skill><name>{}</name><description>{}</description><location>{}</location></skill>\n", xml(&skill.name), xml(&skill.description), xml(&skill.path.to_string_lossy())));
            }
            prompt.push_str("</available_skills>\n");
        }
        prompt.push_str(&format!(
            "\nCurrent working directory: {}",
            self.cwd.display()
        ));
        prompt
    }

    /// Resolve explicit skills before queueing a message. Disabled automatic
    /// invocation does not disable an explicit user request.
    pub fn expand_skill(&self, text: &str) -> Result<String> {
        Ok(self.expand_skill_with_metadata(text)?.0)
    }

    pub fn expand_skill_with_metadata(
        &self,
        text: &str,
    ) -> Result<(String, Option<serde_json::Value>)> {
        let Some(invocation) = text.strip_prefix("/skill:") else {
            return Ok((text.to_owned(), None));
        };
        let split = invocation
            .find(char::is_whitespace)
            .unwrap_or(invocation.len());
        let (name, arguments) = invocation.split_at(split);
        let skill = self
            .skills
            .iter()
            .find(|skill| skill.name == name)
            .with_context(|| format!("unknown skill {name}"))?;
        let raw = std::fs::read_to_string(&skill.path)
            .with_context(|| format!("cannot load skill {}", skill.path.display()))?;
        let loaded = parse_skill(&skill.path, &raw)?;
        ensure!(
            loaded.name == skill.name,
            "skill name changed at {}; reload context before invoking it",
            skill.path.display()
        );
        let base = skill.path.parent().context("skill path has no parent")?;
        let mut expanded = format!(
            "<skill name=\"{}\" location=\"{}\" hash=\"{}\">\nReferences are relative to {}.\n\n{}\n</skill>",
            xml(name),
            xml(&skill.path.to_string_lossy()),
            loaded.hash,
            base.display(),
            loaded.body.as_deref().unwrap_or_default()
        );
        let arguments = arguments.trim();
        if !arguments.is_empty() {
            expanded.push_str("\n\n");
            expanded.push_str(arguments);
        }
        Ok((
            expanded,
            Some(
                serde_json::json!({"name":loaded.name,"path":loaded.path,"hash":loaded.hash,"source":"explicit","truncated":false}),
            ),
        ))
    }
}

/// Load a skill explicitly selected by a v2 agent. Discovery alone never grants
/// tools or adds the body to an invocation.
pub fn read_skill(path: &Path) -> Result<Skill> {
    let path = std::fs::canonicalize(path)
        .with_context(|| format!("cannot resolve skill {}", path.display()))?;
    let metadata = std::fs::metadata(&path)?;
    ensure!(
        metadata.is_file(),
        "skill source is not a regular file: {}",
        path.display()
    );
    ensure!(
        metadata.len() <= 1024 * 1024,
        "skill exceeds 1 MiB: {}",
        path.display()
    );
    use std::io::Read;
    let mut raw = String::new();
    std::fs::File::open(&path)?
        .take(1024 * 1024 + 1)
        .read_to_string(&mut raw)?;
    ensure!(
        raw.len() <= 1024 * 1024,
        "skill exceeds 1 MiB: {}",
        path.display()
    );
    parse_skill(&path, &raw)
}

pub(crate) fn load_snapshot(
    cwd: &Path,
    extra_dirs: &[PathBuf],
    home: Option<&Path>,
) -> Result<ContextSnapshot> {
    let cwd = std::fs::canonicalize(cwd)
        .with_context(|| format!("cannot open workspace {}", cwd.display()))?;
    ensure!(
        cwd.is_dir(),
        "workspace is not a directory: {}",
        cwd.display()
    );
    let mut snapshot = ContextSnapshot {
        cwd: cwd.clone(),
        ..Default::default()
    };
    let mut instruction_dirs = Vec::new();
    if let Some(home) = home {
        instruction_dirs.push(home.join(".pi/agent"));
    }
    instruction_dirs.extend(
        cwd.ancestors()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .map(Path::to_path_buf),
    );
    let mut seen_instructions = HashSet::new();
    for directory in instruction_dirs {
        for name in [
            "AGENTS.override.md",
            "AGENTS.md",
            "AGENTS.MD",
            "CLAUDE.md",
            "CLAUDE.MD",
        ] {
            let path = directory.join(name);
            if !path.is_file() {
                continue;
            }
            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    let canonical = std::fs::canonicalize(&path).unwrap_or(path);
                    if seen_instructions.insert(canonical.clone()) {
                        snapshot.instructions.push(Instruction {
                            path: canonical,
                            hash: hash(content.as_bytes()),
                            content: content.trim_start_matches('\u{feff}').to_owned(),
                        });
                    }
                    break;
                }
                Err(error) => snapshot
                    .diagnostics
                    .push(format!("cannot read {}: {error}", path.display())),
            }
        }
    }
    let mut roots = Vec::new();
    for path in extra_dirs {
        let path = expand_root(path, &cwd, home)?;
        if !path.exists() {
            snapshot
                .diagnostics
                .push(format!("skill path does not exist: {}", path.display()));
        }
        roots.push((path, DiscoveryMode::Explicit));
    }
    roots.push((cwd.join(".pi/skills"), DiscoveryMode::Pi));
    for ancestor in cwd.ancestors() {
        let directory = ancestor.join(".agents/skills");
        if home.is_none_or(|home| directory != home.join(".agents/skills")) {
            roots.push((directory, DiscoveryMode::Agents));
        }
        if ancestor.join(".git").exists() {
            break;
        }
    }
    if let Some(home) = home {
        roots.push((home.join(".pi/agent/skills"), DiscoveryMode::Pi));
        roots.push((home.join(".agents/skills"), DiscoveryMode::Agents));
    }
    let mut seen_files = HashSet::new();
    let mut names = HashMap::<String, PathBuf>::new();
    for (root, mode) in roots {
        for path in skill_files(&root, mode, &mut snapshot.diagnostics) {
            let canonical = match std::fs::canonicalize(&path) {
                Ok(path) => path,
                Err(error) => {
                    snapshot
                        .diagnostics
                        .push(format!("cannot resolve skill {}: {error}", path.display()));
                    continue;
                }
            };
            if !seen_files.insert(canonical.clone()) {
                continue;
            }
            let loaded = std::fs::read_to_string(&canonical)
                .map_err(anyhow::Error::from)
                .and_then(|content| parse_skill(&canonical, &content));
            match loaded {
                Ok(skill) => {
                    if let Some(winner) = names.get(&skill.name) {
                        snapshot.diagnostics.push(format!(
                            "skill name collision '{}': using {}, ignoring {}",
                            skill.name,
                            winner.display(),
                            canonical.display()
                        ));
                    } else {
                        names.insert(skill.name.clone(), canonical);
                        snapshot.skills.push(Skill {
                            body: None,
                            ..skill
                        });
                    }
                }
                Err(error) => {
                    // Ordinary Markdown files are not necessarily skills. A
                    // declared SKILL.md always receives a useful diagnostic.
                    if path.file_name().is_some_and(|name| name == "SKILL.md") {
                        snapshot
                            .diagnostics
                            .push(format!("cannot load skill {}: {error:#}", path.display()));
                    }
                }
            }
        }
    }
    Ok(snapshot)
}

#[derive(Clone, Copy)]
enum DiscoveryMode {
    Pi,
    Agents,
    Explicit,
}

fn skill_files(root: &Path, mode: DiscoveryMode, diagnostics: &mut Vec<String>) -> Vec<PathBuf> {
    if root.is_file() {
        return vec![root.to_path_buf()];
    }
    if !root.is_dir() {
        return Vec::new();
    }
    let filter_root = root.to_path_buf();
    let mut builder = ignore::WalkBuilder::new(root);
    builder
        .parents(false)
        .git_global(false)
        .git_exclude(false)
        .require_git(false)
        .follow_links(true)
        .add_custom_ignore_filename(".fdignore")
        .sort_by_file_path(|left, right| left.cmp(right))
        .filter_entry(move |entry| {
            if entry.file_name() == "node_modules" {
                return false;
            }
            if entry.path() == filter_root {
                return true;
            }
            let mut ancestors = entry.path().ancestors().skip(1);
            if entry.file_name() == "SKILL.md" {
                ancestors.next();
            }
            !ancestors
                .take_while(|path| path.starts_with(&filter_root))
                .any(|directory| directory.join("SKILL.md").is_file())
        });
    let mut files = Vec::new();
    for entry in builder.build() {
        match entry {
            Ok(entry) if entry.file_type().is_some_and(|kind| kind.is_file()) => {
                let path = entry.path();
                let declared = entry.file_name() == "SKILL.md";
                let root_file = path.parent() == Some(root);
                let markdown = path.extension().is_some_and(|extension| extension == "md");
                if declared
                    || markdown
                        && match mode {
                            DiscoveryMode::Pi => root_file,
                            DiscoveryMode::Agents => !root_file,
                            DiscoveryMode::Explicit => true,
                        }
                {
                    files.push(path.to_path_buf());
                }
            }
            Ok(_) => {}
            Err(error) => diagnostics.push(format!("skill discovery: {error}")),
        }
    }
    files
}

fn parse_skill(path: &Path, raw: &str) -> Result<Skill> {
    let normalized = raw.trim_start_matches('\u{feff}').replace("\r\n", "\n");
    let mut lines = normalized.lines();
    ensure!(
        lines.next().is_some_and(|line| line.trim() == "---"),
        "missing skill frontmatter"
    );
    let mut frontmatter = Vec::new();
    let mut closed = false;
    for line in lines.by_ref() {
        if line.trim() == "---" {
            closed = true;
            break;
        }
        frontmatter.push(line);
    }
    ensure!(closed, "missing closing skill frontmatter delimiter");
    let fields: serde_yaml::Value =
        serde_yaml::from_str(&frontmatter.join("\n")).context("invalid skill frontmatter")?;
    let name = fields["name"]
        .as_str()
        .filter(|name| !name.trim().is_empty())
        .or_else(|| path.parent()?.file_name()?.to_str())
        .context("skill name is missing")?;
    let description = fields["description"]
        .as_str()
        .filter(|description| !description.trim().is_empty())
        .context("skill description is missing")?;
    let manual_only = fields["disable-model-invocation"]
        .as_bool()
        .unwrap_or(false)
        || fields["trigger"].as_bool().unwrap_or(false);
    // Pi's optional fields can differ from ADK's schema (notably allowed-tools).
    // Feed only the supported core through ADK rather than duplicating its parser.
    let adapted = adk_skill::SkillFrontmatter {
        name: name.to_owned(),
        description: description.to_owned(),
        trigger: Some(manual_only),
        allowed_tools: match &fields["allowed-tools"] {
            serde_yaml::Value::String(tools) => {
                tools.split_whitespace().map(str::to_owned).collect()
            }
            serde_yaml::Value::Sequence(tools) => tools
                .iter()
                .filter_map(|tool| tool.as_str().map(str::to_owned))
                .collect(),
            _ => Vec::new(),
        },
        ..Default::default()
    };
    let body = lines.collect::<Vec<_>>().join("\n");
    let adapted = format!("---\n{}---\n{}", serde_yaml::to_string(&adapted)?, body);
    let parsed = adk_skill::parse_skill_markdown(path, &adapted)?;
    Ok(Skill {
        name: parsed.name,
        description: parsed.description,
        path: path.to_path_buf(),
        body: Some(parsed.body),
        hash: hash(raw.as_bytes()),
        manual_only,
    })
}

fn expand_root(path: &Path, cwd: &Path, home: Option<&Path>) -> Result<PathBuf> {
    let text = path.to_string_lossy();
    if text == "~" || text.starts_with("~/") {
        return Ok(home
            .context("HOME is unavailable for skill path expansion")?
            .join(text.strip_prefix("~/").unwrap_or("")));
    }
    Ok(if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    })
}

fn hash(content: &[u8]) -> String {
    format!("{:x}", Sha256::digest(content))
}
fn xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, content: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }

    fn skill(path: &Path, name: &str, body: &str) {
        write(
            path,
            &format!("---\nname: {name}\ndescription: Description for {name}\n---\n{body}\n"),
        );
    }

    #[tokio::test]
    async fn source_home_override_isolates_discovery_without_changing_process_home() {
        let root = tempfile::tempdir().unwrap();
        let source_home = root.path().join("sources");
        let workspace = root.path().join("workspace");
        write(&source_home.join(".pi/agent/AGENTS.md"), "fixture-global");
        write(&workspace.join("AGENTS.md"), "fixture-project");
        skill(
            &source_home.join(".agents/skills/custom/SKILL.md"),
            "fixture-home-skill",
            "attached",
        );
        skill(
            &workspace.join(".agents/skills/local/SKILL.md"),
            "fixture-project-skill",
            "local",
        );
        let process_home = std::env::var_os("HOME");
        let snapshot = ContextSnapshot::load_with_home(&workspace, &[], Some(&source_home))
            .await
            .unwrap();
        assert_eq!(std::env::var_os("HOME"), process_home);
        assert_eq!(
            snapshot
                .instructions
                .iter()
                .map(|item| item.content.as_str())
                .collect::<Vec<_>>(),
            vec!["fixture-global", "fixture-project"]
        );
        assert_eq!(snapshot.skills.len(), 2);
        assert!(
            snapshot
                .skills
                .iter()
                .all(|skill| skill.path.starts_with(root.path()))
        );
        let empty = root.path().join("empty-sources");
        std::fs::create_dir_all(&empty).unwrap();
        let isolated = ContextSnapshot::load_with_home(&workspace, &[], Some(&empty))
            .await
            .unwrap();
        assert_eq!(isolated.instructions.len(), 1);
        assert_eq!(isolated.skills.len(), 1);
        assert_eq!(isolated.skills[0].name, "fixture-project-skill");
    }

    #[test]
    fn instructions_layer_global_ancestors_and_local_override() {
        let directory = tempfile::tempdir().unwrap();
        let home = directory.path().join("home");
        let repo = directory.path().join("repo");
        let cwd = repo.join("inner");
        write(&home.join(".pi/agent/AGENTS.md"), "global-guidance");
        write(&repo.join("AGENTS.md"), "parent-guidance");
        write(&cwd.join("AGENTS.md"), "shadowed-guidance");
        write(&cwd.join("AGENTS.override.md"), "local-guidance");
        write(&cwd.join("nested/AGENTS.md"), "descendant-guidance");
        let snapshot = load_snapshot(&cwd, &[], Some(&home)).unwrap();
        let bodies = snapshot
            .instructions
            .iter()
            .map(|item| item.content.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            bodies,
            ["global-guidance", "parent-guidance", "local-guidance"]
        );
        assert!(
            snapshot
                .instructions
                .iter()
                .all(|item| item.hash.len() == 64)
        );
        let prompt = snapshot.system_prompt();
        assert!(prompt.find("global-guidance").unwrap() < prompt.find("local-guidance").unwrap());
        assert!(!prompt.contains("descendant-guidance"));
    }

    #[test]
    fn serialized_instructions_keep_the_content_and_hash_loaded_for_the_run() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("AGENTS.md");
        write(&path, "Instructions at run creation\n");
        let snapshot = load_snapshot(directory.path(), &[], None).unwrap();
        let expected = snapshot
            .instructions
            .iter()
            .find(|instruction| instruction.path == path)
            .unwrap();
        assert_eq!(expected.hash, hash(b"Instructions at run creation\n"));
        let persisted = serde_json::to_vec(&snapshot).unwrap();
        write(&path, "Instructions changed afterwards\n");
        let recovered: ContextSnapshot = serde_json::from_slice(&persisted).unwrap();
        let instruction = recovered
            .instructions
            .iter()
            .find(|instruction| instruction.path == path)
            .unwrap();
        assert_eq!(instruction.content, expected.content);
        assert_eq!(instruction.hash, expected.hash);
        assert!(recovered.system_prompt().contains(&expected.hash));
        assert!(!recovered.system_prompt().contains("changed afterwards"));
    }

    #[test]
    fn skill_roots_prioritize_nearby_files_and_report_collisions() {
        let directory = tempfile::tempdir().unwrap();
        let home = directory.path().join("home");
        let repo = directory.path().join("repo");
        let cwd = repo.join("inner");
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        skill(&cwd.join(".pi/skills/winner/SKILL.md"), "shared", "nearest");
        skill(
            &cwd.join(".agents/skills/loser/SKILL.md"),
            "shared",
            "other-nearby",
        );
        skill(
            &repo.join(".agents/skills/parent/SKILL.md"),
            "parent",
            "parent-body",
        );
        skill(
            &directory.path().join(".agents/skills/outside/SKILL.md"),
            "outside",
            "outside-body",
        );
        skill(
            &home.join(".pi/agent/skills/global/SKILL.md"),
            "shared",
            "global",
        );
        skill(
            &home.join(".agents/skills/nested/leaf.md"),
            "portable",
            "portable-body",
        );
        skill(
            &home.join(".agents/skills/ignored.md"),
            "ignored",
            "ignored-body",
        );
        let snapshot = load_snapshot(&cwd, &[], Some(&home)).unwrap();
        assert_eq!(
            snapshot
                .skills
                .iter()
                .find(|skill| skill.name == "shared")
                .unwrap()
                .path,
            cwd.join(".pi/skills/winner/SKILL.md")
        );
        assert!(snapshot.skills.iter().all(|skill| skill.body.is_none()));
        assert!(snapshot.skills.iter().any(|skill| skill.name == "parent"));
        assert!(snapshot.skills.iter().any(|skill| skill.name == "portable"));
        assert!(
            !snapshot
                .skills
                .iter()
                .any(|skill| matches!(skill.name.as_str(), "ignored" | "outside"))
        );
        assert_eq!(
            snapshot
                .diagnostics
                .iter()
                .filter(|entry| entry.contains("collision"))
                .count(),
            2
        );
        let explicit = directory.path().join("explicit/SKILL.md");
        skill(&explicit, "shared", "explicit-body");
        let overridden = load_snapshot(&cwd, std::slice::from_ref(&explicit), Some(&home)).unwrap();
        assert_eq!(
            overridden
                .skills
                .iter()
                .find(|skill| skill.name == "shared")
                .unwrap()
                .path,
            explicit
        );
    }

    #[test]
    fn explicit_disabled_skill_loads_fresh_body_and_keeps_arguments() {
        let directory = tempfile::tempdir().unwrap();
        let cwd = directory.path().join("repo");
        let path = cwd.join(".pi/skills/manual/SKILL.md");
        write(
            &path,
            "---\ndescription: Manual skill\ndisable-model-invocation: true\nallowed-tools: read exec\n---\nnot-in-system-skill-body\n",
        );
        let snapshot = load_snapshot(&cwd, &[], None).unwrap();
        assert_eq!(snapshot.skills[0].name, "manual");
        assert!(snapshot.skills[0].manual_only);
        assert!(
            !snapshot
                .system_prompt()
                .contains("not-in-system-skill-body")
        );
        assert!(!snapshot.system_prompt().contains("<available_skills>"));
        write(
            &path,
            "---\nname: manual\ndescription: Manual skill\ndisable-model-invocation: true\n---\nnew-body\n",
        );
        let expanded = snapshot
            .expand_skill("/skill:manual first argument\nsecond argument")
            .unwrap();
        assert!(expanded.contains("new-body"));
        assert!(expanded.contains(path.parent().unwrap().to_str().unwrap()));
        assert!(expanded.ends_with("first argument\nsecond argument"));
        assert!(snapshot.expand_skill("/skill:missing").is_err());
        assert_eq!(
            snapshot.expand_skill("ordinary text").unwrap(),
            "ordinary text"
        );
        std::fs::remove_file(path).unwrap();
        assert!(snapshot.expand_skill("/skill:manual").is_err());
    }

    #[test]
    fn discovery_stops_at_skill_root_and_honors_ignores() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join(".pi/skills");
        skill(&root.join("outer/SKILL.md"), "outer", "outer-body");
        skill(&root.join("outer/nested/SKILL.md"), "nested", "nested-body");
        skill(&root.join("excluded/SKILL.md"), "excluded", "excluded-body");
        write(&root.join(".ignore"), "excluded/\n");
        let snapshot = load_snapshot(directory.path(), &[], None).unwrap();
        assert!(snapshot.skills.iter().any(|skill| skill.name == "outer"));
        assert!(
            !snapshot
                .skills
                .iter()
                .any(|skill| matches!(skill.name.as_str(), "nested" | "excluded"))
        );
    }

    #[cfg(unix)]
    #[test]
    fn repeated_symlink_to_a_skill_does_not_duplicate_the_catalog() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("skill/SKILL.md");
        skill(&source, "one", "body");
        let alias = directory.path().join("alias");
        std::os::unix::fs::symlink(source.parent().unwrap(), &alias).unwrap();
        let snapshot = load_snapshot(
            directory.path(),
            &[source.parent().unwrap().to_path_buf(), alias],
            None,
        )
        .unwrap();
        assert_eq!(
            snapshot
                .skills
                .iter()
                .filter(|skill| skill.name == "one")
                .count(),
            1
        );
        assert!(
            !snapshot
                .diagnostics
                .iter()
                .any(|entry| entry.contains("collision"))
        );
    }
}
