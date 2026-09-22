mod support;
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use serde_json::{Value, json};
use std::{path::Path, time::Duration};
use tower::ServiceExt;

async fn app(root: &Path) -> Router {
    app_with_service(root).await.0
}
async fn app_with_service(root: &Path) -> (Router, zf_execution::service::ExecutionService) {
    let workspace = root.join("workspace");
    tokio::fs::create_dir_all(&workspace).await.unwrap();
    support::open_router(root.join("data"), workspace, vec![], {
        let home = root.join("home");
        std::fs::create_dir_all(&home).unwrap();
        home
    })
    .await
    .unwrap()
}

async fn request(app: &Router, method: &str, path: &str, body: Option<Value>) -> Value {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header("content-type", "application/json")
                .body(Body::from(
                    body.map(|body| body.to_string()).unwrap_or_default(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let value: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 20_000_000).await.unwrap()).unwrap();
    assert_eq!(
        status,
        StatusCode::OK,
        "{}",
        value.get("error").unwrap_or(&Value::Null)
    );
    value
}

async fn wait_for(app: &Router, id: &str, ready: impl Fn(&Value) -> bool) -> Value {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let run = request(app, "GET", &format!("/api/runs/{id}"), None).await;
            assert_ne!(run["status"], "error", "{}", run["error"]);
            if ready(&run) {
                return run;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("run did not reach the expected timeline boundary")
}

fn node(id: &str, kind: &str, config: Value) -> Value {
    json!({"id":id,"position":{"x":0,"y":0},"data":{"kind":kind,"label":id,"config":config}})
}

fn edge(source: &str, target: &str, handle: Option<&str>) -> Value {
    json!({"id":format!("{source}-{target}"),"source":source,"target":target,"sourceHandle":handle})
}

fn labels(run: &Value) -> Vec<String> {
    run["timeline"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| {
            if entry["kind"] == "message" {
                format!(
                    "{}:{}",
                    entry["role"].as_str().unwrap(),
                    entry["text"].as_str().unwrap()
                )
            } else {
                format!("tool:{}", entry["activity"]["name"].as_str().unwrap())
            }
        })
        .collect()
}

fn split_context_config(tools: &[&str]) -> Value {
    use zf_context::context_source;
    use zf_context::starters;
    use zf_storage::context_store;
    let strategy = starters::strategy("commentary", "Commentary", tools);
    let source = context_source::generate(&strategy).unwrap();
    json!({"modelNode":"model","fanIn":"any",
        "attachments":{"tools":{"items":tools.iter().map(|name|json!({"id":name,"name":name})).collect::<Vec<_>>()}},
        "contextProgram":{"strategy":strategy,"source":source,"hash":context_store::hash(source.as_bytes()),"types":{},"bindings":starters::bindings()}
    })
}

#[tokio::test]
async fn real_workspace_tools_precede_the_answer_in_persisted_snapshots_and_after_restart() {
    let root = tempfile::tempdir().unwrap();
    let (router, service) = app_with_service(root.path()).await;
    tokio::fs::write(root.path().join("workspace/note.txt"), "avant")
        .await
        .unwrap();
    let composition = json!({"id":"timeline-tools","name":"Chronologie des outils","settings":{"maxConcurrency":1},"nodes":[
        node("s","start",json!({})),
        node("prompt","input",json!({"prompt":"Quel fichier vérifier ?","field":"input"})),
        node("model","agent",json!({"provider":"fixture","model":"fixture","fanIn":"any","tools":["read","edit","exec"],"fixtureSteps":[
            {"tool":"read","args":{"path":"note.txt"}},
            {"tool":"edit","args":{"path":"note.txt","old_string":"avant","new_string":"après"}},
            {"tool":"exec","args":{"command":"cat note.txt"}},
            {"text":"**Terminé**"}
        ]})),
        node("calls","condition",json!({"field":"hasToolCalls","equals":true})),
        node("tools","tool",json!({"tool":"execute_next_call","retry":{"maxAttempts":1}})),
        node("out","output",json!({"text":"{{output}}"})),node("end","end",json!({}))
    ],"edges":[edge("s","prompt",None),edge("prompt","model",None),edge("model","calls",None),edge("calls","tools",Some("true")),edge("tools","model",None),edge("calls","out",Some("false")),edge("out","end",None)]});
    let started = request(
        &router,
        "POST",
        "/api/runs",
        Some(json!({"composition":composition,"input":{}})),
    )
    .await;
    let id = started["id"].as_str().unwrap();
    let waiting = wait_for(&router, id, |run| run["status"] == "waiting").await;
    assert_eq!(waiting["interactive"], true);
    assert_eq!(waiting["wait"]["nodePath"], "prompt");
    assert!(labels(&waiting).is_empty());
    request(
        &router,
        "POST",
        &format!("/api/runs/{id}/answer"),
        Some(json!({"waitId":waiting["wait"]["id"],"value":"Vérifie le fichier"})),
    )
    .await;
    let done = wait_for(&router, id, |run| run["status"] == "completed").await;
    assert_eq!(
        labels(&done),
        [
            "user:Vérifie le fichier",
            "tool:read",
            "tool:edit",
            "tool:exec",
            "assistant:**Terminé**"
        ]
    );
    let entries = done["timeline"].as_array().unwrap();
    assert!(
        entries
            .windows(2)
            .all(|pair| pair[0]["seq"].as_i64().unwrap() < pair[1]["seq"].as_i64().unwrap())
    );
    assert!(
        entries
            .iter()
            .filter(|entry| entry["kind"] == "tool")
            .all(|entry| entry["activity"]["status"] == "completed")
    );
    assert_eq!(
        tokio::fs::read_to_string(root.path().join("workspace/note.txt"))
            .await
            .unwrap(),
        "après"
    );
    let snapshot = request(
        &router,
        "GET",
        &format!("/api/runs/{id}/snapshot?after=9223372036854775807"),
        None,
    )
    .await;
    assert_eq!(
        snapshot["type"], "bootstrap",
        "A cursor beyond the head requires a fresh bootstrap"
    );
    assert!(snapshot.get("events").is_none());
    assert_eq!(labels(&snapshot["run"]), labels(&done));
    for (compact, original) in snapshot["run"]["timeline"]
        .as_array()
        .unwrap()
        .iter()
        .zip(entries)
    {
        assert_eq!(compact["id"], original["id"]);
        assert_eq!(compact["seq"], original["seq"]);
        if compact["kind"] == "tool" {
            assert!(compact["activity"]["argumentsRef"].is_string());
            assert!(compact["activity"].get("arguments").is_none());
        }
    }
    service.shutdown().await.unwrap();
    drop(router);
    drop(service);
    let (restarted, service) = app_with_service(root.path()).await;
    let saved = request(&restarted, "GET", &format!("/api/runs/{id}"), None).await;
    assert_eq!(saved["timeline"], done["timeline"]);
    assert_eq!(saved["messages"], done["messages"]);
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn steering_and_followup_are_placed_at_consumption_between_real_graph_passages() {
    let root = tempfile::tempdir().unwrap();
    let router = app(root.path()).await;
    let composition = json!({"id":"timeline-messages","name":"Chronologie des messages","settings":{"maxConcurrency":1},"nodes":[
        node("s","start",json!({})),
        node("delay","tool",json!({"tool":"delay","arguments":{"milliseconds":800}})),
        node("steer","steering",json!({"fanIn":"any"})),
        node("model","agent",json!({"provider":"fixture","model":"fixture","fixtureSteps":[{"text":"Réponse"}]})),
        node("out","output",json!({"text":"{{output}}"})),
        node("inbox","inbox",json!({"prompt":"Continuer ?"}))
    ],"edges":[edge("s","delay",None),edge("delay","steer",None),edge("steer","model",None),edge("model","out",None),edge("out","inbox",None),edge("inbox","steer",None)]});
    let started = request(
        &router,
        "POST",
        "/api/runs",
        Some(json!({"composition":composition,"input":{"input":"Départ"}})),
    )
    .await;
    let id = started["id"].as_str().unwrap();
    wait_for(&router, id, |run| {
        run["timeline"].as_array().is_some_and(|entries| {
            entries
                .iter()
                .any(|entry| entry["kind"] == "tool" && entry["activity"]["status"] == "running")
        })
    })
    .await;
    request(
        &router,
        "POST",
        &format!("/api/runs/{id}/messages"),
        Some(json!({"id":"steering-message","kind":"steering","text":"Commencer par les tests"})),
    )
    .await;
    let first = wait_for(&router, id, |run| run["status"] == "waiting").await;
    assert_eq!(
        labels(&first),
        [
            "user:Départ",
            "tool:delay",
            "user:Commencer par les tests",
            "assistant:Réponse"
        ]
    );
    assert_eq!(first["queue"][0]["status"], "consumed");
    assert_eq!(first["timeline"][1]["activity"]["status"], "completed");
    request(
        &router,
        "POST",
        &format!("/api/runs/{id}/messages"),
        Some(json!({"id":"followup-message","kind":"followup","text":"Préparer le rapport"})),
    )
    .await;
    let second = wait_for(&router, id, |run| {
        run["status"] == "waiting" && run["queue"][1]["status"] == "consumed"
    })
    .await;
    assert_eq!(
        labels(&second),
        [
            "user:Départ",
            "tool:delay",
            "user:Commencer par les tests",
            "assistant:Réponse",
            "user:Préparer le rapport",
            "assistant:Réponse"
        ]
    );
    let timeline = second["timeline"].as_array().unwrap();
    assert_ne!(timeline[3]["id"], timeline[5]["id"]);
    let transcript: Vec<_> = second["messages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|message| message["text"].as_str().unwrap())
        .collect();
    assert_eq!(
        transcript,
        [
            "Départ",
            "Commencer par les tests",
            "Réponse",
            "Préparer le rapport",
            "Réponse"
        ]
    );
}

#[tokio::test]
async fn split_model_commentary_precedes_tools_and_only_final_output_reaches_the_inbox() {
    let root = tempfile::tempdir().unwrap();
    let (router, service) = app_with_service(root.path()).await;
    let workspace = root.path().join("workspace");
    tokio::fs::write(workspace.join("note.txt"), "A fixture source")
        .await
        .unwrap();
    let composition = json!({
        "formatVersion":3,"id":"commentary-loop","name":"Commentary loop",
        "settings":{"maxConcurrency":1,"recursionLimit":100},
        "nodes":[
            node("start","start",json!({})),
            node("context","context",split_context_config(&["exec", "read", "write"])),
            node("model","model",json!({"contextNode":"context","provider":"fixture","field":"prediction","fixtureSteps":[
                {"text":"Je vais lancer une recherche.","calls":[
                    {"tool":"exec","args":{"command":"printf started > started; for i in $(seq 1 200); do test -e release && break; sleep 0.01; done; printf x >> effects; printf result"}},
                    {"tool":"read","args":{"path":"note.txt"}}
                ]},
                {"text":"Je vais lancer une recherche.","tool":"write","args":{"path":"report.txt","content":"Verified"}},
                {"text":"Recherche terminée."}
            ]})),
            node("calls","condition",json!({"predicate":{"kind":"compare","field":"hasToolCalls","operator":"eq","value":true}})),
            node("tools","tool",json!({"tool":"execute_next_call","fanIn":"any","retry":{"maxAttempts":1}})),
            node("remaining","condition",json!({"predicate":{"kind":"compare","field":"hasToolCalls","operator":"eq","value":true}})),
            node("out","output",json!({"text":"{{prediction}}"})),
            node("inbox","inbox",json!({"field":"input","prompt":"Sur quoi continuer ?","responseType":"text"}))
        ],"edges":[
            edge("start","context",None),edge("context","model",None),edge("model","calls",None),
            edge("calls","tools",Some("true")),edge("tools","remaining",None),
            edge("remaining","tools",Some("true")),edge("remaining","context",Some("false")),
            edge("calls","out",Some("false")),edge("out","inbox",None),edge("inbox","context",None)
        ]
    });
    let started = request(
        &router,
        "POST",
        "/api/runs",
        Some(json!({"composition":composition,"input":{"input":"Fais la recherche"}})),
    )
    .await;
    let id = started["id"].as_str().unwrap();
    let running = wait_for(&router, id, |run| {
        run["timeline"].as_array().is_some_and(|entries| {
            entries.iter().any(|entry| {
                entry["activity"]["name"] == "exec" && entry["activity"]["status"] == "running"
            })
        })
    })
    .await;
    assert_eq!(running["status"], "running");
    assert!(running["wait"].is_null());
    assert_eq!(
        labels(&running),
        [
            "user:Fais la recherche",
            "assistant:Je vais lancer une recherche.",
            "tool:exec"
        ]
    );
    // The fixture holds its external write while the real daemon persists the
    // message and tool-start event. No answer endpoint resumes this execution.
    assert!(!workspace.join("effects").exists());
    tokio::fs::write(workspace.join("release"), "continue")
        .await
        .unwrap();
    let waiting = wait_for(&router, id, |run| run["status"] == "waiting").await;
    assert_eq!(waiting["wait"]["nodePath"], "inbox");
    assert_eq!(
        labels(&waiting),
        [
            "user:Fais la recherche",
            "assistant:Je vais lancer une recherche.",
            "tool:exec",
            "tool:read",
            "assistant:Je vais lancer une recherche.",
            "tool:write",
            "assistant:Recherche terminée."
        ]
    );
    let timeline = waiting["timeline"].as_array().unwrap();
    assert_ne!(timeline[1]["id"], timeline[4]["id"]);
    for (index, path) in [(1, "model"), (4, "model"), (6, "out")] {
        let origin = &timeline[index]["origin"];
        assert_eq!(origin["nodePath"], path);
        assert!(
            waiting["activities"]
                .as_array()
                .unwrap()
                .iter()
                .any(|activity| {
                    activity["occurrenceId"] == origin["occurrenceId"]
                        && activity["nodePath"] == path
                        && activity["status"] == "completed"
                })
        );
    }
    assert!(
        timeline
            .windows(2)
            .all(|pair| { pair[0]["seq"].as_i64().unwrap() < pair[1]["seq"].as_i64().unwrap() })
    );
    let transcript = waiting["messages"].as_array().unwrap();
    assert_eq!(transcript.len(), 4);
    assert_eq!(transcript[1]["text"], transcript[2]["text"]);
    assert_ne!(transcript[1]["id"], transcript[2]["id"]);
    assert_eq!(transcript[3]["text"], "Recherche terminée.");
    assert_eq!(
        tokio::fs::read_to_string(workspace.join("effects"))
            .await
            .unwrap(),
        "x"
    );
    assert_eq!(
        tokio::fs::read_to_string(workspace.join("report.txt"))
            .await
            .unwrap(),
        "Verified"
    );
    service.shutdown().await.unwrap();
    drop(router);
    drop(service);
    let (restarted, service) = app_with_service(root.path()).await;
    let saved = request(&restarted, "GET", &format!("/api/runs/{id}"), None).await;
    assert_eq!(saved["timeline"], waiting["timeline"]);
    assert_eq!(saved["messages"], waiting["messages"]);
    assert_eq!(saved["wait"], waiting["wait"]);
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn completing_after_tools_does_not_republish_intermediate_text_as_a_fallback_answer() {
    let root = tempfile::tempdir().unwrap();
    let router = app(root.path()).await;
    let composition = json!({"formatVersion":3,"id":"commentary-once","name":"Commentary once","nodes":[
        node("start","start",json!({})),
        node("context","context",split_context_config(&["inspect_json"])),
        node("model","model",json!({"contextNode":"context","provider":"fixture","fixtureSteps":[
            {"text":"Je vérifie les données.","tool":"inspect_json","args":{"value":{"check":true}}}
        ]})),
        node("tools","tool",json!({"tool":"execute_calls"})),
        node("end","end",json!({}))
    ],"edges":[edge("start","context",None),edge("context","model",None),edge("model","tools",None),edge("tools","end",None)]});
    let started = request(
        &router,
        "POST",
        "/api/runs",
        Some(json!({"composition":composition,"input":{"input":"Vérifie"}})),
    )
    .await;
    let id = started["id"].as_str().unwrap();
    let done = wait_for(&router, id, |run| run["status"] == "completed").await;
    assert_eq!(
        labels(&done),
        ["assistant:Je vérifie les données.", "tool:inspect_json"]
    );
    assert_eq!(done["messages"].as_array().unwrap().len(), 1);
    assert!(done["wait"].is_null());
}
