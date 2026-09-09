//! Behavioral experiments: composition, boundaries, failure, and restart.

use std::sync::Arc;

use adk_graph::prelude::*;
use adk_memory::InMemoryMemoryService;
use zedflow_lab::flows;

#[tokio::test]
async fn research_rejects_empty_queries() -> anyhow::Result<()> {
    let result = flows::research::build()?
        .invoke(
            State::from([("query".into(), json!("  "))]),
            ExecutionConfig::new("empty"),
        )
        .await;
    assert!(matches!(
        result,
        Err(GraphError::NodeExecutionFailed { .. })
    ));
    Ok(())
}

#[tokio::test]
async fn agent_returns_from_research_without_leaking_private_channels() -> anyhow::Result<()> {
    let graph = flows::agent_loop::build(Arc::new(flows::agent_loop::FixtureModel))?;
    let result = graph
        .invoke(
            State::from([("question".into(), json!("graph composition"))]),
            ExecutionConfig::new("agent-test"),
        )
        .await?;
    assert_eq!(result["evidence"]["kind"], "local_fixture");
    assert!(
        result["response"]
            .as_str()
            .is_some_and(|r| r.starts_with("Fixture answer:"))
    );
    assert!(!result.contains_key("query"));
    assert!(!result.contains_key("sources"));
    Ok(())
}

#[tokio::test]
async fn shared_store_survives_a_graph_but_respects_project_scope() -> anyhow::Result<()> {
    let store = Arc::new(InMemoryMemoryService::new());
    let writer = flows::shared_memory::build(Arc::clone(&store), "rust")?;
    writer
        .invoke(
            State::from([("note".into(), json!("graph Rust testing"))]),
            ExecutionConfig::new("write"),
        )
        .await?;
    drop(writer);
    let same = flows::shared_memory::build(Arc::clone(&store), "rust")?
        .invoke(State::new(), ExecutionConfig::new("same"))
        .await?;
    let other = flows::shared_memory::build(store, "typescript")?
        .invoke(State::new(), ExecutionConfig::new("other"))
        .await?;
    assert_eq!(same["matches"], json!(["graph Rust testing"]));
    assert_eq!(other["matches"], json!([]));
    assert!(
        !same.contains_key("note"),
        "memory and per-run state are separate"
    );
    Ok(())
}

#[tokio::test]
async fn sqlite_resume_keeps_state_and_does_not_repeat_preparation() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let url = format!(
        "sqlite://{}?mode=rwc",
        directory.path().join("checkpoints.db").display()
    );
    let graph = flows::checkpoint::build(SqliteCheckpointer::new(&url).await?, true)?;
    let paused = graph
        .invoke(State::new(), ExecutionConfig::new("restart"))
        .await;
    assert!(matches!(paused, Err(GraphError::Interrupted(_))));
    drop(graph);

    let checkpointer = SqliteCheckpointer::new(&url).await?;
    let checkpoint = checkpointer.load("restart").await?.expect("saved pause");
    assert_eq!(checkpoint.pending_nodes, ["deliver"]);
    let resumed = flows::checkpoint::build(checkpointer, true)?
        .invoke(
            State::new(),
            ExecutionConfig::new("restart").with_resume_from(&checkpoint.checkpoint_id),
        )
        .await?;
    assert_eq!(resumed["preparations"], 1);
    assert_eq!(resumed["delivered"], "fixture artifact");
    Ok(())
}
