//! A lab inspector over ADK topology and recorded streams, not a second flow definition.

use std::result::Result;
use std::{net::SocketAddr, sync::Arc};

use adk_core::Agent;
use adk_graph::{GraphAgent, prelude::*};
use adk_memory::InMemoryMemoryService;
use axum::{
    Json, Router,
    extract::{Path, State as AppState},
    http::StatusCode,
    response::Html,
    routing::{get, post},
};
use futures::StreamExt;
use serde::Deserialize;

use crate::flows;

#[derive(Clone)]
struct Lab {
    database: String,
    memory: Arc<InMemoryMemoryService>,
}

type ApiError = (StatusCode, Json<Value>);

fn failure(error: impl std::fmt::Display) -> ApiError {
    (
        StatusCode::BAD_REQUEST,
        Json(json!({"error": error.to_string()})),
    )
}

/// Build the HTTP inspector. The memory service is shared for this server's lifetime.
pub fn router(database: String) -> Router {
    Router::new()
        .route(
            "/",
            get(|| async { Html(include_str!("../web/index.html")) }),
        )
        .route(
            "/app.js",
            get(|| async {
                (
                    [("content-type", "text/javascript; charset=utf-8")],
                    include_str!("../web/app.js"),
                )
            }),
        )
        .route(
            "/style.css",
            get(|| async {
                (
                    [("content-type", "text/css; charset=utf-8")],
                    include_str!("../web/style.css"),
                )
            }),
        )
        .route("/api/flows", get(list))
        .route("/api/flows/{id}", get(inspect))
        .route("/api/flows/{id}/run", post(run))
        .with_state(Lab {
            database,
            memory: Arc::new(InMemoryMemoryService::new()),
        })
}

/// Serve the lab on an explicit interface, with Ctrl-C shutdown.
pub async fn serve(listen: SocketAddr, database: String) -> anyhow::Result<()> {
    // Fail before announcing readiness if persistence cannot be opened.
    SqliteCheckpointer::new(&database).await?;
    let listener = tokio::net::TcpListener::bind(listen).await?;
    println!("Zedflow lab: http://{}", listener.local_addr()?);
    axum::serve(listener, router(database))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}

async fn list() -> Json<Value> {
    Json(json!([
        {"id":"agent", "title":"Boucle agentique", "description":"Décider, explorer un sous-graphe, puis revenir avec des preuves.", "input":{"question":"Comment composer des graphes ADK ?"}},
        {"id":"research", "title":"Recherche", "description":"Valider une requête et préparer des preuves à partir de fixtures locales.", "input":{"query":"Comment composer des graphes ADK ?"}},
        {"id":"memory", "title":"Mémoire partagée", "description":"Écrire et retrouver des notes dans un périmètre de projet.", "input":{"note":"Rust graph workflow runs cargo check before tests.","query":"graph"}},
        {"id":"checkpoint", "title":"Pause et reprise", "description":"Préparer une valeur, suspendre le run et reprendre depuis SQLite.", "input":{}}
    ]))
}

fn source(id: &str) -> Result<(&'static str, &'static str), ApiError> {
    match id {
        "agent" => Ok((
            "flows/agent_loop.rs",
            include_str!("../flows/agent_loop.rs"),
        )),
        "research" => Ok(("flows/research.rs", include_str!("../flows/research.rs"))),
        "memory" => Ok((
            "flows/shared_memory.rs",
            include_str!("../flows/shared_memory.rs"),
        )),
        "checkpoint" => Ok((
            "flows/checkpoint.rs",
            include_str!("../flows/checkpoint.rs"),
        )),
        _ => Err(failure("unknown flow")),
    }
}

fn build(
    id: &str,
    memory: Arc<InMemoryMemoryService>,
    project: &str,
    cp: SqliteCheckpointer,
) -> anyhow::Result<GraphAgent> {
    let graph = match id {
        "agent" => flows::agent_loop::build(Arc::new(flows::agent_loop::FixtureModel))?
            .with_checkpointer(cp),
        "research" => flows::research::build()?.with_checkpointer(cp),
        "memory" => flows::shared_memory::build(memory, project)?.with_checkpointer(cp),
        "checkpoint" => flows::checkpoint::build(cp, true)?,
        _ => anyhow::bail!("unknown flow"),
    };
    Ok(GraphAgent::from_graph(&format!("{id}_root"), graph))
}

async fn inspect(
    AppState(lab): AppState<Lab>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let (file, code) = source(&id)?;
    let cp = SqliteCheckpointer::in_memory().await.map_err(failure)?;
    let agent = build(&id, lab.memory, "rust-workspace", cp).map_err(failure)?;
    Ok(Json(json!({"id":id, "file":file, "source":code,
        "topology":agent.topology(), "channels":agent.graph().state_channels(),
        "limitations":"Topologie ADK : les libellés des conditions et les sorties END ne sont pas exportés. Un sous-graphe est représenté par son nœud d’appel."})))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RunRequest {
    #[serde(default)]
    input: State,
    #[serde(default = "default_project")]
    project: String,
    resume: Option<String>,
}

fn default_project() -> String {
    "rust-workspace".into()
}

async fn run(
    AppState(lab): AppState<Lab>,
    Path(id): Path<String>,
    Json(request): Json<RunRequest>,
) -> Result<Json<Value>, ApiError> {
    source(&id)?;
    let cp = SqliteCheckpointer::new(&lab.database)
        .await
        .map_err(failure)?;
    let mut config;
    let thread;
    if let Some(previous) = request.resume {
        if id != "checkpoint" || !previous.starts_with("checkpoint:") || !request.input.is_empty() {
            return Err(failure("resume expects a checkpoint run and empty input"));
        }
        let saved = cp
            .load(&previous)
            .await
            .map_err(failure)?
            .ok_or_else(|| failure("checkpoint not found"))?;
        if saved.pending_nodes.is_empty() {
            return Err(failure("run already completed"));
        }
        thread = previous;
        config = ExecutionConfig::new(&thread).with_resume_from(&saved.checkpoint_id);
    } else {
        thread = format!("{id}:{}", uuid::Uuid::new_v4());
        config = ExecutionConfig::new(&thread);
    }
    config = config.with_metadata("flow", json!(id));
    let agent = build(&id, lab.memory, &request.project, cp).map_err(failure)?;
    let stream = agent
        .graph()
        .stream(request.input.clone(), config, StreamMode::Debug);
    futures::pin_mut!(stream);
    let mut events = Vec::new();
    let mut last_checkpoint = None;
    let mut status = "incomplete";
    while let Some(event) = stream.next().await {
        // Debug events and value snapshots are separate ADK stream modes. Read the
        // real saved checkpoint between yields instead of executing the graph twice.
        if let Some(cp) = agent.graph().checkpointer()
            && let Some(saved) = cp.load(&thread).await.map_err(failure)?
            && last_checkpoint.as_ref() != Some(&saved.checkpoint_id)
        {
            events.push(json!({"type":"checkpoint", "origin":"adk_checkpointer",
                "state":saved.state, "step":saved.step, "checkpoint_id":saved.checkpoint_id,
                "pending_nodes":saved.pending_nodes}));
            last_checkpoint = Some(saved.checkpoint_id);
        }
        match event {
            Ok(event) => {
                match &event {
                    StreamEvent::Done { .. } => status = "completed",
                    StreamEvent::Interrupted { .. } => status = "paused",
                    StreamEvent::Error { .. } => status = "error",
                    _ => {}
                }
                events.push(serde_json::to_value(event).map_err(failure)?);
            }
            Err(GraphError::Interrupted(_)) => status = "paused",
            Err(error) => {
                status = "error";
                events.push(json!({"type":"error", "message":error.to_string()}));
                break;
            }
        }
    }
    let checkpoint = if let Some(cp) = agent.graph().checkpointer() {
        cp.load(&thread).await.map_err(failure)?
    } else {
        None
    };
    Ok(Json(
        json!({"thread":thread, "status":status, "events":events,
        "input":request.input, "checkpoint":checkpoint, "topology":agent.topology()}),
    ))
}
