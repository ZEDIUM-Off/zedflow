use adk_core::{Content, FunctionResponseData, Part};
use adk_graph::{ExecutionConfig, Node, NodeContext, State};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use zf_storage::flow_store::FlowStore;

use zf_context::context::ContextStrategy;
use zf_context::context_source;
use zf_runtime::inference;
use zf_storage::context_store::ContextStore;

use zf_context::starters;
use zf_runtime::models;
use zf_runtime::runtime::RunServices;
use zf_runtime::workspace_context::ContextSnapshot;
use zf_storage::workspaces::Workspace;

#[tokio::test]
async fn editable_defaults_survive_reinitialization_and_example_is_idempotent() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("project");
    let docs = temp.path().join("docs");
    std::fs::create_dir_all(&path).unwrap();
    std::fs::create_dir_all(&docs).unwrap();
    let workspace = Workspace {
        id: "fixture".into(),
        name: "Fixture".into(),
        path: path.clone(),
        open: true,
    };
    let flows = FlowStore::new(
        temp.path().join("home"),
        std::sync::Arc::new(zf_compiler::graph_compiler::GraphValidator::new(
            &zf_runtime::materialize::RuntimePrimitives,
        )),
    );
    drop(open_app(&path).await.unwrap());
    let store = ContextStore::new(path.clone());
    let existing = store.read("workspace-default").await.unwrap();
    let mut strategy = existing.strategy.unwrap();
    strategy.name = "My edited policy".into();
    store.save(&strategy, Some(&existing.hash)).await.unwrap();
    drop(open_app(&path).await.unwrap());
    assert_eq!(
        store
            .read("workspace-default")
            .await
            .unwrap()
            .strategy
            .unwrap()
            .name,
        "My edited policy"
    );
    let installed = install_example(&workspace.path, "../docs").await.unwrap();
    let repeated = install_example(&workspace.path, "../docs").await.unwrap();
    assert_eq!(installed["root"]["key"], repeated["root"]["key"]);
    assert_eq!(installed["worker"]["hash"], repeated["worker"]["hash"]);
    assert_eq!(flows.list(&workspace).await.unwrap().len(), 2);
    let runtime = zf_execution::preparation::prepare(
        &flows,
        &workspace,
        &zf_execution::preparation::RuntimeSelection {
            flow: installed["root"]["key"].as_str().unwrap().into(),
            entry: "main".into(),
            bridges: vec!["working-system".into()],
            flow_hashes: BTreeMap::new(),
            bridge_hashes: BTreeMap::new(),
            contexts: BTreeMap::new(),
        },
    )
    .await
    .unwrap();
    assert_eq!(runtime.flows.len(), 2);
    assert!(runtime.interactive());
    assert_eq!(
        runtime.flows["working-system/documentation"]
            .composition
            .settings
            .working_directory
            .as_deref(),
        Some("../docs")
    );
    runtime
        .validate(&zf_runtime::materialize::RuntimePrimitives)
        .unwrap();
}

/// The pre-studio starter is intentionally independent of the current builder.
/// Its exact output is the compatibility baseline for existing linked flows.
fn legacy_strategy(id: &str, name: &str, tools: &[&str]) -> ContextStrategy {
    let text = json!({"kind":"text"});
    let object = json!({"kind":"record","fields":{}});
    let emit = |id: &str, role: &str, format: &str| json!({"kind":"emit","id":id,"role":role,"format":format,"value":{"kind":"resource","name":id}});
    serde_json::from_value(json!({
        "version":2,"id":id,"name":name,
        "requirements":{
            "instructions":text,"skills":text,"files":text,"input":text,
            "history":{"kind":"list","item":object}
        },
        "capabilities":tools.iter().map(|id|json!({"id":id,"input":object,"output":object})).collect::<Vec<_>>(),
        "program":[
            {"kind":"group","id":"guidance","label":"Instructions et skills","items":[emit("instructions","instruction","text"),emit("skills","instruction","text")]},
            {"kind":"group","id":"documents","label":"Fichiers sélectionnés","items":[emit("files","data","text")]},
            {"kind":"if","id":"conversation","condition":{"kind":"present","value":{"kind":"resource","name":"history"}},"then":[emit("history","data","adkMessages")],"else":[emit("input","data","text")]}
        ]
    }))
    .unwrap()
}

fn prepared_program(
    strategy: &ContextStrategy,
    bindings: &Value,
) -> zf_context::resources::ContextProgram {
    let source = context_source::generate(strategy).unwrap();
    // Exercise the persisted source codec and frozen-artifact verification too.
    assert_eq!(context_source::parse(&source).unwrap(), *strategy);
    inference::program(&json!({
        "strategy":strategy,"source":source,
        "hash":zf_storage::context_store::hash(source.as_bytes()),
        "bindings":bindings
    }))
    .unwrap()
}

#[tokio::test]
async fn legacy_default_sources_remain_untouched_until_explicit_save() {
    let temp = tempfile::tempdir().unwrap();
    let store = ContextStore::new(temp.path().into());
    let legacy = legacy_strategy(
        "workspace-default",
        "Assistant de workspace",
        &["read", "write", "edit", "exec"],
    );
    let saved = store.save(&legacy, None).await.unwrap();
    drop(open_app(temp.path()).await.unwrap());
    let preserved = store.read("workspace-default").await.unwrap();
    assert_eq!(preserved.hash, saved.hash);
    assert_eq!(preserved.source, saved.source);
    assert_eq!(preserved.strategy.unwrap(), legacy);
}

#[tokio::test]
async fn structured_starters_preserve_exact_requests_and_conversation_boundaries() {
    let temp = tempfile::tempdir().unwrap();
    let services = RunServices::new(
        "starter-compatibility".into(),
        temp.path().into(),
        temp.path().join("data"),
        ContextSnapshot {
            cwd: temp.path().into(),
            ..Default::default()
        },
        json!({}),
        vec![],
    )
    .unwrap();
    let mut model = Content::new("model");
    model.parts = vec![
        Part::Thinking {
            thinking: "Inspect both records before answering.".into(),
            signature: Some("thinking-signature".into()),
        },
        Part::Text {
            text: "I will inspect the records.".into(),
        },
        Part::FunctionCall {
            name: "inspect_json".into(),
            args: json!({"record":{"value":7,"enabled":true,"empty":null}}),
            id: Some("call-a".into()),
            thought_signature: Some("call-signature".into()),
        },
        Part::FunctionCall {
            name: "read".into(),
            args: json!({"path":"notes.md"}),
            id: Some("call-b".into()),
            thought_signature: None,
        },
    ];
    let result = Content {
        role: "user".into(),
        parts: vec![
            Part::FunctionResponse {
                function_response: FunctionResponseData::new(
                    "read",
                    json!({"error":"missing","retry":false}),
                ),
                id: Some("call-b".into()),
                annotations: None,
            },
            Part::FunctionResponse {
                function_response: FunctionResponseData::new(
                    "inspect_json",
                    json!({"count":1,"values":[7]}),
                ),
                id: Some("call-a".into()),
                annotations: None,
            },
        ],
    };
    let messages = vec![
        Content::new("user").with_text("Inspect the records."),
        model,
        result,
    ];
    let completed = [
        messages.clone(),
        vec![Content::new("model").with_text("One record was inspected.")],
    ]
    .concat();
    let state_bindings = json!({
        "instructions":{"kind":"state","field":"instructions"},
        "skills":{"kind":"state","field":"skills"},
        "files":{"kind":"state","field":"files"},
        "input":{"kind":"state","field":"input"},
        "history":{"kind":"state","field":"messages"}
    });
    let mut conversation_bindings = state_bindings.clone();
    conversation_bindings["history"] = starters::bindings()["history"].clone();
    let cases = [
        ("absent", None, false, false),
        ("empty", Some(json!([])), false, false),
        ("complete", Some(json!(completed)), false, false),
        ("tool-results", Some(json!(messages)), false, false),
        ("first-input", None, true, true),
        ("tool-continuation", Some(json!(messages)), true, false),
        ("new-input-after-tool", Some(json!(messages)), true, true),
        ("next-turn", Some(json!(completed)), true, true),
    ];
    for (id, name, tools) in [
        ("conversation-default", "Conversation", &[][..]),
        (
            "workspace-default",
            "Assistant de workspace",
            &["read", "write", "edit", "exec"][..],
        ),
        (
            "tools-default",
            "Inspection de données",
            &["inspect_json"][..],
        ),
        (
            "working-system-context",
            "Working System · documentation",
            &["read", "exec"][..],
        ),
    ] {
        let old = legacy_strategy(id, name, tools);
        let new = starters::strategy(id, name, tools);
        let config = json!({"__zedflowVersion":3,"attachments":{"tools":{"items":tools.iter().map(|id|json!({"id":id,"name":id})).collect::<Vec<_>>()}}});
        for (case, history, conversation, fresh) in &cases {
            let mut state = State::from([
                (
                    "instructions".into(),
                    json!("Follow the workspace instructions.\nPreserve sources."),
                ),
                (
                    "skills".into(),
                    json!("Available: review.\nActivated: review rules."),
                ),
                ("files".into(), json!("# notes.md\nRésumé Unicode 🌍\n")),
                ("input".into(), json!("Continue with this new input.")),
                ("__zedflow:input:input".into(), json!(2)),
                (
                    "__zedflow:model-input:model".into(),
                    json!(if *fresh { 1 } else { 2 }),
                ),
            ]);
            if let Some(history) = history {
                state.insert("messages".into(), history.clone());
            }
            let bindings = if *conversation {
                &conversation_bindings
            } else {
                &state_bindings
            };
            let old = inference::prepare(
                &config,
                &prepared_program(&old, bindings),
                &services,
                "model",
                &state,
                "fixture",
            )
            .await
            .unwrap();
            let new = inference::prepare(
                &config,
                &prepared_program(&new, bindings),
                &services,
                "model",
                &state,
                "fixture",
            )
            .await
            .unwrap();
            assert!(old.needs.is_empty() && new.needs.is_empty(), "{id}/{case}");
            assert!(old.wait.is_none() && new.wait.is_none(), "{id}/{case}");
            let serialized = serde_json::to_value(&new.contents).unwrap();
            assert_eq!(
                serialized,
                serde_json::to_value(&old.contents).unwrap(),
                "{id}/{case}"
            );
            assert_eq!(new.tools, old.tools, "{id}/{case}");
            let mut expected = vec![
                Content::new("system").with_text(state["instructions"].as_str().unwrap()),
                Content::new("system").with_text(state["skills"].as_str().unwrap()),
                Content::new("user").with_text(state["files"].as_str().unwrap()),
            ];
            if let Some(history) = history {
                expected.extend(serde_json::from_value::<Vec<Content>>(history.clone()).unwrap());
            }
            if (!*conversation && history.is_none())
                || (*conversation && (*fresh || history.is_none()))
            {
                expected.push(Content::new("user").with_text(state["input"].as_str().unwrap()));
            }
            assert_eq!(
                serialized,
                serde_json::to_value(expected).unwrap(),
                "{id}/{case}"
            );
            assert_eq!(new.tools.len(), tools.len(), "{id}/{case}");
        }
    }
}

#[tokio::test]
async fn invalid_existing_example_definitions_are_reported_without_overwriting_them() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("project");
    std::fs::create_dir_all(&path).unwrap();
    std::fs::create_dir(temp.path().join("docs")).unwrap();
    let workspace = Workspace {
        id: "fixture".into(),
        name: "Fixture".into(),
        path: path.clone(),
        open: true,
    };
    let installed = install_example(&workspace.path, "../docs").await.unwrap();
    for relative in [
        ".zedflow/context/working-system-context.rs",
        ".zedflow/context/conversation-default.rs",
        ".zedflow/bridges/working-system.rs",
    ] {
        let file = path.join(relative);
        let original = std::fs::read(&file).unwrap();
        let invalid = b"fn broken_definition( {";
        std::fs::write(&file, invalid).unwrap();
        let error = install_example(&workspace.path, "../docs")
            .await
            .unwrap_err()
            .to_string();
        if relative.contains("/context/") {
            let id = std::path::Path::new(relative)
                .file_stem()
                .unwrap()
                .to_str()
                .unwrap();
            assert!(
                error.contains(&format!("Invalid starter context: {id}")),
                "{error}"
            );
        } else {
            assert!(error.contains("Invalid starter bridge"), "{error}");
        }
        assert_eq!(std::fs::read(&file).unwrap(), invalid);
        std::fs::write(&file, original).unwrap();
    }
    let restored = install_example(&workspace.path, "../docs").await.unwrap();
    assert_eq!(restored["root"]["hash"], installed["root"]["hash"]);
    assert_eq!(restored["worker"]["hash"], installed["worker"]["hash"]);
    assert_eq!(restored["bridge"]["hash"], installed["bridge"]["hash"]);
}

#[tokio::test]
async fn scoped_cwd_changes_sources_and_commands_but_shares_receipts_and_cancellation() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("project");
    let docs = temp.path().join("docs");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::create_dir_all(&docs).unwrap();
    std::fs::write(docs.join("AGENTS.md"), "DOCUMENTATION INSTRUCTIONS").unwrap();
    let services = RunServices::new(
        "cwd-test".into(),
        root.clone(),
        temp.path().join("data"),
        ContextSnapshot {
            cwd: root.clone(),
            ..Default::default()
        },
        json!({}),
        vec![],
    )
    .unwrap();
    services.set_context_sources(vec![], Some(temp.path().join("home")));
    let scoped = services
        .for_composition(
            &serde_json::from_value(
                install_example(&root, "../docs").await.unwrap()["worker"]["composition"].clone(),
            )
            .unwrap(),
            "worker/",
        )
        .unwrap();
    assert_eq!(services.cwd, root);
    assert_eq!(scoped.cwd, docs);
    assert!(
        scoped
            .context
            .instructions
            .iter()
            .any(|item| item.content.contains("DOCUMENTATION INSTRUCTIONS"))
    );
    scoped.set_binding("worker/model".into(), json!({"provider":"fixture"}));
    assert_eq!(
        services.binding("worker/model"),
        Some(json!({"provider":"fixture"}))
    );
    let first = scoped
        .execute_tool("worker/tools", "cwd", "exec", json!({"command":"pwd"}))
        .await
        .unwrap();
    assert!(
        first.to_string().contains(docs.to_str().unwrap()),
        "{first}"
    );
    // Same receipt identity has the same result through the parent view; no new command.
    assert_eq!(
        services
            .execute_tool("worker/tools", "cwd", "exec", json!({"command":"pwd"}))
            .await
            .unwrap(),
        first
    );
    assert!(services.for_working_directory(Some("missing")).is_err());
    scoped.cancel.cancel();
    assert!(services.cancel.is_cancelled());
}

#[tokio::test]
async fn default_context_preserves_history_without_accumulating_injected_files() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(
        temp.path().join("reference.txt"),
        "UNIQUE_REFERENCE_CONTENT",
    )
    .unwrap();
    let services = RunServices::new(
        "history-test".into(),
        temp.path().into(),
        temp.path().join("data"),
        ContextSnapshot {
            cwd: temp.path().into(),
            ..Default::default()
        },
        json!({}),
        vec![],
    )
    .unwrap();
    let strategy = starters::strategy("conversation-default", "Conversation", &[]);
    let source = context_source::generate(&strategy).unwrap();
    let context_cfg = json!({"modelNode":"model","__zedflowVersion":3,"contextProgram":{"strategy":strategy,"source":source,"hash":zf_storage::context_store::hash(source.as_bytes()),"bindings":starters::bindings()},"attachments":{"files":{"items":[{"id":"ref","path":"reference.txt"}]},"instructions":{"items":[{"id":"style","source":{"kind":"text","text":"Keep answers concise"}}]}}});
    // Fixture returns a small response so echoing the request cannot itself grow history.
    let model_cfg = json!({"contextNode":"context","__zedflowVersion":3,"provider":"fixture","fixtureSteps":[{"text":"answer"}],"inputField":"input","historyField":"messages"});
    let prepare = models::context_node_with_services(
        "context",
        &context_cfg,
        &model_cfg,
        "context",
        services.clone(),
    )
    .unwrap();
    let model = models::inference_node_with_services(
        "model",
        &model_cfg,
        &context_cfg,
        "model",
        services.clone(),
    )
    .unwrap();
    let mut state = State::new();
    for turn in 1..=3 {
        state.insert("input".into(), json!(format!("question {turn}")));
        state.insert("__zedflow:input:input".into(), json!(turn));
        let output = prepare
            .execute(&NodeContext::new(
                state.clone(),
                ExecutionConfig::new("history-test"),
                turn * 2,
            ))
            .await
            .unwrap();
        let id = output.updates["__zedflow:prepared:context"]
            .as_str()
            .unwrap()
            .to_string();
        let record = services
            .read_record("prepared-requests", &id)
            .await
            .unwrap()
            .unwrap();
        let content = record["contents"].to_string();
        assert_eq!(
            content.matches("UNIQUE_REFERENCE_CONTENT").count(),
            1,
            "{content}"
        );
        for previous in 1..=turn {
            assert_eq!(
                content.matches(&format!("question {previous}")).count(),
                1,
                "{content}"
            );
        }
        state.extend(output.updates);
        state.extend(
            model
                .execute(&NodeContext::new(
                    state.clone(),
                    ExecutionConfig::new("history-test"),
                    turn * 2 + 1,
                ))
                .await
                .unwrap()
                .updates,
        );
        assert_eq!(state["messages"].as_array().unwrap().len(), turn * 2);
        assert!(
            !state["messages"]
                .to_string()
                .contains("UNIQUE_REFERENCE_CONTENT")
        );
    }
}

// Exercise the public HTTP boundary now owning starter installation, with isolated sources.
async fn open_app(workspace: &std::path::Path) -> anyhow::Result<axum::Router> {
    let home = workspace.join(".test-home");
    std::fs::create_dir_all(&home)?;
    zf_serve::server::router_with_home(
        workspace.join(format!(".test-data-{}", uuid::Uuid::new_v4())),
        workspace.into(),
        vec![],
        {
            std::fs::create_dir_all(&home).unwrap();
            home
        },
    )
    .await
}
async fn install_example(workspace: &std::path::Path, directory: &str) -> anyhow::Result<Value> {
    use axum::{
        body::{Body, to_bytes},
        http::Request,
    };
    use tower::ServiceExt;
    let app = open_app(workspace).await?;
    let response = app
        .oneshot(
            Request::post("/api/examples/working-system")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(
                    &json!({"workingDirectory":directory}),
                )?))?,
        )
        .await?;
    let status = response.status();
    let value: Value = serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await?)?;
    anyhow::ensure!(status.is_success(), "{value}");
    Ok(value)
}
