use adk_graph::{
    CompiledGraph, ExecutionConfig, State,
    checkpoint::{Checkpointer, MemoryCheckpointer, SqliteCheckpointer},
};
use serde_json::{Value, json};
use std::sync::Arc;
use zf_flows::schema::Composition;
use zf_runtime::operations;
use zf_runtime::runtime::RunServices;

async fn fixture_services(
    cwd: std::path::PathBuf,
    data: std::path::PathBuf,
) -> anyhow::Result<Arc<RunServices>> {
    let context = zf_runtime::workspace_context::ContextSnapshot::load_with_home(
        &cwd,
        &[],
        Some(&cwd.join("fixture-home")),
    )
    .await?;
    RunServices::new(
        uuid::Uuid::new_v4().to_string(),
        cwd,
        data,
        context,
        json!({}),
        vec![],
    )
}

fn node(id: &str, kind: &str, config: Value) -> Value {
    json!({"id":id,"position":{"x":0,"y":0},"data":{"label":id,"kind":kind,"config":config}})
}
fn flow(nodes: Vec<Value>, edges: &[(&str, &str)]) -> Composition {
    serde_json::from_value(json!({"id":"runtime-test","name":"Runtime test","nodes":nodes,
        "edges":edges.iter().enumerate().map(|(i,(source,target))|json!({"id":i.to_string(),"source":source,"target":target})).collect::<Vec<_>>() })).unwrap()
}
fn wrap(child: &Composition, id: &str) -> Composition {
    flow(
        vec![
            node("s", "start", json!({})),
            node(id, "subgraph", json!({"composition":child})),
            node("out", "output", json!({"text":"{{output}}"})),
            node("e", "end", json!({})),
        ],
        &[("s", id), (id, "out"), ("out", "e")],
    )
}
async fn paused(graph: &CompiledGraph, input: State, config: ExecutionConfig) -> Value {
    match graph.invoke(input, config).await.unwrap_err() {
        adk_graph::error::GraphError::Interrupted(pause) => match pause.interrupt {
            adk_graph::Interrupt::Dynamic { data, .. } => data.unwrap(),
            interrupt => panic!("expected dynamic pause: {interrupt}"),
        },
        error => panic!("expected graph pause: {error}"),
    }
}
fn wait_data(mut value: &Value) -> &Value {
    while value.get("nodePath").is_none() {
        value = value.get("data").expect("nested wait data");
    }
    value
}

#[test]
fn parallel_waits_are_rejected_at_validation_including_nested_runtime_models() {
    let inner = flow(
        vec![
            node("s", "start", json!({})),
            node("model", "agent", json!({"modelBinding":"runtime"})),
            node("e", "end", json!({})),
        ],
        &[("s", "model"), ("model", "e")],
    );
    let mut doc = flow(
        vec![
            node("s", "start", json!({})),
            node("ask", "input", json!({})),
            node("child", "subgraph", json!({"composition":inner})),
            node("e", "end", json!({})),
        ],
        &[("s", "ask"), ("s", "child"), ("ask", "e"), ("child", "e")],
    );
    // Serial admission does not turn independent branches into a single wait.
    doc.settings.max_concurrency = Some(1);
    let error =
        zf_compiler::graph_compiler::validate(&doc, &zf_runtime::materialize::RuntimePrimitives)
            .unwrap_err()
            .to_string();
    assert!(error.contains("attentes parallèles"), "{error}");
    assert!(error.contains("child/model"), "{error}");
    // Export admission now requires a validated source; rejection occurs at
    // that earlier boundary rather than generating invalid Cargo sources.
    let rejected = zf_flows::flow_source::render(
        &doc,
        &zf_compiler::graph_compiler::GraphValidator::new(
            &zf_runtime::materialize::RuntimePrimitives,
        ),
    )
    .and_then(|source| {
        zf_compiler::export::export_single(
            &doc,
            &source,
            None,
            &zf_runtime::materialize::RuntimePrimitives,
            &zf_runtime::runtime_export::support(),
        )
    });
    assert!(rejected.is_err());
}

#[test]
fn mutually_exclusive_waits_and_a_common_wait_after_all_join_remain_valid() {
    let mut conditional = flow(
        vec![
            node("s", "start", json!({})),
            node("route", "condition", json!({"field":"input","equals":true})),
            node("yes", "inbox", json!({})),
            node("no", "input", json!({})),
            node("e", "end", json!({})),
        ],
        &[
            ("s", "route"),
            ("route", "yes"),
            ("route", "no"),
            ("yes", "e"),
            ("no", "e"),
        ],
    );
    conditional.edges[1].source_handle = Some("true".into());
    conditional.edges[2].source_handle = Some("false".into());
    zf_compiler::graph_compiler::validate(
        &conditional,
        &zf_runtime::materialize::RuntimePrimitives,
    )
    .unwrap();
    let mut joined = flow(
        vec![
            node("s", "start", json!({})),
            node("a", "set", json!({"field":"a","value":1})),
            node("b", "set", json!({"field":"b","value":2})),
            node("ask", "inbox", json!({"fanIn":"all"})),
            node("e", "end", json!({})),
        ],
        &[
            ("s", "a"),
            ("s", "b"),
            ("a", "ask"),
            ("b", "ask"),
            ("ask", "e"),
        ],
    );
    zf_compiler::graph_compiler::validate(&joined, &zf_runtime::materialize::RuntimePrimitives)
        .unwrap();
    joined.nodes[3].data.config["fanIn"] = json!("any");
    assert!(
        zf_compiler::graph_compiler::validate(&joined, &zf_runtime::materialize::RuntimePrimitives)
            .unwrap_err()
            .to_string()
            .contains("attentes parallèles"),
        "an any-arrival route cannot synchronize the independent branches"
    );
}

#[test]
fn a_waiting_branch_can_run_alongside_a_branch_that_never_waits() {
    let doc = flow(
        vec![
            node("s", "start", json!({})),
            node("ask", "inbox", json!({})),
            node("work", "set", json!({"field":"work","value":1})),
            node("e", "end", json!({})),
        ],
        &[("s", "ask"), ("s", "work"), ("ask", "e"), ("work", "e")],
    );
    zf_compiler::graph_compiler::validate(&doc, &zf_runtime::materialize::RuntimePrimitives)
        .unwrap();
}

#[tokio::test]
async fn nested_model_wait_rebuilds_without_replaying_effects_or_resetting_child_input() {
    let directory = tempfile::tempdir().unwrap();
    let child = flow(
        vec![
            node("s", "start", json!({})),
            node(
                "change",
                "set",
                json!({"field":"input","value":"{{input}}-child"}),
            ),
            node(
                "effect",
                "tool",
                json!({"tool":"exec","arguments":{"command":"printf x >> visits"}}),
            ),
            node("model", "agent", json!({"modelBinding":"runtime"})),
            node("out", "output", json!({"text":"{{input}}"})),
            node("e", "end", json!({})),
        ],
        &[
            ("s", "change"),
            ("change", "effect"),
            ("effect", "model"),
            ("model", "out"),
            ("out", "e"),
        ],
    );
    let doc = wrap(&wrap(&child, "inner"), "outer");
    let database = format!(
        "sqlite://{}?mode=rwc",
        directory.path().join("checkpoints.db").display()
    );
    let cp: Arc<dyn Checkpointer> = Arc::new(SqliteCheckpointer::new(&database).await.unwrap());
    let services = fixture_services(directory.path().into(), directory.path().join("data"))
        .await
        .unwrap();
    let graph =
        zf_runtime::materialize::build_with_services(&doc, services, None, Some(cp.clone()))
            .unwrap();
    let wait = paused(
        &graph,
        State::from([("input".into(), json!("original"))]),
        ExecutionConfig::new("nested"),
    )
    .await;
    assert_eq!(wait_data(&wait)["nodePath"], "outer/inner/model");
    assert_eq!(wait_data(&wait)["kind"], "model_selection");
    assert_eq!(
        std::fs::read_to_string(directory.path().join("visits")).unwrap(),
        "x"
    );
    let saved = cp.load("nested").await.unwrap().unwrap();
    drop(graph);
    drop(cp);
    let cp: Arc<dyn Checkpointer> = Arc::new(SqliteCheckpointer::new(&database).await.unwrap());
    let services = fixture_services(directory.path().into(), directory.path().join("data"))
        .await
        .unwrap();
    services.set_binding("outer/inner/model".into(), json!({"provider":"fixture"}));
    let graph =
        zf_runtime::materialize::build_with_services(&doc, services, None, Some(cp)).unwrap();
    let state = graph
        .invoke(
            State::new(),
            ExecutionConfig::new("nested").with_resume_from(&saved.checkpoint_id),
        )
        .await
        .unwrap();
    assert_eq!(state["response"], "original-child");
    assert_eq!(
        std::fs::read_to_string(directory.path().join("visits")).unwrap(),
        "x"
    );
}

#[tokio::test]
async fn reentering_a_completed_subgraph_creates_a_new_occurrence() {
    let directory = tempfile::tempdir().unwrap();
    let child = flow(
        vec![
            node("s", "start", json!({})),
            node(
                "effect",
                "tool",
                json!({"tool":"exec","arguments":{"command":"printf x >> visits"}}),
            ),
            node("out", "output", json!({"text":"done"})),
            node("e", "end", json!({})),
        ],
        &[("s", "effect"), ("effect", "out"), ("out", "e")],
    );
    let mut doc = flow(
        vec![
            node("s", "start", json!({})),
            node(
                "child",
                "subgraph",
                json!({"composition":child,"fanIn":"any"}),
            ),
            node("count", "set", json!({"field":"visits","value":1})),
            node("route", "condition", json!({"field":"visits","equals":2})),
            node("e", "end", json!({})),
        ],
        &[
            ("s", "child"),
            ("child", "count"),
            ("count", "route"),
            ("route", "e"),
            ("route", "child"),
        ],
    );
    doc.edges[3].source_handle = Some("true".into());
    doc.edges[4].source_handle = Some("false".into());
    let mut value = serde_json::to_value(doc).unwrap();
    value["channels"] = json!([{"name":"visits","reducer":"sum","default":0}]);
    // ADK counter values are floats; condition equality is exact JSON equality.
    value["nodes"][3]["data"]["config"]["equals"] = json!(2.0);
    let doc = serde_json::from_value(value).unwrap();
    let services = fixture_services(directory.path().into(), directory.path().join("data"))
        .await
        .unwrap();
    let graph = zf_runtime::materialize::build_with_services(&doc, services, None, None).unwrap();
    let state = graph
        .invoke(State::new(), ExecutionConfig::new("reentry"))
        .await
        .unwrap();
    assert_eq!(state["visits"], 2.0);
    assert_eq!(
        std::fs::read_to_string(directory.path().join("visits")).unwrap(),
        "xx"
    );
}

#[tokio::test]
async fn nested_inbox_accepts_distinct_answers_and_does_not_reconsume_old_parent_answer() {
    let directory = tempfile::tempdir().unwrap();
    let child = flow(
        vec![
            node("s", "start", json!({})),
            node("ask", "inbox", json!({"fanIn":"any"})),
            node(
                "remember",
                "set",
                json!({"field":"answers","value":["received"]}),
            ),
        ],
        &[("s", "ask"), ("ask", "remember"), ("remember", "ask")],
    );
    let mut child_value = serde_json::to_value(child).unwrap();
    child_value["channels"] = json!([{"name":"answers","reducer":"append","default":[]}]);
    let doc = wrap(&serde_json::from_value(child_value).unwrap(), "child");
    let cp: Arc<dyn Checkpointer> = Arc::new(MemoryCheckpointer::new());
    let services = fixture_services(directory.path().into(), directory.path().join("data"))
        .await
        .unwrap();
    let graph =
        zf_runtime::materialize::build_with_services(&doc, services, None, Some(cp.clone()))
            .unwrap();
    let wait = paused(&graph, State::new(), ExecutionConfig::new("inbox")).await;
    let child_thread = wait["thread"].as_str().unwrap();
    for (index, answer) in [(1, "same"), (2, "same")] {
        let saved = cp.load("inbox").await.unwrap().unwrap();
        paused(
            &graph,
            State::from([(
                "answer:child/ask".into(),
                json!({"__zedflowAnswerId":format!("wait-{index}"),"value":answer}),
            )]),
            ExecutionConfig::new("inbox").with_resume_from(&saved.checkpoint_id),
        )
        .await;
        let child = cp.load(child_thread).await.unwrap().unwrap();
        assert_eq!(child.state["answers"].as_array().unwrap().len(), index);
        assert_eq!(child.state["input"], answer);
        let saved = cp.load("inbox").await.unwrap().unwrap();
        paused(
            &graph,
            State::new(),
            ExecutionConfig::new("inbox").with_resume_from(&saved.checkpoint_id),
        )
        .await;
        let child = cp.load(child_thread).await.unwrap().unwrap();
        assert_eq!(
            child.state["answers"].as_array().unwrap().len(),
            index,
            "stale parent answer must not count again"
        );
    }
}

#[tokio::test]
async fn each_tool_commits_before_steering_and_failures_remain_model_results() {
    let directory = tempfile::tempdir().unwrap();
    let services = fixture_services(directory.path().into(), directory.path().join("data"))
        .await
        .unwrap();
    services.replace_queue(vec![
        json!({"id":"steer","kind":"steering","text":"new instruction","status":"pending"}),
        json!({"id":"follow","kind":"followup","text":"next task","status":"pending"}),
    ]);
    let calls = json!([{"id":"first","name":"format_text","args":{"text":"hello","mode":"uppercase"}},{"id":"second","name":"read","args":{"path":"missing.txt"}}]);
    let mut state = State::from([
        ("toolCalls".into(), calls),
        ("hasToolCalls".into(), json!(true)),
    ]);
    let tools = json!({"nodeId":"tools","tool":"execute_next_call"});
    let ctx = |state: State, step| {
        adk_graph::NodeContext::new(state, ExecutionConfig::new("batch"), step)
    };
    let first = operations::execute_with_services(
        "tool",
        &tools,
        ctx(state.clone(), 0),
        "tools",
        services.clone(),
    )
    .await
    .unwrap();
    state.extend(first.updates);
    assert_eq!(state["toolCalls"].as_array().unwrap().len(), 1);
    assert!(
        operations::execute_with_services(
            "steering",
            &json!({"nodeId":"control"}),
            ctx(state.clone(), 1),
            "control",
            services.clone()
        )
        .await
        .is_err()
    );
    let second = operations::execute_with_services(
        "tool",
        &tools,
        ctx(state.clone(), 2),
        "tools",
        services.clone(),
    )
    .await
    .unwrap();
    state.extend(second.updates);
    assert_eq!(state["hasToolCalls"], false);
    assert_eq!(state["toolResults"].as_array().unwrap().len(), 2);
    assert!(state["toolResults"][1]["result"]["error"].is_string());
    let steering = operations::execute_with_services(
        "steering",
        &json!({"nodeId":"control"}),
        ctx(state.clone(), 3),
        "control",
        services.clone(),
    )
    .await
    .unwrap();
    state.extend(steering.updates);
    assert_eq!(state["input"], "new instruction");
    assert_eq!(state["__zedflow:consumedMessages"], json!(["steer"]));
    let inbox = operations::execute_with_services(
        "inbox",
        &json!({"nodeId":"inbox"}),
        ctx(state, 4),
        "inbox",
        services,
    )
    .await
    .unwrap();
    assert_eq!(inbox.updates["input"], "next task");
}

#[tokio::test]
async fn generated_nested_runtime_resumes_with_the_same_tool_receipts() {
    if std::env::var_os("ZEDFLOW_TEST_CODEGEN").is_none() {
        return;
    }
    let child = flow(
        vec![
            node("s", "start", json!({})),
            node(
                "effect",
                "tool",
                json!({"tool":"exec","arguments":{"command":"printf x >> visits"}}),
            ),
            node("model", "agent", json!({"modelBinding":"runtime"})),
            node("out", "output", json!({"text":"{{input}}"})),
            node("e", "end", json!({})),
        ],
        &[
            ("s", "effect"),
            ("effect", "model"),
            ("model", "out"),
            ("out", "e"),
        ],
    );
    let doc = wrap(&child, "child");
    let generated = zf_compiler::export::export_single(
        &doc,
        &zf_flows::flow_source::render(
            &doc,
            &zf_compiler::graph_compiler::GraphValidator::new(
                &zf_runtime::materialize::RuntimePrimitives,
            ),
        )
        .unwrap(),
        None,
        &zf_runtime::materialize::RuntimePrimitives,
        &zf_runtime::runtime_export::support(),
    )
    .unwrap();
    let directory = tempfile::tempdir().unwrap();
    for (relative, content) in &generated.files {
        let path = directory.path().join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }
    // Seed this fixture's durable context so the generated executable never
    // discovers personal instructions or skills through the process HOME.
    let snapshot = zf_runtime::workspace_context::ContextSnapshot::load_with_home(
        directory.path(),
        &[],
        Some(&directory.path().join("fixture-home")),
    )
    .await
    .unwrap();
    let run_data = directory.path().join("data/export");
    std::fs::create_dir_all(&run_data).unwrap();
    std::fs::write(
        run_data.join("context.json"),
        serde_json::to_vec(&snapshot).unwrap(),
    )
    .unwrap();
    let mut command = tokio::process::Command::new("cargo");
    command
        .args(["run", "--offline", "--quiet", "--manifest-path"])
        .arg(directory.path().join("Cargo.toml"))
        .args(["--", "--workspace"])
        .arg(directory.path())
        .arg("--home")
        .arg(directory.path().join(".fixture-home"))
        .arg("--data")
        .arg(directory.path().join("data"))
        .args(["--run-id", "export", "--input", r#"{"input":"original"}"#]);
    let output = command.output().await.unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let wait: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(wait["status"], "waiting");
    assert_eq!(
        std::fs::read_to_string(directory.path().join("visits")).unwrap(),
        "x"
    );
    command.args([
        "--models",
        r#"{"child/model":{"provider":"fixture"}}"#,
        "--input",
        "{}",
    ]);
    let output = command.output().await.unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let state: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(state["status"], "completed", "{state}");
    let state = &state["state"];
    assert_eq!(state["response"], "original");
    assert_eq!(
        std::fs::read_to_string(directory.path().join("visits")).unwrap(),
        "x"
    );
}

#[tokio::test]
async fn nested_queue_acknowledgements_are_durable_before_child_returns() {
    use zf_runtime::subgraphs::collect_consumed_messages;
    let directory = tempfile::tempdir().unwrap();
    let child = flow(
        vec![
            node("s", "start", json!({})),
            node("ask", "inbox", json!({"fanIn":"any"})),
            node(
                "effect",
                "tool",
                json!({"tool":"exec","arguments":{"command":"printf x >> visits"}}),
            ),
        ],
        &[("s", "ask"), ("ask", "effect"), ("effect", "ask")],
    );
    let doc = wrap(&wrap(&child, "inner"), "outer");
    let database = format!(
        "sqlite://{}?mode=rwc",
        directory.path().join("checkpoints.db").display()
    );
    let cp: Arc<dyn Checkpointer> = Arc::new(SqliteCheckpointer::new(&database).await.unwrap());
    let queue =
        vec![json!({"id":"steer","kind":"steering","text":"instruction","status":"pending"})];
    let services = fixture_services(directory.path().into(), directory.path().join("data"))
        .await
        .unwrap();
    services.replace_queue(queue.clone());
    let graph = zf_runtime::materialize::build_with_services(
        &doc,
        services.clone(),
        None,
        Some(cp.clone()),
    )
    .unwrap();
    paused(&graph, State::new(), ExecutionConfig::new("nested-queue")).await;
    let saved = cp.load("nested-queue").await.unwrap().unwrap();
    assert!(
        saved
            .state
            .get("__zedflow:consumedMessages")
            .is_none_or(|ids| ids.as_array().is_none_or(Vec::is_empty)),
        "the parent is still paused inside its child"
    );
    assert_eq!(
        collect_consumed_messages(cp.as_ref(), "nested-queue", &saved.state)
            .await
            .unwrap(),
        vec!["steer"]
    );
    assert_eq!(
        std::fs::read_to_string(directory.path().join("visits")).unwrap(),
        "x"
    );
    services.cancel.cancel();
    let wait = paused(
        &graph,
        State::new(),
        ExecutionConfig::new("nested-queue").with_resume_from(&saved.checkpoint_id),
    )
    .await;
    assert_eq!(wait_data(&wait)["kind"], "stopped");
    let stopped = cp.load("nested-queue").await.unwrap().unwrap();
    drop(graph);
    drop(services);
    drop(cp);
    let cp: Arc<dyn Checkpointer> = Arc::new(SqliteCheckpointer::new(&database).await.unwrap());
    let services = fixture_services(directory.path().into(), directory.path().join("data"))
        .await
        .unwrap();
    // Simulate a restart before the application acknowledged the message in its
    // queue database: the child checkpoint must still prevent another delivery.
    services.replace_queue(queue);
    let graph =
        zf_runtime::materialize::build_with_services(&doc, services, None, Some(cp.clone()))
            .unwrap();
    let wait = paused(
        &graph,
        State::new(),
        ExecutionConfig::new("nested-queue").with_resume_from(&stopped.checkpoint_id),
    )
    .await;
    assert_eq!(wait_data(&wait)["kind"], "input");
    let saved = cp.load("nested-queue").await.unwrap().unwrap();
    assert_eq!(
        collect_consumed_messages(cp.as_ref(), "nested-queue", &saved.state)
            .await
            .unwrap(),
        vec!["steer"]
    );
    assert_eq!(
        std::fs::read_to_string(directory.path().join("visits")).unwrap(),
        "x"
    );
}
