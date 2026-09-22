use adk_graph::prelude::*;
use anyhow::Result;
use serde_json::{Value, json};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use zf_context::context::*;
use zf_context::context_source;
use zf_context::window;
use zf_context::window::WindowPatch;
use zf_core::identity::Permission;
use zf_core::identity::Revision;
use zf_core::identity::Scope;
use zf_runtime::agent_capabilities;
use zf_runtime::agent_capabilities::EffectiveContext;
use zf_runtime::inference;
use zf_storage::content_store::ContentStore;
use zf_storage::data::DataRegistry;
use zf_storage::data::WindowRegistry;

use zf_context::window_preparation::WindowSelectionCommand;
use zf_runtime::models;
use zf_runtime::runtime::BranchInvocation;
use zf_runtime::runtime::DynamicCapabilities;
use zf_runtime::runtime::RouteOutcome;
use zf_runtime::runtime::RunServices;
use zf_runtime::workspace_context::ContextSnapshot;

struct Fixture {
    _root: tempfile::TempDir,
    services: Arc<RunServices>,
    store: ContentStore,
}
impl Fixture {
    async fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(root.path().join("data.sqlite"))
                    .create_if_missing(true)
                    .foreign_keys(true),
            )
            .await
            .unwrap();
        let store = ContentStore::new(pool.clone()).await.unwrap();
        let services = RunServices::new(
            "preparation".into(),
            root.path().into(),
            root.path().join("data"),
            ContextSnapshot {
                cwd: root.path().into(),
                ..Default::default()
            },
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
    fn ctx(&self, step: usize, text: &str) -> NodeContext {
        NodeContext::new(
            State::from([("input".into(), json!(text))]),
            ExecutionConfig::new("preparation/root"),
            step,
        )
    }
    async fn run(&self, cfg: &Value, ctx: &NodeContext) -> NodeOutput {
        models::node_with_services("agent", cfg, "root/agent", self.services.clone())
            .unwrap()
            .execute(ctx)
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
fn text_strategy() -> ContextStrategy {
    ContextStrategy::new("prepare", "Prepare")
        .require("input", zf_core::types::DataType::Text)
        .with_program(vec![ContextBlock::emit(
            "prompt",
            FragmentRole::Data,
            FragmentFormat::Text,
            ContextExpr::resource("input"),
        )])
}
fn config(strategy: &ContextStrategy, bindings: Value) -> Value {
    let source = context_source::generate(strategy).unwrap();
    use sha2::Digest;
    json!({"__zedflowVersion":3,"provider":"fixture","contextProgram":{"strategy":strategy,"source":source,"hash":format!("{:x}",sha2::Sha256::digest(source.as_bytes())),"types":{},"bindings":bindings},"fixtureSteps":[{"echoRequest":true}]})
}
fn prompt(output: &NodeOutput) -> String {
    let echoed: Value = serde_json::from_str(output.updates["output"].as_str().unwrap()).unwrap();
    echoed["contents"][0]["parts"][0]["text"]
        .as_str()
        .unwrap()
        .into()
}

#[tokio::test]
async fn actual_passage_captures_revision_retries_exactly_and_consumes_one_future_choice() {
    let f = Fixture::new().await;
    let mut cfg = config(
        &text_strategy(),
        json!({"input":{"kind":"state","field":"input"}}),
    );
    cfg["contextProgram"]["window"] = json!({"alias":"agent-window"});
    let original = f.run(&cfg, &f.ctx(0, "first")).await;
    assert_eq!(prompt(&original), "first");
    let snapshot = f.snapshot(&original).await;
    let revision: Revision =
        serde_json::from_value(snapshot["prepared"]["window"]["revision"].clone()).unwrap();
    assert_eq!(
        snapshot["prepared"]["window"]["origin"]["threadId"],
        "preparation/root"
    );
    assert_eq!(snapshot["prepared"]["window"]["origin"]["step"], 0);
    let registry = WindowRegistry::new(f.services.data_registry().unwrap());
    let edited = registry
        .patch_unique(
            &Scope::Flow("root".into()),
            "agent-window",
            &revision,
            &[WindowPatch::Representation {
                id: "prompt".into(),
                format: FragmentFormat::Text,
                value: json!("edited explicitly"),
            }],
            "manual-edit",
        )
        .await
        .unwrap();
    let command = WindowSelectionCommand {
        id: uuid::Uuid::new_v4().to_string(),
        node_path: "root/agent".into(),
        alias: "agent-window".into(),
        revision: edited.revision.clone(),
        program_hash: zf_runtime::inference::program(&cfg["contextProgram"])
            .unwrap()
            .revision()
            .unwrap(),
    };
    zf_runtime::resources::window_preparation::queue_selection(&f.services, &command)
        .await
        .unwrap();
    zf_runtime::resources::window_preparation::queue_selection(&f.services, &command)
        .await
        .unwrap();
    let retry = f
        .run(
            &cfg,
            &f.ctx(0, "changed state cannot rewrite a captured passage"),
        )
        .await;
    assert_eq!(prompt(&retry), "first");
    let selected = f.run(&cfg, &f.ctx(1, "second")).await;
    assert_eq!(prompt(&selected), "edited explicitly");
    assert_eq!(
        f.snapshot(&selected).await["prepared"]["window"]["selectionCommandId"],
        command.id
    );
    let next = f.run(&cfg, &f.ctx(2, "third")).await;
    assert_eq!(prompt(&next), "third");
    assert_eq!(
        window::decode(
            &registry
                .revision(&Scope::Flow("root".into()), "agent-window", &revision)
                .await
                .unwrap()
                .value
        )
        .unwrap()
        .items
        .len(),
        1
    );
    assert_eq!(
        f.store
            .records(&f.services.id)
            .await
            .unwrap()
            .iter()
            .filter(|r| r.kind == "window-selection-used")
            .count(),
        1
    );
}

#[tokio::test]
async fn window_selection_refuses_changed_linked_types_even_when_strategy_source_is_identical() {
    let f = Fixture::new().await;
    let mut original = config(
        &text_strategy(),
        json!({"input":{"kind":"state","field":"input"}}),
    );
    original["contextProgram"]["window"] = json!({"alias":"versioned-window"});
    let output = f
        .run(&original, &f.ctx(0, "captured before publication"))
        .await;
    let snapshot = f.snapshot(&output).await;
    let revision: Revision =
        serde_json::from_value(snapshot["prepared"]["window"]["revision"].clone()).unwrap();
    let program_hash = inference::program(&original["contextProgram"])
        .unwrap()
        .revision()
        .unwrap();
    zf_runtime::resources::window_preparation::queue_selection(
        &f.services,
        &WindowSelectionCommand {
            id: uuid::Uuid::new_v4().to_string(),
            node_path: "root/agent".into(),
            alias: "versioned-window".into(),
            revision,
            program_hash: program_hash.clone(),
        },
    )
    .await
    .unwrap();
    let mut next = original.clone();
    next["contextProgram"]["types"] = json!({"AdditionalDomainType":{"kind":"number"}});
    assert_eq!(
        next["contextProgram"]["hash"],
        original["contextProgram"]["hash"]
    );
    assert_ne!(
        inference::program(&next["contextProgram"])
            .unwrap()
            .revision()
            .unwrap(),
        program_hash
    );
    let node =
        models::node_with_services("agent", &next, "root/agent", f.services.clone()).unwrap();
    let error = node
        .execute(&f.ctx(1, "must not use obsolete prepared text"))
        .await
        .err()
        .unwrap();
    assert!(error.to_string().contains("no longer matches"), "{error}");
    assert_eq!(
        f.store
            .records(&f.services.id)
            .await
            .unwrap()
            .iter()
            .filter(|r| r.kind == "model-calls")
            .count(),
        1
    );
}

#[tokio::test]
async fn window_capability_checks_frozen_alias_grant_and_recovers_started_patch_receipt() {
    let f = Fixture::new().await;
    let mut cfg = config(
        &text_strategy(),
        json!({"input":{"kind":"state","field":"input"}}),
    );
    cfg["contextProgram"]["window"] = json!({"alias":"agent-window"});
    let original = f.run(&cfg, &f.ctx(0, "original")).await;
    let raw = f.snapshot(&original).await;
    let revision: Revision =
        serde_json::from_value(raw["prepared"]["window"]["revision"].clone()).unwrap();
    let data = f.services.data_registry().unwrap();
    data.grant(
        &Scope::Flow("root".into()),
        "agent-window",
        &Scope::Flow("editor".into()),
        "target",
        Permission::Write,
    )
    .await
    .unwrap();
    let registry = WindowRegistry::new(data);
    let snapshot = EffectiveContext {
        invocation_id: uuid::Uuid::new_v4().to_string(),
        agent_path: "editor/agent".into(),
        origin: json!({"nodePath":"editor/agent","occurrenceId":"real-editor-passage"}),
        tools: vec!["context_window_patch".into()],
        system: String::new(),
        files: String::new(),
        resources: vec![],
        skill_catalog: vec![],
        prepared: Some(json!({"windowGrants":[{"alias":"target","permission":"write"}]})),
    };
    agent_capabilities::persist_snapshot(&f.services, &snapshot)
        .await
        .unwrap();
    let args = json!({"alias":"target","expectedRevision":revision,"patches":[{"kind":"representation","id":"prompt","format":"text","value":"editor revision"}]});
    let mut calls = vec![json!({"id":"provider-call","name":"context_window_patch","args":args})];
    agent_capabilities::seal_calls(&f.services, &snapshot, &mut calls)
        .await
        .unwrap();
    let (owner, call_id) = agent_capabilities::authorize_call(&f.services, &calls[0])
        .await
        .unwrap();
    let known = zf_runtime::resources::window::execute_tool(
        &f.services,
        &owner,
        &call_id,
        "context_window_patch",
        &args,
    )
    .await
    .unwrap();
    use sha2::Digest;
    let key = format!(
        "{:x}",
        sha2::Sha256::digest(format!("{}\0{owner}\0{call_id}", f.services.id))
    );
    f.store
        .put_record(
            &f.services.id,
            "receipts",
            &key,
            &json!({"name":"context_window_patch","arguments":args,"status":"started"}),
        )
        .await
        .unwrap();
    let recovered = f
        .services
        .execute_tool(&owner, &call_id, "context_window_patch", args.clone())
        .await
        .unwrap();
    assert_eq!(recovered, known);
    let head = registry
        .read(&Scope::Flow("root".into()), "agent-window")
        .await
        .unwrap();
    assert_eq!(json!(head.revision), known["revision"]);
    assert!(
        zf_runtime::resources::window::execute_tool(
            &f.services,
            "intruder/agent",
            &call_id,
            "context_window_patch",
            &args
        )
        .await
        .is_err()
    );
    let mut forged = args;
    forged["alias"] = json!("other-target");
    assert!(
        zf_runtime::resources::window::execute_tool(
            &f.services,
            &owner,
            &call_id,
            "context_window_patch",
            &forged
        )
        .await
        .is_err()
    );
    assert_eq!(
        registry
            .read(&Scope::Flow("root".into()), "agent-window")
            .await
            .unwrap()
            .revision,
        head.revision
    );
}

struct ProducerHost {
    version: String,
    calls: Mutex<Vec<BranchInvocation>>,
    waiting: AtomicBool,
    fail: AtomicBool,
}
#[async_trait::async_trait]
impl DynamicCapabilities for ProducerHost {
    fn tools(&self, _: &str) -> Vec<Value> {
        vec![]
    }
    async fn route_contract(&self, _: &str, _: &str, route: Option<&str>) -> Result<Value> {
        Ok(
            json!({"routeId":route,"mode":"callAwait","invocation":"context","targetHash":self.version,"input":{"kind":"text"},"output":{"kind":"text"}}),
        )
    }
    async fn invoke_branch(&self, call: BranchInvocation) -> Result<RouteOutcome> {
        self.calls.lock().unwrap().push(call.clone());
        if self.fail.load(Ordering::SeqCst) {
            anyhow::bail!("producer failed explicitly");
        }
        if self.waiting.swap(false, Ordering::SeqCst) {
            return Ok(RouteOutcome::Waiting {
                visit_id: "producer-visit".into(),
                thread_id: "native-child".into(),
                wait: json!({"kind":"model_selection","nodePath":"producer/agent"}),
            });
        }
        Ok(RouteOutcome::Completed {
            visit_id: "producer-visit".into(),
            thread_id: "native-child".into(),
            result: if call.input == "empty" {
                json!("")
            } else {
                json!(format!("{}:{}", self.version, call.input.as_str().unwrap()))
            },
        })
    }
    async fn invoke(&self, _: &str, _: &str, _: &str, _: Value) -> Result<Value> {
        anyhow::bail!("not a tool route")
    }
}
fn producer_host(version: &str, waiting: bool) -> Arc<ProducerHost> {
    Arc::new(ProducerHost {
        version: version.into(),
        calls: Mutex::new(vec![]),
        waiting: AtomicBool::new(waiting),
        fail: AtomicBool::new(false),
    })
}
fn producer_config() -> Value {
    let strategy = text_strategy()
        .require("derived", zf_core::types::DataType::Text)
        .with_program(vec![ContextBlock::emit(
            "derived",
            FragmentRole::Data,
            FragmentFormat::Text,
            ContextExpr::resource("derived"),
        )]);
    config(
        &strategy,
        json!({"input":{"kind":"state","field":"input"},"derived":{"kind":"produced","producer":{"branch":"produce","routeId":"root/producer","input":{"kind":"resource","name":"input"}}}}),
    )
}

#[tokio::test]
async fn reader_is_acquired_when_only_a_demanded_producer_needs_it_and_changed_source_invalidates_result()
 {
    let f = Fixture::new().await;
    let host = producer_host("derive", false);
    f.services.set_dynamic_capabilities(host.clone());
    let mut cfg = producer_config();
    cfg["contextProgram"]["bindings"]["input"] = json!({"kind":"reader","reader":"file.text","input":{"kind":"literal","value":{"path":"producer-input.md"}}});
    std::fs::write(f.services.cwd.join("producer-input.md"), "first").unwrap();
    let output = f.run(&cfg, &f.ctx(0, "not implicitly selected")).await;
    assert_eq!(prompt(&output), "derive:first");
    assert_eq!(host.calls.lock().unwrap().len(), 1);
    assert_eq!(
        prompt(&f.run(&cfg, &f.ctx(1, "same source")).await),
        "derive:first"
    );
    assert_eq!(
        host.calls.lock().unwrap().len(),
        1,
        "Durable result reuses exact source identity"
    );
    std::fs::write(f.services.cwd.join("producer-input.md"), "second").unwrap();
    assert_eq!(
        prompt(&f.run(&cfg, &f.ctx(2, "changed source")).await),
        "derive:second"
    );
    assert_eq!(host.calls.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn producers_are_demanded_lazily_cache_empty_values_and_invalidate_inputs_and_target_revision()
 {
    let f = Fixture::new().await;
    let host = producer_host("v1", false);
    f.services.set_dynamic_capabilities(host.clone());
    let cfg = producer_config();
    assert_eq!(prompt(&f.run(&cfg, &f.ctx(0, "a")).await), "v1:a");
    assert_eq!(prompt(&f.run(&cfg, &f.ctx(1, "a")).await), "v1:a");
    assert_eq!(host.calls.lock().unwrap().len(), 1);
    assert_eq!(prompt(&f.run(&cfg, &f.ctx(2, "empty")).await), "");
    assert_eq!(prompt(&f.run(&cfg, &f.ctx(3, "empty")).await), "");
    assert_eq!(host.calls.lock().unwrap().len(), 2);
    let v2 = producer_host("v2", false);
    f.services.set_dynamic_capabilities(v2.clone());
    assert_eq!(prompt(&f.run(&cfg, &f.ctx(4, "a")).await), "v2:a");
    assert_eq!(v2.calls.lock().unwrap().len(), 1);
    let mut inactive = producer_config();
    let strategy = text_strategy().require("derived", zf_core::types::DataType::Text);
    inactive["contextProgram"] =
        config(&strategy, inactive["contextProgram"]["bindings"].clone())["contextProgram"].clone();
    assert_eq!(
        prompt(&f.run(&inactive, &f.ctx(5, "inactive")).await),
        "inactive"
    );
    assert_eq!(v2.calls.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn producer_wait_resumes_same_identity_and_never_calls_model_before_result() {
    let f = Fixture::new().await;
    let host = producer_host("v1", true);
    f.services.set_dynamic_capabilities(host.clone());
    let cfg = producer_config();
    let ctx = f.ctx(7, "question");
    let waiting = f.run(&cfg, &ctx).await;
    assert!(waiting.interrupt.is_some());
    assert_eq!(
        waiting.updates["contextNeeds"]["kind"],
        "context_production"
    );
    assert!(!waiting.updates.contains_key("modelResponse"));
    assert_eq!(prompt(&f.run(&cfg, &ctx).await), "v1:question");
    let calls = host.calls.lock().unwrap();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].call_id, calls[1].call_id);
    assert_eq!(calls[0].input, calls[1].input);
    assert_eq!(
        calls[0].invocation,
        zf_flows::composition::InvocationKind::Context
    );
}

#[tokio::test]
async fn active_producer_cycles_and_incompatible_contracts_fail_before_any_effect() {
    let f = Fixture::new().await;
    let host = producer_host("v1", false);
    f.services.set_dynamic_capabilities(host.clone());
    let mut cfg = producer_config();
    cfg["contextProgram"]["bindings"]["input"] = json!({"kind":"produced","producer":{"branch":"produce","routeId":"root/producer","input":{"kind":"resource","name":"derived"}}});
    let program = inference::program(&cfg["contextProgram"]).unwrap();
    let error = inference::prepare_at(
        &cfg,
        &program,
        &f.services,
        "root/agent",
        &f.ctx(0, "unused"),
        "fixture",
    )
    .await
    .err()
    .unwrap();
    assert!(error.to_string().contains("producer_cycle"));
    assert!(host.calls.lock().unwrap().is_empty());
    let mut mismatch = producer_config();
    mismatch["contextProgram"]["bindings"]["derived"]["producer"]["input"] =
        json!({"kind":"literal","dataType":{"kind":"number"},"value":123});
    // Literal syntax is decoded by the typed AST; the route's Text input cannot coerce it.
    let program = inference::program(&mismatch["contextProgram"]).unwrap();
    assert!(
        inference::prepare_at(
            &mismatch,
            &program,
            &f.services,
            "root/agent",
            &f.ctx(1, "unused"),
            "fixture"
        )
        .await
        .is_err()
    );
    assert!(host.calls.lock().unwrap().is_empty());
}

fn graph_node(id: &str, kind: &str, mut config: Value) -> Value {
    config.as_object_mut().unwrap().remove("__zedflowVersion");
    json!({"id":id,"position":{"x":0,"y":0},"data":{"kind":kind,"label":id,"config":config}})
}
fn graph_edge(from: &str, to: &str) -> Value {
    json!({"id":format!("{from}-{to}"),"source":from,"target":to})
}

async fn install_native(
    f: &Fixture,
    root: zf_flows::schema::Composition,
    worker: zf_flows::schema::Composition,
    bridge: Value,
) -> Arc<zf_execution::route_runtime::RouteRuntime> {
    use zf_compiler::prepared::FrozenFlow;
    use zf_compiler::prepared::PreparedRuntime;
    use zf_execution::route_runtime::RouteRuntime;
    use zf_flows::composition::CompositionCatalog;
    use zf_flows::composition::ResolveRequest;
    use zf_flows::flow_contract;
    let catalog: CompositionCatalog=serde_json::from_value(json!({"flows":{"root-flow":flow_contract::read(&root).unwrap().unwrap().contract,"worker-flow":flow_contract::read(&worker).unwrap().unwrap().contract},"bridges":{"bridge":bridge}})).unwrap();
    let graph = zf_compiler::resolve::resolve(
        &catalog,
        &ResolveRequest {
            flow: "root-flow".into(),
            entry: "main".into(),
            bridges: vec!["bridge".into()],
        },
    )
    .unwrap();
    let freeze = |composition: zf_flows::schema::Composition| {
        let source = zf_flows::flow_source::render(
            &composition,
            &zf_compiler::graph_compiler::GraphValidator::new(
                &zf_runtime::materialize::RuntimePrimitives,
            ),
        )
        .unwrap();
        FrozenFlow {
            key: composition.id.clone(),
            hash: zf_storage::flow_store::hash(source.as_bytes()),
            source,
            exports: flow_contract::validate(&composition).unwrap().unwrap(),
            composition,
        }
    };
    let prepared = PreparedRuntime {
        graph,
        flows: std::collections::BTreeMap::from([
            ("root".into(), freeze(root)),
            ("bridge/worker".into(), freeze(worker)),
        ]),
        definitions: Default::default(),
    };
    let cp = Arc::new(zf_runtime::stored_checkpointer::StoredCheckpointer::new(
        zf_storage::contracts::CheckpointStore::new(f.store.clone())
            .await
            .unwrap(),
    ));
    let host = RouteRuntime::new(prepared, &f.services, cp, None).unwrap();
    f.services.set_dynamic_capabilities(host.clone());
    host.initialize(&State::new()).await.unwrap();
    host
}

#[tokio::test]
async fn native_adk_producer_effect_is_durable_and_reused_by_subsequent_model_passages() {
    let f = Fixture::new().await;
    let mut cfg = producer_config();
    cfg["contextProgram"]["bindings"]["derived"]["producer"]["routeId"] = json!("bridge/call");
    let root_exports = json!({"contract":{"entries":{"main":{"input":{"kind":"text"}}},"branches":{"produce":{"contract":{"input":{"kind":"text"},"output":{"kind":"text"}},"invocations":["context"]}}},"entries":{"main":{"node":"agent","inputField":"input"}},"branches":{"produce":"agent"}});
    let root=serde_json::from_value(json!({"formatVersion":3,"id":"root-flow","name":"Root","nodes":[graph_node("s","start",json!({"exports":root_exports})),graph_node("agent","agent",cfg.clone()),graph_node("e","end",json!({}))],"edges":[graph_edge("s","agent"),graph_edge("agent","e")]})).unwrap();
    let exports = json!({"contract":{"entries":{"main":{"input":{"kind":"text"},"output":{"kind":"text"}}}},"entries":{"main":{"node":"effect","inputField":"input","outputField":"output"}}});
    let worker=serde_json::from_value(json!({"formatVersion":3,"id":"worker-flow","name":"Producer","nodes":[graph_node("s","start",json!({"exports":exports})),graph_node("effect","tool",json!({"tool":"exec","arguments":{"command":"printf x >> producer-effects.txt"}})),graph_node("result","set",json!({"field":"output","value":"{{input}}"})),graph_node("e","end",json!({}))],"edges":[graph_edge("s","effect"),graph_edge("effect","result"),graph_edge("result","e")]})).unwrap();
    let bridge = json!({"imports":{"worker":{"flow":"worker-flow"}},"connections":{"call":{"from":{"instance":"root","port":"produce"},"to":{"instance":"worker","port":"main"},"mode":"callAwait","invocation":"context"}}});
    let _host = install_native(&f, root, worker, bridge).await;
    assert_eq!(
        prompt(&f.run(&cfg, &f.ctx(0, "produced text")).await),
        "produced text"
    );
    assert_eq!(
        prompt(&f.run(&cfg, &f.ctx(1, "produced text")).await),
        "produced text"
    );
    assert_eq!(
        std::fs::read_to_string(f.services.cwd.join("producer-effects.txt")).unwrap(),
        "x"
    );
    assert_eq!(
        f.store
            .records(&f.services.id)
            .await
            .unwrap()
            .iter()
            .filter(|r| r.kind == "route-visits")
            .count(),
        1
    );
}

#[tokio::test]
async fn native_editor_agent_uses_its_own_context_and_bridge_grant_to_patch_the_target_window() {
    use std::collections::BTreeMap;
    use zf_core::types::DataType;
    let f = Fixture::new().await;
    let mut cfg = config(
        &text_strategy(),
        json!({"input":{"kind":"state","field":"input"}}),
    );
    cfg["contextProgram"]["window"] =
        json!({"alias":"prepared","prepare":{"branch":"review","routeId":"bridge/review"}});
    let handle = json!({"kind":"record","fields":{"alias":{"kind":"text"},"entityId":{"kind":"text"},"revision":{"kind":"text"},"contentRef":{"kind":"text"}}});
    let window_type = json!({"kind":"record","fields":{}});
    let root_exports = json!({"contract":{"entries":{"main":{"input":{"kind":"text"}}},"branches":{"review":{"contract":{"input":handle,"output":{"kind":"text"}},"invocations":["context"]}},"data":{"prepared":{"dataType":window_type,"permissions":{"read":true,"write":true}}}},"entries":{"main":{"node":"agent","inputField":"input"}},"branches":{"review":"agent"},"data":{"prepared":"windowChannel"}});
    let root=serde_json::from_value(json!({"formatVersion":3,"id":"root-flow","name":"Root","channels":[{"name":"windowChannel","reducer":"overwrite"}],"nodes":[graph_node("s","start",json!({"exports":root_exports})),graph_node("agent","agent",cfg.clone()),graph_node("e","end",json!({}))],"edges":[graph_edge("s","agent"),graph_edge("agent","e")]})).unwrap();
    let strategy=ContextStrategy::new("editor","Editor own program").require("handle",serde_json::from_value(handle.clone()).unwrap()).capability(window::capability("context_window_patch").unwrap()).with_program(vec![ContextBlock::emit("patch-arguments",FragmentRole::Data,FragmentFormat::Json,ContextExpr::record(BTreeMap::from([
        ("alias".into(),ContextExpr::literal(DataType::Text,json!("target"))),
        ("expectedRevision".into(),ContextExpr::field(ContextExpr::resource("handle"),"revision")),
        ("patches".into(),ContextExpr::literal(DataType::List{item:Box::new(DataType::Record{fields:BTreeMap::new()})},json!([{"kind":"representation","id":"prompt","format":"text","value":"reviewed by the composed editor"}]))),
    ])))]);
    let mut editor = config(
        &strategy,
        json!({"handle":{"kind":"state","field":"input"}}),
    );
    editor["windowGrants"] = json!([{"alias":"target","permission":"write"}]);
    editor["fixtureSteps"] = json!([{"tool":"context_window_patch","argsFromInput":true}]);
    let exports = json!({"contract":{"entries":{"main":{"input":handle,"output":{"kind":"text"}}},"requires":{"target":{"dataType":window_type,"permissions":{"read":true,"write":true}}}},"entries":{"main":{"node":"editor","inputField":"input","outputField":"output"}},"requires":{"target":"targetWindow"}});
    let worker=serde_json::from_value(json!({"formatVersion":3,"id":"worker-flow","name":"Editor","channels":[{"name":"targetWindow","reducer":"overwrite"}],"nodes":[graph_node("s","start",json!({"exports":exports})),graph_node("editor","agent",editor),graph_node("dispatch","tool",json!({"tool":"execute_next_call"})),graph_node("result","set",json!({"field":"output","value":"review complete"})),graph_node("e","end",json!({}))],"edges":[graph_edge("s","editor"),graph_edge("editor","dispatch"),graph_edge("dispatch","result"),graph_edge("result","e")]})).unwrap();
    let bridge = json!({"imports":{"worker":{"flow":"worker-flow"}},"connections":{"review":{"from":{"instance":"root","port":"review"},"to":{"instance":"worker","port":"main"},"mode":"callAwait","invocation":"context"}},"bindings":{"window":{"from":{"instance":"root","port":"prepared"},"to":{"instance":"worker","port":"target"},"permissions":{"read":true,"write":true}}}});
    let _host = install_native(&f, root, worker, bridge).await;
    let result = f.run(&cfg, &f.ctx(0, "target initial text")).await;
    assert_eq!(prompt(&result), "reviewed by the composed editor");
    let target = f.snapshot(&result).await;
    let window = f
        .services
        .data_registry()
        .unwrap()
        .snapshot(&Scope::Flow("root".into()), "prepared")
        .await
        .unwrap();
    assert_eq!(
        target["prepared"]["window"]["revision"],
        json!(window.revision)
    );
    let calls = f.store.records(&f.services.id).await.unwrap();
    let mut editor_snapshot = None;
    for record in calls
        .into_iter()
        .filter(|r| r.kind == "capability-snapshots")
    {
        let snapshot = f.store.resolve(&record.value_ref).await.unwrap();
        if snapshot["agentPath"] == "bridge/worker/editor" {
            editor_snapshot = Some(snapshot);
        }
    }
    let editor_snapshot = editor_snapshot.unwrap();
    assert_eq!(
        editor_snapshot["prepared"]["program"]["strategy"]["id"],
        "editor"
    );
    assert_eq!(editor_snapshot["tools"], json!(["context_window_patch"]));
    assert_eq!(
        editor_snapshot["prepared"]["windowGrants"][0]["alias"],
        "target"
    );
}
