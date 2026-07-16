use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use futures::executor::block_on;
use zedflow_agent::harness::env::nodejs::NodeExecutionEnv;
use zedflow_agent::harness::prompt_templates::{
    PromptTemplateDiagnosticCode, load_prompt_templates, load_sourced_prompt_templates,
};
use zedflow_agent::harness::types::PromptTemplate;

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
            "zedflow-agent-prompt-templates-{}-{nanos}",
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

#[test]
fn loads_markdown_templates_non_recursively_from_one_or_more_dirs() {
    let root = TempDir::new();
    fs::create_dir_all(root.join("a/nested")).expect("create a/nested");
    fs::create_dir_all(root.join("b")).expect("create b");
    fs::write(
        root.join("a/one.md"),
        "---\ndescription: One template\n---\nHello $1",
    )
    .expect("write one");
    fs::write(root.join("a/nested/ignored.md"), "Ignored").expect("write ignored");
    fs::write(root.join("b/two.md"), "First line description\nBody").expect("write two");

    let result = block_on(load_prompt_templates(&env(&root), ["a", "b"]));

    assert_eq!(result.diagnostics, []);
    assert_eq!(
        result.prompt_templates,
        vec![
            PromptTemplate {
                name: "one".to_string(),
                description: Some("One template".to_string()),
                content: "Hello $1".to_string(),
            },
            PromptTemplate {
                name: "two".to_string(),
                description: Some("First line description".to_string()),
                content: "First line description\nBody".to_string(),
            },
        ]
    );
}

#[test]
fn preserves_source_info_for_sourced_prompt_templates() {
    let root = TempDir::new();
    fs::create_dir_all(root.join("prompts")).expect("create prompts");
    fs::write(
        root.join("prompts/example.md"),
        "---\ndescription: Example\n---\nExample body",
    )
    .expect("write example");

    let result = block_on(load_sourced_prompt_templates::<
        Source,
        PromptTemplate,
        fn(PromptTemplate, &Source) -> PromptTemplate,
    >(
        &env(&root),
        [("prompts".to_string(), Source { kind: "project" })],
        None,
    ));

    assert_eq!(result.diagnostics, []);
    assert_eq!(result.prompt_templates.len(), 1);
    assert_eq!(
        result.prompt_templates[0].source,
        Source { kind: "project" }
    );
    assert_eq!(
        result.prompt_templates[0].prompt_template,
        PromptTemplate {
            name: "example".to_string(),
            description: Some("Example".to_string()),
            content: "Example body".to_string(),
        }
    );
}

#[test]
fn attaches_source_info_to_prompt_template_diagnostics() {
    let root = TempDir::new();
    fs::write(
        root.join("broken.md"),
        "---\ndescription: [unterminated\n---\nBody",
    )
    .expect("write broken");

    let result = block_on(load_sourced_prompt_templates::<
        Source,
        PromptTemplate,
        fn(PromptTemplate, &Source) -> PromptTemplate,
    >(
        &env(&root),
        [("broken.md".to_string(), Source { kind: "user" })],
        None,
    ));

    assert_eq!(result.prompt_templates, []);
    assert_eq!(result.diagnostics.len(), 1);
    let diagnostic = &result.diagnostics[0];
    assert_eq!(diagnostic.source, Source { kind: "user" });
    assert_eq!(
        diagnostic.diagnostic.code,
        PromptTemplateDiagnosticCode::ParseFailed
    );
    assert_eq!(diagnostic.diagnostic.diagnostic_type, "warning");
    assert_eq!(
        diagnostic.diagnostic.path,
        root.join("broken.md").to_string_lossy()
    );
}

#[cfg(unix)]
#[test]
fn loads_explicit_markdown_files_and_symlinked_files() {
    use std::os::unix::fs::symlink;

    let root = TempDir::new();
    fs::write(
        root.join("target.md"),
        "---\ndescription: Target\n---\nTarget body",
    )
    .expect("write target");
    symlink(root.join("target.md"), root.join("link.md")).expect("symlink target");

    let result = block_on(load_prompt_templates(&env(&root), ["target.md", "link.md"]));

    assert_eq!(
        result.prompt_templates,
        vec![
            PromptTemplate {
                name: "target".to_string(),
                description: Some("Target".to_string()),
                content: "Target body".to_string(),
            },
            PromptTemplate {
                name: "link".to_string(),
                description: Some("Target".to_string()),
                content: "Target body".to_string(),
            },
        ]
    );
}

#[test]
fn substitutes_command_arguments() {
    let template = PromptTemplate {
        name: "one".to_string(),
        description: None,
        content: "$1 ${@:2} $ARGUMENTS".to_string(),
    };

    assert_eq!(
        zedflow_agent::format_prompt_template_invocation(
            &template,
            &["hello world".to_string(), "test".to_string()]
        ),
        "hello world test hello world test"
    );
}
