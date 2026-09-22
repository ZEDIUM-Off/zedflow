use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use serde_json::{Value, json};
use std::{path::PathBuf, time::Duration};
use tempfile::TempDir;
use tower::ServiceExt;
use zf_flows::composition::BridgeDefinition;
use zf_flows::composition::Connection;
use zf_flows::composition::Endpoint;
use zf_flows::composition::InvocationKind;
use zf_flows::composition::RouteMode;
use zf_storage::content_store::ContentStore;

async fn api(app: &Router, method: &str, path: &str, body: Option<Value>) -> Value {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header("content-type", "application/json")
                .body(body.map_or(Body::empty(), |v| Body::from(v.to_string())))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 16 * 1024 * 1024)
        .await
        .unwrap();
    let value: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(status, StatusCode::OK, "{method} {path}: {value}");
    value
}
fn node(id: &str, kind: &str, config: Value) -> Value {
    json!({"id":id,"position":{"x":0,"y":0},"data":{"kind":kind,"label":id,"config":config}})
}
fn flow(id: &str, child: bool) -> Value {
    let contract = json!({"input":{"kind":"text"},"output":{"kind":"text"}});
    let mut exports = json!({"interactive":child,"contract":{"entries":{"main":contract}},"entries":{"main":{"node":"start","inputField":"input","outputField":"output"}}});
    let mut nodes = vec![node("start", "start", json!({}))];
    if child {
        nodes.push(node(
            "question",
            "input",
            json!({"field":"output","prompt":"Child question"}),
        ));
    } else {
        exports["contract"]["branches"] =
            json!({"work":{"contract":contract,"invocations":["node"]}});
        exports["branches"] = json!({"work":"route"});
        exports["contract"]["entries"]["quick"] = contract;
        exports["entries"]["quick"] =
            json!({"node":"after","inputField":"input","outputField":"output"});
        nodes.push(node("route", "route", json!({"branch":"work"})));
        nodes.push(node(
            "after",
            "set",
            json!({"field":"output","value":"{{output}}"}),
        ));
    }
    nodes[0]["data"]["config"] = json!({"exports":exports});
    nodes.push(node("end", "end", json!({})));
    let edges: Vec<_> = nodes
        .windows(2)
        .enumerate()
        .map(|(i, pair)| json!({"id":i.to_string(),"source":pair[0]["id"],"target":pair[1]["id"]}))
        .collect();
    json!({"formatVersion":3,"id":id,"name":id,"nodes":nodes,"edges":edges})
}
struct Fixture {
    _temp: TempDir,
    app: Router,
    data: PathBuf,
    workspace: String,
    root: Value,
}
impl Fixture {
    async fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let workspace_path = temp.path().join("workspace");
        std::fs::create_dir(&workspace_path).unwrap();
        let data = temp.path().join("data");
        let app = zf_serve::server::router_with_home(data.clone(), workspace_path, vec![], {
            let home = temp.path().join("home");
            std::fs::create_dir_all(&home).unwrap();
            home
        })
        .await
        .unwrap();
        let workspace = api(&app, "GET", "/api/workspaces", None).await[0]["id"]
            .as_str()
            .unwrap()
            .to_owned();
        let root = api(
            &app,
            "POST",
            "/api/flows",
            Some(json!({"workspaceId":workspace,"composition":flow("root",false)})),
        )
        .await;
        let child = api(
            &app,
            "POST",
            "/api/flows",
            Some(json!({"workspaceId":workspace,"composition":flow("child",true)})),
        )
        .await;
        let idle = BridgeDefinition::new().import("unused", child["key"].as_str().unwrap());
        api(
            &app,
            "POST",
            "/api/bridges",
            Some(json!({"workspaceId":workspace,"key":"idle","bridge":idle})),
        )
        .await;
        let route = BridgeDefinition::new()
            .import("child", child["key"].as_str().unwrap())
            .connect(
                "work",
                Connection::new(
                    Endpoint::new("root", "work"),
                    Endpoint::new("child", "main"),
                    RouteMode::CallAwait,
                    InvocationKind::Node,
                ),
            );
        api(
            &app,
            "POST",
            "/api/bridges",
            Some(json!({"workspaceId":workspace,"key":"call","bridge":route})),
        )
        .await;
        Self {
            _temp: temp,
            app,
            data,
            workspace,
            root,
        }
    }
    fn selection(&self, entry: &str, bridges: Value) -> Value {
        json!({"flow":self.root["key"],"entry":entry,"bridges":bridges})
    }
    async fn prepare(&self, selection: &Value) -> Value {
        api(
            &self.app,
            "POST",
            "/api/runtime-graphs/prepare",
            Some(json!({"workspaceId":self.workspace,"selection":selection})),
        )
        .await
    }
    async fn boundary(&self, id: &str) -> Value {
        tokio::time::timeout(Duration::from_secs(20), async {
            loop {
                let run = api(
                    &self.app,
                    "GET",
                    &format!("/api/runs/{id}?workspaceId={}", self.workspace),
                    None,
                )
                .await;
                if run["status"] != "running" && run["runtimeActive"] != true {
                    return run;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap()
    }
}

#[tokio::test]
async fn reachable_interactive_child_makes_autonomous_root_a_session_and_answer_resumes_it() {
    let f = Fixture::new().await;
    let selection = f.selection("main", json!(["call", "idle"]));
    let prepared = f.prepare(&selection).await;
    assert_eq!(
        prepared["runtime"]["flows"]["root"]["exports"]["interactive"],
        false
    );
    assert_eq!(prepared["overview"]["interactive"], true);
    let started=api(&f.app,"POST","/api/runs",Some(json!({"workspaceId":f.workspace,"runtimeSelection":selection,"input":{"input":"Start"}}))).await;
    let id = started["id"].as_str().unwrap();
    let waiting = f.boundary(id).await;
    assert_eq!(waiting["status"], "waiting", "{}", waiting["error"]);
    assert_eq!(waiting["interactive"], true);
    assert_eq!(waiting["wait"]["nodePath"], "call/child/question");
    let snapshot = api(
        &f.app,
        "GET",
        &format!("/api/runs/{id}/snapshot?workspaceId={}", f.workspace),
        None,
    )
    .await;
    assert_eq!(snapshot["run"]["interactive"], true, "{snapshot}");
    assert_eq!(waiting["messages"][0]["text"], "Start");
    api(
        &f.app,
        "POST",
        &format!("/api/runs/{id}/answer?workspaceId={}", f.workspace),
        Some(json!({"waitId":waiting["wait"]["id"],"value":"Child answer"})),
    )
    .await;
    let completed = f.boundary(id).await;
    assert_eq!(completed["status"], "completed", "{}", completed["error"]);
    assert_eq!(completed["state"]["output"], "Child answer");
    assert_eq!(completed["interactive"], true);
}

#[tokio::test]
async fn unused_imports_and_routes_outside_selected_entry_do_not_create_interaction() {
    let f = Fixture::new().await;
    for bridges in [json!(["idle"]), json!(["idle", "call"])] {
        let selection = f.selection("quick", bridges);
        let prepared = f.prepare(&selection).await;
        assert!(
            prepared["runtime"]["flows"]
                .as_object()
                .unwrap()
                .values()
                .any(|flow| flow["exports"]["interactive"] == true)
        );
        assert_eq!(prepared["overview"]["interactive"], false);
        let started=api(&f.app,"POST","/api/runs",Some(json!({"workspaceId":f.workspace,"runtimeSelection":selection,"input":{"input":"Start"}}))).await;
        let run = f.boundary(started["id"].as_str().unwrap()).await;
        assert_eq!(run["status"], "completed", "{}", run["error"]);
        assert_eq!(run["interactive"], false);
        assert_eq!(run["messages"], json!([]));
        assert!(
            !run["activities"]
                .as_array()
                .unwrap()
                .iter()
                .any(|activity| activity["path"].as_str().unwrap().contains("/question"))
        );
    }
}

#[tokio::test]
async fn interaction_backfill_reads_runtime_refs_and_preserves_opaque_histories() {
    let f = Fixture::new().await;
    let interactive = f.prepare(&f.selection("main", json!(["call"]))).await;
    let autonomous = f
        .prepare(&f.selection("quick", json!(["call", "idle"])))
        .await;
    let pool =
        sqlx::SqlitePool::connect(&format!("sqlite://{}", f.data.join("zedflow.db").display()))
            .await
            .unwrap();
    let store = ContentStore::from_pool(pool.clone());
    let mut originals = vec![];
    for (id, runtime, prior, expected) in [
        ("missing", &interactive["runtime"], None, true),
        ("old-root-only", &interactive["runtime"], Some(false), true),
        ("unused", &autonomous["runtime"], None, false),
    ] {
        let reference = store.intern(runtime).await.unwrap();
        let mut run = json!({"id":id,"runtimeGraphRef":reference,"compositionRef":format!("sha256:{}","0".repeat(64)),"stateRef":format!("sha256:{}","1".repeat(64)),"messagesRef":format!("sha256:{}","2".repeat(64))});
        if let Some(prior) = prior {
            run["interactive"] = json!(prior);
        }
        sqlx::query("INSERT INTO runs(id,document)VALUES(?,?)")
            .bind(id)
            .bind(run.to_string())
            .execute(&pool)
            .await
            .unwrap();
        originals.push((id, run, expected));
    }
    assert_eq!(
        zf_storage::session_store::backfill_interactive(
            &pool,
            &zf_runtime::archive_validation::RuntimeArchiveValidation
        )
        .await
        .unwrap(),
        3
    );
    for (id, mut expected, interactive) in originals {
        expected["interactive"] = json!(interactive);
        let raw: String = sqlx::query_scalar("SELECT document FROM runs WHERE id=?")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(serde_json::from_str::<Value>(&raw).unwrap(), expected);
    }
    assert_eq!(
        zf_storage::session_store::backfill_interactive(
            &pool,
            &zf_runtime::archive_validation::RuntimeArchiveValidation
        )
        .await
        .unwrap(),
        0
    );
}
