use adk_graph::{
    ExecutionConfig, Node, NodeOutput, State, StateGraph, checkpoint::Checkpointer,
    node::FunctionNode,
};
use serde_json::{Value, json};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::sync::Semaphore;
use zf_flows::schema::Composition;
use zf_runtime::revisions::Compatibility;
use zf_runtime::revisions::NodeFactory;
use zf_runtime::revisions::RevisionDefinition;
use zf_runtime::revisions::RevisionRuntime;
use zf_runtime::revisions::compatibility;
use zf_runtime::stored_checkpointer::StoredCheckpointer;
use zf_storage::content_store::ContentStore;
fn node(id: &str) -> Value {
    let kind = if id == "s" {
        "start"
    } else if id == "e" {
        "end"
    } else {
        "set"
    };
    let config = if ["s", "e"].contains(&id) {
        json!({})
    } else {
        json!({"field":id,"value":"old"})
    };
    json!({"id":id,"position":{"x":0,"y":0},"data":{"kind":kind,"label":id,"config":config}})
}
fn doc(parallel: bool) -> Composition {
    let ids = if parallel {
        vec!["s", "a", "b", "middle", "join", "e"]
    } else {
        vec!["s", "gate", "after", "e"]
    };
    let edges = if parallel {
        vec![
            ("s", "a"),
            ("s", "b"),
            ("a", "join"),
            ("b", "middle"),
            ("middle", "join"),
            ("join", "e"),
        ]
    } else {
        vec![("s", "gate"), ("gate", "after"), ("after", "e")]
    };
    serde_json::from_value(json!({"formatVersion":3,"id":"versioned-flow","name":"Versioned","nodes":ids.into_iter().map(node).collect::<Vec<_>>(),"edges":edges.into_iter().map(|(a,b)|json!({"id":format!("{a}-{b}"),"source":a,"target":b})).collect::<Vec<_>>()})).unwrap()
}
fn definition(doc: Composition) -> RevisionDefinition {
    let source = zf_flows::flow_source::render(
        &doc,
        &zf_compiler::graph_compiler::GraphValidator::new(
            &zf_runtime::materialize::RuntimePrimitives,
        ),
    )
    .unwrap();
    RevisionDefinition {
        package: None,
        context_selections: Default::default(),
        key: "fixture-flow".into(),
        hash: zf_storage::flow_store::hash(source.as_bytes()),
        source,
        composition: doc,
    }
}
fn updated(doc: &Composition) -> Composition {
    let mut next = doc.clone();
    for n in &mut next.nodes {
        if n.data.kind == "set" {
            n.data.config["value"] = json!("new");
        }
    }
    next
}
async fn fixture() -> (tempfile::TempDir, ContentStore, Arc<StoredCheckpointer>) {
    let root = tempfile::tempdir().unwrap();
    let pool = SqlitePoolOptions::new()
        .connect_with(
            SqliteConnectOptions::new()
                .filename(root.path().join("runtime.sqlite"))
                .create_if_missing(true),
        )
        .await
        .unwrap();
    let store = ContentStore::new(pool).await.unwrap();
    let cp = Arc::new(StoredCheckpointer::new(
        zf_storage::contracts::CheckpointStore::new(store.clone())
            .await
            .unwrap(),
    ));
    (root, store, cp)
}
fn graph(
    doc: &Composition,
    revisions: Arc<RevisionRuntime>,
    factory: NodeFactory,
    cp: Arc<StoredCheckpointer>,
) -> adk_graph::CompiledGraph {
    let mut graph = StateGraph::new(zf_runtime::operations::state_schema(&json!([])).unwrap());
    for spec in doc
        .nodes
        .iter()
        .filter(|n| !["start", "end"].contains(&n.data.kind.as_str()))
    {
        let initial = factory(doc, spec).unwrap();
        graph.nodes.insert(
            spec.id.clone(),
            revisions.wrap("root", &spec.id, initial, factory.clone()),
        );
    }
    for edge in &doc.edges {
        graph = graph.add_edge(
            if edge.source == "s" {
                adk_graph::START
            } else {
                &edge.source
            },
            if edge.target == "e" {
                adk_graph::END
            } else {
                &edge.target
            },
        );
    }
    graph
        .compile()
        .unwrap()
        .with_checkpointer_arc(cp)
        .with_max_concurrency(1)
}

#[tokio::test]
async fn publication_keeps_parallel_step_and_inflight_call_pinned_without_losing_deferred_join() {
    let (_temp, store, cp) = fixture().await;
    let old = doc(true);
    let next = updated(&old);
    let old_def = definition(old.clone());
    let new_def = definition(next);
    let revisions = RevisionRuntime::new(
        store.clone(),
        "parallel",
        BTreeMap::from([("root".into(), old_def.clone())]),
    )
    .await
    .unwrap();
    let entered = Arc::new(Semaphore::new(0));
    let release = Arc::new(Semaphore::new(0));
    let once = Arc::new(AtomicBool::new(false));
    let observed = Arc::new(Mutex::new(Vec::new()));
    let factory: NodeFactory = {
        let entered = entered.clone();
        let release = release.clone();
        let once = once.clone();
        let observed = observed.clone();
        Arc::new(move |_, spec| {
            let id = spec.id.clone();
            let value = spec.data.config["value"].clone();
            let entered = entered.clone();
            let release = release.clone();
            let once = once.clone();
            let observed = observed.clone();
            Ok(Arc::new(FunctionNode::new(&spec.id, move |ctx| {
                let id = id.clone();
                let value = value.clone();
                let entered = entered.clone();
                let release = release.clone();
                let once = once.clone();
                let observed = observed.clone();
                async move {
                    if !once.swap(true, Ordering::SeqCst) {
                        entered.add_permits(1);
                        release.acquire().await.unwrap().forget();
                    }
                    let revision = zf_runtime::revisions::current_revision().unwrap();
                    observed
                        .lock()
                        .unwrap()
                        .push((id.clone(), ctx.step, revision["hash"].clone()));
                    Ok(NodeOutput::new().with_update(&id, value))
                }
            })) as Arc<dyn Node>)
        })
    };
    let graph = graph(&old, revisions.clone(), factory, cp.clone());
    let task = tokio::spawn(async move {
        graph
            .invoke(State::new(), ExecutionConfig::new("parallel"))
            .await
    });
    entered.acquire().await.unwrap().forget();
    assert_eq!(
        revisions.publish("root", new_def.clone()).await.unwrap(),
        Compatibility::Live
    );
    release.add_permits(1);
    let result = tokio::time::timeout(std::time::Duration::from_secs(5), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(result["a"], "old");
    assert_eq!(result["b"], "old");
    assert_eq!(result["middle"], "new");
    assert_eq!(result["join"], "new");
    {
        let observed = observed.lock().unwrap();
        assert_eq!(observed.iter().filter(|(id, _, _)| id == "join").count(), 1);
        for (id, step, hash) in observed.iter() {
            assert_eq!(
                hash,
                &json!(if *step == 0 {
                    &old_def.hash
                } else {
                    &new_def.hash
                }),
                "{id}"
            );
        }
    }
    assert!(
        cp.load("parallel")
            .await
            .unwrap()
            .unwrap()
            .pending_nodes
            .is_empty()
    );
}

#[tokio::test]
async fn interrupted_step_keeps_its_revision_after_restart_then_next_step_adopts() {
    let (_temp, store, cp) = fixture().await;
    let old = doc(false);
    let old_def = definition(old.clone());
    let revisions = RevisionRuntime::new(
        store.clone(),
        "resume",
        BTreeMap::from([("root".into(), old_def.clone())]),
    )
    .await
    .unwrap();
    let factory: NodeFactory = Arc::new(|_, spec| {
        let id = spec.id.clone();
        let value = spec.data.config["value"].clone();
        Ok(Arc::new(FunctionNode::new(&spec.id, move |ctx| {
            let id = id.clone();
            let value = value.clone();
            async move {
                if id == "gate" && ctx.state.get("answer") != Some(&json!(true)) {
                    return Ok(NodeOutput::new().with_interrupt(
                        adk_graph::interrupt::interrupt_with_data(
                            "wait",
                            json!({"kind":"fixture"}),
                        ),
                    ));
                }
                Ok(NodeOutput::new().with_update(&id, value))
            }
        })) as Arc<dyn Node>)
    });
    assert!(
        graph(&old, revisions.clone(), factory.clone(), cp.clone())
            .invoke(State::new(), ExecutionConfig::new("resume"))
            .await
            .is_err()
    );
    revisions
        .publish("root", definition(updated(&old)))
        .await
        .unwrap();
    let restarted =
        RevisionRuntime::new(store, "resume", BTreeMap::from([("root".into(), old_def)]))
            .await
            .unwrap();
    let result = graph(&old, restarted, factory, cp)
        .invoke(
            State::from([("answer".into(), json!(true))]),
            ExecutionConfig::new("resume"),
        )
        .await
        .unwrap();
    assert_eq!(result["gate"], "old");
    assert_eq!(result["after"], "new");
}

#[tokio::test]
async fn pending_tool_calls_keep_active_contract_until_the_following_step() {
    let (_temp, store, cp) = fixture().await;
    let old = doc(false);
    let revisions = RevisionRuntime::new(
        store.clone(),
        "calls",
        BTreeMap::from([("root".into(), definition(old.clone()))]),
    )
    .await
    .unwrap();
    revisions
        .publish("root", definition(updated(&old)))
        .await
        .unwrap();
    let factory: NodeFactory = Arc::new(|_, spec| {
        let id = spec.id.clone();
        let value = spec.data.config["value"].clone();
        Ok(Arc::new(FunctionNode::new(&spec.id, move |_| {
            let id = id.clone();
            let value = value.clone();
            async move {
                Ok(NodeOutput::new()
                    .with_update(&id, value)
                    .with_update("toolCalls", json!([])))
            }
        })) as Arc<dyn Node>)
    });
    let result = graph(&old, revisions, factory, cp)
        .invoke(
            State::from([("toolCalls".into(), json!([{"id":"pending"}]))]),
            ExecutionConfig::new("calls"),
        )
        .await
        .unwrap();
    assert_eq!(result["gate"], "old");
    assert_eq!(result["after"], "new");
    let pins = store.records("calls").await.unwrap();
    let mut found = false;
    for p in pins.into_iter().filter(|p| p.kind == "revision-steps") {
        let value = store.resolve(&p.value_ref).await.unwrap();
        if value["step"] == 0 {
            assert_eq!(value["diagnostic"]["code"], "pending_tool_calls");
            found = true;
        }
    }
    assert!(found);
}

#[test]
fn structural_changes_require_proven_sequential_frontier_and_stable_channels() {
    let old = doc(false);
    let mut reordered = old.clone();
    reordered.edges[0].target = "after".into();
    reordered.edges[1].source = "after".into();
    reordered.edges[1].target = "gate".into();
    reordered.edges[2].source = "gate".into();
    assert!(matches!(
        compatibility(&old, &reordered).unwrap(),
        Compatibility::SequentialBoundary { .. }
    ));
    let before = doc(true);
    let mut parallel = before.clone();
    parallel
        .edges
        .iter_mut()
        .find(|e| e.source == "a")
        .unwrap()
        .target = "middle".into();
    assert!(matches!(
        compatibility(&before, &parallel).unwrap(),
        Compatibility::Incompatible { .. }
    ));
    let mut schema = old.clone();
    schema.settings.strict_channels = true;
    assert!(matches!(
        compatibility(&old, &schema).unwrap(),
        Compatibility::Incompatible { .. }
    ));
}

#[tokio::test]
async fn publication_batch_rolls_back_all_heads_and_contents_on_commit_failure() {
    use zf_runtime::revisions::RevisionPublication;
    use zf_runtime::revisions::publish_batch;
    let (_temp, store, _cp) = fixture().await;
    let before = doc(false);
    let old = definition(before.clone());
    for run in ["a", "b"] {
        RevisionRuntime::new(
            store.clone(),
            run,
            BTreeMap::from([("".into(), old.clone())]),
        )
        .await
        .unwrap();
    }
    let initial_a = store
        .records("a")
        .await
        .unwrap()
        .into_iter()
        .find(|r| r.kind == "revision-heads")
        .unwrap()
        .value_ref;
    let initial_b = store
        .records("b")
        .await
        .unwrap()
        .into_iter()
        .find(|r| r.kind == "revision-heads")
        .unwrap()
        .value_ref;
    let before_count: i64 = sqlx::query_scalar("SELECT count(*) FROM zf_content")
        .fetch_one(store.pool())
        .await
        .unwrap();
    sqlx::query("CREATE TRIGGER reject_test_publication BEFORE UPDATE ON zf_records WHEN NEW.scope='b' AND NEW.kind='revision-heads' BEGIN SELECT RAISE(ABORT,'fixture failure'); END").execute(store.pool()).await.unwrap();
    let publications: Vec<_> = ["a", "b"]
        .into_iter()
        .map(|run| RevisionPublication {
            run_id: run.into(),
            instance: "".into(),
            baseline: before.clone(),
            definition: definition(updated(&before)),
        })
        .collect();
    assert!(publish_batch(&store, &publications).await.is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM zf_content")
            .fetch_one(store.pool())
            .await
            .unwrap(),
        before_count
    );
    for (run, expected) in [("a", initial_a), ("b", initial_b)] {
        assert_eq!(
            store
                .records(run)
                .await
                .unwrap()
                .into_iter()
                .find(|r| r.kind == "revision-heads")
                .unwrap()
                .value_ref,
            expected
        );
    }
    sqlx::query("DROP TRIGGER reject_test_publication")
        .execute(store.pool())
        .await
        .unwrap();
    assert_eq!(
        publish_batch(&store, &publications).await.unwrap(),
        vec![Compatibility::Live, Compatibility::Live]
    );
}

#[tokio::test]
async fn unique_publication_replay_does_not_restore_older_heads_and_rejects_identity_reuse() {
    use zf_runtime::revisions::RevisionPublication;
    use zf_runtime::revisions::publish_batch;
    use zf_runtime::revisions::publish_batch_unique;
    let (_temp, store, _cp) = fixture().await;
    let before = doc(false);
    let old = definition(before.clone());
    RevisionRuntime::new(store.clone(), "unique", BTreeMap::from([("".into(), old)]))
        .await
        .unwrap();
    let publication = RevisionPublication {
        run_id: "unique".into(),
        instance: "".into(),
        baseline: before.clone(),
        definition: definition(updated(&before)),
    };
    let batch_id = uuid::Uuid::new_v4().to_string();
    publish_batch_unique(&store, &batch_id, std::slice::from_ref(&publication))
        .await
        .unwrap();
    let mut later = publication.clone();
    later
        .definition
        .composition
        .nodes
        .iter_mut()
        .find(|n| n.id == "gate")
        .unwrap()
        .data
        .config["value"] = json!("third");
    later.definition = definition(later.definition.composition);
    publish_batch(&store, std::slice::from_ref(&later))
        .await
        .unwrap();
    let advanced = store
        .records("unique")
        .await
        .unwrap()
        .into_iter()
        .find(|r| r.kind == "revision-heads")
        .unwrap()
        .value_ref;
    publish_batch_unique(&store, &batch_id, std::slice::from_ref(&publication))
        .await
        .unwrap();
    assert_eq!(
        store
            .records("unique")
            .await
            .unwrap()
            .into_iter()
            .find(|r| r.kind == "revision-heads")
            .unwrap()
            .value_ref,
        advanced
    );
    assert!(
        publish_batch_unique(&store, &batch_id, &[later])
            .await
            .is_err()
    );
}

#[tokio::test]
async fn actual_compiler_supports_historical_empty_root_and_nested_config_adoption() {
    use zf_runtime::runtime::RunServices;
    use zf_runtime::workspace_context::ContextSnapshot;
    let (temp, store, cp) = fixture().await;
    let child:Composition=serde_json::from_value(json!({"formatVersion":3,"id":"child","name":"Child","nodes":[node("s"),{"id":"inner","position":{"x":0,"y":0},"data":{"kind":"set","label":"Inner","config":{"field":"response","value":"old"}}},node("e")],"edges":[{"id":"s-inner","source":"s","target":"inner"},{"id":"inner-e","source":"inner","target":"e"}]})).unwrap();
    let mut old = doc(false);
    let gate = old.nodes.iter_mut().find(|n| n.id == "gate").unwrap();
    gate.data.kind = "subgraph".into();
    gate.data.config = json!({"composition":child});
    let mut next = old.clone();
    next.nodes
        .iter_mut()
        .find(|n| n.id == "gate")
        .unwrap()
        .data
        .config["composition"]["nodes"][1]["data"]["config"]["value"] = json!("new");
    assert_eq!(compatibility(&old, &next).unwrap(), Compatibility::Live);
    let services = RunServices::new(
        "legacy-scope".into(),
        temp.path().into(),
        temp.path().join("data"),
        ContextSnapshot {
            cwd: temp.path().into(),
            ..Default::default()
        },
        json!({}),
        vec![],
    )
    .unwrap();
    services.set_content_store(store.clone());
    let revisions = RevisionRuntime::new(
        store.clone(),
        &services.id,
        BTreeMap::from([("".into(), definition(old.clone()))]),
    )
    .await
    .unwrap();
    services.set_revisions(revisions.clone()).unwrap();
    let compiled =
        zf_runtime::materialize::build_with_services(&old, services, None, Some(cp)).unwrap();
    revisions
        .publish("", definition(next.clone()))
        .await
        .unwrap();
    compiled
        .invoke(State::new(), ExecutionConfig::new("legacy-scope"))
        .await
        .unwrap();
    let mut nested = false;
    for record in store
        .records("legacy-scope")
        .await
        .unwrap()
        .into_iter()
        .filter(|r| r.kind == "revision-steps")
    {
        let pin = store.resolve(&record.value_ref).await.unwrap();
        if pin["scope"] == "gate" {
            assert_eq!(pin["hash"], definition(next.clone()).hash);
            nested = true;
        }
    }
    assert!(nested, "nested scopes retain their historical paths");
}

#[tokio::test]
async fn sequential_boundary_has_no_effect_and_stale_marker_does_not_mask_real_wait() {
    use zf_runtime::revisions::load_boundary;
    let (_temp, store, cp) = fixture().await;
    let old = doc(false);
    let mut next = old.clone();
    next.edges[0].target = "after".into();
    next.edges[1].source = "after".into();
    next.edges[1].target = "gate".into();
    next.edges[2].source = "gate".into();
    let revisions = RevisionRuntime::new(
        store.clone(),
        "boundary",
        BTreeMap::from([("root".into(), definition(old.clone()))]),
    )
    .await
    .unwrap();
    revisions
        .publish("root", definition(next.clone()))
        .await
        .unwrap();
    let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let factory: NodeFactory =
        {
            let count = count.clone();
            Arc::new(move |_, spec| {
                let count = count.clone();
                Ok(Arc::new(FunctionNode::new(&spec.id, move |_| {
                    count.fetch_add(1, Ordering::SeqCst);
                    async move {
                        Ok(NodeOutput::new().with_interrupt(
                            adk_graph::interrupt::interrupt_with_data(
                                "actual wait",
                                json!({"kind":"fixture"}),
                            ),
                        ))
                    }
                })) as Arc<dyn Node>)
            })
        };
    assert!(
        graph(&old, revisions.clone(), factory.clone(), cp.clone())
            .invoke(State::new(), ExecutionConfig::new("boundary"))
            .await
            .is_err()
    );
    assert_eq!(count.load(Ordering::SeqCst), 0);
    let checkpoint = cp.load("boundary").await.unwrap().unwrap();
    assert_eq!(checkpoint.pending_nodes, vec!["gate"]);
    assert_eq!(
        load_boundary(&store, "boundary", "boundary", 0, "root")
            .await
            .unwrap()
            .unwrap()["toHash"],
        definition(next.clone()).hash
    );
    let rebased = revisions
        .rebased("root", definition(next.clone()))
        .await
        .unwrap();
    assert!(
        graph(&next, rebased, factory, cp)
            .invoke(State::new(), ExecutionConfig::new("boundary"))
            .await
            .is_err()
    );
    assert_eq!(count.load(Ordering::SeqCst), 1);
    assert!(
        load_boundary(&store, "boundary", "boundary", 0, "root")
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn missing_new_state_requirement_retains_prior_revision_without_interrupt() {
    use zf_context::context::ContextBlock;
    use zf_context::context::ContextExpr;
    use zf_context::context::ContextStrategy;
    use zf_context::context::FragmentFormat;
    use zf_context::context::FragmentRole;
    use zf_context::context_source;
    use zf_core::types::DataType;
    let make_program = |needs: bool| {
        let strategy = if needs {
            ContextStrategy::new("policy", "Policy")
                .require("future", DataType::Text)
                .with_program(vec![ContextBlock::emit(
                    "value",
                    FragmentRole::Data,
                    FragmentFormat::Text,
                    ContextExpr::resource("future"),
                )])
        } else {
            ContextStrategy::new("policy", "Policy").with_program(vec![ContextBlock::emit(
                "value",
                FragmentRole::Data,
                FragmentFormat::Text,
                ContextExpr::literal(DataType::Text, json!("old")),
            )])
        };
        let source = context_source::generate(&strategy).unwrap();
        json!({"strategy":strategy,"hash":zf_storage::flow_store::hash(source.as_bytes()),"source":source,"types":{},"bindings":if needs{json!({"future":{"kind":"state","field":"future"}})}else{json!({})}})
    };
    let (_temp, store, cp) = fixture().await;
    let mut old = doc(false);
    let gate = old.nodes.iter_mut().find(|n| n.id == "gate").unwrap();
    gate.data.kind = "agent".into();
    gate.data.config =
        json!({"provider":"fixture","contextProgram":make_program(false),"tag":"old"});
    let mut next = old.clone();
    let gate = next.nodes.iter_mut().find(|n| n.id == "gate").unwrap();
    gate.data.config["contextProgram"] = make_program(true);
    gate.data.config["tag"] = json!("new");
    let revisions = RevisionRuntime::new(
        store.clone(),
        "state-guard",
        BTreeMap::from([("root".into(), definition(old.clone()))]),
    )
    .await
    .unwrap();
    revisions.publish("root", definition(next)).await.unwrap();
    let factory: NodeFactory = Arc::new(|_, spec| {
        let id = spec.id.clone();
        let value = spec
            .data
            .config
            .get("tag")
            .cloned()
            .unwrap_or(json!("after"));
        Ok(Arc::new(FunctionNode::new(&spec.id, move |_| {
            let id = id.clone();
            let value = value.clone();
            async move { Ok(NodeOutput::new().with_update(&id, value)) }
        })) as Arc<dyn Node>)
    });
    let state = graph(&old, revisions, factory, cp)
        .invoke(State::new(), ExecutionConfig::new("state-guard"))
        .await
        .unwrap();
    assert_eq!(state["gate"], "old");
    let mut guarded = false;
    for record in store
        .records("state-guard")
        .await
        .unwrap()
        .into_iter()
        .filter(|r| r.kind == "revision-steps")
    {
        let pin = store.resolve(&record.value_ref).await.unwrap();
        if pin["step"] == 0 {
            assert_eq!(pin["diagnostic"]["code"], "context_state_incompatible");
            guarded = true;
        }
    }
    assert!(guarded);
}

#[tokio::test]
async fn matching_revision_reuses_the_exact_native_node_before_switching_factory() {
    let (_temp, store, _cp) = fixture().await;
    let old = doc(false);
    let runtime = RevisionRuntime::new(
        store,
        "native",
        BTreeMap::from([("root".into(), definition(old.clone()))]),
    )
    .await
    .unwrap();
    let initial = Arc::new(FunctionNode::new("gate", |_| async {
        Ok(NodeOutput::new().with_update("source", json!("native")))
    }));
    let factory: NodeFactory = Arc::new(|_, node| {
        Ok(Arc::new(FunctionNode::new(&node.id, |_| async {
            Ok(NodeOutput::new().with_update("source", json!("factory")))
        })))
    });
    let node = runtime.wrap("root", "gate", initial, factory);
    let first = node
        .execute(&adk_graph::NodeContext::new(
            State::new(),
            ExecutionConfig::new("native"),
            0,
        ))
        .await
        .unwrap();
    assert_eq!(first.updates["source"], "native");
    runtime
        .publish("root", definition(updated(&old)))
        .await
        .unwrap();
    let second = node
        .execute(&adk_graph::NodeContext::new(
            State::new(),
            ExecutionConfig::new("native"),
            1,
        ))
        .await
        .unwrap();
    assert_eq!(second.updates["source"], "factory");
}
