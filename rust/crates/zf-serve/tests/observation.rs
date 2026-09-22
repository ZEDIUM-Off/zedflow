//! Observe a genuinely running ADK node through the public API, then reconnect its event log.
use anyhow::{Context, Result};
use axum::{
    Router,
    body::{Body, Bytes, to_bytes},
    http::{Request, StatusCode},
};
use futures::{Stream, StreamExt};
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use tower::ServiceExt;

async fn call(app: &Router, path: &str, body: Option<Value>) -> Result<Value> {
    let mut request = Request::builder().uri(path);
    if body.is_some() {
        request = request
            .method("POST")
            .header("content-type", "application/json");
    }
    let response = app
        .clone()
        .oneshot(request.body(Body::from(body.map(|v| v.to_string()).unwrap_or_default()))?)
        .await?;
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 2_000_000).await?;
    assert_eq!(
        status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&bytes)
    );
    Ok(serde_json::from_slice(&bytes)?)
}

fn composition() -> Value {
    json!({"id":"observation","name":"Observation réelle","nodes":[
        {"id":"s","position":{"x":0,"y":0},"data":{"kind":"start","label":"Début"}},
        {"id":"work","position":{"x":200,"y":0},"data":{"kind":"tool","label":"Travail observable","config":{"tool":"delay","arguments":{"milliseconds":500},"field":"output","ui":{"renderer":"json"}}}},
        {"id":"e","position":{"x":400,"y":0},"data":{"kind":"end","label":"Fin"}}
    ],"edges":[{"id":"s-work","source":"s","target":"work"},{"id":"work-e","source":"work","target":"e"}]})
}

async fn next_snapshot<S>(stream: &mut S, pending: &mut String) -> Result<Value>
where
    S: Stream<Item = Result<Bytes, axum::Error>> + Unpin,
{
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if let Some(end) = pending.find("\n\n") {
                let frame = pending[..end].to_owned();
                pending.drain(..end + 2);
                if let Some(data) = frame.lines().find_map(|line| line.strip_prefix("data: ")) {
                    let payload: Value = serde_json::from_str(data)?;
                    assert!(payload.get("error").is_none(), "{payload}");
                    return Ok(payload);
                }
            } else {
                let chunk = stream.next().await.context("SSE ended before snapshot")??;
                pending.push_str(std::str::from_utf8(&chunk)?);
            }
        }
    })
    .await?
}

fn apply_frame(projection: &mut Value, payload: &Value) {
    match payload["type"].as_str() {
        Some("bootstrap") => *projection = payload["run"].clone(),
        Some("delta") => {
            for op in payload["ops"].as_array().unwrap() {
                let collection = op["collection"].as_str().unwrap();
                if collection == "meta" {
                    for (key, value) in op["value"].as_object().unwrap() {
                        projection[key] = value.clone();
                    }
                } else {
                    let field = match collection {
                        "activities" => "occurrenceId",
                        "toolActivities" => "callId",
                        "contextSnapshots" => "invocationId",
                        _ => "id",
                    };
                    let list = projection[collection].as_array_mut().unwrap();
                    if let Some(item) = list.iter_mut().find(|item| item[field] == op["id"]) {
                        *item = op["value"].clone();
                    } else {
                        list.push(op["value"].clone());
                    }
                }
            }
        }
        Some("heartbeat") => assert!(payload.get("run").is_none()),
        _ => panic!("unexpected frame {payload}"),
    }
}

#[tokio::test]
async fn running_tool_is_visible_before_completion_and_sse_resumes_without_replay() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let app = zf_serve::server::router_with_home(
        directory.path().into(),
        directory.path().into(),
        vec![],
        {
            let home = directory.path().join("fixture-home");
            std::fs::create_dir_all(&home).unwrap();
            home
        },
    )
    .await?;
    let started = Instant::now();
    let run = call(
        &app,
        "/api/runs",
        Some(json!({"composition":composition(),"input":{}})),
    )
    .await?;
    let id = run["id"].as_str().context("run id")?;
    let response = app
        .clone()
        .oneshot(Request::get(format!("/api/runs/{id}/events?after=0")).body(Body::empty())?)
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let mut stream = response.into_body().into_data_stream();
    let mut pending = String::new();

    let mut projection = json!({});
    let running = loop {
        let mut payload = next_snapshot(&mut stream, &mut pending).await?;
        apply_frame(&mut projection, &payload);
        payload["run"] = projection.clone();
        assert_ne!(payload["run"]["status"], "error", "{payload}");
        assert_ne!(
            payload["run"]["status"], "completed",
            "The API must expose work while the node is still executing"
        );
        if payload["run"]["activeNode"] == "work" {
            break payload;
        }
    };
    let active = running["run"]["activities"]
        .as_array()
        .context("activities")?
        .iter()
        .find(|v| v["node"] == "work")
        .context("running tool activity")?;
    assert_eq!(active["status"], "running");
    assert_eq!(active["kind"], "tool");
    assert!(
        active.get("durationMs").is_none(),
        "Unfinished work has no fabricated final duration"
    );
    let occurrence = active["occurrenceId"].clone();
    let cursor = running["cursor"].as_i64().context("running cursor")?;
    assert!(cursor > 0);
    let current = call(&app, &format!("/api/runs/{id}"), None).await?;
    assert_eq!(
        current["status"], "running",
        "Snapshot must arrive before real completion"
    );
    drop(stream);

    // EventSource reconnect retains its original URL: Last-Event-ID must override its after=0.
    let response = app
        .clone()
        .oneshot(
            Request::get(format!("/api/runs/{id}/events?after=0"))
                .header("last-event-id", cursor.to_string())
                .body(Body::empty())?,
        )
        .await?;
    let mut stream = response.into_body().into_data_stream();
    pending.clear();
    let mut delivered_cursor = cursor;
    let completed = loop {
        let mut payload = next_snapshot(&mut stream, &mut pending).await?;
        if payload["type"] == "delta" {
            assert_eq!(payload["baseRevision"], delivered_cursor);
            assert!(payload["revision"].as_i64().unwrap() > delivered_cursor);
        }
        apply_frame(&mut projection, &payload);
        delivered_cursor = payload["cursor"].as_i64().context("cursor")?;
        payload["run"] = projection.clone();
        assert_ne!(payload["run"]["status"], "error", "{payload}");
        if payload["run"]["status"] == "completed" {
            break payload;
        }
    };
    let finished = completed["run"]["activities"]
        .as_array()
        .context("finished activities")?
        .iter()
        .find(|v| v["occurrenceId"] == occurrence)
        .context("same tool occurrence")?;
    assert_eq!(finished["status"], "completed");
    assert!(
        finished["durationMs"]
            .as_u64()
            .context("measured duration")?
            >= 450
    );
    assert!(started.elapsed() >= Duration::from_millis(500));
    assert!(
        completed["run"]["activeNodes"]
            .as_array()
            .context("active nodes")?
            .is_empty()
    );
    Ok(())
}

#[tokio::test]
async fn timeout_retries_have_distinct_interrupted_occurrences_without_stale_running_nodes()
-> Result<()> {
    let directory = tempfile::tempdir()?;
    let app = zf_serve::server::router_with_home(
        directory.path().into(),
        directory.path().into(),
        vec![],
        {
            let home = directory.path().join("fixture-home");
            std::fs::create_dir_all(&home).unwrap();
            home
        },
    )
    .await?;
    let mut doc = composition();
    doc["settings"] = json!({
        "timeoutMs":20,
        "retry":{"maxAttempts":2,"initialDelayMs":5,"maxDelayMs":5,"backoffFactor":1.0,"jitter":0.0,"retryOn":"timeout"}
    });
    let run = call(
        &app,
        "/api/runs",
        Some(json!({"composition":doc,"input":{}})),
    )
    .await?;
    let id = run["id"].as_str().context("run id")?;
    let failed = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let run = call(&app, &format!("/api/runs/{id}"), None).await?;
            match run["status"].as_str() {
                Some("error") => return Ok::<_, anyhow::Error>(run),
                Some("completed" | "waiting" | "paused") => {
                    anyhow::bail!("A timed-out tool must fail after exhausting retries: {run}")
                }
                _ => tokio::time::sleep(Duration::from_millis(10)).await,
            }
        }
    })
    .await??;
    let activities = failed["activities"].as_array().context("activities")?;
    let attempts: Vec<_> = activities
        .iter()
        .filter(|activity| activity["node"] == "work")
        .collect();
    assert_eq!(
        attempts.len(),
        2,
        "Retry policy must produce two observable attempts: {failed}"
    );
    let mut identities = std::collections::HashSet::new();
    for attempt in attempts {
        assert_eq!(
            attempt["status"], "interrupted",
            "Dropped futures must not remain running: {attempt}"
        );
        let identity = attempt["occurrenceId"]
            .as_str()
            .context("attempt occurrence identity")?;
        assert!(!identity.is_empty());
        assert!(
            identities.insert(identity),
            "Each retry needs a separate occurrence identity"
        );
        assert!(attempt["durationMs"].as_u64().context("attempt duration")? >= 15);
        assert!(attempt["endedAt"].as_u64().is_some());
        assert!(attempt["error"].as_str().is_some());
    }
    assert!(
        activities
            .iter()
            .all(|activity| activity["status"] != "running")
    );
    assert!(failed["activeNode"].is_null());
    assert!(
        failed["activeNodes"]
            .as_array()
            .context("active nodes")?
            .is_empty()
    );
    Ok(())
}

async fn preparation_fixture(
    revisions: bool,
    timeout: bool,
) -> Result<(
    tempfile::TempDir,
    zf_storage::content_store::ContentStore,
    adk_graph::CompiledGraph,
    zf_runtime::event_sink::EventReceiver,
    String,
)> {
    let directory = tempfile::tempdir()?;
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(directory.path().join("content.sqlite"))
                .create_if_missing(true),
        )
        .await?;
    let store = zf_storage::content_store::ContentStore::new(pool).await?;
    let context = zf_runtime::workspace_context::ContextSnapshot::load_with_home(
        directory.path(),
        &[],
        Some(&directory.path().join("fixture-home")),
    )
    .await?;
    let services = zf_runtime::runtime::RunServices::new(
        "preparation-fixture".into(),
        directory.path().into(),
        directory.path().join("runtime"),
        context,
        json!({}),
        vec![],
    )?;
    services.set_content_store(store.clone());
    let mut doc = composition();
    if timeout {
        doc["settings"] = json!({
            "timeoutMs":20,
            "retry":{"maxAttempts":2,"initialDelayMs":5,"maxDelayMs":5,"backoffFactor":1.0,"jitter":0.0,"retryOn":"timeout"}
        });
    } else {
        doc["nodes"][1]["data"]["config"]["arguments"]["milliseconds"] = json!(1);
    }
    let doc: zf_flows::schema::Composition = serde_json::from_value(doc)?;
    let source = zf_flows::flow_source::render(
        &doc,
        &zf_compiler::graph_compiler::GraphValidator::new(
            &zf_runtime::materialize::RuntimePrimitives,
        ),
    )?;
    let hash = zf_storage::flow_store::hash(source.as_bytes());
    if revisions {
        let revision = zf_runtime::revisions::RevisionDefinition {
            package: None,
            context_selections: Default::default(),
            key: "fixture-definition".into(),
            hash: hash.clone(),
            source,
            composition: doc.clone(),
        };
        services.set_revisions(
            zf_runtime::revisions::RevisionRuntime::new(
                store.clone(),
                &services.id,
                std::collections::BTreeMap::from([(String::new(), revision)]),
            )
            .await?,
        )?;
    }
    let (sender, receiver) = zf_runtime::event_sink::channel(64);
    let graph = zf_runtime::materialize::build_with_services(&doc, services, Some(sender), None)?;
    Ok((directory, store, graph, receiver, hash))
}

async fn observe_graph(
    graph: adk_graph::CompiledGraph,
    mut receiver: zf_runtime::event_sink::EventReceiver,
) -> Result<(adk_graph::error::Result<adk_graph::State>, Vec<Value>)> {
    tokio::time::timeout(Duration::from_secs(5), async move {
        let execution = tokio::spawn(async move {
            graph
                .invoke(
                    adk_graph::State::new(),
                    adk_graph::ExecutionConfig::new("preparation-fixture"),
                )
                .await
        });
        let mut events = Vec::new();
        while let Some(event) = receiver.recv().await {
            events.push(event);
        }
        Ok((execution.await?, events))
    })
    .await?
}

#[tokio::test]
async fn timeout_during_revision_selection_or_snapshot_capture_retains_each_attempt() -> Result<()>
{
    for revisions in [true, false] {
        let (_directory, store, graph, receiver, _) = preparation_fixture(revisions, true).await?;
        // Exhaust the single-connection pool after setup. With revisions this
        // blocks the selector; without them it blocks the exact input capture.
        // No wall-clock race with a fast database is needed to reach either gap.
        let connection = store.pool().acquire().await?;
        let (result, events) = observe_graph(graph, receiver).await?;
        assert!(result.is_err());
        let attempts: Vec<_> = events
            .iter()
            .filter(|event| event["type"] == "node_activity")
            .collect();
        assert_eq!(attempts.len(), 2, "{events:?}");
        assert_ne!(attempts[0]["occurrenceId"], attempts[1]["occurrenceId"]);
        for attempt in attempts {
            assert_eq!(attempt["status"], "interrupted");
            assert_eq!(attempt["phase"], "preparing");
            assert_eq!(attempt["path"], "work");
            assert!(attempt["durationMs"].as_u64().unwrap() >= 15);
            assert!(
                attempt["flowRevision"].is_null(),
                "No revision was selected"
            );
            assert!(attempt["output"].is_null(), "No operation executed");
        }
        assert!(events.iter().all(|event| event["type"] != "tool_call"));
        drop(connection);
    }
    Ok(())
}

#[tokio::test]
async fn prepared_attempt_hands_its_identity_to_the_selected_observer_once() -> Result<()> {
    let (_directory, store, graph, receiver, hash) = preparation_fixture(true, false).await?;
    let (result, events) = observe_graph(graph, receiver).await?;
    result?;
    let attempts: Vec<_> = events
        .iter()
        .filter(|event| event["type"] == "node_activity")
        .collect();
    assert_eq!(attempts.len(), 2, "Only running and completed: {events:?}");
    assert_eq!(attempts[0]["status"], "running");
    assert_eq!(attempts[1]["status"], "completed");
    assert_eq!(attempts[0]["occurrenceId"], attempts[1]["occurrenceId"]);
    assert_eq!(attempts[0]["startedAt"], attempts[1]["startedAt"]);
    for attempt in attempts {
        assert_eq!(attempt["flowRevision"]["hash"], hash);
        assert_eq!(
            store.resolve(attempt["inputRef"].as_str().unwrap()).await?,
            json!({"milliseconds":1})
        );
    }
    Ok(())
}

#[tokio::test]
async fn timeout_attempts_survive_a_receiver_retained_until_execution_finishes() -> Result<()> {
    for revisions in [true, false] {
        let (_directory, store, graph, mut receiver, _) =
            preparation_fixture(revisions, true).await?;
        let connection = store.pool().acquire().await?;
        let result = tokio::time::timeout(
            Duration::from_secs(5),
            graph.invoke(
                adk_graph::State::new(),
                adk_graph::ExecutionConfig::new("preparation-fixture"),
            ),
        )
        .await?;
        assert!(result.is_err());
        drop(graph);
        let mut attempts = Vec::new();
        while let Some(event) = receiver.recv().await {
            if event["type"] == "node_activity" {
                attempts.push(event);
            }
        }
        assert_eq!(attempts.len(), 2, "{attempts:?}");
        assert_ne!(attempts[0]["occurrenceId"], attempts[1]["occurrenceId"]);
        assert!(
            attempts
                .iter()
                .all(|event| event["status"] == "interrupted" && event["flowRevision"].is_null())
        );
        drop(connection);
    }
    Ok(())
}

#[tokio::test]
async fn snapshot_storage_error_is_observed_as_error_instead_of_cancellation() -> Result<()> {
    for revisions in [false, true] {
        let (_directory, store, graph, receiver, _) = preparation_fixture(revisions, false).await?;
        store.pool().close().await;
        let (result, events) = observe_graph(graph, receiver).await?;
        let error = result
            .expect_err("Closed snapshot store must fail")
            .to_string();
        let attempts: Vec<_> = events
            .iter()
            .filter(|event| event["type"] == "node_activity")
            .collect();
        assert_eq!(attempts.len(), 1, "{events:?}");
        assert_eq!(attempts[0]["status"], "error");
        let observed_error = attempts[0]["error"]
            .as_str()
            .context("observed storage error")?;
        assert!(error.contains(observed_error), "{error} / {observed_error}");
        assert!(!observed_error.contains("annulation"));
        assert!(attempts[0]["output"].is_null());
    }
    Ok(())
}

#[tokio::test]
async fn full_observation_transport_rejects_before_effect_and_counts_retry_on_any() -> Result<()> {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let (sender, mut receiver) = zf_runtime::event_sink::channel(1);
    sender.send(json!({"type":"fixture_busy"})).await?;
    let calls = Arc::new(AtomicUsize::new(0));
    let captured = calls.clone();
    let inner = Arc::new(adk_graph::node::FunctionNode::new("work", move |_| {
        captured.fetch_add(1, Ordering::SeqCst);
        async { Ok(adk_graph::NodeOutput::new()) }
    }));
    let node = zf_runtime::observation::wrap(
        inner,
        sender.clone(),
        "Work".into(),
        "tool".into(),
        json!({}),
        "work".into(),
        None,
    );
    let mut graph = adk_graph::StateGraph::new(zf_runtime::operations::state_schema(&json!([]))?);
    graph.nodes.insert("work".into(), node);
    graph = graph
        .add_edge(adk_graph::START, "work")
        .add_edge("work", adk_graph::END);
    let graph = zf_runtime::operations::configure(
        graph.compile()?,
        &json!({
            "timeoutMs":20,
            "retry":{"maxAttempts":3,"initialDelayMs":5,"maxDelayMs":5,"backoffFactor":1.0,"jitter":0.0,"retryOn":"any"}
        }),
        &[],
    );
    let result = tokio::time::timeout(
        Duration::from_secs(5),
        graph.invoke(
            adk_graph::State::new(),
            adk_graph::ExecutionConfig::new("saturation"),
        ),
    )
    .await?;
    let error = result
        .expect_err("Saturated observation must reject admission")
        .to_string();
    assert!(error.contains("Observation admission saturated"), "{error}");
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    drop(graph);
    drop(sender);
    let mut events = Vec::new();
    while let Some(event) = receiver.recv().await {
        events.push(event);
    }
    assert_eq!(
        events.len(),
        3,
        "Bounded queue, terminal and rejection: {events:?}"
    );
    assert_eq!(events[0]["type"], "fixture_busy");
    assert_eq!(events[1]["status"], "interrupted");
    assert_eq!(events[2]["status"], "error");
    assert_eq!(events[2]["phase"], "admission");
    assert_eq!(events[2]["rejectedAttempts"], 2);
    assert_ne!(events[1]["occurrenceId"], events[2]["occurrenceId"]);
    assert_ne!(
        events[2]["occurrenceId"],
        events[2]["lastRejectedOccurrenceId"]
    );
    assert_eq!(
        events[2]["error"],
        "Observation admission saturated: previous terminal is still pending; invocation rejected before execution"
    );
    Ok(())
}

#[tokio::test]
async fn routing_is_independent_of_idle_and_slow_subscribers() -> Result<()> {
    for clients in [0, 3] {
        let directory = tempfile::tempdir()?;
        let app = zf_serve::server::router_with_home(
            directory.path().into(),
            directory.path().into(),
            vec![],
            {
                let home = directory.path().join("fixture-home");
                std::fs::create_dir_all(&home).unwrap();
                home
            },
        )
        .await?;
        let node = |id: &str, kind: &str, config: Value| json!({"id":id,"position":{"x":0,"y":0},"data":{"label":id,"kind":kind,"config":config}});
        let mut nodes = vec![
            node("s", "start", json!({})),
            node(
                "admit",
                "tool",
                json!({"tool":"delay","arguments":{"milliseconds":40}}),
            ),
        ];
        let mut edges = vec![
            json!({"id":"start","source":"s","target":"admit"}),
            json!({"id":"admit","source":"admit","target":"r0"}),
        ];
        for i in 0..12 {
            let id = format!("r{i}");
            nodes.push(node(
                &id,
                "condition",
                json!({"field":"gate","equals":true}),
            ));
            let next = if i == 11 {
                "e".into()
            } else {
                format!("r{}", i + 1)
            };
            edges.push(
                json!({"id":format!("yes{i}"),"source":id,"sourceHandle":"true","target":next}),
            );
            edges.push(
                json!({"id":format!("no{i}"),"source":id,"sourceHandle":"false","target":"e"}),
            );
        }
        nodes.push(node("e", "end", json!({})));
        let history=json!((0..100).map(|i|json!({"role":"user","parts":[{"text":format!("message {i}: {}","fixture ".repeat(128))}]})).collect::<Vec<_>>());
        let started = Instant::now();
        let run=call(&app,"/api/runs",Some(json!({"composition":{"id":"route-load","name":"Route load","nodes":nodes,"edges":edges},"input":{"gate":true,"messages":history}}))).await?;
        let id = run["id"].as_str().context("run id")?;
        // Keep response bodies open without polling them: network backpressure must
        // not propagate into graph progression or database commits.
        let mut unread = Vec::new();
        for _ in 0..clients {
            unread.push(
                app.clone()
                    .oneshot(Request::get(format!("/api/runs/{id}/events")).body(Body::empty())?)
                    .await?,
            );
        }
        let completed = tokio::time::timeout(Duration::from_secs(15), async {
            loop {
                let runs = call(&app, "/api/runs", None).await?;
                let status = runs
                    .as_array()
                    .context("runs")?
                    .iter()
                    .find(|r| r["id"] == id)
                    .context("summary")?["status"]
                    .clone();
                if status == "completed" {
                    return Ok::<_, anyhow::Error>(());
                }
                anyhow::ensure!(status != "error", "routing fixture failed");
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await?;
        completed?;
        let elapsed = started.elapsed();
        let run = call(&app, &format!("/api/runs/{id}/snapshot"), None).await?["run"].clone();
        let state = call(&app, &format!("/api/runs/{id}/state"), None).await?;
        assert_eq!(state["state"]["messages"], history);
        assert_eq!(
            run["activities"]
                .as_array()
                .context("activities")?
                .iter()
                .filter(|a| a["kind"] == "condition")
                .count(),
            12
        );
        let metrics = call(&app, &format!("/api/runs/{id}/metrics"), None).await?;
        let db = sqlx::SqlitePool::connect(&format!(
            "sqlite://{}",
            directory.path().join("zedflow.db").display()
        ))
        .await?;
        let journal_bytes: i64 =
            sqlx::query_scalar("SELECT COALESCE(SUM(length(document)),0) FROM events WHERE run=?")
                .bind(id)
                .fetch_one(&db)
                .await?;
        let metadata_bytes: i64 =
            sqlx::query_scalar("SELECT length(document) FROM runs WHERE id=?")
                .bind(id)
                .fetch_one(&db)
                .await?;
        assert!(
            journal_bytes < 100_000,
            "journal must reference history rather than copy it: {journal_bytes}"
        );
        assert!(
            metadata_bytes < 10_000,
            "run metadata must stay small: {metadata_bytes}"
        );
        let gaps: Vec<_> = run["activities"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|a| a["gapBeforeMs"].as_u64())
            .collect();
        eprintln!(
            "routing clients={clients} elapsed_ms={} journal_bytes={journal_bytes} metadata_bytes={metadata_bytes} gaps_ms={gaps:?} batches={}",
            elapsed.as_millis(),
            metrics["batches"]
        );
        drop(unread);
    }
    Ok(())
}
