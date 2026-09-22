use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::PathBuf, time::Duration};
use tempfile::TempDir;
use tower::ServiceExt;
use zf_context::context::ContextBlock;
use zf_context::context::ContextExpr;
use zf_context::context::ContextStrategy;
use zf_context::context::FragmentFormat;
use zf_context::context::FragmentRole;
use zf_core::types::DataType;
use zf_flows::composition::BridgeDefinition;
use zf_flows::composition::Connection;
use zf_flows::composition::Endpoint;
use zf_flows::composition::InvocationKind;
use zf_flows::composition::RouteMode;

async fn request(
    app: &Router,
    method: &str,
    path: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
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
    (
        status,
        serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| json!({"raw":String::from_utf8_lossy(&bytes)})),
    )
}
async fn ok(app: &Router, method: &str, path: &str, body: Option<Value>) -> Value {
    let (status, value) = request(app, method, path, body).await;
    assert_eq!(status, StatusCode::OK, "{method} {path}: {value}");
    value
}
fn policy(id: &str, marker: &str) -> ContextStrategy {
    ContextStrategy::new(id, id)
        .require("question", DataType::Text)
        .with_program(vec![
            ContextBlock::emit(
                "marker",
                FragmentRole::Data,
                FragmentFormat::Text,
                ContextExpr::literal(DataType::Text, json!(marker)),
            ),
            ContextBlock::emit(
                "question",
                FragmentRole::Data,
                FragmentFormat::Text,
                ContextExpr::resource("question"),
            ),
        ])
}
fn node(id: &str, kind: &str, config: Value) -> Value {
    json!({"id":id,"position":{"x":0,"y":0},"data":{"kind":kind,"label":id,"config":config}})
}
fn document(id: &str, root: bool) -> Value {
    let contract = json!({"input":{"kind":"text"},"output":{"kind":"text"}});
    let mut exports = json!({"contract":{"entries":{"main":contract}},"entries":{"main":{"node":"start","inputField":"input","outputField":"response"}},"interactive":root});
    if root {
        exports["contract"]["branches"] = json!({"one":{"contract":contract,"invocations":["node"]},"two":{"contract":contract,"invocations":["node"]}});
        exports["branches"] = json!({"one":"one","two":"two"});
    }
    let mut nodes = vec![node("start", "start", json!({"exports":exports}))];
    if root {
        nodes.push(node(
            "gate",
            "input",
            json!({"field":"input","prompt":"Start both workers?"}),
        ));
    }
    nodes.push(node("agent", "agent", json!({"provider":"fixture","contextBindings":{"question":{"kind":"state","field":"input"}},"fixtureSteps":[{"echoRequest":true}]})));
    if root {
        nodes.push(node("one", "route", json!({"branch":"one"})));
        nodes.push(node("two", "route", json!({"branch":"two"})));
    }
    nodes.push(node("output", "output", json!({"inputField":"output"})));
    nodes.push(node("end", "end", json!({})));
    let edges: Vec<_> = nodes
        .windows(2)
        .enumerate()
        .map(|(i, n)| json!({"id":i.to_string(),"source":n[0]["id"],"target":n[1]["id"]}))
        .collect();
    json!({"formatVersion":3,"id":id,"name":id,"nodes":nodes,"edges":edges})
}
struct Fixture {
    _temp: TempDir,
    app: Router,
    workspace: String,
    root: Value,
    worker: Value,
    alpha: Value,
    beta: Value,
    incompatible: Value,
    selection: Value,
    files: BTreeMap<PathBuf, Vec<u8>>,
}
impl Fixture {
    async fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("workspace");
        std::fs::create_dir(&path).unwrap();
        let app = zf_serve::server::router_with_home(temp.path().join("data"), path, vec![], {
            let home = temp.path().join("home");
            std::fs::create_dir_all(&home).unwrap();
            home
        })
        .await
        .unwrap();
        let workspaces = ok(&app, "GET", "/api/workspaces", None).await;
        let workspace = workspaces[0]["id"].as_str().unwrap().to_owned();
        let alpha = ok(
            &app,
            "POST",
            "/api/context-strategies",
            Some(json!({"workspaceId":workspace,"strategy":policy("alpha","ALPHA first")})),
        )
        .await;
        let beta = ok(
            &app,
            "POST",
            "/api/context-strategies",
            Some(json!({"workspaceId":workspace,"strategy":policy("beta","BETA only")})),
        )
        .await;
        let incompatible = ok(&app, "POST", "/api/context-strategies", Some(json!({"workspaceId":workspace,"strategy":policy("incompatible","INCOMPATIBLE").require("missing",DataType::Text)}))).await;
        let root = ok(
            &app,
            "POST",
            "/api/flows",
            Some(json!({"workspaceId":workspace,"composition":document("root-flow",true)})),
        )
        .await;
        let worker = ok(
            &app,
            "POST",
            "/api/flows",
            Some(json!({"workspaceId":workspace,"composition":document("worker-flow",false)})),
        )
        .await;
        let bridge = BridgeDefinition::new()
            .import("one", worker["key"].as_str().unwrap())
            .import("two", worker["key"].as_str().unwrap())
            .connect(
                "one",
                Connection::new(
                    Endpoint::new("root", "one"),
                    Endpoint::new("one", "main"),
                    RouteMode::CallAwait,
                    InvocationKind::Node,
                ),
            )
            .connect(
                "two",
                Connection::new(
                    Endpoint::new("root", "two"),
                    Endpoint::new("two", "main"),
                    RouteMode::CallAwait,
                    InvocationKind::Node,
                ),
            );
        let bridge = ok(
            &app,
            "POST",
            "/api/bridges",
            Some(json!({"workspaceId":workspace,"key":"pair","bridge":bridge})),
        )
        .await;
        let selection = json!({"flow":root["key"],"entry":"main","bridges":["pair"],"contexts":{
            "root/agent":{"key":alpha["key"],"hash":alpha["hash"]},
            "pair/one/agent":{"key":alpha["key"],"hash":alpha["hash"]},
            "pair/two/agent":{"key":beta["key"],"hash":beta["hash"]}
        }});
        let files = [&root, &worker, &alpha, &beta, &incompatible, &bridge]
            .into_iter()
            .flat_map(|file| {
                let path = PathBuf::from(file["path"].as_str().unwrap());
                let paths = if path.is_dir() {
                    let manifest: Value =
                        serde_json::from_slice(&std::fs::read(path.join("flow.json")).unwrap())
                            .unwrap();
                    std::iter::once(path.join("flow.json"))
                        .chain(
                            manifest["files"]
                                .as_array()
                                .unwrap()
                                .iter()
                                .map(|name| path.join(name.as_str().unwrap())),
                        )
                        .collect::<Vec<_>>()
                } else {
                    vec![path]
                };
                paths
                    .into_iter()
                    .map(|path| {
                        let bytes = std::fs::read(&path).unwrap();
                        (path, bytes)
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        Self {
            _temp: temp,
            app,
            workspace,
            root,
            worker,
            alpha,
            beta,
            incompatible,
            selection,
            files,
        }
    }
    fn unchanged_files(&self) {
        for (path, bytes) in &self.files {
            assert_eq!(&std::fs::read(path).unwrap(), bytes, "{}", path.display());
        }
    }
    async fn no_runs(&self) {
        assert_eq!(
            ok(
                &self.app,
                "GET",
                &format!("/api/runs?workspaceId={}", self.workspace),
                None
            )
            .await,
            json!([])
        );
    }
    async fn prepare(&self, selection: &Value) -> (StatusCode, Value) {
        request(
            &self.app,
            "POST",
            "/api/runtime-graphs/prepare",
            Some(json!({"workspaceId":self.workspace,"selection":selection})),
        )
        .await
    }
    async fn stable(&self, id: &str, status: &str) -> Value {
        tokio::time::timeout(Duration::from_secs(30), async {
            loop {
                let run = ok(
                    &self.app,
                    "GET",
                    &format!("/api/runs/{id}?workspaceId={}", self.workspace),
                    None,
                )
                .await;
                assert_ne!(run["status"], "error", "{}", run["error"]);
                if run["status"] == status && run["runtimeActive"] != true {
                    break run;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap()
    }
    async fn published(&self, id: &str, instance: &str) -> Value {
        let versions = ok(
            &self.app,
            "GET",
            &format!("/api/runs/{id}/revisions?workspaceId={}", self.workspace),
            None,
        )
        .await;
        let version = versions["instances"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["instance"] == instance)
            .unwrap();
        ok(
            &self.app,
            "GET",
            &format!(
                "/api/runs/{id}/definition?workspaceId={}&nodePath={instance}/agent&hash={}",
                self.workspace,
                version["publishedRevision"].as_str().unwrap()
            ),
            None,
        )
        .await
    }
}
fn agent(definition: &Value) -> &Value {
    &definition["composition"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == "agent")
        .unwrap()["data"]["config"]
}
#[tokio::test]
async fn per_instance_context_choices_freeze_distinct_sources_without_editing_files_and_invalid_choices_create_no_run()
 {
    let f = Fixture::new().await;
    let (status, prepared) = f.prepare(&f.selection).await;
    assert_eq!(status, StatusCode::OK, "{prepared}");
    let runtime = &prepared["runtime"];
    assert_eq!(
        runtime["flows"]["pair/one"]["key"],
        runtime["flows"]["pair/two"]["key"]
    );
    assert_ne!(
        runtime["flows"]["pair/one"]["hash"],
        runtime["flows"]["pair/two"]["hash"]
    );
    assert_eq!(
        agent(&runtime["flows"]["pair/one"])["contextProgram"]["hash"],
        f.alpha["hash"]
    );
    assert_eq!(
        agent(&runtime["flows"]["pair/two"])["contextProgram"]["hash"],
        f.beta["hash"]
    );
    assert_eq!(
        runtime["definitions"]["contextSelections"],
        f.selection["contexts"]
    );
    for (path, choice, expected) in [
        (
            "unknown/agent",
            json!({"key":f.alpha["key"],"hash":f.alpha["hash"]}),
            StatusCode::BAD_REQUEST,
        ),
        (
            "root/absent",
            json!({"key":f.alpha["key"],"hash":f.alpha["hash"]}),
            StatusCode::BAD_REQUEST,
        ),
        (
            "root/gate",
            json!({"key":f.alpha["key"],"hash":f.alpha["hash"]}),
            StatusCode::BAD_REQUEST,
        ),
        (
            "pair/one/agent",
            json!({"key":f.alpha["key"],"hash":"0".repeat(64)}),
            StatusCode::CONFLICT,
        ),
        (
            "pair/one/agent",
            json!({"key":f.incompatible["key"],"hash":f.incompatible["hash"]}),
            StatusCode::BAD_REQUEST,
        ),
    ] {
        let mut selection = f.selection.clone();
        selection["contexts"][path] = choice;
        let (status, value) = f.prepare(&selection).await;
        assert_eq!(status, expected, "{path}: {value}");
        let (status,value)=request(&f.app,"POST","/api/runs",Some(json!({"workspaceId":f.workspace,"runtimeSelection":selection,"input":{"input":"hello"}}))).await;
        assert_eq!(status, expected, "{path}: {value}");
    }
    f.no_runs().await;
    f.unchanged_files();
}

#[tokio::test]
async fn saved_flow_and_selected_strategy_preserve_each_instance_choice_before_root_resume() {
    let f = Fixture::new().await;
    let started=ok(&f.app,"POST","/api/runs",Some(json!({"workspaceId":f.workspace,"runtimeSelection":f.selection,"input":{"input":"initial"}}))).await;
    let id = started["id"].as_str().unwrap();
    let waiting = f.stable(id, "waiting").await;
    assert_eq!(waiting["wait"]["nodePath"], "root/gate");
    f.unchanged_files();
    let mut worker = f.worker["composition"].clone();
    worker["name"] = json!("Worker edited");
    let saved=ok(&f.app,"POST","/api/flows",Some(json!({"workspaceId":f.workspace,"key":f.worker["key"],"expectedHash":f.worker["hash"],"composition":worker}))).await;
    let one = f.published(id, "pair/one").await;
    let two = f.published(id, "pair/two").await;
    assert_eq!(agent(&one)["contextProgram"]["hash"], f.alpha["hash"]);
    assert_eq!(agent(&two)["contextProgram"]["hash"], f.beta["hash"]);
    let alpha=ok(&f.app,"POST","/api/context-strategies",Some(json!({"workspaceId":f.workspace,"strategy":policy("alpha","ALPHA second"),"expectedHash":f.alpha["hash"]}))).await;
    // Save the authored source once more after its selected strategy advanced.
    // The initial choice pin must not reset the instance to its old program.
    worker["name"] = json!("Worker edited again");
    ok(&f.app,"POST","/api/flows",Some(json!({"workspaceId":f.workspace,"key":f.worker["key"],"expectedHash":saved["hash"],"composition":worker}))).await;
    let mut root = f.root["composition"].clone();
    root["name"] = json!("Root edited");
    ok(&f.app,"POST","/api/flows",Some(json!({"workspaceId":f.workspace,"key":f.root["key"],"expectedHash":f.root["hash"],"composition":root}))).await;
    for (instance, key, hash) in [
        ("root", "alpha", &alpha["hash"]),
        ("pair/one", "alpha", &alpha["hash"]),
        ("pair/two", "beta", &f.beta["hash"]),
    ] {
        let definition = f.published(id, instance).await;
        assert_eq!(agent(&definition)["contextStrategy"]["key"], key);
        assert_eq!(&agent(&definition)["contextProgram"]["hash"], hash);
    }
    ok(
        &f.app,
        "POST",
        &format!("/api/runs/{id}/answer?workspaceId={}", f.workspace),
        Some(json!({"waitId":waiting["wait"]["id"],"value":"explicit user answer"})),
    )
    .await;
    let completed = f.stable(id, "completed").await;
    assert_eq!(
        completed["runtimeGraph"]["definitions"]["contextSelections"],
        f.selection["contexts"]
    );
    for (path, marker, excluded) in [
        ("root/agent", "ALPHA second", "BETA only"),
        ("pair/one/agent", "ALPHA second", "BETA only"),
        ("pair/two/agent", "BETA only", "ALPHA second"),
    ] {
        let activity = completed["activities"]
            .as_array()
            .unwrap()
            .iter()
            .find(|a| a["path"] == path && a["status"] == "completed")
            .unwrap();
        let output = activity["output"]["output"].as_str().unwrap();
        let request: Value = serde_json::from_str(output).unwrap();
        let content = request["contents"].to_string();
        assert!(content.contains(marker), "{path}: {content}");
        assert!(
            content.contains("explicit user answer"),
            "{path}: {content}"
        );
        assert!(!content.contains(excluded), "{path}: {content}");
        assert!(!content.contains("ALPHA first"), "{path}: {content}");
    }
}
