use adk_graph::{ExecutionConfig, NodeContext, NodeOutput, State, node::FunctionNode};
use serde_json::{Value, json};
use std::{collections::HashSet, sync::Arc, time::Instant};
use zf_flows::schema::Composition;
use zf_runtime::observation;
use zf_runtime::runtime::RunServices;
use zf_runtime::stored_checkpointer::StoredCheckpointer;
use zf_runtime::workspace_context::ContextSnapshot;
use zf_storage::content_store::ContentStore;

async fn store() -> ContentStore {
    ContentStore::new(
        sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap(),
    )
    .await
    .unwrap()
}
fn node(id: &str, kind: &str, config: Value) -> Value {
    json!({"id":id,"position":{"x":0,"y":0},"data":{"label":id,"kind":kind,"config":config}})
}
fn routes(version: u32) -> Composition {
    let mut nodes = vec![node("s", "start", json!({}))];
    let mut edges = vec![json!({"id":"entry","source":"s","target":"r0"})];
    for index in 0..6 {
        let id = format!("r{index}");
        let config = if version == 1 {
            json!({"field":"gate","equals":true})
        } else {
            json!({"predicate":{"kind":"compare","field":"gate","operator":"eq","value":true}})
        };
        nodes.push(node(&id, "condition", config));
        let next = if index == 5 {
            "out".into()
        } else {
            format!("r{}", index + 1)
        };
        edges.push(
            json!({"id":format!("yes-{index}"),"source":id,"sourceHandle":"true","target":next}),
        );
        edges.push(
            json!({"id":format!("no-{index}"),"source":id,"sourceHandle":"false","target":"e"}),
        );
    }
    nodes.push(node("out", "output", json!({"text":"done"})));
    nodes.push(node("e", "end", json!({})));
    edges.push(json!({"id":"finish","source":"out","target":"e"}));
    serde_json::from_value(json!({"formatVersion":version,"id":"storage-regression","name":"Storage regression","nodes":nodes,"edges":edges})).unwrap()
}
#[tokio::test]
async fn ten_hundred_thousand_messages_share_history_across_real_graph_routes() {
    for version in [1, 2] {
        let mut sizes = Vec::new();
        for count in [10, 100, 1000] {
            let root = tempfile::tempdir().unwrap();
            let store = store().await;
            let services = RunServices::new(
                "regression".into(),
                root.path().into(),
                root.path().join("data"),
                ContextSnapshot::default(),
                json!({}),
                vec![],
            )
            .unwrap();
            services.set_content_store(store.clone());
            let (sender, mut events) = zf_runtime::event_sink::channel(64);
            let cp = Arc::new(
                StoredCheckpointer::new(
                    zf_storage::contracts::CheckpointStore::new(store.clone())
                        .await
                        .unwrap(),
                )
                .with_sender(sender.clone()),
            );
            let graph = zf_runtime::materialize::build_with_services(
                &routes(version),
                services,
                Some(sender.clone()),
                Some(cp.clone()),
            )
            .unwrap();
            let history=json!((0..count).map(|index|json!({"role":"user","parts":[{"text":format!("message-{index}: {}","payload ".repeat(64))}]})).collect::<Vec<_>>());
            let initial = State::from([
                ("messages".into(), history.clone()),
                ("gate".into(), json!(true)),
                ("input".into(), json!("work")),
            ]);
            let started = Instant::now();
            let final_state = graph
                .invoke(initial, ExecutionConfig::new("regression"))
                .await
                .unwrap();
            let elapsed = started.elapsed();
            assert_eq!(final_state["messages"], history);
            let mut input_refs = HashSet::new();
            let mut route_count = 0;
            let mut max_event = 0;
            while let Ok(event) = events.try_recv() {
                max_event = max_event.max(serde_json::to_vec(&event).unwrap().len());
                if event["type"] == "node_activity"
                    && event["kind"] == "condition"
                    && event["status"] == "running"
                {
                    route_count += 1;
                    assert!(event.get("input").is_none());
                    let id = event["inputRef"].as_str().unwrap();
                    assert_eq!(store.resolve(id).await.unwrap()["messages"], history);
                    input_refs.insert(id.to_owned());
                }
            }
            assert_eq!(route_count, 6);
            if version == 1 {
                assert_eq!(
                    input_refs.len(),
                    1,
                    "identical actual state shares one reference across steps"
                );
            }
            assert!(
                max_event < 4096,
                "observational event unexpectedly materializes history: {max_event}"
            );
            let bytes: i64 =
                sqlx::query_scalar("SELECT SUM(length(CAST(body AS BLOB))) FROM zf_content")
                    .fetch_one(store.pool())
                    .await
                    .unwrap();
            let blobs: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM zf_content")
                .fetch_one(store.pool())
                .await
                .unwrap();
            let checkpoints = cp
                .storage()
                .list_run_headers("regression")
                .await
                .unwrap()
                .len();
            let pages: i64 = sqlx::query_scalar("PRAGMA page_count")
                .fetch_one(store.pool())
                .await
                .unwrap();
            let page_size: i64 = sqlx::query_scalar("PRAGMA page_size")
                .fetch_one(store.pool())
                .await
                .unwrap();
            println!("sqlite allocated_bytes={}", pages * page_size);
            println!(
                "storage version={version} messages={count} bytes={bytes} blobs={blobs} checkpoints={checkpoints} max_event={max_event} graph_ms={:.2}",
                elapsed.as_secs_f64() * 1000.
            );
            sizes.push(bytes);
        }
        assert!(
            sizes[1] < sizes[0] * 12,
            "10→100 messages must not approach quadratic persistence"
        );
        assert!(
            sizes[2] < sizes[1] * 12,
            "100→1000 messages must not approach quadratic persistence"
        );
        assert!(
            sizes[2] < 2_000_000,
            "one 512KB history must not repeat for every route/checkpoint"
        );
    }
}
#[tokio::test]
async fn occurrence_input_is_exact_even_when_thread_and_step_repeat() {
    let store = store().await;
    let (sender, mut events) = zf_runtime::event_sink::channel(64);
    let inner = Arc::new(FunctionNode::new("probe", |ctx| async move {
        Ok(NodeOutput::new().with_update("seen", ctx.state["input"].clone()))
    }));
    let observed = observation::wrap(
        inner,
        sender,
        "Probe".into(),
        "condition".into(),
        json!({}),
        "outer/child/probe".into(),
        Some(store.clone()),
    );
    let first = NodeContext::new(
        State::from([("input".into(), json!("before wait"))]),
        ExecutionConfig::new("run/child@3"),
        7,
    );
    let resumed = NodeContext::new(
        State::from([("input".into(), json!("fresh answer"))]),
        ExecutionConfig::new("run/child@3"),
        7,
    );
    observed.execute(&first).await.unwrap();
    observed.execute(&resumed).await.unwrap();
    let mut inputs = Vec::new();
    while let Ok(event) = events.try_recv() {
        if event["status"] == "running" {
            inputs.push((
                event["occurrenceId"].clone(),
                event["inputRef"].as_str().unwrap().to_owned(),
            ));
        }
    }
    assert_ne!(inputs[0].0, inputs[1].0);
    assert_ne!(inputs[0].1, inputs[1].1);
    assert_eq!(
        store.resolve(&inputs[0].1).await.unwrap(),
        json!(first.state)
    );
    assert_eq!(
        store.resolve(&inputs[1].1).await.unwrap(),
        json!(resumed.state)
    );
}
#[tokio::test]
async fn growing_tool_previews_share_utf8_chunks_instead_of_every_full_prefix() {
    let store = store().await;
    let mut text = String::new();
    let mut sizes = Vec::new();
    let started = Instant::now();
    for index in 1..=1000 {
        text.push_str(&format!("{index:04}: {}\n", "é漢🙂".repeat(24)));
        let id = store.intern(&json!(text)).await.unwrap();
        if [10, 100, 1000].contains(&index) {
            assert_eq!(store.resolve(&id).await.unwrap(), json!(text));
            let bytes: i64 =
                sqlx::query_scalar("SELECT SUM(length(CAST(body AS BLOB))) FROM zf_content")
                    .fetch_one(store.pool())
                    .await
                    .unwrap();
            println!(
                "tool previews updates={index} source_bytes={} stored_bytes={bytes}",
                text.len()
            );
            sizes.push(bytes);
        }
    }
    assert!(
        sizes[2] < sizes[1] * 15,
        "growing previews must not persist quadratic prefixes"
    );
    assert!(
        sizes[2] < 5_000_000,
        "only new chunks and bounded partial suffixes should be stored"
    );
    println!(
        "tool previews 1000 updates encode_and_commit_ms={:.2}",
        started.elapsed().as_secs_f64() * 1000.
    );
}
