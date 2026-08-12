//! RPC-only entry-point helpers.

use crate::{
    agent_session::AgentSession,
    agent_session_runtime::AgentSessionRuntime,
    cli::{Args, parse_args},
    config,
    core::{
        resource_loader::{DefaultResourceLoader, ResourceExtensionPaths},
        settings_manager::SettingsManager,
        tools::{
            bash::create_bash_tool, edit::create_edit_tool, find::create_find_tool,
            grep::create_grep_tool, ls::create_ls_tool, read::create_read_tool,
            write::create_write_tool,
        },
    },
    defaults::DEFAULT_THINKING_LEVEL,
    modes::rpc::rpc_mode::run_rpc_loop_with_runtime_and_session_file,
};
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use zedflow_agent::harness::{
    env::nodejs::NodeExecutionEnv,
    session::{
        InMemorySessionStorage, InMemorySessionStorageOptions, JsonlSessionRepo,
        repo_utils::to_shared_session,
    },
    types::{
        AgentHarnessOptions, AgentHarnessResources, FileSystem, JsonlSessionCreateOptions,
        JsonlSessionListOptions, JsonlSessionMetadata, PromptTemplate as HarnessPromptTemplate,
        Session as AgentSessionTrait, SessionForkOptions, SessionForkPosition, SessionMetadata,
        Skill as HarnessSkill,
    },
};
use zedflow_ai::{
    Model, Models,
    auth::types::{ApiKeyAuth, ApiKeyResolveInput, AuthFuture, AuthResult, ResolvedAuth},
    providers::all::builtin_models,
};

struct RuntimeApiKeyAuth {
    api_key: String,
    fallback: Option<Arc<dyn ApiKeyAuth>>,
}

impl ApiKeyAuth for RuntimeApiKeyAuth {
    fn name(&self) -> &str {
        "--api-key override"
    }

    fn resolve<'a>(
        &'a self,
        input: ApiKeyResolveInput<'a>,
    ) -> AuthFuture<'a, AuthResult<Option<ResolvedAuth>>> {
        Box::pin(async move {
            let mut resolved = if let Some(fallback) = &self.fallback {
                fallback.resolve(input).await?.unwrap_or_default()
            } else {
                ResolvedAuth::default()
            };
            resolved.auth.api_key = Some(self.api_key.clone());
            resolved.source = Some("--api-key".to_owned());
            Ok(Some(resolved))
        })
    }
}

#[must_use]
pub fn rpc_args(args: &[String]) -> crate::cli::Args {
    let mut combined = vec!["--mode".to_owned(), "rpc".to_owned()];
    combined.extend_from_slice(args);
    parse_args(combined)
}

pub fn run<R: BufRead, W: Write + Send + 'static>(reader: R, writer: W) -> io::Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    run_with_args(&args, reader, writer)
}

fn run_with_args<R: BufRead, W: Write + Send + 'static>(
    args: &[String],
    reader: R,
    writer: W,
) -> io::Result<()> {
    let runtime = create_runtime_for_args(args)?;
    let cwd = std::env::current_dir()?;
    let session_file = rpc_session_file(&rpc_args(args), &cwd);
    run_rpc_loop_with_runtime_and_session_file(reader, writer, &runtime, session_file)
}

/// Build the session runtime shared by RPC and single-shot print modes.
pub fn create_runtime_for_args(args: &[String]) -> io::Result<AgentSessionRuntime> {
    create_runtime(parse_args(args.iter().cloned()), None)
}

/// Fork a persistent session at its current leaf, matching Pi's `/clone` action.
pub fn create_runtime_for_fork(source: &str, entry_id: String) -> io::Result<AgentSessionRuntime> {
    create_runtime(
        parse_args(["--fork".to_owned(), source.to_owned()]),
        Some(SessionForkOptions {
            entry_id: Some(entry_id),
            position: Some(SessionForkPosition::At),
            id: None,
        }),
    )
}

fn create_runtime(
    parsed: Args,
    fork_options: Option<SessionForkOptions>,
) -> io::Result<AgentSessionRuntime> {
    let cwd = std::env::current_dir()?;
    let cwd_string = cwd.to_string_lossy().into_owned();
    let settings = SettingsManager::create(&cwd, config::get_agent_dir());
    let mut models = builtin_models();
    let model = configured_model(&parsed, &settings, &mut models);
    let env = Arc::new(NodeExecutionEnv::with_cwd(&cwd_string));
    let session = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| io::Error::other(error.to_string()))?
        .block_on(create_session(
            &parsed,
            &cwd,
            &cwd_string,
            &settings,
            &env,
            fork_options,
        ))
        .map_err(io::Error::other)?;
    let tools = configured_tools(&parsed, &cwd);
    let active_tool_names = tools.iter().map(|tool| tool.tool.name.clone()).collect();
    let resources = configured_resources(&parsed, &cwd);
    let session = AgentSession::new(AgentHarnessOptions {
        env,
        session,
        models,
        tools: Some(tools),
        resources: Some(resources),
        system_prompt: parsed
            .system_prompt
            .map(zedflow_agent::harness::types::SystemPrompt::Text),
        stream_options: None,
        model,
        thinking_level: Some(parsed.thinking.unwrap_or(DEFAULT_THINKING_LEVEL)),
        active_tool_names: Some(active_tool_names),
        steering_mode: Some(queue_mode(&settings.get_steering_mode())),
        follow_up_mode: Some(queue_mode(&settings.get_follow_up_mode())),
    })
    .map_err(|error| io::Error::other(error.to_string()))?;
    Ok(AgentSessionRuntime::new(session, cwd_string))
}

fn configured_tools(args: &Args, cwd: &Path) -> Vec<zedflow_agent::types::AgentTool> {
    if args.no_tools || args.no_builtin_tools {
        return Vec::new();
    }
    let mut tools = vec![
        create_read_tool(cwd),
        create_bash_tool(cwd),
        create_write_tool(cwd),
        create_edit_tool(cwd),
        create_grep_tool(cwd),
        create_find_tool(cwd),
        create_ls_tool(cwd),
    ];
    if !args.tools.is_empty() {
        tools.retain(|tool| args.tools.iter().any(|name| name == &tool.tool.name));
    }
    tools.retain(|tool| {
        !args
            .exclude_tools
            .iter()
            .any(|name| name == &tool.tool.name)
    });
    tools
}

fn configured_resources(args: &Args, cwd: &Path) -> AgentHarnessResources {
    let mut loader = DefaultResourceLoader::new(cwd, config::get_agent_dir());
    loader.extend_resources(ResourceExtensionPaths {
        skill_paths: args.skills.iter().map(PathBuf::from).collect(),
        ..Default::default()
    });
    loader.reload();
    let skills = if args.no_skills {
        Vec::new()
    } else {
        loader
            .get_skills()
            .skills
            .iter()
            .filter_map(|skill| {
                fs::read_to_string(&skill.file_path)
                    .ok()
                    .map(|content| HarnessSkill {
                        name: skill.name.clone(),
                        description: skill.description.clone(),
                        content,
                        file_path: skill.file_path.clone(),
                        disable_model_invocation: Some(skill.disable_model_invocation),
                    })
            })
            .collect::<Vec<_>>()
    };
    AgentHarnessResources {
        prompt_templates: Some(load_prompt_templates(args, cwd)),
        skills: Some(skills),
    }
}

fn load_prompt_templates(args: &Args, cwd: &Path) -> Vec<HarnessPromptTemplate> {
    let mut templates = Vec::new();
    if !args.no_prompt_templates {
        for directory in [
            config::get_agent_dir().join("prompts"),
            cwd.join(config::CONFIG_DIR_NAME).join("prompts"),
        ] {
            load_prompt_template_path(&directory, &mut templates);
        }
    }
    for path in &args.prompt_templates {
        let path = Path::new(path);
        load_prompt_template_path(
            if path.is_absolute() {
                path.to_path_buf()
            } else {
                cwd.join(path)
            }
            .as_path(),
            &mut templates,
        );
    }
    templates
}

fn load_prompt_template_path(path: &Path, templates: &mut Vec<HarnessPromptTemplate>) {
    if path.is_dir() {
        let Ok(entries) = fs::read_dir(path) else {
            return;
        };
        let mut entries = entries
            .flatten()
            .map(|entry| entry.path())
            .collect::<Vec<_>>();
        entries.sort();
        for child in entries {
            if child.extension().is_some_and(|extension| extension == "md") {
                load_prompt_template_path(&child, templates);
            }
        }
        return;
    }
    if path.extension().is_none_or(|extension| extension != "md") {
        return;
    }
    let Ok(raw) = fs::read_to_string(path) else {
        return;
    };
    let raw = raw.replace("\r\n", "\n").replace('\r', "\n");
    let (frontmatter, content) = if let Some(rest) = raw.strip_prefix("---\n") {
        if let Some(end) = rest.find("\n---") {
            let yaml = &rest[..end];
            let content = rest.get(end + 4..).unwrap_or_default().trim().to_owned();
            let Ok(frontmatter) = serde_yaml::from_str::<serde_yaml::Value>(yaml) else {
                return;
            };
            (Some(frontmatter), content)
        } else {
            (None, raw.clone())
        }
    } else {
        (None, raw.clone())
    };
    let description = frontmatter
        .as_ref()
        .and_then(|value| value.get("description"))
        .and_then(serde_yaml::Value::as_str)
        .map(str::to_owned)
        .or_else(|| {
            content
                .lines()
                .find(|line| !line.trim().is_empty())
                .map(|line| {
                    let line = line.trim();
                    let shortened = line.chars().take(60).collect::<String>();
                    if line.chars().count() > 60 {
                        format!("{shortened}...")
                    } else {
                        shortened
                    }
                })
        });
    let Some(name) = path.file_stem().and_then(|name| name.to_str()) else {
        return;
    };
    templates.push(HarnessPromptTemplate {
        name: name.to_owned(),
        description,
        content: content.to_owned(),
    });
}

async fn create_session(
    args: &Args,
    cwd: &Path,
    cwd_string: &str,
    settings: &SettingsManager,
    env: &Arc<NodeExecutionEnv>,
    fork_options: Option<SessionForkOptions>,
) -> Result<Arc<dyn AgentSessionTrait>, String> {
    if args.no_session {
        let storage = if let Some(id) = args.session_id.clone() {
            InMemorySessionStorage::new(Some(InMemorySessionStorageOptions {
                entries: None,
                metadata: Some(SessionMetadata {
                    id,
                    created_at: zedflow_agent::harness::session::create_timestamp(),
                }),
            }))
            .map_err(|error| error.to_string())?
        } else {
            InMemorySessionStorage::default()
        };
        return Ok(Arc::new(to_shared_session(Arc::new(storage))) as Arc<dyn AgentSessionTrait>);
    }

    let session_root = args
        .session_dir
        .as_deref()
        .map(PathBuf::from)
        .or_else(|| settings.get_session_dir())
        .unwrap_or_else(config::get_sessions_dir);
    let repo = JsonlSessionRepo::new(
        Arc::clone(env) as Arc<dyn FileSystem>,
        session_root.to_string_lossy().into_owned(),
    );

    if let Some(source_arg) = args.fork.as_deref() {
        let metadata = find_session_metadata(&repo, env, source_arg, cwd, cwd_string).await?;
        return repo
            .fork(
                metadata,
                JsonlSessionCreateOptions {
                    id: args.session_id.clone(),
                    cwd: cwd_string.to_owned(),
                    parent_session_path: None,
                },
                fork_options.unwrap_or_default(),
            )
            .await
            .map(|session| Arc::new(session) as Arc<dyn AgentSessionTrait>)
            .map_err(|error| error.to_string());
    }

    if let Some(session_arg) = args.session.as_deref() {
        let path = if session_arg.contains('/')
            || session_arg.contains('\\')
            || session_arg.ends_with(".jsonl")
        {
            let path = Path::new(session_arg);
            if path.is_absolute() {
                path.to_path_buf()
            } else {
                cwd.join(path)
            }
        } else {
            let sessions = repo
                .list(JsonlSessionListOptions {
                    cwd: Some(cwd_string.to_owned()),
                })
                .await
                .map_err(|error| error.to_string())?;
            let mut metadata = sessions.into_iter().find(|session| {
                session.base.id == session_arg || session.base.id.starts_with(session_arg)
            });
            let mut fork_global = false;
            if metadata.is_none() {
                metadata = repo
                    .list(JsonlSessionListOptions { cwd: None })
                    .await
                    .map_err(|error| error.to_string())?
                    .into_iter()
                    .find(|session| {
                        session.base.id == session_arg || session.base.id.starts_with(session_arg)
                    });
                fork_global = metadata.is_some();
            }
            let metadata =
                metadata.ok_or_else(|| format!("No session found matching '{session_arg}'"))?;
            if fork_global {
                return repo
                    .fork(
                        metadata,
                        JsonlSessionCreateOptions {
                            id: args.session_id.clone(),
                            cwd: cwd_string.to_owned(),
                            parent_session_path: None,
                        },
                        SessionForkOptions::default(),
                    )
                    .await
                    .map(|session| Arc::new(session) as Arc<dyn AgentSessionTrait>)
                    .map_err(|error| error.to_string());
            }
            return repo
                .open(metadata)
                .await
                .map(|session| Arc::new(session) as Arc<dyn AgentSessionTrait>)
                .map_err(|error| error.to_string());
        };

        let path = path.to_string_lossy().into_owned();
        if env
            .exists(&path, None)
            .await
            .map_err(|error| error.to_string())?
        {
            let metadata =
                zedflow_agent::harness::session::load_jsonl_session_metadata(env.as_ref(), &path)
                    .await
                    .map_err(|error| error.to_string())?;
            if metadata.cwd != cwd_string {
                return repo
                    .fork(
                        metadata,
                        JsonlSessionCreateOptions {
                            id: args.session_id.clone(),
                            cwd: cwd_string.to_owned(),
                            parent_session_path: None,
                        },
                        SessionForkOptions::default(),
                    )
                    .await
                    .map(|session| Arc::new(session) as Arc<dyn AgentSessionTrait>)
                    .map_err(|error| error.to_string());
            }
            return repo
                .open(metadata)
                .await
                .map(|session| Arc::new(session) as Arc<dyn AgentSessionTrait>)
                .map_err(|error| error.to_string());
        }

        return Err(format!("Session path does not exist: {path}"));
    }

    let sessions = repo
        .list(JsonlSessionListOptions {
            cwd: Some(cwd_string.to_owned()),
        })
        .await
        .map_err(|error| error.to_string())?;
    if (args.continue_session || args.resume) && sessions.first().is_some() {
        return repo
            .open(sessions.into_iter().next().expect("session exists"))
            .await
            .map(|session| Arc::new(session) as Arc<dyn AgentSessionTrait>)
            .map_err(|error| error.to_string());
    }
    if let Some(session_id) = args.session_id.as_deref() {
        if let Some(metadata) = sessions
            .into_iter()
            .find(|session| session.base.id == session_id)
        {
            return repo
                .open(metadata)
                .await
                .map(|session| Arc::new(session) as Arc<dyn AgentSessionTrait>)
                .map_err(|error| error.to_string());
        }
    }

    repo.create(zedflow_agent::harness::types::JsonlSessionCreateOptions {
        id: args.session_id.clone(),
        cwd: cwd_string.to_owned(),
        parent_session_path: None,
    })
    .await
    .map(|session| Arc::new(session) as Arc<dyn AgentSessionTrait>)
    .map_err(|error| error.to_string())
}

async fn find_session_metadata(
    repo: &JsonlSessionRepo,
    env: &Arc<NodeExecutionEnv>,
    argument: &str,
    cwd: &Path,
    cwd_string: &str,
) -> Result<JsonlSessionMetadata, String> {
    let path_like =
        argument.contains('/') || argument.contains('\\') || argument.ends_with(".jsonl");
    if path_like {
        let path = Path::new(argument);
        let path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            cwd.join(path)
        };
        let path = path.to_string_lossy().into_owned();
        if !env
            .exists(&path, None)
            .await
            .map_err(|error| error.to_string())?
        {
            return Err(format!("Session path does not exist: {path}"));
        }
        return zedflow_agent::harness::session::load_jsonl_session_metadata(env.as_ref(), &path)
            .await
            .map_err(|error| error.to_string());
    }
    let mut sessions = repo
        .list(JsonlSessionListOptions {
            cwd: Some(cwd_string.to_owned()),
        })
        .await
        .map_err(|error| error.to_string())?;
    if !sessions
        .iter()
        .any(|session| session.base.id == argument || session.base.id.starts_with(argument))
    {
        sessions = repo
            .list(JsonlSessionListOptions { cwd: None })
            .await
            .map_err(|error| error.to_string())?;
    }
    sessions
        .into_iter()
        .find(|session| session.base.id == argument || session.base.id.starts_with(argument))
        .ok_or_else(|| format!("No session found matching '{argument}'"))
}

fn rpc_session_file(args: &Args, cwd: &Path) -> Option<String> {
    let session = args.session.as_deref()?;
    if !(session.contains('/') || session.contains('\\') || session.ends_with(".jsonl")) {
        return None;
    }
    Some(if Path::new(session).is_absolute() {
        session.to_owned()
    } else {
        cwd.join(session).to_string_lossy().into_owned()
    })
}

fn queue_mode(value: &str) -> zedflow_agent::types::QueueMode {
    match value {
        "all" => zedflow_agent::types::QueueMode::All,
        _ => zedflow_agent::types::QueueMode::OneAtATime,
    }
}

fn configured_model(args: &Args, settings: &SettingsManager, models: &mut Models) -> Model {
    let configured_provider = settings.get_default_provider();
    let configured_model = settings.get_default_model();
    let provider = args
        .provider
        .as_deref()
        .or(configured_provider.as_deref())
        .map(str::to_owned);
    let requested = args.model.as_deref().or(configured_model.as_deref());

    let model = requested
        .and_then(|requested| {
            let (provider, id) = requested
                .split_once('/')
                .map_or((provider.as_deref(), requested), |(provider, id)| {
                    (Some(provider), id)
                });
            provider
                .and_then(|provider| models.get_model(provider, id))
                .or_else(|| {
                    models
                        .get_models(None)
                        .into_iter()
                        .find(|model| model.id == id)
                })
        })
        .or_else(|| {
            provider.and_then(|provider| models.get_models(Some(&provider)).into_iter().next())
        })
        .or_else(|| models.get_models(None).into_iter().next())
        .unwrap_or_default();

    if let (Some(api_key), Some(mut provider)) = (
        args.api_key.clone(),
        models.get_provider(&model.provider).cloned(),
    ) {
        provider.auth.api_key = Some(Arc::new(RuntimeApiKeyAuth {
            api_key,
            fallback: provider.auth.api_key.clone(),
        }));
        models.set_provider(provider);
    }

    model
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Write};
    use std::sync::{Arc, Mutex};
    use zedflow_ai::auth::types::{AuthContext, AuthModel};

    struct EmptyAuthContext;

    impl AuthContext for EmptyAuthContext {
        fn env<'a>(&'a self, _name: &'a str) -> AuthFuture<'a, Option<String>> {
            Box::pin(async { None })
        }

        fn file_exists<'a>(&'a self, _path: &'a str) -> AuthFuture<'a, bool> {
            Box::pin(async { false })
        }
    }

    struct SharedWriter(Arc<Mutex<Vec<u8>>>);

    impl Write for SharedWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0.lock().expect("writer lock").extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn run_dispatches_state_commands_to_the_runtime() {
        let input = Cursor::new(
            br#"{"id":"state-1","type":"get_state"}
"#,
        );
        let output = Arc::new(Mutex::new(Vec::new()));

        run_with_args(
            &["--no-session".to_owned()],
            input,
            SharedWriter(Arc::clone(&output)),
        )
        .expect("RPC runtime should start");

        let output = output.lock().expect("writer lock");
        let response: serde_json::Value =
            serde_json::from_slice(&output).expect("valid JSON response");
        assert_eq!(response["id"], "state-1");
        assert_eq!(response["command"], "get_state");
        assert_eq!(response["success"], true);
        assert!(response["data"].is_object());
    }

    #[test]
    fn session_dir_flag_creates_a_persistent_session() {
        let root = std::env::temp_dir().join(format!(
            "zedflow-rpc-session-{}",
            zedflow_agent::harness::session::create_session_id()
        ));
        let cwd = std::env::current_dir().expect("current directory");
        let cwd_string = cwd.to_string_lossy().into_owned();
        let settings = SettingsManager::in_memory(Default::default());
        let args = rpc_args(&[
            "--session-dir".to_owned(),
            root.to_string_lossy().into_owned(),
        ]);
        let env = Arc::new(NodeExecutionEnv::with_cwd(&cwd_string));
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime")
            .block_on(create_session(
                &args,
                &cwd,
                &cwd_string,
                &settings,
                &env,
                None,
            ))
            .expect("persistent session");

        assert!(
            std::fs::read_dir(root)
                .expect("session root")
                .flatten()
                .any(|entry| entry.path().is_dir())
        );
    }

    #[test]
    fn missing_session_path_is_rejected_instead_of_created() {
        let cwd = std::env::current_dir().expect("current directory");
        let cwd_string = cwd.to_string_lossy().into_owned();
        let settings = SettingsManager::in_memory(Default::default());
        let missing = std::env::temp_dir().join(format!(
            "zedflow-rpc-missing-{}.jsonl",
            zedflow_agent::harness::session::create_session_id()
        ));
        let args = rpc_args(&[
            "--session".to_owned(),
            missing.to_string_lossy().into_owned(),
        ]);
        let env = Arc::new(NodeExecutionEnv::with_cwd(&cwd_string));
        let result = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime")
            .block_on(create_session(
                &args,
                &cwd,
                &cwd_string,
                &settings,
                &env,
                None,
            ));
        let error = match result {
            Ok(_) => panic!("missing path must fail"),
            Err(error) => error,
        };
        assert!(error.contains("does not exist"));
        assert!(!missing.exists());
    }

    #[test]
    fn configured_model_uses_rpc_provider_model_and_api_key_flags() {
        let args = rpc_args(&[
            "--provider".to_owned(),
            "openai".to_owned(),
            "--model".to_owned(),
            "gpt-4".to_owned(),
            "--api-key".to_owned(),
            "runtime-key".to_owned(),
        ]);
        let settings = SettingsManager::in_memory(Default::default());
        let mut models = builtin_models();
        let model = configured_model(&args, &settings, &mut models);
        let auth_model = AuthModel {
            provider: model.provider.clone(),
            api: model.api.clone(),
            id: model.id.clone(),
            base_url: Some(model.base_url.clone()),
        };
        let auth = models
            .get_provider(&model.provider)
            .and_then(|provider| provider.auth.api_key.as_ref())
            .expect("provider API-key auth");
        let resolved = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("runtime")
            .block_on(auth.resolve(ApiKeyResolveInput {
                model: &auth_model,
                ctx: &EmptyAuthContext,
                credential: None,
            }))
            .expect("resolve auth")
            .expect("resolved auth");

        assert_eq!(model.provider, "openai");
        assert_eq!(model.id, "gpt-4");
        assert_eq!(resolved.auth.api_key.as_deref(), Some("runtime-key"));
    }

    #[test]
    fn prompt_template_loader_strips_frontmatter() {
        let path = std::env::temp_dir().join(format!(
            "zedflow-rpc-prompt-{}.md",
            zedflow_agent::harness::session::create_session_id()
        ));
        std::fs::write(
            &path,
            "---\ndescription: summarize the input\n---\nUse $ARGUMENTS.",
        )
        .expect("prompt template");
        let mut templates = Vec::new();
        load_prompt_template_path(&path, &mut templates);
        std::fs::remove_file(&path).expect("remove prompt template");

        assert_eq!(templates.len(), 1);
        assert_eq!(
            templates[0].name,
            path.file_stem().unwrap().to_str().unwrap()
        );
        assert_eq!(
            templates[0].description.as_deref(),
            Some("summarize the input")
        );
        assert_eq!(templates[0].content, "Use $ARGUMENTS.");
    }

    #[test]
    fn malformed_prompt_frontmatter_is_not_loaded() {
        let path = std::env::temp_dir().join(format!(
            "zedflow-rpc-invalid-prompt-{}.md",
            zedflow_agent::harness::session::create_session_id()
        ));
        std::fs::write(&path, "---\ndescription: [unterminated\n---\nignored")
            .expect("prompt template");
        let mut templates = Vec::new();
        load_prompt_template_path(&path, &mut templates);
        std::fs::remove_file(&path).expect("remove prompt template");

        assert!(templates.is_empty());
    }

    #[test]
    fn rpc_registers_the_bash_tool() {
        let tools = configured_tools(&rpc_args(&[]), Path::new("."));
        assert!(tools.iter().any(|tool| tool.tool.name == "bash"));
    }
}
