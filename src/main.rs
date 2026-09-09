//! Small launcher for the ADK experiments; deliberately no workspace resolver.

use std::sync::Arc;

use adk_graph::prelude::*;
use adk_memory::InMemoryMemoryService;
use anyhow::{Context, bail, ensure};
use clap::{Parser, Subcommand};
use futures::StreamExt;
use zedflow_lab::flows;

#[derive(Parser)]
#[command(
    version,
    about = "Zedflow ADK-Rust lab — direct upstream graph experiments"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Inspect compiled graphs and replay fixture runs in a browser.
    #[cfg(feature = "web")]
    Web {
        #[arg(long, default_value = "127.0.0.1:3141")]
        listen: std::net::SocketAddr,
        #[arg(long, default_value = "sqlite://.lab/viewer.db?mode=rwc")]
        database: String,
    },
    /// List experiments and their responsibility.
    List,
    /// Run local fixture research (no web requests).
    Research {
        #[arg(default_value = "How do ADK subgraphs compose?")]
        query: String,
    },
    /// Run an agent → research subgraph → agent cycle.
    Agent {
        #[arg(default_value = "How do ADK subgraphs compose?")]
        question: String,
        /// Use Gemini instead of the offline fixture. Requires GOOGLE_API_KEY.
        #[arg(long)]
        live: bool,
        /// Required with --live; pick a model available to your account.
        #[arg(long, requires = "live", required_if_eq("live", "true"))]
        model: Option<String>,
    },
    /// Write and recall a memory across graphs, then demonstrate project isolation.
    Memory,
    /// Persist a paused graph, or resume its pending node in another process.
    Checkpoint {
        #[arg(value_enum)]
        action: CheckpointAction,
        /// SQLite URL, e.g. sqlite:///tmp/zedflow-checkpoints.db?mode=rwc
        #[arg(long)]
        database: String,
        #[arg(long, default_value = "checkpoint-demo")]
        thread: String,
    },
}

#[derive(Clone, clap::ValueEnum)]
enum CheckpointAction {
    Start,
    Resume,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    match Cli::parse().command {
        #[cfg(feature = "web")]
        Command::Web { listen, database } => {
            std::fs::create_dir_all(".lab").context("create local lab directory")?;
            zedflow_lab::web::serve(listen, database).await?;
        }
        Command::List => {
            println!(
                "research   validate → retrieve fixtures → prepare evidence\nagent      decide → research subgraph → decide (fixture or Gemini)\nmemory     remember → recall, shared service and project isolation\ncheckpoint prepare → pause → resume deliver, SQLite persistence"
            );
        }
        Command::Research { query } => {
            let state = State::from([("query".into(), json!(query))]);
            trace(&flows::research::build()?, state, "research-demo").await?;
        }
        Command::Agent {
            question,
            live,
            model,
        } => {
            let model: Arc<dyn adk_core::Llm> = if live {
                let key =
                    std::env::var("GOOGLE_API_KEY").context("--live requires GOOGLE_API_KEY")?;
                let name = model.context("--live requires --model")?;
                Arc::new(adk_model::gemini::GeminiModel::new(key, name)?)
            } else {
                Arc::new(flows::agent_loop::FixtureModel)
            };
            let state = State::from([("question".into(), json!(question))]);
            trace(&flows::agent_loop::build(model)?, state, "agent-demo").await?;
        }
        Command::Memory => {
            let memory = Arc::new(InMemoryMemoryService::new());
            let rust = flows::shared_memory::build(Arc::clone(&memory), "rust-workspace")?;
            let input = State::from([
                (
                    "note".into(),
                    json!("Rust graph workflow runs cargo check before tests."),
                ),
                ("query".into(), json!("graph")),
            ]);
            trace(&rust, input, "memory-write").await?;
            let rust_reader = flows::shared_memory::build(Arc::clone(&memory), "rust-workspace")?;
            trace(&rust_reader, State::new(), "memory-same-project").await?;
            let typescript = flows::shared_memory::build(memory, "typescript-workspace")?;
            trace(&typescript, State::new(), "memory-other-project").await?;
        }
        Command::Checkpoint {
            action,
            database,
            thread,
        } => {
            let checkpointer = SqliteCheckpointer::new(&database)
                .await
                .context("open checkpoint database")?;
            let previous = checkpointer.load(&thread).await?;
            let config = match action {
                CheckpointAction::Start => {
                    ensure!(
                        previous.is_none(),
                        "thread already exists; resume it or choose another --thread"
                    );
                    ExecutionConfig::new(&thread)
                }
                CheckpointAction::Resume => {
                    let checkpoint = previous.context("no checkpoint for this thread")?;
                    ensure!(
                        !checkpoint.pending_nodes.is_empty(),
                        "thread already completed"
                    );
                    ExecutionConfig::new(&thread).with_resume_from(&checkpoint.checkpoint_id)
                }
            };
            let graph = flows::checkpoint::build(checkpointer, true)?;
            match graph.invoke(State::new(), config).await {
                Ok(state) => println!(
                    "{}",
                    json!({"status": "completed", "thread": thread, "state": state})
                ),
                Err(GraphError::Interrupted(_)) => {
                    let checkpoint = graph
                        .checkpointer()
                        .context("missing checkpointer")?
                        .load(&thread)
                        .await?
                        .context("pause did not save a checkpoint")?;
                    println!("{}", json!({"status": "paused", "checkpoint": checkpoint}));
                }
                Err(error) => return Err(error.into()),
            }
        }
    }
    Ok(())
}

async fn trace(graph: &CompiledGraph, input: State, thread: &str) -> anyhow::Result<()> {
    let stream = graph.stream(input, ExecutionConfig::new(thread), StreamMode::Debug);
    futures::pin_mut!(stream);
    let mut done = false;
    while let Some(event) = stream.next().await {
        let event = event?;
        println!("{}", json!({"thread": thread, "event": event}));
        match event {
            StreamEvent::Done { .. } => done = true,
            StreamEvent::Error { message, .. } => bail!("{message}"),
            _ => {}
        }
    }
    ensure!(done, "graph stream ended without a completed result");
    Ok(())
}
