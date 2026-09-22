mod support;
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use serde_json::{Value, json};
use tower::ServiceExt;

async fn request(
    app: &Router,
    method: &str,
    path: &str,
    value: Option<Value>,
) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header("content-type", "application/json")
                .body(Body::from(value.map(|v| v.to_string()).unwrap_or_default()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 20_000_000).await.unwrap();
    let value: Value = serde_json::from_slice(&bytes).unwrap();
    if status == StatusCode::OK && method != "GET" && path.starts_with("/api/runs") {
        assert_eq!(
            value.as_object().unwrap().len(),
            3,
            "command ACK must not retransmit the run: {path}"
        );
        assert!(value["id"].is_string());
        assert!(value["workspaceId"].is_string());
        assert!(value["revision"].is_i64());
    }
    (status, value)
}
async fn wait(app: &Router, id: &str, status: &str) -> Value {
    let mut latest = Value::Null;
    for _ in 0..300 {
        latest = request(app, "GET", &format!("/api/runs/{id}"), None)
            .await
            .1;
        if latest["status"] == status {
            return latest;
        }
        assert_ne!(latest["status"], "error", "{}", summary(&latest));
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    panic!("did not reach {status}: {}", summary(&latest))
}
fn summary(run: &Value) -> Value {
    json!({"status":run["status"],"error":run["error"],"wait":run["wait"],"nodes":run["nodeActivities"],"tools":run["toolActivities"],"messageCount":run["messages"].as_array().map(Vec::len)})
}
fn node(id: &str, kind: &str, config: Value) -> Value {
    json!({"id":id,"position":{"x":0,"y":0},"data":{"kind":kind,"label":id,"config":config}})
}
fn edge(source: &str, target: &str, handle: Option<&str>) -> Value {
    json!({"id":format!("{source}-{target}"),"source":source,"target":target,"sourceHandle":handle})
}
fn harness(steps: Value) -> Value {
    json!({"id":"harness","name":"Harness API","revision":0,"settings":{"maxConcurrency":1},"nodes":[
        node("s","start",json!({})),node("context","context",json!({})),node("steer","steering",json!({"fanIn":"any"})),
        node("m","agent",json!({"modelBinding":"runtime","fixtureSteps":steps,"tools":["read","write","edit","exec"],"fanIn":"any"})),
        node("calls","condition",json!({"field":"hasToolCalls","equals":true})),
        node("tools","tool",json!({"tool":"execute_next_call","fanIn":"any","retry":{"maxAttempts":1}})),
        node("remaining","condition",json!({"field":"hasToolCalls","equals":true})),
        node("out","output",json!({"text":"{{output}}"})),node("inbox","inbox",json!({"prompt":"Continuer ?"}))
    ],"edges":[edge("s","context",None),edge("context","steer",None),edge("steer","m",None),edge("m","calls",None),edge("calls","tools",Some("true")),edge("calls","out",Some("false")),edge("tools","remaining",None),edge("remaining","tools",Some("true")),edge("remaining","steer",Some("false")),edge("out","inbox",None),edge("inbox","steer",None)]})
}

#[tokio::test]
async fn runtime_model_wait_and_revision_are_shared_between_clients() {
    let dir = tempfile::tempdir().unwrap();
    let app =
        zf_serve::server::router_with_home(dir.path().join("data"), dir.path().into(), vec![], {
            let home = dir.path().join("fixture-home");
            std::fs::create_dir_all(&home).unwrap();
            home
        })
        .await
        .unwrap();
    let (status,run)=request(&app,"POST","/api/runs",Some(json!({"composition":harness(json!([{ "text":"Bonjour"} ])),"input":{"input":"Bonjour"}}))).await;
    assert_eq!(status, StatusCode::OK, "{}", summary(&run));
    let id = run["id"].as_str().unwrap();
    let blocked = wait(&app, id, "waiting").await;
    assert_eq!(blocked["wait"]["kind"], "model_selection");
    assert_eq!(blocked["wait"]["nodePath"], "m");
    assert!(
        blocked["activities"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a["path"] == "m" && a["occurrenceId"] == blocked["wait"]["occurrenceId"])
    );
    let payload =
        json!({"nodePath":"m","selection":{"provider":"fixture","model":"fixture"},"revision":0});
    let path = format!("/api/runs/{id}/models");
    let (a, b) = tokio::join!(
        request(&app, "PATCH", &path, Some(payload.clone())),
        request(&app, "PATCH", &path, Some(payload))
    );
    assert!([a.0, b.0].contains(&StatusCode::OK));
    assert!([a.0, b.0].contains(&StatusCode::CONFLICT));
    let ready = wait(&app, id, "waiting").await;
    assert_eq!(ready["wait"]["kind"], "input", "{}", summary(&ready));
    assert_eq!(ready["modelRevision"], 1);
    assert_eq!(
        ready["messages"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|m| m["role"] == "assistant")
            .count(),
        1
    );
    let answer = json!({"waitId":ready["wait"]["id"],"value":"Bonjour"});
    assert_eq!(
        request(
            &app,
            "POST",
            &format!("/api/runs/{id}/answer"),
            Some(answer)
        )
        .await
        .0,
        StatusCode::OK
    );
    let again = wait(&app, id, "waiting").await;
    assert_eq!(
        again["messages"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|m| m["role"] == "user" && m["text"] == "Bonjour")
            .count(),
        2
    );
}

#[tokio::test]
async fn harness_uses_real_file_tools_and_exec_through_graph() {
    let dir = tempfile::tempdir().unwrap();
    let app =
        zf_serve::server::router_with_home(dir.path().join("data"), dir.path().into(), vec![], {
            let home = dir.path().join("fixture-home");
            std::fs::create_dir_all(&home).unwrap();
            home
        })
        .await
        .unwrap();
    let steps = json!([
        {"tool":"write","args":{"path":"hello.txt","content":"avant"}},
        {"tool":"read","args":{"path":"hello.txt"}},
        {"tool":"edit","args":{"path":"hello.txt","old_string":"avant","new_string":"après"}},
        {"tool":"exec","args":{"command":"cat hello.txt"}},
        {"text":"Mission accomplie"}
    ]);
    let (status,run)=request(&app,"POST","/api/runs",Some(json!({"composition":harness(steps),"input":{"input":"Travaille"},"modelBindings":{"m":{"provider":"fixture","model":"fixture"}}}))).await;
    assert_eq!(status, StatusCode::OK, "{}", summary(&run));
    let ready = wait(&app, run["id"].as_str().unwrap(), "waiting").await;
    assert_eq!(
        std::fs::read_to_string(dir.path().join("hello.txt")).unwrap(),
        "après"
    );
    assert_eq!(
        ready["state"]["response"],
        "Mission accomplie",
        "{}",
        summary(&ready)
    );
    assert_eq!(
        ready["toolActivities"].as_array().unwrap().len(),
        4,
        "{}",
        summary(&ready)
    );
    assert!(
        ready["toolActivities"]
            .as_array()
            .unwrap()
            .iter()
            .all(|a| a["status"] == "completed")
    );
}

#[tokio::test]
async fn child_model_selection_survives_router_restart_without_repeating_effect() {
    let dir = tempfile::tempdir().unwrap();
    let child = json!({"id":"child","name":"Child","revision":0,"nodes":[
        node("s","start",json!({})),node("effect","tool",json!({"tool":"exec","arguments":{"command":"printf x >> effect.txt"}})),
        node("m","agent",json!({"modelBinding":"runtime"})),node("out","output",json!({"text":"{{output}}"})),node("e","end",json!({}))
    ],"edges":[edge("s","effect",None),edge("effect","m",None),edge("m","out",None),edge("out","e",None)]});
    let parent = json!({"id":"parent","name":"Parent","revision":0,"nodes":[node("s","start",json!({})),node("child","subgraph",json!({"composition":child})),node("out","output",json!({"text":"{{output}}"})),node("e","end",json!({}))],"edges":[edge("s","child",None),edge("child","out",None),edge("out","e",None)]});
    let (app, service) =
        support::open_router(dir.path().join("data"), dir.path().into(), vec![], {
            let home = dir.path().join("fixture-home");
            std::fs::create_dir_all(&home).unwrap();
            home
        })
        .await
        .unwrap();
    let (status, run) = request(
        &app,
        "POST",
        "/api/runs",
        Some(json!({"composition":parent,"input":{"input":"Hello"}})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{}", summary(&run));
    let id = run["id"].as_str().unwrap();
    let blocked = wait(&app, id, "waiting").await;
    assert_eq!(
        blocked["wait"]["nodePath"],
        "child/m",
        "{}",
        summary(&blocked)
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("effect.txt")).unwrap(),
        "x"
    );
    service.shutdown().await.unwrap();
    drop(app);
    drop(service);
    let (app, service) =
        support::open_router(dir.path().join("data"), dir.path().into(), vec![], {
            let home = dir.path().join("fixture-home");
            std::fs::create_dir_all(&home).unwrap();
            home
        })
        .await
        .unwrap();
    let (status,response)=request(&app,"POST",&format!("/api/runs/{id}/answer"),Some(json!({"waitId":blocked["wait"]["id"],"value":{"provider":"fixture","model":"fixture"}}))).await;
    assert_eq!(status, StatusCode::OK, "{}", summary(&response));
    wait(&app, id, "completed").await;
    assert_eq!(
        std::fs::read_to_string(dir.path().join("effect.txt")).unwrap(),
        "x"
    );
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn queued_messages_are_idempotent_cancellable_and_consumed_once() {
    let dir = tempfile::tempdir().unwrap();
    let app =
        zf_serve::server::router_with_home(dir.path().join("data"), dir.path().into(), vec![], {
            let home = dir.path().join("fixture-home");
            std::fs::create_dir_all(&home).unwrap();
            home
        })
        .await
        .unwrap();
    let (_,run)=request(&app,"POST","/api/runs",Some(json!({"composition":harness(json!([{ "text":"Réponse"} ])),"input":{"input":"Départ"}}))).await;
    let id = run["id"].as_str().unwrap();
    let blocked = wait(&app, id, "waiting").await;
    let path = format!("/api/runs/{id}/messages");
    let steering = json!({"id":"steer-1","kind":"steering","text":"Corrige ceci"});
    let (a, b) = tokio::join!(
        request(&app, "POST", &path, Some(steering.clone())),
        request(&app, "POST", &path, Some(steering))
    );
    assert_eq!((a.0, b.0), (StatusCode::OK, StatusCode::OK));
    assert_eq!(
        request(
            &app,
            "POST",
            &path,
            Some(json!({"id":"steer-1","kind":"steering","text":"autre"}))
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    for (message, text) in [("follow-1", "Puis ceci"), ("cancel-1", "Annulé")] {
        assert_eq!(
            request(
                &app,
                "POST",
                &path,
                Some(json!({"id":message,"kind":"followup","text":text}))
            )
            .await
            .0,
            StatusCode::OK
        );
    }
    assert_eq!(
        request(&app, "DELETE", &format!("{path}/cancel-1"), None)
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(request(&app,"POST",&format!("/api/runs/{id}/answer"),Some(json!({"waitId":blocked["wait"]["id"],"value":{"provider":"fixture","model":"fixture"}}))).await.0,StatusCode::OK);
    let ready = wait(&app, id, "waiting").await;
    assert_eq!(ready["wait"]["kind"], "input", "{}", summary(&ready));
    let queue = ready["queue"].as_array().unwrap();
    assert_eq!(queue.len(), 3);
    assert_eq!(
        queue.iter().filter(|m| m["status"] == "consumed").count(),
        2
    );
    assert_eq!(
        queue.iter().filter(|m| m["status"] == "cancelled").count(),
        1
    );
    let messages = ready["messages"].as_array().unwrap();
    assert_eq!(messages.iter().filter(|m| m["id"] == "steer-1").count(), 1);
    assert_eq!(messages.iter().filter(|m| m["id"] == "follow-1").count(), 1);
    assert!(!messages.iter().any(|m| m["text"] == "Annulé"));
    assert_eq!(
        request(&app, "DELETE", &format!("{path}/steer-1"), None)
            .await
            .0,
        StatusCode::CONFLICT
    );
}

#[tokio::test]
async fn abort_stops_exec_and_resume_keeps_completed_tool_receipt() {
    let dir = tempfile::tempdir().unwrap();
    let app =
        zf_serve::server::router_with_home(dir.path().join("data"), dir.path().into(), vec![], {
            let home = dir.path().join("fixture-home");
            std::fs::create_dir_all(&home).unwrap();
            home
        })
        .await
        .unwrap();
    let steps = json!([{"tool":"exec","args":{"command":"printf x >> effect.txt; sleep 30; printf unexpected >> effect.txt"}},{"text":"Repris"}]);
    let (_,run)=request(&app,"POST","/api/runs",Some(json!({"composition":harness(steps),"input":{"input":"Départ"},"modelBindings":{"m":{"provider":"fixture","model":"fixture"}}}))).await;
    let id = run["id"].as_str().unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while !dir.path().join("effect.txt").exists() {
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        request(&app, "POST", &format!("/api/runs/{id}/abort"), None)
            .await
            .0,
        StatusCode::OK
    );
    let stopped = wait(&app, id, "stopped").await;
    assert!(stopped["checkpoint"].is_string());
    assert_eq!(
        std::fs::read_to_string(dir.path().join("effect.txt")).unwrap(),
        "x"
    );
    assert_eq!(
        request(
            &app,
            "POST",
            &format!("/api/runs/{id}/resume"),
            Some(json!({}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let resumed = wait(&app, id, "waiting").await;
    assert_eq!(
        resumed["state"]["response"],
        "Repris",
        "{}",
        summary(&resumed)
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("effect.txt")).unwrap(),
        "x"
    );
}

#[tokio::test]
async fn skills_load_progressively_and_explicit_invocation_refreshes_content() {
    let dir = tempfile::tempdir().unwrap();
    let skills = dir.path().join("skills/demo");
    std::fs::create_dir_all(&skills).unwrap();
    let file = skills.join("SKILL.md");
    std::fs::write(
        &file,
        "---\nname: demo\ndescription: Test workflow\n---\nInstruction initiale.",
    )
    .unwrap();
    std::fs::write(dir.path().join("AGENTS.md"), "Instruction projet locale.").unwrap();
    let app = zf_serve::server::router_with_home(
        dir.path().join("data"),
        dir.path().into(),
        vec![skills],
        {
            let home = dir.path().join("fixture-home");
            std::fs::create_dir_all(&home).unwrap();
            home
        },
    )
    .await
    .unwrap();
    let (_,run)=request(&app,"POST","/api/runs",Some(json!({"composition":harness(json!([{ "text":"Chargé"} ])),"input":{"input":"/skill:demo exemple"},"modelBindings":{"m":{"provider":"fixture","model":"fixture"}}}))).await;
    let id = run["id"].as_str().unwrap();
    let ready = wait(&app, id, "waiting").await;
    assert!(
        ready["context"]["instructions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["content"] == "Instruction projet locale.")
    );
    assert!(
        ready["context"]["skills"]
            .as_array()
            .unwrap()
            .iter()
            .all(|s| s.get("body").is_none())
    );
    assert_eq!(
        ready["context"]["loadedSkills"].as_array().unwrap().len(),
        1
    );
    assert_eq!(ready["messages"][0]["text"], "/skill:demo exemple");
    assert!(
        ready["state"]["input"]
            .as_str()
            .unwrap()
            .contains("Instruction initiale.")
    );
    std::fs::write(
        &file,
        "---\nname: demo\ndescription: Test workflow\n---\nInstruction actualisée.",
    )
    .unwrap();
    assert_eq!(
        request(
            &app,
            "POST",
            &format!("/api/runs/{id}/answer"),
            Some(json!({"waitId":ready["wait"]["id"],"value":"/skill:demo suite"}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let refreshed = wait(&app, id, "waiting").await;
    assert!(
        refreshed["state"]["input"]
            .as_str()
            .unwrap()
            .contains("Instruction actualisée.")
    );
    assert_eq!(
        refreshed["context"]["loadedSkills"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}

#[tokio::test]
async fn accepted_answer_survives_crash_before_executor_takes_its_first_checkpoint() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    let app = zf_serve::server::router_with_home(data.clone(), dir.path().into(), vec![], {
        let home = dir.path().join("fixture-home");
        std::fs::create_dir_all(&home).unwrap();
        home
    })
    .await
    .unwrap();
    let (_,run)=request(&app,"POST","/api/runs",Some(json!({"composition":harness(json!([{ "text":"Réponse"} ])),"input":{"input":"Départ"},"modelBindings":{"m":{"provider":"fixture","model":"fixture"}}}))).await;
    let id = run["id"].as_str().unwrap();
    let mut saved = wait(&app, id, "waiting").await;
    let wait_id = saved["wait"]["id"].clone();
    // Reconstruct a process lost after durable command acceptance but before
    // the spawned executor's first poll: its ADK checkpoint is still the wait.
    saved["resumeInput"] = json!({"answer:inbox":{"__zedflowAnswerId":wait_id,"value":"Réponse acceptée avant le crash"}});
    saved["resumeCheckpoint"] = saved["checkpoint"].clone();
    saved["status"] = json!("running");
    saved["wait"] = Value::Null;
    saved["messages"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":wait_id,"role":"user","text":"Réponse acceptée avant le crash"}));
    drop(app);
    let db = sqlx::SqlitePool::connect(&format!("sqlite://{}", data.join("zedflow.db").display()))
        .await
        .unwrap();
    zf_storage::session_store::save(&db, id, &saved)
        .await
        .unwrap();
    db.close().await;
    let app = zf_serve::server::router_with_home(data, dir.path().into(), vec![], {
        let home = dir.path().join("fixture-home");
        std::fs::create_dir_all(&home).unwrap();
        home
    })
    .await
    .unwrap();
    assert_eq!(
        request(&app, "GET", &format!("/api/runs/{id}"), None)
            .await
            .1["status"],
        "interrupted"
    );
    assert_eq!(
        request(
            &app,
            "POST",
            &format!("/api/runs/{id}/resume"),
            Some(json!({}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let recovered = wait(&app, id, "waiting").await;
    assert_eq!(
        recovered["state"]["input"],
        "Réponse acceptée avant le crash",
        "{}",
        summary(&recovered)
    );
    assert!(recovered["resumeInput"].is_null());
    assert_eq!(
        recovered["messages"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|m| m["id"] == wait_id)
            .count(),
        1
    );
}

#[tokio::test]
async fn messages_consumed_inside_a_waiting_subgraph_are_acknowledged_in_the_api() {
    let dir = tempfile::tempdir().unwrap();
    let child = harness(json!([{ "text":"Enfant"} ]));
    let parent = json!({"id":"parent-inbox","name":"Parent inbox","revision":0,"nodes":[node("s","start",json!({})),node("child","subgraph",json!({"composition":child})),node("e","end",json!({}))],"edges":[edge("s","child",None),edge("child","e",None)]});
    let (app, service) =
        support::open_router(dir.path().join("data"), dir.path().into(), vec![], {
            let home = dir.path().join("fixture-home");
            std::fs::create_dir_all(&home).unwrap();
            home
        })
        .await
        .unwrap();
    let (_, run) = request(
        &app,
        "POST",
        "/api/runs",
        Some(json!({"composition":parent,"input":{"input":"Départ"}})),
    )
    .await;
    let id = run["id"].as_str().unwrap();
    let blocked = wait(&app, id, "waiting").await;
    assert_eq!(blocked["wait"]["nodePath"], "child/m");
    assert_eq!(
        request(
            &app,
            "POST",
            &format!("/api/runs/{id}/messages"),
            Some(json!({"id":"nested-follow","kind":"followup","text":"Suite dans enfant"}))
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(request(&app,"POST",&format!("/api/runs/{id}/answer"),Some(json!({"waitId":blocked["wait"]["id"],"value":{"provider":"fixture","model":"fixture"}}))).await.0,StatusCode::OK);
    let ready = wait(&app, id, "waiting").await;
    assert_eq!(
        ready["wait"]["nodePath"],
        "child/inbox",
        "{}",
        summary(&ready)
    );
    assert_eq!(
        ready["queue"][0]["status"],
        "consumed",
        "{}",
        summary(&ready)
    );
    assert_eq!(
        ready["messages"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|m| m["id"] == "nested-follow")
            .count(),
        1
    );
    service.shutdown().await.unwrap();
    drop(app);
    drop(service);
    let (app, service) =
        support::open_router(dir.path().join("data"), dir.path().into(), vec![], {
            let home = dir.path().join("fixture-home");
            std::fs::create_dir_all(&home).unwrap();
            home
        })
        .await
        .unwrap();
    assert_eq!(
        request(
            &app,
            "DELETE",
            &format!("/api/runs/{id}/messages/nested-follow"),
            None
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn ordinary_tool_failure_returns_to_the_model_and_the_run_can_repair_it() {
    let dir = tempfile::tempdir().unwrap();
    let app =
        zf_serve::server::router_with_home(dir.path().join("data"), dir.path().into(), vec![], {
            let home = dir.path().join("fixture-home");
            std::fs::create_dir_all(&home).unwrap();
            home
        })
        .await
        .unwrap();
    let steps = json!([
        {"tool":"read","args":{"path":"missing.txt"}},
        {"tool":"write","args":{"path":"missing.txt","content":"réparé"}},
        {"tool":"exec","args":{"command":"test -f missing.txt && cat missing.txt"}},
        {"text":"Fichier réparé et vérifié"}
    ]);
    let (_,run)=request(&app,"POST","/api/runs",Some(json!({"composition":harness(steps),"input":{"input":"Répare le fichier absent"},"modelBindings":{"m":{"provider":"fixture","model":"fixture"}}}))).await;
    let ready = wait(&app, run["id"].as_str().unwrap(), "waiting").await;
    assert_eq!(ready["toolActivities"][0]["status"], "failed");
    assert_eq!(ready["toolActivities"][1]["status"], "completed");
    assert_eq!(ready["toolActivities"][2]["status"], "completed");
    assert_eq!(
        std::fs::read_to_string(dir.path().join("missing.txt")).unwrap(),
        "réparé"
    );
    assert_eq!(ready["state"]["response"], "Fichier réparé et vérifié");
    assert!(ready["state"]["messages"].to_string().contains("error"));
}

#[tokio::test]
async fn a_queued_skill_keeps_its_submitted_content_after_source_change_and_restart() {
    let dir = tempfile::tempdir().unwrap();
    let skills = dir.path().join("skills/demo");
    std::fs::create_dir_all(&skills).unwrap();
    let file = skills.join("SKILL.md");
    std::fs::write(
        &file,
        "---\nname: demo\ndescription: Test workflow\n---\nContenu envoyé initial.",
    )
    .unwrap();
    let (app, service) = support::open_router(
        dir.path().join("data"),
        dir.path().into(),
        vec![skills.clone()],
        {
            let home = dir.path().join("fixture-home");
            std::fs::create_dir_all(&home).unwrap();
            home
        },
    )
    .await
    .unwrap();
    let (_,run)=request(&app,"POST","/api/runs",Some(json!({"composition":harness(json!([{ "text":"Réponse"} ])),"input":{"input":"Départ"}}))).await;
    let id = run["id"].as_str().unwrap();
    let blocked = wait(&app, id, "waiting").await;
    let (status, _) = request(
        &app,
        "POST",
        &format!("/api/runs/{id}/messages"),
        Some(json!({"id":"skill-queued","kind":"followup","text":"/skill:demo mes arguments"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let queued = request(&app, "GET", &format!("/api/runs/{id}"), None)
        .await
        .1;
    let submitted = queued["queue"][0]["text"].clone();
    assert!(
        submitted
            .as_str()
            .unwrap()
            .contains("Contenu envoyé initial.")
    );
    let hash = queued["context"]["loadedSkills"][0]["hash"].clone();
    std::fs::write(
        &file,
        "---\nname: demo\ndescription: Test workflow\n---\nContenu changé après envoi.",
    )
    .unwrap();
    service.shutdown().await.unwrap();
    drop(app);
    drop(service);
    let (app, service) =
        support::open_router(dir.path().join("data"), dir.path().into(), vec![skills], {
            let home = dir.path().join("fixture-home");
            std::fs::create_dir_all(&home).unwrap();
            home
        })
        .await
        .unwrap();
    assert_eq!(request(&app,"POST",&format!("/api/runs/{id}/answer"),Some(json!({"waitId":blocked["wait"]["id"],"value":{"provider":"fixture","model":"fixture"}}))).await.0,StatusCode::OK);
    let ready = wait(&app, id, "waiting").await;
    assert_eq!(ready["state"]["input"], submitted);
    assert_eq!(ready["queue"][0]["status"], "consumed");
    assert_eq!(ready["context"]["loadedSkills"][0]["hash"], hash);
    assert!(
        ready["messages"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["text"] == "/skill:demo mes arguments")
    );
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn steering_sent_during_a_tool_batch_waits_for_its_last_call() {
    let dir = tempfile::tempdir().unwrap();
    let app =
        zf_serve::server::router_with_home(dir.path().join("data"), dir.path().into(), vec![], {
            let home = dir.path().join("fixture-home");
            std::fs::create_dir_all(&home).unwrap();
            home
        })
        .await
        .unwrap();
    let steps = json!([
        {"calls":[
            {"tool":"exec","args":{"command":"printf first > sequence; touch first.started; while [ ! -f release ]; do sleep .02; done; printf '\\nfirst-done' >> sequence"}},
            {"tool":"exec","args":{"command":"printf '\\nsecond' >> sequence; cat sequence"}}
        ]},
        {"text":"Lot terminé"}
    ]);
    let (status,run)=request(&app,"POST","/api/runs",Some(json!({"composition":harness(steps),"input":{"input":"Commence"},"modelBindings":{"m":{"provider":"fixture","model":"fixture"}}}))).await;
    assert_eq!(status, StatusCode::OK, "{}", summary(&run));
    let id = run["id"].as_str().unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while !dir.path().join("first.started").exists() {
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    for (message, kind, text) in [
        ("during-batch", "steering", "Réoriente après le lot"),
        ("after-response", "followup", "Ensuite seulement"),
    ] {
        let (status, _) = request(
            &app,
            "POST",
            &format!("/api/runs/{id}/messages"),
            Some(json!({"id":message,"kind":kind,"text":text})),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let queued = request(&app, "GET", &format!("/api/runs/{id}"), None)
            .await
            .1;
        assert!(
            queued["queue"]
                .as_array()
                .unwrap()
                .iter()
                .all(|m| m["status"] == "pending")
        );
    }
    std::fs::write(dir.path().join("release"), "").unwrap();
    let ready = wait(&app, id, "waiting").await;
    assert_eq!(
        std::fs::read_to_string(dir.path().join("sequence")).unwrap(),
        "first\nfirst-done\nsecond"
    );
    let activities = ready["activities"].as_array().unwrap();
    let calls: Vec<_> = activities
        .iter()
        .filter(|a| a["node"] == "tools" && a["status"] == "completed")
        .collect();
    assert_eq!(calls.len(), 2);
    let steering = activities
        .iter()
        .find(|a| a["node"] == "steer" && a["output"]["input"] == "Réoriente après le lot")
        .unwrap();
    assert!(
        calls
            .iter()
            .all(|a| a["step"].as_u64().unwrap() < steering["step"].as_u64().unwrap())
    );
    let followup = activities
        .iter()
        .find(|a| a["node"] == "inbox" && a["output"]["input"] == "Ensuite seulement")
        .unwrap();
    assert!(activities.iter().any(|a| a["node"] == "out"
        && a["status"] == "completed"
        && a["step"].as_u64().unwrap() < followup["step"].as_u64().unwrap()));
    assert!(
        ready["queue"]
            .as_array()
            .unwrap()
            .iter()
            .all(|m| m["status"] == "consumed")
    );
    for message in ["during-batch", "after-response"] {
        assert_eq!(
            ready["messages"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|m| m["id"] == message)
                .count(),
            1
        );
    }
}

#[tokio::test]
async fn reading_a_skill_preserves_frozen_context_and_publishes_its_loaded_summary() {
    let dir = tempfile::tempdir().unwrap();
    let skill = dir.path().join(".agents/skills/demo/SKILL.md");
    std::fs::create_dir_all(skill.parent().unwrap()).unwrap();
    std::fs::write(
        &skill,
        "---\nname: demo\ndescription: Skill fixture\n---\nFrozen skill body.",
    )
    .unwrap();
    std::fs::write(dir.path().join("AGENTS.md"), "Frozen instructions.").unwrap();
    let (app, service) =
        support::open_router(dir.path().join("data"), dir.path().into(), vec![], {
            let home = dir.path().join("fixture-home");
            std::fs::create_dir_all(&home).unwrap();
            home
        })
        .await
        .unwrap();
    let (_,started)=request(&app,"POST","/api/runs",Some(json!({"composition":harness(json!([
        {"tool":"read","args":{"path":skill}}, {"text":"Loaded"}
    ])),"input":{"input":"Read skill"},"modelBindings":{"m":{"provider":"fixture","model":"fixture"}}}))).await;
    let id = started["id"].as_str().unwrap();
    let ready = wait(&app, id, "waiting").await;
    assert_eq!(ready["context"]["cwd"], json!(dir.path()));
    assert!(
        ready["context"]["instructions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["content"] == "Frozen instructions.")
    );
    assert!(
        ready["context"]["skills"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["name"] == "demo")
    );
    assert_eq!(
        ready["context"]["loadedSkills"].as_array().unwrap().len(),
        1
    );
    let (_, delta) = request(
        &app,
        "GET",
        &format!("/api/runs/{id}/snapshot?after=1"),
        None,
    )
    .await;
    let mut cursor = 1;
    let mut frame = delta;
    let mut found = false;
    while frame["type"] == "delta" {
        assert!(frame["revision"].as_i64().unwrap() > cursor);
        cursor = frame["revision"].as_i64().unwrap();
        found |= frame["ops"].as_array().unwrap().iter().any(|op| {
            op["value"]["context"]["loadedSkills"]
                .as_array()
                .is_some_and(|v| !v.is_empty())
        });
        frame = request(
            &app,
            "GET",
            &format!("/api/runs/{id}/snapshot?after={cursor}"),
            None,
        )
        .await
        .1;
    }
    assert!(
        found,
        "The loaded skill summary must be delivered without a bootstrap"
    );
    service.shutdown().await.unwrap();
    drop(app);
    drop(service);
    let (restarted, service) =
        support::open_router(dir.path().join("data"), dir.path().into(), vec![], {
            let home = dir.path().join("fixture-home");
            std::fs::create_dir_all(&home).unwrap();
            home
        })
        .await
        .unwrap();
    let saved = request(&restarted, "GET", &format!("/api/runs/{id}"), None)
        .await
        .1;
    assert_eq!(saved["context"], ready["context"]);
    service.shutdown().await.unwrap();
}
