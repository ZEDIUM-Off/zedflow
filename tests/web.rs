#![cfg(feature = "web")]
//! Verify that visualization metadata and executed traces refer to the same ADK graph.

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use serde_json::{Value, json};
use tower::ServiceExt;

async fn request(
    app: &Router,
    path: &str,
    payload: Option<Value>,
) -> anyhow::Result<(StatusCode, Value)> {
    let builder = Request::builder().uri(path);
    let request = if let Some(payload) = payload {
        builder
            .method("POST")
            .header("content-type", "application/json")
            .body(Body::from(payload.to_string()))?
    } else {
        builder.body(Body::empty())?
    };
    let response = app.clone().oneshot(request).await?;
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 2_000_000).await?;
    Ok((status, serde_json::from_slice(&bytes)?))
}

fn setup() -> anyhow::Result<(tempfile::TempDir, Router)> {
    let dir = tempfile::tempdir()?;
    let database = format!("sqlite://{}?mode=rwc", dir.path().join("lab.db").display());
    Ok((dir, zedflow_lab::web::router(database)))
}

#[tokio::test]
async fn displayed_agent_topology_matches_executed_nodes_and_records_states() -> anyhow::Result<()>
{
    let (_dir, app) = setup()?;
    let (status, graph) = request(&app, "/api/flows/agent", None).await?;
    assert_eq!(status, StatusCode::OK);
    let (_, run) = request(
        &app,
        "/api/flows/agent/run",
        Some(json!({"input":{"question":"graph"}})),
    )
    .await?;
    assert_eq!(run["status"], "completed");
    assert_eq!(graph["topology"], run["topology"]);
    let members = graph["topology"]["members"].as_array().expect("members");
    let events = run["events"].as_array().expect("events");
    for event in events.iter().filter(|e| e["type"] == "node_start") {
        assert!(members.iter().any(|m| m["name"] == event["node"]));
    }
    assert_eq!(
        events
            .iter()
            .filter(|e| e["type"] == "node_start" && e["node"] == "decide")
            .count(),
        2
    );
    assert!(events.iter().any(|e| e["type"] == "checkpoint"));
    assert!(
        run["checkpoint"]["state"]["response"]
            .as_str()
            .is_some_and(|s| s.starts_with("Fixture answer:"))
    );
    Ok(())
}

#[tokio::test]
async fn browser_checkpoint_resume_uses_saved_state_and_rejects_duplicate_resume()
-> anyhow::Result<()> {
    let (_dir, app) = setup()?;
    let (_, paused) = request(&app, "/api/flows/checkpoint/run", Some(json!({"input":{}}))).await?;
    assert_eq!(paused["status"], "paused");
    assert_eq!(paused["checkpoint"]["pending_nodes"], json!(["deliver"]));
    let body = json!({"input":{}, "resume":paused["thread"]});
    let (_, resumed) = request(&app, "/api/flows/checkpoint/run", Some(body.clone())).await?;
    assert_eq!(resumed["status"], "completed");
    assert_eq!(resumed["checkpoint"]["state"]["preparations"], 1);
    let (status, _) = request(&app, "/api/flows/checkpoint/run", Some(body)).await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    Ok(())
}

#[tokio::test]
async fn browser_memory_is_shared_between_requests_but_isolated_by_project() -> anyhow::Result<()> {
    let (_dir, app) = setup()?;
    request(
        &app,
        "/api/flows/memory/run",
        Some(json!({"input":{"note":"graph Rust"},"project":"rust"})),
    )
    .await?;
    let (_, same) = request(
        &app,
        "/api/flows/memory/run",
        Some(json!({"input":{},"project":"rust"})),
    )
    .await?;
    let (_, other) = request(
        &app,
        "/api/flows/memory/run",
        Some(json!({"input":{},"project":"ts"})),
    )
    .await?;
    assert_eq!(
        same["checkpoint"]["state"]["matches"],
        json!(["graph Rust"])
    );
    assert_eq!(other["checkpoint"]["state"]["matches"], json!([]));
    Ok(())
}

#[tokio::test]
async fn browser_reports_failure_instead_of_a_completed_run() -> anyhow::Result<()> {
    let (_dir, app) = setup()?;
    let (_, failed) = request(
        &app,
        "/api/flows/research/run",
        Some(json!({"input":{"query":""}})),
    )
    .await?;
    assert_eq!(failed["status"], "error");
    assert!(
        failed["events"]
            .as_array()
            .expect("events")
            .iter()
            .any(|e| e["type"] == "error")
    );
    let (status, _) = request(&app, "/api/flows/missing", None).await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    Ok(())
}
