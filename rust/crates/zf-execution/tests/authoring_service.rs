use anyhow::{Result, ensure};
use async_trait::async_trait;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use tempfile::TempDir;
use zf_compiler::programs::{SourceKind, SourceOverride};
use zf_context::{context::ContextStrategy, context_source};
use zf_execution::{
    authoring::{StoreBridge, StoreFlow},
    commands::{Actor, CommandAuthorizer, CommandKind, ExecutionError},
    service::{ExecutionOptions, ExecutionService},
};
use zf_flows::{composition::BridgeDefinition, schema::Composition};
use zf_storage::{
    context_store,
    workspaces::{self, Workspace},
};

#[derive(Default)]
struct Policy {
    calls: Mutex<Vec<(CommandKind, String)>>,
}
#[async_trait]
impl CommandAuthorizer for Policy {
    async fn authorize(
        &self,
        actor: &Actor,
        command: CommandKind,
        workspace: &Workspace,
        _: Option<&Value>,
    ) -> Result<()> {
        ensure!(
            actor.id == "author",
            ExecutionError::Forbidden("Unknown author".into())
        );
        self.calls
            .lock()
            .unwrap()
            .push((command, workspace.id.clone()));
        Ok(())
    }
}

struct Fixture {
    root: TempDir,
    service: ExecutionService,
    actor: Actor,
    policy: Arc<Policy>,
}
impl Fixture {
    async fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        for folder in ["workspace", "home"] {
            std::fs::create_dir(root.path().join(folder)).unwrap();
        }
        let policy = Arc::new(Policy::default());
        let service = ExecutionService::open(ExecutionOptions {
            data: root.path().join("data"),
            workspace: root.path().join("workspace"),
            flow_home: root.path().join("home"),
            context_home: Some(root.path().join("home")),
            skill_dirs: vec![],
            authorizer: policy.clone(),
        })
        .await
        .unwrap();
        let actor = Actor {
            id: "author".into(),
            workspace_id: service.default_workspace_id().into(),
        };
        Self {
            root,
            service,
            actor,
            policy,
        }
    }
}

fn source(label: &str, expected_hash: Option<String>) -> SourceOverride {
    SourceOverride {
        kind: SourceKind::Strategy,
        key: "policy".into(),
        source: context_source::generate(&ContextStrategy::new("policy", label)).unwrap(),
        expected_hash,
    }
}
fn node(id: &str, kind: &str, config: Value) -> Value {
    json!({"id":id,"position":{"x":0,"y":0},"data":{"kind":kind,"label":id,"config":config}})
}
fn flow() -> Composition {
    serde_json::from_value(json!({"formatVersion":3,"id":"fixture","name":"Fixture","nodes":[
        node("s","start",json!({})), node("work","set",json!({"field":"output","value":"result"})), node("e","end",json!({}))
    ],"edges":[{"id":"a","source":"s","target":"work"},{"id":"b","source":"work","target":"e"}]})).unwrap()
}
fn store_flow(composition: Composition) -> StoreFlow {
    StoreFlow {
        composition,
        scope: None,
        key: None,
        expected_hash: None,
    }
}
fn bridge(worker: &str) -> BridgeDefinition {
    serde_json::from_value(json!({"imports":{"worker":{"flow":worker}},"connections":{"call":{"from":{"instance":"root","port":"delegate"},"to":{"instance":"worker","port":"main"},"invocation":"node","mode":"callAwait"}}})).unwrap()
}

fn files_below(root: &Path) -> BTreeMap<PathBuf, Option<Vec<u8>>> {
    let mut snapshot = BTreeMap::new();
    let mut pending = vec![root.join("workspace"), root.join("home")];
    while let Some(path) = pending.pop() {
        if path.is_dir() {
            snapshot.insert(path.clone(), None);
            pending.extend(
                std::fs::read_dir(path)
                    .unwrap()
                    .map(|entry| entry.unwrap().path()),
            );
        } else {
            snapshot.insert(path.clone(), Some(std::fs::read(path).unwrap()));
        }
    }
    snapshot
}

#[tokio::test]
async fn maintenance_and_authorization_reject_every_authoring_entry_before_filesystem_writes() {
    let f = Fixture::new().await;
    let before = files_below(f.root.path());
    let maintenance = f.service.try_begin_maintenance().unwrap();
    for error in [
        f.service
            .store_flow(&f.actor, store_flow(flow()))
            .await
            .unwrap_err(),
        f.service
            .store_source(&f.actor, source("Policy", None))
            .await
            .unwrap_err(),
        f.service
            .store_bridge(
                &f.actor,
                StoreBridge {
                    key: "bridge".into(),
                    bridge: bridge("worker"),
                    expected_hash: None,
                },
            )
            .await
            .unwrap_err(),
    ] {
        assert!(matches!(
            error.downcast_ref::<ExecutionError>(),
            Some(ExecutionError::Busy(_))
        ));
    }
    assert!(f.policy.calls.lock().unwrap().is_empty());
    drop(maintenance);
    let denied = Actor {
        id: "stranger".into(),
        workspace_id: f.actor.workspace_id.clone(),
    };
    for error in [
        f.service
            .store_flow(&denied, store_flow(flow()))
            .await
            .unwrap_err(),
        f.service
            .store_source(&denied, source("Policy", None))
            .await
            .unwrap_err(),
        f.service
            .store_bridge(
                &denied,
                StoreBridge {
                    key: "bridge".into(),
                    bridge: bridge("worker"),
                    expected_hash: None,
                },
            )
            .await
            .unwrap_err(),
    ] {
        assert!(matches!(
            error.downcast_ref::<ExecutionError>(),
            Some(ExecutionError::Forbidden(_))
        ));
    }
    assert_eq!(files_below(f.root.path()), before);
    f.service.shutdown().await.unwrap();
}

#[tokio::test]
async fn source_acceptance_preserves_exact_bytes_and_serializes_competing_revisions() {
    let f = Fixture::new().await;
    let mut initial = source("First", None);
    initial.source.push_str("\n// Keep these authored bytes.\n");
    let bytes = initial.source.clone();
    let first = f.service.store_source(&f.actor, initial).await.unwrap();
    assert_eq!(first.source.as_deref(), Some(bytes.as_str()));
    assert_eq!(first.hash, context_store::hash(bytes.as_bytes()));
    assert_eq!(tokio::fs::read_to_string(&first.path).await.unwrap(), bytes);
    let (a, b) = tokio::join!(
        f.service
            .store_source(&f.actor, source("Second", Some(first.hash.clone()))),
        f.service
            .store_source(&f.actor, source("Third", Some(first.hash.clone())))
    );
    let winner = match (a, b) {
        (Ok(file), Err(error)) | (Err(error), Ok(file)) => {
            assert!(
                error.downcast_ref::<context_store::Conflict>().is_some(),
                "{error:#}"
            );
            file
        }
        other => panic!("exactly one revision must win: {other:?}"),
    };
    let actual = tokio::fs::read_to_string(&winner.path).await.unwrap();
    assert_eq!(winner.source.as_deref(), Some(actual.as_str()));
    assert_eq!(winner.hash, context_store::hash(actual.as_bytes()));
    assert!(
        f.policy
            .calls
            .lock()
            .unwrap()
            .iter()
            .all(|(kind, _)| *kind == CommandKind::Authoring)
    );
    f.service.shutdown().await.unwrap();
}

#[tokio::test]
async fn authoring_uses_the_admitted_workspace_record_and_rejects_unknown_workspaces() {
    let f = Fixture::new().await;
    let other_path = f.root.path().join("other");
    std::fs::create_dir(&other_path).unwrap();
    let other = workspaces::open(&f.service.database(), &other_path)
        .await
        .unwrap();
    let actor = Actor {
        id: "author".into(),
        workspace_id: other.id.clone(),
    };
    let accepted = f
        .service
        .store_source(&actor, source("Other", None))
        .await
        .unwrap();
    assert_eq!(accepted.path, other.path.join(".zedflow/context/policy.rs"));
    assert!(
        !f.root
            .path()
            .join("workspace/.zedflow/context/policy.rs")
            .exists()
    );
    assert_eq!(
        f.policy.calls.lock().unwrap().as_slice(),
        &[(CommandKind::Authoring, other.id)]
    );
    let unknown = Actor {
        id: "author".into(),
        workspace_id: "missing".into(),
    };
    assert!(
        f.service
            .store_source(&unknown, source("Unknown", None))
            .await
            .is_err()
    );
    assert_eq!(f.policy.calls.lock().unwrap().len(), 1);
    f.service.shutdown().await.unwrap();
}

#[tokio::test]
async fn flow_and_bridge_acceptance_share_real_catalogue_preflight_without_http() {
    let f = Fixture::new().await;
    let contract = json!({"input":{"kind":"text"},"output":{"kind":"text"}});
    let mut root = flow();
    root.id = "root-flow".into();
    root.nodes[0].data.config = json!({"exports":{"contract":{"entries":{"main":contract},"branches":{"delegate":{"contract":contract,"invocations":["node"]}}},"entries":{"main":{"node":"work","inputField":"input","outputField":"output"}},"branches":{"delegate":"work"}}});
    root.nodes[1] =
        serde_json::from_value(node("work", "route", json!({"branch":"delegate"}))).unwrap();
    let mut worker = flow();
    worker.id = "worker-flow".into();
    worker.nodes[0].data.config = json!({"exports":{"contract":{"entries":{"main":contract}},"entries":{"main":{"node":"work","inputField":"input","outputField":"output"}}}});
    let root_file = f
        .service
        .store_flow(&f.actor, store_flow(root))
        .await
        .unwrap();
    let worker_file = f
        .service
        .store_flow(&f.actor, store_flow(worker))
        .await
        .unwrap();
    for file in [&root_file, &worker_file] {
        let actual = tokio::fs::read_to_string(file.path.join("flow.rs"))
            .await
            .unwrap();
        assert_eq!(file.source.as_deref(), Some(actual.as_str()));
        assert_eq!(file.source_hash, context_store::hash(actual.as_bytes()));
        assert_eq!(file.package.as_ref().unwrap().root, file.hash);
    }
    let accepted = f
        .service
        .store_bridge(
            &f.actor,
            StoreBridge {
                key: "delegate".into(),
                bridge: bridge(&worker_file.key),
                expected_hash: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(
        tokio::fs::read_to_string(&accepted.path).await.unwrap(),
        accepted.source.unwrap()
    );
    let error = f
        .service
        .store_bridge(
            &f.actor,
            StoreBridge {
                key: "broken".into(),
                bridge: bridge("absent"),
                expected_hash: None,
            },
        )
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("no valid composition"));
    assert!(!accepted.path.parent().unwrap().join("broken.rs").exists());
    f.service.shutdown().await.unwrap();
}
