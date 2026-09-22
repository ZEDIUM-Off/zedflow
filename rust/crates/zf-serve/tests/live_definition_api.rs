use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use serde_json::{Value, json};
use tower::ServiceExt;
use zf_flows::flow_source;
use zf_flows::schema::Composition;
use zf_runtime::revisions::RevisionDefinition;
use zf_runtime::revisions::RevisionPublication;
use zf_runtime::revisions::publish_batch;
use zf_storage::content_store::ContentStore;
use zf_storage::flow_store::hash;

async fn request(app: &Router, method: &str, path: &str, body: Option<Value>) -> Value {
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
    let bytes = to_bytes(response.into_body(), 8 * 1024 * 1024)
        .await
        .unwrap();
    let value: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(status, StatusCode::OK, "{value}");
    value
}
fn node(id: &str, kind: &str, config: Value) -> Value {
    json!({"id":id,"position":{"x":0,"y":0},"data":{"kind":kind,"label":id,"config":config}})
}
fn document() -> Composition {
    serde_json::from_value(json!({"formatVersion":3,"id":"live","name":"Live definition","nodes":[node("start","start",json!({})),node("effect","tool",json!({"tool":"exec","arguments":{"command":"printf x >> effect"}})),node("busy","tool",json!({"tool":"delay","arguments":{"milliseconds":600}})),node("result","set",json!({"field":"output","value":"initial"})),node("end","end",json!({}))],"edges":[{"id":"a","source":"start","target":"effect"},{"id":"b","source":"effect","target":"busy"},{"id":"c","source":"busy","target":"result"},{"id":"d","source":"result","target":"end"}]})).unwrap()
}
async fn exercise(structural: bool, via_save: bool) {
    let root = tempfile::tempdir().unwrap();
    let workspace = root.path().join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    let app =
        zf_serve::server::router_with_home(root.path().join("data"), workspace.clone(), vec![], {
            let home = root.path().join("home");
            std::fs::create_dir_all(&home).unwrap();
            home
        })
        .await
        .unwrap();
    let mut doc = document();
    let saved = if via_save {
        let file = request(&app, "POST", "/api/flows", Some(json!({"composition":doc}))).await;
        doc = serde_json::from_value(file["composition"].clone()).unwrap();
        Some(file)
    } else {
        None
    };
    let start = if let Some(file) = &saved {
        json!({"flowKey":file["key"],"flowHash":file["hash"],"input":{}})
    } else {
        json!({"composition":doc,"input":{}})
    };
    let started = request(&app, "POST", "/api/runs", Some(start)).await;
    let id = started["id"].as_str().unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            let run = request(&app, "GET", &format!("/api/runs/{id}"), None).await;
            assert_eq!(run["status"], "running", "{}", run["error"]);
            if run["activities"]
                .as_array()
                .unwrap()
                .iter()
                .any(|a| a["path"] == "busy" && a["status"] == "running")
            {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let mut next = doc.clone();
    next.revision += 1;
    if structural {
        next.nodes.push(
            serde_json::from_value(node(
                "inserted",
                "set",
                json!({"field":"output","value":"structural"}),
            ))
            .unwrap(),
        );
        next.edges
            .iter_mut()
            .find(|edge| edge.id == "d")
            .unwrap()
            .target = "inserted".into();
        next.edges.push(
            serde_json::from_value(json!({"id":"e","source":"inserted","target":"end"})).unwrap(),
        );
    } else {
        next.nodes
            .iter_mut()
            .find(|node| node.id == "result")
            .unwrap()
            .data
            .config["value"] = json!("updated");
    }
    let source = flow_source::render(
        &next,
        &zf_compiler::graph_compiler::GraphValidator::new(
            &zf_runtime::materialize::RuntimePrimitives,
        ),
    )
    .unwrap();
    let next_hash = hash(source.as_bytes());
    if let Some(file) = saved {
        let result = request(
            &app,
            "POST",
            "/api/flows",
            Some(json!({"key":file["key"],"expectedHash":file["hash"],"composition":next})),
        )
        .await;
        assert_eq!(result["sourceHash"], next_hash);
        assert!(!workspace.join(".zedflow/.source-acceptance.json").exists());
    } else {
        let pool = sqlx::SqlitePool::connect(&format!(
            "sqlite://{}",
            root.path().join("data/zedflow.db").display()
        ))
        .await
        .unwrap();
        let store = ContentStore::new(pool).await.unwrap();
        publish_batch(
            &store,
            &[RevisionPublication {
                run_id: id.into(),
                instance: String::new(),
                baseline: doc,
                definition: RevisionDefinition {
                    package: None,
                    context_selections: Default::default(),
                    key: "live".into(),
                    hash: next_hash.clone(),
                    source,
                    composition: next,
                },
            }],
        )
        .await
        .unwrap();
    }
    let completed = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            let run = request(&app, "GET", &format!("/api/runs/{id}"), None).await;
            if run["status"] != "running" {
                break run;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(completed["status"], "completed", "{}", completed["error"]);
    assert_eq!(
        completed["state"]["output"],
        if structural { "structural" } else { "updated" }
    );
    assert_eq!(
        std::fs::read_to_string(workspace.join("effect")).unwrap(),
        "x"
    );
    let activities = completed["activities"].as_array().unwrap();
    assert_eq!(
        activities.iter().find(|a| a["path"] == "result").unwrap()["flowRevision"]["hash"],
        next_hash
    );
    assert!(activities.iter().all(|a| a["status"] != "waiting"));
}
#[tokio::test]
async fn saved_configuration_is_adopted_at_next_step_and_keeps_inflight_revision() {
    exercise(false, false).await;
}
#[tokio::test]
async fn sequential_structure_is_rebuilt_at_durable_frontier_without_user_wait_or_repeated_effect()
{
    exercise(true, false).await;
}
#[tokio::test]
async fn save_flow_publishes_configuration_before_the_next_step() {
    exercise(false, true).await;
}
#[tokio::test]
async fn save_flow_publishes_sequential_structure_without_replaying_effect() {
    exercise(true, true).await;
}
