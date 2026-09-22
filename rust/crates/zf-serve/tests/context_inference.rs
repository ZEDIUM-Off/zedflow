use adk_core::{Content, FunctionResponseData, Part};
use adk_graph::prelude::*;
use serde_json::{Value, json};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use tempfile::TempDir;
use zf_context::context::ContextBlock;
use zf_context::context::ContextCapability;
use zf_context::context::ContextExpr;
use zf_context::context::ContextPredicate;
use zf_context::context::ContextStrategy;
use zf_context::context::FragmentFormat;
use zf_context::context::FragmentRole;
use zf_context::context_source;
use zf_core::identity::Permission;
use zf_core::identity::Scope;
use zf_core::types::DataType;
use zf_runtime::inference;
use zf_runtime::models;
use zf_runtime::operations;
use zf_runtime::runtime::CURRENT_OCCURRENCE;
use zf_runtime::runtime::DynamicCapabilities;
use zf_runtime::runtime::RunServices;
use zf_runtime::workspace_context::ContextSnapshot;
use zf_runtime::workspace_context::Instruction;
use zf_storage::content_store::ContentStore;
use zf_storage::context_store;
use zf_storage::data::DataRegistry;

struct Fixture {
    _root: TempDir,
    services: Arc<RunServices>,
    store: ContentStore,
}
impl Fixture {
    async fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let cwd = root.path().join("workspace");
        let data = root.path().join("data");
        std::fs::create_dir_all(&cwd).unwrap();
        std::fs::create_dir_all(&data).unwrap();
        let pool = SqlitePoolOptions::new()
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(data.join("storage.sqlite"))
                    .create_if_missing(true),
            )
            .await
            .unwrap();
        let store = ContentStore::new(pool.clone()).await.unwrap();
        let context = ContextSnapshot {
            cwd: cwd.clone(),
            instructions: vec![Instruction {
                path: cwd.join("AGENTS.md"),
                content: "DO NOT INJECT AMBIENT INSTRUCTIONS".into(),
                hash: "ambient".into(),
            }],
            ..Default::default()
        };
        let services = RunServices::new(
            "isolated-runtime".into(),
            cwd,
            data,
            context,
            json!({}),
            vec![],
        )
        .unwrap();
        services.set_content_store(store.clone());
        services
            .set_data_registry(
                DataRegistry::new(pool, store.clone(), &services.id)
                    .await
                    .unwrap(),
            )
            .unwrap();
        Self {
            _root: root,
            services,
            store,
        }
    }

    async fn invoke(&self, path: &str, config: &Value, state: State) -> NodeOutput {
        let node =
            models::node_with_services("agent", config, path, self.services.clone()).unwrap();
        let ctx = NodeContext::new(state, ExecutionConfig::new(&self.services.id), 0);
        CURRENT_OCCURRENCE
            .scope(
                (path.into(), format!("occurrence-{path}")),
                node.execute(&ctx),
            )
            .await
            .unwrap()
    }

    async fn snapshot(&self, output: &NodeOutput) -> Value {
        self.services
            .read_record(
                "capability-snapshots",
                output.updates["modelResponse"]["contextSnapshotId"]
                    .as_str()
                    .unwrap(),
            )
            .await
            .unwrap()
            .unwrap()
    }
}

fn fragment(
    id: &str,
    role: FragmentRole,
    format: FragmentFormat,
    value: ContextExpr,
) -> ContextBlock {
    ContextBlock::emit(id, role, format, value)
}
fn text(id: &str, value: ContextExpr) -> ContextBlock {
    fragment(id, FragmentRole::Data, FragmentFormat::Text, value)
}
fn frozen(strategy: &ContextStrategy, bindings: Value) -> Value {
    let source = context_source::generate(strategy).unwrap();
    json!({"strategy":strategy,"source":source,"hash":context_store::hash(source.as_bytes()),"types":{},"bindings":bindings})
}
fn configured(strategy: &ContextStrategy, bindings: Value) -> Value {
    json!({"__zedflowVersion":3,"provider":"fixture","contextProgram":frozen(strategy,bindings),"fixtureSteps":[{"echoRequest":true}]})
}
fn echoed(output: &NodeOutput) -> Value {
    serde_json::from_str(output.updates["output"].as_str().unwrap()).unwrap()
}

#[tokio::test]
async fn foreach_constructed_messages_reach_the_model_with_embedded_types_and_no_implicit_history()
{
    let f = Fixture::new().await;
    let object = DataType::Record {
        fields: BTreeMap::new(),
    };
    let message = DataType::Record {
        fields: BTreeMap::from([
            ("role".into(), DataType::Text),
            ("content".into(), DataType::Text),
        ]),
    };
    let strategy = ContextStrategy::new_v2("selected-messages", "Conversation sélectionnée")
        .define_type("Message", message)
        .require(
            "messages",
            DataType::List {
                item: Box::new(DataType::Named {
                    name: "Message".into(),
                }),
            },
        )
        .with_program(vec![ContextBlock::for_each(
            "each-message",
            ContextExpr::resource("messages"),
            "message",
            vec![fragment(
                "message",
                FragmentRole::Data,
                FragmentFormat::AdkMessages,
                ContextExpr::list(
                    object.clone(),
                    vec![ContextExpr::record(BTreeMap::from([
                        (
                            "role".into(),
                            ContextExpr::field(ContextExpr::variable("message"), "role"),
                        ),
                        (
                            "parts".into(),
                            ContextExpr::list(
                                object,
                                vec![ContextExpr::record(BTreeMap::from([(
                                    "text".into(),
                                    ContextExpr::truncate(
                                        ContextExpr::field(
                                            ContextExpr::variable("message"),
                                            "content",
                                        ),
                                        4,
                                    ),
                                )]))],
                            ),
                        ),
                    ]))],
                ),
            )],
        )]);
    let config = configured(
        &strategy,
        json!({"messages":{"kind":"state","field":"selected"}}),
    );
    let output = f.invoke("composer", &config, State::from([
        ("selected".into(), json!([{"role":"user","content":"Été🌿 demandé"},{"role":"model","content":"Oui, réponse"}])),
        ("messages".into(), json!([{"role":"user","parts":[{"text":"NEVER INJECT HISTORY"}]}])),
    ])).await;
    let request = echoed(&output);
    assert_eq!(
        request["contents"],
        json!([{"role":"user","parts":[{"text":"Été🌿"}]},{"role":"model","parts":[{"text":"Oui,"}]}])
    );
    assert!(!request.to_string().contains("NEVER INJECT"));
    assert!(!request.to_string().contains("AMBIENT"));
    let snapshot = f.snapshot(&output).await;
    assert_eq!(snapshot["resources"].as_array().unwrap().len(), 1);
    assert_eq!(snapshot["resources"][0]["name"], "messages");
}

#[tokio::test]
async fn large_tool_output_is_read_from_canonical_bytes_and_only_explicit_projection_reaches_model()
{
    let f = Fixture::new().await;
    let result = f
        .services
        .execute_tool(
            "direct-tool",
            "large-output",
            "exec",
            json!({"command":r#"head -c 200000 /dev/zero | tr '\0' X"#}),
        )
        .await
        .unwrap();
    let reference = result["fullOutputRef"].as_str().unwrap();
    let strategy = ContextStrategy::new("large-output", "Large output")
        .require("full", DataType::Text)
        .with_program(vec![text(
            "size",
            ContextExpr::to_json(ContextExpr::measure(
                ContextExpr::resource("full"),
                zf_context::context::MeasureUnit::Bytes,
            )),
        )]);
    let config = configured(
        &strategy,
        json!({"full":{"kind":"reader","reader":"content.text","input":{"kind":"state","field":"tool","pointer":"/source"}}}),
    );
    let output = f
        .invoke(
            "summarize",
            &config,
            State::from([("tool".into(), json!({"source":{"contentRef":reference}}))]),
        )
        .await;
    assert_eq!(echoed(&output)["contents"][0]["parts"][0]["text"], "200000");
    let snapshot = f.snapshot(&output).await;
    assert_eq!(
        snapshot["resources"][0]["value"].as_str().unwrap().len(),
        200000
    );
    assert_eq!(
        zf_storage::content_store::decode_full_output(&f.store.resolve(reference).await.unwrap())
            .unwrap(),
        vec![b'X'; 200000]
    );
}

#[tokio::test]
async fn readers_use_captured_state_and_active_branches_and_report_read_errors_before_model() {
    let f = Fixture::new().await;
    std::fs::write(
        f.services.cwd.join("guide.md"),
        "# Working instructions\n{{literal}}",
    )
    .unwrap();
    let strategy = ContextStrategy::new("sources", "Sources")
        .require("selected", DataType::Boolean)
        .require("guide", DataType::Text)
        .with_program(vec![ContextBlock::If {
            id: "choose".into(),
            condition: ContextPredicate::Eq {
                left: ContextExpr::resource("selected"),
                right: ContextExpr::literal(DataType::Boolean, json!(true)),
            },
            then: vec![text("guide", ContextExpr::resource("guide"))],
            otherwise: vec![text(
                "other",
                ContextExpr::literal(DataType::Text, json!("No source selected")),
            )],
        }]);
    let mut config = configured(
        &strategy,
        json!({"selected":{"kind":"state","field":"selected"},"guide":{"kind":"reader","reader":"file.text","input":{"kind":"state","field":"source","pointer":"/args"}}}),
    );
    let state = State::from([
        ("selected".into(), json!(true)),
        ("source".into(), json!({"args":{"path":"guide.md"}})),
    ]);
    let output = f.invoke("read", &config, state.clone()).await;
    assert_eq!(
        echoed(&output)["contents"][0]["parts"][0]["text"],
        "# Working instructions\n{{literal}}"
    );
    let snapshot = f.snapshot(&output).await;
    assert_eq!(snapshot["resources"][0]["name"], "guide");
    assert_eq!(
        snapshot["resources"][0]["provenance"]["read"]["reader"]["id"],
        "file.text"
    );
    config["contextProgram"]["bindings"]["guide"]["reader"] = json!("plugin.not-linked");
    let inactive = f
        .invoke(
            "inactive",
            &config,
            State::from([("selected".into(), json!(false))]),
        )
        .await;
    assert_eq!(
        echoed(&inactive)["contents"][0]["parts"][0]["text"],
        "No source selected"
    );
    let node = models::node_with_services("agent", &config, "active", f.services.clone()).unwrap();
    let error = node
        .execute(&NodeContext::new(
            state,
            ExecutionConfig::new(&f.services.id),
            0,
        ))
        .await
        .err()
        .unwrap();
    assert!(error.to_string().contains("not installed"), "{error}");
    let calls = f
        .store
        .records(&f.services.id)
        .await
        .unwrap()
        .into_iter()
        .filter(|r| r.kind == "model-calls")
        .collect::<Vec<_>>();
    assert_eq!(
        calls.len(),
        2,
        "Only successful active and inactive requests reached the provider"
    );
}

#[tokio::test]
async fn actual_model_request_uses_only_selected_context_and_captures_each_agent_passage() {
    let f = Fixture::new().await;
    let strategy = ContextStrategy::new("agent-context", "Explicit")
        .require("question", DataType::Text)
        .require("policy", DataType::Text)
        .with_program(vec![
            fragment(
                "policy",
                FragmentRole::Instruction,
                FragmentFormat::Text,
                ContextExpr::resource("policy"),
            ),
            text("question", ContextExpr::resource("question")),
        ]);
    let mut a = configured(
        &strategy,
        json!({"question":{"kind":"state","field":"questionA"},"policy":{"kind":"attachment","itemId":"policy"}}),
    );
    a["instructions"] = json!("old implicit instructions");
    a["attachments"] = json!({"instructions":{"items":[{"id":"policy","source":{"kind":"text","text":"Policy A {{literal}}"}},{"id":"unused","source":{"kind":"text","text":"NEVER SELECTED"}}]},"tools":{"items":[{"id":"exec","name":"exec"}]}});
    let mut b = a.clone();
    b["contextProgram"]["bindings"]["question"]["field"] = json!("questionB");
    b["attachments"]["instructions"]["items"][0]["source"]["text"] = json!("Policy B");
    let state = State::from([
        ("questionA".into(), json!("Alice")),
        ("questionB".into(), json!("Bob")),
        ("input".into(), json!("IMPLICIT INPUT")),
        (
            "messages".into(),
            json!("even invalid legacy history must not be read"),
        ),
    ]);
    let a_result = f.invoke("first/agent", &a, state.clone()).await;
    let b_result = f.invoke("second/agent", &b, state).await;
    let a_request = echoed(&a_result);
    let b_request = echoed(&b_result);
    assert_eq!(
        a_request["contents"],
        json!([
            Content::new("system").with_text("Policy A {{literal}}"),
            Content::new("user").with_text("Alice")
        ])
    );
    assert_eq!(
        b_request["contents"],
        json!([
            Content::new("system").with_text("Policy B"),
            Content::new("user").with_text("Bob")
        ])
    );
    assert_eq!(a_request["tools"], json!({}));
    let a_snapshot = f.snapshot(&a_result).await;
    let b_snapshot = f.snapshot(&b_result).await;
    for field in ["strategy", "source", "hash", "bindings"] {
        assert_eq!(
            a_snapshot["prepared"]["program"][field],
            a["contextProgram"][field]
        );
    }
    assert_eq!(
        a_snapshot["origin"]["occurrenceId"],
        "occurrence-first/agent"
    );
    assert_eq!(b_snapshot["agentPath"], "second/agent");
    assert_ne!(a_snapshot["invocationId"], b_snapshot["invocationId"]);
    let reference = a_result.updates["modelResponse"]["requestRef"]
        .as_str()
        .unwrap();
    let request_record = f.store.resolve(reference).await.unwrap();
    assert_eq!(request_record["request"]["contents"], a_request["contents"]);
    assert_eq!(request_record["request"]["tools"], a_request["tools"]);
}

#[tokio::test]
async fn absent_live_resource_suspends_before_model_and_inactive_branch_needs_nothing() {
    let f = Fixture::new().await;
    let strategy = ContextStrategy::new("conditional", "Conditional")
        .require("enabled", DataType::Boolean)
        .require("document", DataType::Text)
        .with_program(vec![ContextBlock::branch(
            "condition",
            ContextPredicate::equal(
                ContextExpr::resource("enabled"),
                ContextExpr::literal(DataType::Boolean, json!(true)),
            ),
            vec![text("document", ContextExpr::resource("document"))],
            vec![text(
                "empty",
                ContextExpr::literal(DataType::Text, json!("minimal")),
            )],
        )]);
    let config = configured(
        &strategy,
        json!({"enabled":{"kind":"state","field":"enabled"},"document":{"kind":"state","field":"document"}}),
    );
    let waiting = f
        .invoke(
            "agent",
            &config,
            State::from([("enabled".into(), json!(true))]),
        )
        .await;
    assert!(waiting.interrupt.is_some());
    assert_eq!(waiting.updates["contextNeeds"]["kind"], "context_resources");
    assert_eq!(
        waiting.updates["contextNeeds"]["needs"][0]["resource"],
        "document"
    );
    assert!(!waiting.updates.contains_key("modelResponse"));
    assert!(
        f.store
            .records(&f.services.id)
            .await
            .unwrap()
            .iter()
            .all(|record| record.kind != "model-requests")
    );
    let inactive = f
        .invoke(
            "agent",
            &config,
            State::from([("enabled".into(), json!(false))]),
        )
        .await;
    assert!(inactive.interrupt.is_none());
    assert_eq!(
        echoed(&inactive)["contents"],
        json!([Content::new("user").with_text("minimal")])
    );
    let ready = f
        .invoke(
            "agent",
            &config,
            State::from([
                ("enabled".into(), json!(true)),
                ("document".into(), json!("Produced by an ADK node")),
            ]),
        )
        .await;
    assert_eq!(
        echoed(&ready)["contents"][0]["parts"][0]["text"],
        "Produced by an ADK node"
    );
}

#[tokio::test]
async fn entity_alias_revision_is_captured_and_remains_retrievable_after_publication() {
    let f = Fixture::new().await;
    let registry = f.services.data_registry().unwrap();
    let first = registry
        .create(&Scope::Flow("writer".into()), "answer", &json!("before"))
        .await
        .unwrap();
    registry
        .grant(
            &Scope::Flow("writer".into()),
            "answer",
            &Scope::Flow("reader".into()),
            "incoming",
            Permission::Read,
        )
        .await
        .unwrap();
    let strategy = ContextStrategy::new("entity", "Entity")
        .require("answer", DataType::Text)
        .with_program(vec![text("answer", ContextExpr::resource("answer"))]);
    let config = configured(
        &strategy,
        json!({"answer":{"kind":"entity","scope":{"kind":"flow","id":"reader"},"alias":"incoming"}}),
    );
    let before = f.invoke("reader/agent", &config, State::new()).await;
    registry
        .publish(
            &Scope::Flow("writer".into()),
            "answer",
            &first.revision,
            &json!("after"),
        )
        .await
        .unwrap();
    let after = f.invoke("reader/agent", &config, State::new()).await;
    assert_eq!(echoed(&before)["contents"][0]["parts"][0]["text"], "before");
    assert_eq!(echoed(&after)["contents"][0]["parts"][0]["text"], "after");
    let snapshot = f.snapshot(&before).await;
    assert_eq!(
        snapshot["resources"][0]["provenance"]["revision"],
        json!(first.revision)
    );
    assert_eq!(
        snapshot["resources"][0]["provenance"]["entityId"],
        json!(first.entity_id)
    );
    let mut pinned = config;
    pinned["contextProgram"]["bindings"]["answer"]["revision"] = json!(first.revision);
    assert_eq!(
        echoed(&f.invoke("reader/agent", &pinned, State::new()).await)["contents"][0]["parts"][0]["text"],
        "before"
    );
}

#[tokio::test]
async fn explicitly_selected_adk_history_keeps_tool_pairs_and_rejects_orphan_results() {
    let f = Fixture::new().await;
    let record = DataType::Record {
        fields: BTreeMap::new(),
    };
    let strategy = ContextStrategy::new("history", "History")
        .require(
            "history",
            DataType::List {
                item: Box::new(record),
            },
        )
        .with_program(vec![fragment(
            "history",
            FragmentRole::Data,
            FragmentFormat::Json,
            ContextExpr::resource("history"),
        )]);
    let config = configured(
        &strategy,
        json!({"history":{"kind":"state","field":"history","encoding":"adkMessages"}}),
    );
    let call = Content {
        role: "model".into(),
        parts: vec![Part::FunctionCall {
            name: "read".into(),
            args: json!({"path":"a"}),
            id: Some("call-1".into()),
            thought_signature: None,
        }],
    };
    let result = Content {
        role: "user".into(),
        parts: vec![Part::FunctionResponse {
            function_response: FunctionResponseData::new("read", json!({"content":"exact"})),
            id: Some("call-1".into()),
            annotations: None,
        }],
    };
    let history = json!([call, result]);
    let output = f
        .invoke(
            "agent",
            &config,
            State::from([("history".into(), history.clone())]),
        )
        .await;
    assert_eq!(echoed(&output)["contents"], history);
    let node = models::node_with_services("agent", &config, "agent", f.services.clone()).unwrap();
    let ctx = NodeContext::new(
        State::from([("history".into(), json!([result]))]),
        ExecutionConfig::new(&f.services.id),
        0,
    );
    assert!(
        node.execute(&ctx)
            .await
            .err()
            .unwrap()
            .to_string()
            .contains("matching call")
    );
}

#[tokio::test]
async fn v2_json_history_projection_stays_data_and_explicit_messages_keep_strict_pairs() {
    let f = Fixture::new().await;
    let history = json!([
        Content {
            role: "model".into(),
            parts: vec![Part::FunctionCall {
                name: "read".into(),
                args: json!({"path":"a"}),
                id: Some("pair".into()),
                thought_signature: None
            }]
        },
        Content {
            role: "user".into(),
            parts: vec![Part::FunctionResponse {
                function_response: FunctionResponseData::new("read", json!({"content":"exact"})),
                id: Some("pair".into()),
                annotations: None
            }]
        },
    ]);
    let base = ContextStrategy::new_v2("representations", "Representations").require(
        "history",
        DataType::List {
            item: Box::new(DataType::Record {
                fields: BTreeMap::new(),
            }),
        },
    );
    let count = ContextExpr::record(BTreeMap::from([(
        "count".into(),
        ContextExpr::measure(
            ContextExpr::resource("history"),
            zf_context::context::MeasureUnit::Items,
        ),
    )]));
    let strategy = base.clone().with_program(vec![
        fragment("summary", FragmentRole::Data, FragmentFormat::Json, count),
        fragment(
            "conversation",
            FragmentRole::Data,
            FragmentFormat::AdkMessages,
            ContextExpr::resource("history"),
        ),
    ]);
    let bindings = json!({"history":{"kind":"state","field":"history","encoding":"adkMessages"}});
    let output = f
        .invoke(
            "v2",
            &configured(&strategy, bindings.clone()),
            State::from([("history".into(), history.clone())]),
        )
        .await;
    let request = echoed(&output);
    assert_eq!(
        serde_json::from_str::<Value>(request["contents"][0]["parts"][0]["text"].as_str().unwrap())
            .unwrap(),
        json!({"count":2})
    );
    assert_eq!(request["contents"][1], history[0]);
    assert_eq!(request["contents"][2], history[1]);
    let snapshot = f.snapshot(&output).await;
    assert_eq!(
        snapshot["prepared"]["evaluation"]["items"][0]["sources"],
        json!(["history"])
    );
    let messages = base.with_program(vec![fragment(
        "conversation",
        FragmentRole::Data,
        FragmentFormat::AdkMessages,
        ContextExpr::resource("history"),
    )]);
    for (path, history) in [
        ("orphan", json!([history[1]])),
        ("malformed", json!([{"wrong":"not content"}])),
    ] {
        let config = configured(&messages, bindings.clone());
        let node = models::node_with_services("agent", &config, path, f.services.clone()).unwrap();
        let ctx = NodeContext::new(
            State::from([("history".into(), history)]),
            ExecutionConfig::new(&f.services.id),
            0,
        );
        assert!(
            node.execute(&ctx).await.is_err(),
            "{path} must never become ordinary JSON implicitly"
        );
    }
}

#[tokio::test]
async fn selected_history_can_be_projected_split_or_rendered_explicitly_without_orphan_tools() {
    let f = Fixture::new().await;
    let item_type = DataType::Record {
        fields: BTreeMap::from([
            ("role".into(), DataType::Text),
            (
                "parts".into(),
                DataType::List {
                    item: Box::new(DataType::Record {
                        fields: BTreeMap::new(),
                    }),
                },
            ),
        ]),
    };
    let call = json!(Content {
        role: "model".into(),
        parts: vec![Part::FunctionCall {
            name: "read".into(),
            args: json!({"path":"guide.md"}),
            id: Some("selected-call".into()),
            thought_signature: None,
        }]
    });
    let response = json!(Content {
        role: "user".into(),
        parts: vec![Part::FunctionResponse {
            function_response: FunctionResponseData::new("read", json!({"content":"large source"})),
            id: Some("selected-call".into()),
            annotations: None,
        }]
    });
    let history = json!([
        call,
        response,
        Content::new("model").with_text("Excluded by selection")
    ]);
    let requirement = || {
        ContextStrategy::new("selected", "Selected history").require(
            "history",
            DataType::List {
                item: Box::new(item_type.clone()),
            },
        )
    };
    let select = ContextExpr::take(ContextExpr::resource("history"), 2);
    let projected = ContextExpr::map(
        select.clone(),
        "message",
        ContextExpr::project(ContextExpr::variable("message"), &["role", "parts"]),
    );
    let bindings = json!({"history":{"kind":"state","field":"history","encoding":"adkMessages"}});
    let strategy = requirement().with_program(vec![fragment(
        "selection",
        FragmentRole::Data,
        FragmentFormat::Json,
        projected,
    )]);
    let selected = f
        .invoke(
            "projected",
            &configured(&strategy, bindings.clone()),
            State::from([("history".into(), history.clone())]),
        )
        .await;
    assert_eq!(echoed(&selected)["contents"], json!([call, response]));

    let by_role = |role: &str| {
        ContextExpr::filter(
            select.clone(),
            "message",
            ContextPredicate::Eq {
                left: ContextExpr::field(ContextExpr::variable("message"), "role"),
                right: ContextExpr::literal(DataType::Text, json!(role)),
            },
        )
    };
    let split = requirement().with_program(vec![
        fragment(
            "calls",
            FragmentRole::Data,
            FragmentFormat::Json,
            by_role("model"),
        ),
        fragment(
            "results",
            FragmentRole::Data,
            FragmentFormat::Json,
            by_role("user"),
        ),
    ]);
    let split_output = f
        .invoke(
            "split",
            &configured(&split, bindings.clone()),
            State::from([("history".into(), history.clone())]),
        )
        .await;
    assert_eq!(echoed(&split_output)["contents"], json!([call, response]));

    let rendered = requirement().with_program(vec![text(
        "explicit-json",
        ContextExpr::to_json(select.clone()),
    )]);
    let rendered = f
        .invoke(
            "rendered",
            &configured(&rendered, bindings.clone()),
            State::from([("history".into(), history.clone())]),
        )
        .await;
    let request = echoed(&rendered);
    assert_eq!(request["contents"].as_array().unwrap().len(), 1);
    let text = request["contents"][0]["parts"][0]["text"].as_str().unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(text).unwrap(),
        json!([call, response])
    );

    for (name, program) in [
        (
            "orphan",
            vec![fragment(
                "only-result",
                FragmentRole::Data,
                FragmentFormat::Json,
                by_role("user"),
            )],
        ),
        (
            "duplicate",
            vec![
                fragment(
                    "first",
                    FragmentRole::Data,
                    FragmentFormat::Json,
                    select.clone(),
                ),
                fragment("again", FragmentRole::Data, FragmentFormat::Json, select),
            ],
        ),
    ] {
        let invalid = configured(&requirement().with_program(program), bindings.clone());
        let node = models::node_with_services("agent", &invalid, name, f.services.clone()).unwrap();
        let ctx = NodeContext::new(
            State::from([("history".into(), history.clone())]),
            ExecutionConfig::new(&f.services.id),
            0,
        );
        assert!(
            node.execute(&ctx).await.is_err(),
            "{name} must fail before provider request"
        );
    }
}

#[tokio::test]
async fn media_refs_resolve_exact_bytes_and_unsupported_modalities_fail_explicitly() {
    let f = Fixture::new().await;
    let bytes = json!({"encoding":"base64","byteLength":3,"chunks":["AQID"]});
    let reference = f.store.intern(&bytes).await.unwrap();
    let strategy = ContextStrategy::new("media", "Media")
        .require(
            "image",
            DataType::Media {
                media_type: "image/png".into(),
            },
        )
        .with_program(vec![fragment(
            "image",
            FragmentRole::Data,
            FragmentFormat::Media,
            ContextExpr::resource("image"),
        )]);
    let config = configured(&strategy, json!({"image":{"kind":"state","field":"image"}}));
    let output = f
        .invoke(
            "agent",
            &config,
            State::from([(
                "image".into(),
                json!({"mediaType":"image/png","contentRef":reference}),
            )]),
        )
        .await;
    let contents: Vec<Content> =
        serde_json::from_value(echoed(&output)["contents"].clone()).unwrap();
    assert!(
        matches!(&contents[0].parts[0],Part::InlineData {mime_type,data,..} if mime_type=="image/png" && data==&[1,2,3])
    );
    let audio_strategy = ContextStrategy::new("audio", "Audio")
        .require(
            "audio",
            DataType::Media {
                media_type: "audio/wav".into(),
            },
        )
        .with_program(vec![fragment(
            "audio",
            FragmentRole::Data,
            FragmentFormat::Media,
            ContextExpr::resource("audio"),
        )]);
    let audio = configured(
        &audio_strategy,
        json!({"audio":{"kind":"state","field":"audio"}}),
    );
    let error = inference::prepare(
        &audio,
        &inference::program(&audio["contextProgram"]).unwrap(),
        &f.services,
        "agent",
        &State::from([(
            "audio".into(),
            json!({"mediaType":"audio/wav","contentRef":reference}),
        )]),
        "codex",
    )
    .await
    .err()
    .unwrap();
    assert!(
        error
            .to_string()
            .contains("Unsupported modality for codex: audio/wav")
    );
}

struct Route(AtomicUsize);
#[async_trait::async_trait]
impl DynamicCapabilities for Route {
    fn tools(&self, path: &str) -> Vec<Value> {
        if path == "agent" {
            vec![
                json!({"name":"delegate","description":"Explicit route","parameters":{"type":"object","properties":{"task":{"type":"string"}},"required":["task"]}}),
            ]
        } else {
            vec![]
        }
    }
    async fn invoke(
        &self,
        _path: &str,
        _call_id: &str,
        _name: &str,
        arguments: Value,
    ) -> anyhow::Result<Value> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(json!({"task":arguments["task"],"status":"done"}))
    }
}

#[tokio::test]
async fn selected_dynamic_capability_requires_grant_and_durable_provenance_before_any_effect() {
    let f = Fixture::new().await;
    let route = Arc::new(Route(AtomicUsize::new(0)));
    f.services.set_dynamic_capabilities(route.clone());
    let capability = ContextCapability::new(
        "delegate",
        DataType::Record {
            fields: BTreeMap::from([("task".into(), DataType::Text)]),
        },
        DataType::Record {
            fields: BTreeMap::new(),
        },
    );
    let strategy = ContextStrategy::new("tool", "Tool")
        .capability(capability.clone())
        .with_program(vec![text(
            "request",
            ContextExpr::literal(DataType::Text, json!("delegate explicitly")),
        )]);
    let mut config = configured(&strategy, json!({}));
    assert!(models::node_with_services("agent", &config, "agent", f.services.clone()).is_err());
    config["capabilityGrants"] = json!([capability]);
    config["fixtureSteps"] = json!([{"tool":"delegate","args":{"task":"one"}}]);
    let result = f.invoke("agent", &config, State::new()).await;
    assert_eq!(route.0.load(Ordering::SeqCst), 0);
    let state = result.updates.clone();
    let tool_config = json!({"__zedflowVersion":3,"tool":"execute_next_call"});
    let mut forged = state.clone();
    forged.get_mut("toolCalls").unwrap()[0]["args"]["task"] = json!("forged");
    let denied = operations::execute_with_services(
        "tool",
        &tool_config,
        NodeContext::new(forged, ExecutionConfig::new(&f.services.id), 1),
        "dispatch",
        f.services.clone(),
    )
    .await
    .unwrap();
    assert_eq!(denied.updates["toolResults"][0]["result"]["denied"], true);
    assert_eq!(route.0.load(Ordering::SeqCst), 0);
    for _ in 0..2 {
        let executed = operations::execute_with_services(
            "tool",
            &tool_config,
            NodeContext::new(state.clone(), ExecutionConfig::new(&f.services.id), 1),
            "dispatch",
            f.services.clone(),
        )
        .await
        .unwrap();
        assert_eq!(executed.updates["toolResults"][0]["result"]["task"], "one");
    }
    assert_eq!(route.0.load(Ordering::SeqCst), 1);
}
