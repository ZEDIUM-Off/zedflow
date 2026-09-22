//! HTTP recovery snapshots expose current state while preserving event pagination.
mod support;
use anyhow::{Context, Result};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{HeaderMap, Request, StatusCode},
};
use serde_json::{Value, json};
use sqlx::SqlitePool;
use std::time::Duration;
use tower::ServiceExt;

async fn call(app: &Router, path: &str, body: Option<Value>) -> Result<(HeaderMap, Value)> {
    let mut request = Request::builder().uri(path);
    if body.is_some() {
        request = request
            .method("POST")
            .header("Content-Type", "application/json");
    }
    let response = app
        .clone()
        .oneshot(request.body(Body::from(body.map(|v| v.to_string()).unwrap_or_default()))?)
        .await?;
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = to_bytes(response.into_body(), 2_000_000).await?;
    assert_eq!(
        status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&bytes)
    );
    Ok((headers, serde_json::from_slice(&bytes)?))
}

#[tokio::test]
async fn snapshots_keep_current_revision_separate_from_paginated_event_cursor() -> Result<()> {
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
    let db = SqlitePool::connect(&format!(
        "sqlite://{}",
        directory.path().join("zedflow.db").display()
    ))
    .await?;
    let (_, health) = call(&app, "/api/health", None).await?;
    let document = json!({"id":"history","workspaceId":health["defaultWorkspaceId"],"status":"completed","state":{"processed":205}});
    let mut transaction = db.begin().await?;
    sqlx::query("INSERT INTO runs(id,document) VALUES(?,?)")
        .bind("history")
        .bind(document.to_string())
        .execute(&mut *transaction)
        .await?;
    for index in 1..=205 {
        sqlx::query("INSERT INTO events(run,document) VALUES(?,?)")
            .bind("history")
            .bind(json!({"type":"fixture_step","index":index}).to_string())
            .execute(&mut *transaction)
            .await?;
    }
    // Another run's later event must not advance this run's revision or cursor.
    sqlx::query("INSERT INTO events(run,document) VALUES(?,?)")
        .bind("other-run")
        .bind(json!({"type":"other"}).to_string())
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;

    let (_, bootstrap) = call(&app, "/api/runs/history/snapshot", None).await?;
    assert_eq!(bootstrap["type"], "bootstrap");
    assert_eq!(bootstrap["revision"], 205);
    assert_eq!(bootstrap["run"]["state"], json!({}));
    let (_, idle) = call(&app, "/api/runs/history/snapshot?after=205", None).await?;
    assert_eq!(idle["type"], "heartbeat");
    assert!(idle.get("run").is_none());
    let mut cursor = 0;
    let mut delivered = Vec::new();
    for expected_count in [100, 100, 5, 0] {
        let (_, payload) = call(
            &app,
            &format!("/api/runs/history/event-history?after={cursor}"),
            None,
        )
        .await?;

        let events = payload["events"].as_array().context("event page")?;
        assert_eq!(events.len(), expected_count);
        for event in events {
            let sequence = event["seq"].as_i64().context("event sequence")?;
            assert!(sequence > cursor, "An acknowledged event was replayed");
            assert_eq!(event["event"]["index"], sequence);
            delivered.push(sequence);
        }
        cursor = payload["cursor"].as_i64().context("page cursor")?;
        assert_eq!(cursor, delivered.len() as i64);
    }
    assert_eq!(delivered, (1..=205).collect::<Vec<i64>>());

    let response = app
        .clone()
        .oneshot(Request::get("/api/runs/history/events").body(Body::empty())?)
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()["cache-control"],
        "no-store, no-transform"
    );
    assert_eq!(response.headers()["x-accel-buffering"], "no");
    assert_eq!(response.headers()["content-type"], "text/event-stream");
    // Drop the never-ending SSE body rather than waiting for it to complete.
    drop(response);
    db.close().await;
    Ok(())
}

fn composition() -> Value {
    json!({"id":"snapshot-tool","name":"Instantané en cours","nodes":[
        {"id":"start","position":{"x":0,"y":0},"data":{"kind":"start","label":"Début"}},
        {"id":"tool","position":{"x":200,"y":0},"data":{"kind":"tool","label":"Délai observable","config":{"tool":"delay","arguments":{"milliseconds":500},"field":"output"}}},
        {"id":"end","position":{"x":400,"y":0},"data":{"kind":"end","label":"Fin"}}
    ],"edges":[{"id":"a","source":"start","target":"tool"},{"id":"b","source":"tool","target":"end"}]})
}

#[tokio::test]
async fn http_snapshots_show_running_tool_then_completion_without_an_event_stream() -> Result<()> {
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
    let (_, run) = call(
        &app,
        "/api/runs",
        Some(json!({"composition":composition(),"input":{}})),
    )
    .await?;
    let id = run["id"].as_str().context("run id")?;
    let running = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let (headers, payload) = call(&app, &format!("/api/runs/{id}/snapshot"), None).await?;
            assert_eq!(headers["cache-control"], "no-store");
            assert_eq!(payload["run"]["status"], "running", "{payload}");
            if payload["run"]["activeNodes"] == json!(["tool"]) {
                return Ok::<_, anyhow::Error>(payload);
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await??;
    let activity = running["run"]["activities"]
        .as_array()
        .context("activities")?
        .iter()
        .find(|activity| activity["node"] == "tool")
        .context("running tool")?;
    assert_eq!(activity["status"], "running");
    assert!(activity.get("durationMs").is_none());
    let occurrence = activity["occurrenceId"].clone();
    let initial_revision = running["revision"].as_i64().context("initial revision")?;
    let mut cursor = running["cursor"].as_i64().context("initial cursor")?;
    assert!(initial_revision > 0);
    let (_, latest_run) = call(&app, &format!("/api/runs/{id}"), None).await?;
    assert_eq!(
        latest_run["status"], "running",
        "The snapshot must arrive before real completion"
    );

    let mut projection = running["run"].clone();
    let completed = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let (_, payload) = call(
                &app,
                &format!("/api/runs/{id}/snapshot?after={cursor}"),
                None,
            )
            .await?;
            if payload["type"] == "delta" {
                assert_eq!(payload["baseRevision"], cursor);
                assert!(payload.get("run").is_none());
                for op in payload["ops"].as_array().context("operations")? {
                    let collection = op["collection"].as_str().context("collection")?;
                    if collection == "meta" {
                        for (key, value) in op["value"].as_object().context("metadata")? {
                            projection[key] = value.clone();
                        }
                    } else {
                        let list = projection[collection]
                            .as_array_mut()
                            .context("collection array")?;
                        let key = match collection {
                            "activities" => "occurrenceId",
                            "toolActivities" => "callId",
                            "contextSnapshots" => "invocationId",
                            _ => "id",
                        };
                        if let Some(item) = list.iter_mut().find(|item| item[key] == op["id"]) {
                            *item = op["value"].clone();
                        } else {
                            list.push(op["value"].clone());
                        }
                    }
                }
            }
            cursor = payload["cursor"].as_i64().context("cursor")?;
            if projection["status"] == "completed" {
                return Ok::<_, anyhow::Error>(json!({"run":projection,"revision":cursor}));
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await??;
    let activity = completed["run"]["activities"]
        .as_array()
        .context("completed activities")?
        .iter()
        .find(|activity| activity["occurrenceId"] == occurrence)
        .context("same completed occurrence")?;
    assert_eq!(activity["status"], "completed");
    assert!(activity["durationMs"].as_u64().context("duration")? >= 500);
    assert_eq!(completed["run"]["activeNodes"], json!([]));
    assert!(completed["revision"].as_i64().context("final revision")? > initial_revision);
    Ok(())
}

#[tokio::test]
async fn restart_publishes_a_new_revision_for_an_interrupted_session() -> Result<()> {
    let root = tempfile::tempdir()?;
    let (app, service) = support::open_router(root.path().into(), root.path().into(), vec![], {
        let home = root.path().join("home");
        std::fs::create_dir_all(&home).unwrap();
        home
    })
    .await?;
    let workspace = call(&app, "/api/health", None).await?.1["defaultWorkspaceId"].clone();
    let db = SqlitePool::connect(&format!(
        "sqlite://{}",
        root.path().join("zedflow.db").display()
    ))
    .await?;
    zf_storage::session_store::save(&db,"crashed",&json!({"id":"crashed","workspaceId":workspace,"timelineVersion":1,"status":"running","activities":[{"occurrenceId":"attempt","status":"running"}]})).await?;
    sqlx::query("INSERT INTO events(seq,run,document) VALUES(5,'crashed','{}')")
        .execute(&db)
        .await?;
    service.shutdown().await?;
    drop(app);
    drop(service);
    let (restarted, service) =
        support::open_router(root.path().into(), root.path().into(), vec![], {
            let home = root.path().join("home");
            std::fs::create_dir_all(&home).unwrap();
            home
        })
        .await?;
    let frame = call(&restarted, "/api/runs/crashed/snapshot?after=5", None)
        .await?
        .1;
    assert_eq!(frame["type"], "delta");
    assert_eq!(frame["baseRevision"], 5);
    assert!(frame["revision"].as_i64().unwrap() > 5);
    assert!(
        frame["ops"]
            .as_array()
            .unwrap()
            .iter()
            .any(|op| op["value"]["status"] == "interrupted")
    );
    service.shutdown().await?;
    Ok(())
}
