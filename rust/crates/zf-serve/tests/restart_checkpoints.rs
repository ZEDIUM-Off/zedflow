//! Simulate process loss after ADK commits, before any checkpoint notice is published.
use adk_graph::{ExecutionConfig, State, checkpoint::Checkpointer};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use serde_json::{Value, json};
use std::{path::PathBuf, sync::Arc};
use tower::ServiceExt;
use zf_flows::flow_source;
use zf_flows::schema::Composition;
use zf_runtime::runtime::RunServices;
use zf_runtime::stored_checkpointer::StoredCheckpointer;
use zf_runtime::workspace_context::ContextSnapshot;
mod support;
use zf_execution::service::ExecutionService;
use zf_storage::content_store::ContentStore;
use zf_storage::session_store;
use zf_storage::workspaces;

const ID: &str = "unpublished-checkpoint";
fn node(id: &str, kind: &str, config: Value) -> Value {
    json!({"id":id,"position":{"x":0,"y":0},"data":{"label":id,"kind":kind,"config":config}})
}
fn flow(nodes: Vec<Value>, edges: &[(&str, &str)]) -> Composition {
    serde_json::from_value(json!({"id":"restart-fixture","name":"Restart fixture","nodes":nodes,
        "edges":edges.iter().enumerate().map(|(i,(source,target))|json!({"id":i.to_string(),"source":source,"target":target})).collect::<Vec<_>>()})).unwrap()
}
fn effect() -> Value {
    node(
        "effect",
        "tool",
        json!({"tool":"exec","arguments":{"command":"printf x >> visits"}}),
    )
}
struct Fixture {
    _directory: tempfile::TempDir,
    workspace: PathBuf,
    data: PathBuf,
    home: PathBuf,
    store: ContentStore,
    cp: Arc<StoredCheckpointer>,
    services: Arc<RunServices>,
}
impl Fixture {
    async fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let workspace = directory.path().to_path_buf();
        let data = workspace.join("data");
        let home = workspace.join("isolated-home");
        let (app, service) =
            support::open_router(data.clone(), workspace.clone(), vec![], home.clone())
                .await
                .unwrap();
        service.shutdown().await.unwrap();
        drop(app);
        drop(service);
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(data.join("zedflow.db"))
                    .synchronous(sqlx::sqlite::SqliteSynchronous::Full),
            )
            .await
            .unwrap();
        let store = ContentStore::new(pool).await.unwrap();
        let cp = Arc::new(StoredCheckpointer::new(
            zf_storage::contracts::CheckpointStore::new(store.clone())
                .await
                .unwrap(),
        ));
        let services = RunServices::new(
            ID.into(),
            workspace.clone(),
            data.join("runs").join(ID),
            ContextSnapshot {
                cwd: workspace.clone(),
                ..Default::default()
            },
            json!({}),
            vec![],
        )
        .unwrap();
        services.set_content_store(store.clone());
        Self {
            _directory: directory,
            workspace,
            data,
            home,
            store,
            cp,
            services,
        }
    }
    async fn persist(&self, doc: &Composition, patch: Value) {
        let mut run = json!({"id":ID,"name":"Checkpoint crash fixture","status":"running",
            "workspaceId":workspaces::path_id(&self.workspace),"workspacePath":self.workspace,
            "composition":doc,"flowSource":flow_source::render(doc, &zf_compiler::graph_compiler::GraphValidator::new(&zf_runtime::materialize::RuntimePrimitives)).unwrap(),
            "context":ContextSnapshot { cwd:self.workspace.clone(), ..Default::default() },
            "state":{},"input":{},"queue":[],"messages":[],"activities":[],"toolActivities":[],
            "contextSnapshots":[],"timeline":[],"timelineVersion":1,"modelBindings":{},
            "activeNodes":["effect"],"createdAt":"2026-09-12T00:00:00Z","updatedAt":"2026-09-12T00:00:00Z"});
        run.as_object_mut()
            .unwrap()
            .extend(patch.as_object().unwrap().clone());
        session_store::save(self.store.pool(), ID, &run)
            .await
            .unwrap();
    }
    async fn restart(&self) -> (Router, ExecutionService) {
        support::open_router(self.data.clone(), self.workspace.clone(), vec![], {
            let home = self.home.clone();
            std::fs::create_dir_all(&home).unwrap();
            home
        })
        .await
        .unwrap()
    }
    fn assert_one_effect(&self) {
        assert_eq!(
            std::fs::read_to_string(self.workspace.join("visits")).unwrap(),
            "x"
        );
    }
}
async fn request(app: &Router, method: &str, suffix: &str) -> Value {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(format!("/api/runs/{ID}{suffix}"))
                .header("content-type", "application/json")
                .body(Body::from(if method == "POST" { "{}" } else { "" }))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let value: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 20_000_000).await.unwrap()).unwrap();
    assert_eq!(status, StatusCode::OK, "{value}");
    value
}
async fn resumed_wait(app: &Router) -> Value {
    let ack = request(app, "POST", "/resume").await;
    assert_eq!(ack.as_object().unwrap().len(), 3);
    for _ in 0..200 {
        let run = request(app, "GET", "").await;
        if run["status"] == "waiting" && run["runtimeActive"] != true {
            return run;
        }
        assert_ne!(run["status"], "error", "{run}");
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    panic!("resumed graph did not reach its input wait");
}

#[tokio::test]
async fn startup_recovers_first_unpublished_checkpoint_and_publishes_its_revision() {
    let f = Fixture::new().await;
    let doc = flow(
        vec![
            node("s", "start", json!({})),
            effect(),
            node("ask", "inbox", json!({})),
            node("e", "end", json!({})),
        ],
        &[("s", "effect"), ("effect", "ask"), ("ask", "e")],
    );
    f.persist(&doc, json!({"activities":[{"occurrenceId":"interrupted-effect","status":"running","nodeId":"effect"}]})).await;
    let graph = zf_runtime::materialize::build_with_services(
        &doc,
        f.services.clone(),
        None,
        Some(f.cp.clone()),
    )
    .unwrap();
    assert!(matches!(
        graph
            .invoke(
                State::from([("input".into(), json!("original"))]),
                ExecutionConfig::new(ID)
            )
            .await,
        Err(adk_graph::GraphError::Interrupted(_))
    ));
    let saved = f.cp.load(ID).await.unwrap().unwrap();
    let mut foreign = saved.clone();
    foreign.thread_id = format!("{ID}-different-run");
    foreign.checkpoint_id = uuid::Uuid::new_v4().to_string();
    foreign.state.insert("foreign".into(), json!(true));
    f.cp.save(&foreign).await.unwrap();
    f.assert_one_effect();
    let stale = session_store::load(f.store.pool(), ID).await.unwrap();
    assert!(stale["checkpoint"].is_null());
    drop(graph);
    let (app, service) = f.restart().await;
    let run = request(&app, "GET", "").await;
    assert_eq!(run["checkpoint"], saved.checkpoint_id);
    assert_eq!(run["state"], serde_json::to_value(&saved.state).unwrap());
    assert_eq!(run["status"], "interrupted");
    assert_eq!(run["activities"][0]["status"], "interrupted");
    let snapshot = request(&app, "GET", "/snapshot").await;
    assert!(snapshot["revision"].as_i64().unwrap() > 0, "{snapshot}");
    assert_eq!(snapshot["run"]["checkpoint"], saved.checkpoint_id);
    assert_eq!(snapshot["run"]["status"], "interrupted");
    let delta = request(&app, "GET", "/snapshot?after=0").await;
    assert_eq!(delta["revision"], snapshot["revision"]);
    resumed_wait(&app).await;
    f.assert_one_effect();
    let before_reopen = request(&app, "GET", "/snapshot").await;
    service.shutdown().await.unwrap();
    drop(app);
    drop(service);
    let (reopened, service) = f.restart().await;
    let once = request(&reopened, "GET", "/snapshot").await;
    assert_eq!(once, before_reopen);
    f.assert_one_effect();
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn startup_recovers_nested_consumption_and_does_not_reapply_accepted_resume_input() {
    let f = Fixture::new().await;
    let child = flow(
        vec![
            node("s", "start", json!({})),
            node("ask", "inbox", json!({"fanIn":"any"})),
            effect(),
        ],
        &[("s", "ask"), ("ask", "effect"), ("effect", "ask")],
    );
    let mut doc = serde_json::to_value(flow(
        vec![
            node("s", "start", json!({})),
            node("child", "subgraph", json!({"composition":child})),
            node("e", "end", json!({})),
        ],
        &[("s", "child"), ("child", "e")],
    ))
    .unwrap();
    doc["channels"] = json!([{"name":"accepted","reducer":"append","default":[]}]);
    let doc: Composition = serde_json::from_value(doc).unwrap();
    let graph = zf_runtime::materialize::build_with_services(
        &doc,
        f.services.clone(),
        None,
        Some(f.cp.clone()),
    )
    .unwrap();
    assert!(matches!(
        graph.invoke(State::new(), ExecutionConfig::new(ID)).await,
        Err(adk_graph::GraphError::Interrupted(_))
    ));
    let previous = f.cp.load(ID).await.unwrap().unwrap();
    let queue = vec![
        json!({"id":"queued-steering","kind":"steering","text":"Continue once","status":"pending"}),
    ];
    let input = json!({"accepted":["once"]});
    f.persist(
        &doc,
        json!({"checkpoint":previous.checkpoint_id,"state":previous.state,
        "resumeCheckpoint":previous.checkpoint_id,"resumeInput":input,"queue":queue}),
    )
    .await;
    f.services.replace_queue(queue);
    assert!(matches!(
        graph
            .invoke(
                serde_json::from_value(input).unwrap(),
                ExecutionConfig::new(ID).with_resume_from(&previous.checkpoint_id)
            )
            .await,
        Err(adk_graph::GraphError::Interrupted(_))
    ));
    let saved = f.cp.load(ID).await.unwrap().unwrap();
    assert_ne!(saved.checkpoint_id, previous.checkpoint_id);
    assert_eq!(saved.state["accepted"], json!(["once"]));
    assert!(
        saved
            .state
            .get("__zedflow:consumedMessages")
            .is_none_or(|v| v.as_array().is_none_or(Vec::is_empty))
    );
    f.assert_one_effect();
    drop(graph);
    let (app, service) = f.restart().await;
    let run = request(&app, "GET", "").await;
    assert_eq!(run["checkpoint"], saved.checkpoint_id);
    assert_eq!(run["state"], serde_json::to_value(&saved.state).unwrap());
    assert_eq!(run["consumedMessages"], json!(["queued-steering"]));
    assert_eq!(run["queue"][0]["status"], "consumed");
    assert!(run["resumeInput"].is_null());
    assert!(run["resumeCheckpoint"].is_null());
    let resumed = resumed_wait(&app).await;
    assert_eq!(resumed["state"]["accepted"], json!(["once"]));
    assert_eq!(resumed["queue"][0]["status"], "consumed");
    f.assert_one_effect();
    let before_reopen = request(&app, "GET", "/snapshot").await;
    service.shutdown().await.unwrap();
    drop(app);
    drop(service);
    let (reopened, service) = f.restart().await;
    let once = request(&reopened, "GET", "/snapshot").await;
    assert_eq!(once, before_reopen);
    f.assert_one_effect();
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn recovery_without_checkpoint_preserves_state_and_is_idempotent() {
    let f = Fixture::new().await;
    let doc = flow(
        vec![
            node("s", "start", json!({})),
            node(
                "ask",
                "input",
                json!({"field":"input","responseType":"text"}),
            ),
            node("e", "end", json!({})),
        ],
        &[("s", "ask"), ("ask", "e")],
    );
    f.persist(&doc, json!({"state":{"kept":42}})).await;
    let (app, service) = f.restart().await;
    let run = request(&app, "GET", "").await;
    assert_eq!(run["status"], "interrupted");
    assert_eq!(run["state"], json!({"kept":42}));
    assert!(run["checkpoint"].is_null());
    let before = request(&app, "GET", "/snapshot").await;
    service.shutdown().await.unwrap();
    drop(app);
    drop(service);
    let (app, service) = f.restart().await;
    assert_eq!(request(&app, "GET", "/snapshot").await, before);
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn corrupt_checkpoint_header_stops_recovery_before_publication() {
    let f = Fixture::new().await;
    let doc = flow(
        vec![
            node("s", "start", json!({})),
            node("ask", "inbox", json!({})),
            node("e", "end", json!({})),
        ],
        &[("s", "ask"), ("ask", "e")],
    );
    f.persist(&doc, json!({"state":{"before":"crash"}})).await;
    let graph = zf_runtime::materialize::build_with_services(
        &doc,
        f.services.clone(),
        None,
        Some(f.cp.clone()),
    )
    .unwrap();
    assert!(matches!(
        graph.invoke(State::new(), ExecutionConfig::new(ID)).await,
        Err(adk_graph::GraphError::Interrupted(_))
    ));
    let saved = f.cp.load(ID).await.unwrap().unwrap();
    drop(graph);
    let original: String =
        sqlx::query_scalar("SELECT header FROM zf_checkpoints WHERE checkpoint_id=?")
            .bind(&saved.checkpoint_id)
            .fetch_one(f.store.pool())
            .await
            .unwrap();
    let before = session_store::load(f.store.pool(), ID).await.unwrap();
    for (field, reference, diagnostic) in [
        (
            "stateRef",
            "sha256:wrong",
            "checkpoint header/content mismatch",
        ),
        (
            "checkpointRef",
            "sha256:0000000000000000000000000000000000000000000000000000000000000000",
            "missing content",
        ),
    ] {
        let mut header: Value = serde_json::from_str(&original).unwrap();
        header[field] = json!(reference);
        sqlx::query("UPDATE zf_checkpoints SET header=? WHERE checkpoint_id=?")
            .bind(header.to_string())
            .bind(&saved.checkpoint_id)
            .execute(f.store.pool())
            .await
            .unwrap();
        let reopened =
            support::open_router(f.data.clone(), f.workspace.clone(), vec![], f.home.clone()).await;
        let error = match reopened {
            Ok(_) => panic!("corrupt checkpoint was accepted"),
            Err(error) => error,
        };
        assert!(format!("{error:#}").contains(diagnostic), "{error:#}");
        assert_eq!(
            session_store::load(f.store.pool(), ID).await.unwrap(),
            before
        );
        let publications: i64 = sqlx::query_scalar("SELECT count(*) FROM events WHERE run=?")
            .bind(ID)
            .fetch_one(f.store.pool())
            .await
            .unwrap();
        assert_eq!(publications, 0);
    }
}

#[tokio::test]
async fn recovery_uses_last_committed_sequence_for_equal_checkpoint_timestamps() {
    let f = Fixture::new().await;
    let doc = flow(
        vec![
            node("s", "start", json!({})),
            node("ask", "inbox", json!({})),
            node("e", "end", json!({})),
        ],
        &[("s", "ask"), ("ask", "e")],
    );
    f.persist(&doc, json!({"state":{"stale":true}})).await;
    let graph = zf_runtime::materialize::build_with_services(
        &doc,
        f.services.clone(),
        None,
        Some(f.cp.clone()),
    )
    .unwrap();
    assert!(matches!(
        graph.invoke(State::new(), ExecutionConfig::new(ID)).await,
        Err(adk_graph::GraphError::Interrupted(_))
    ));
    let first = f.cp.load(ID).await.unwrap().unwrap();
    let mut last = first.clone();
    last.checkpoint_id = uuid::Uuid::new_v4().to_string();
    last.state.insert("last_sequence".into(), json!(true));
    f.cp.save(&last).await.unwrap();
    assert_eq!(first.created_at, last.created_at);
    drop(graph);
    let (app, service) = f.restart().await;
    let recovered = request(&app, "GET", "").await;
    assert_eq!(recovered["checkpoint"], last.checkpoint_id);
    assert_eq!(
        recovered["state"],
        serde_json::to_value(&last.state).unwrap()
    );
    service.shutdown().await.unwrap();
}
