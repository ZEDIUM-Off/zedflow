use adk_graph::State;
use anyhow::{Result, ensure};
use async_trait::async_trait;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{path::Path, sync::Arc, time::Duration};
use zf_context::{
    context::FragmentFormat,
    window::{self, PreparedWindow, WindowPatch},
    window_preparation::WindowSelectionCommand,
};
use zf_execution::{
    administration::{WindowEdit, WindowQuery, WorkspaceUpdate},
    commands::{Actor, Answer, CommandAuthorizer, CommandKind, ExecutionError},
    service::{ExecutionOptions, ExecutionService},
    start::{StartDefinition, StartRequest},
};
use zf_storage::{
    data::{DataRegistry, WindowRegistry},
    workspaces::Workspace,
};

struct Policy;
#[async_trait]
impl CommandAuthorizer for Policy {
    async fn authorize(
        &self,
        actor: &Actor,
        _: CommandKind,
        _: &Workspace,
        _: Option<&Value>,
    ) -> Result<()> {
        ensure!(
            actor.id == "operator",
            ExecutionError::Forbidden("Denied".into())
        );
        Ok(())
    }
}
fn options(root: &Path) -> ExecutionOptions {
    ExecutionOptions {
        data: root.join("data"),
        workspace: root.join("workspace"),
        flow_home: root.join("home"),
        context_home: Some(root.join("home")),
        skill_dirs: vec![],
        authorizer: Arc::new(Policy),
    }
}
async fn open(root: &Path) -> ExecutionService {
    for name in ["workspace", "home"] {
        std::fs::create_dir_all(root.join(name)).unwrap();
    }
    ExecutionService::open(options(root)).await.unwrap()
}
fn actor(service: &ExecutionService) -> Actor {
    Actor {
        id: "operator".into(),
        workspace_id: service.default_workspace_id().into(),
    }
}
async fn waiting(service: &ExecutionService, actor: &Actor) -> String {
    let flow=serde_json::from_value(json!({"id":"fixture","name":"Fixture","nodes":[
        {"id":"s","position":{"x":0,"y":0},"data":{"kind":"start","label":"Start","config":{}}},
        {"id":"wait","position":{"x":0,"y":0},"data":{"kind":"input","label":"Wait","config":{"field":"answer","prompt":"Continue?","responseType":"text"}}},
        {"id":"e","position":{"x":0,"y":0},"data":{"kind":"end","label":"End","config":{}}}
    ],"edges":[{"id":"a","source":"s","target":"wait"},{"id":"b","source":"wait","target":"e"}]})).unwrap();
    let run = service
        .start(
            actor,
            StartRequest {
                definition: StartDefinition::Inline(flow),
                input: State::new(),
                model_bindings: json!({}),
                node_path: None,
                prepared_context: None,
                preview_metadata: None,
            },
        )
        .await
        .unwrap();
    let id = run["id"].as_str().unwrap().to_owned();
    tokio::time::timeout(Duration::from_secs(10), service.wait_idle(&id))
        .await
        .unwrap()
        .unwrap();
    id
}

#[tokio::test]
async fn workspace_and_import_admission_precedes_filesystem_mutation() {
    let root = tempfile::tempdir().unwrap();
    let service = open(root.path()).await;
    let actor = actor(&service);
    let denied = Actor {
        id: "denied".into(),
        workspace_id: actor.workspace_id.clone(),
    };
    let other = root.path().join("other");
    std::fs::create_dir(&other).unwrap();
    assert!(service.open_workspace(&denied, &other).await.is_err());
    assert!(!other.join(".zedflow").exists());
    let error = service
        .import_sessions(&denied, &root.path().join("missing"))
        .await
        .unwrap_err();
    assert!(matches!(
        error.downcast_ref::<ExecutionError>(),
        Some(ExecutionError::Forbidden(_))
    ));
    let workspace = service.open_workspace(&actor, &other).await.unwrap();
    let target = Actor {
        workspace_id: workspace.id.clone(),
        ..actor.clone()
    };
    let updated = service
        .update_workspace(
            &target,
            WorkspaceUpdate {
                name: Some("  renamed  ".into()),
                open: Some(false),
            },
        )
        .await
        .unwrap();
    assert_eq!(updated.name, "renamed");
    assert!(!updated.open);
    assert!(
        service
            .update_workspace(
                &target,
                WorkspaceUpdate {
                    name: Some(" ".into()),
                    open: None
                }
            )
            .await
            .is_err()
    );
    assert_eq!(
        zf_storage::workspaces::get(&service.database(), &workspace.id)
            .await
            .unwrap()
            .name,
        "renamed"
    );
    assert!(service.read_scope(&denied, None).await.is_err());
    assert!(service.recover_catalog(&denied).await.is_err());
    let read = service.read_scope(&actor, None).await.unwrap();
    assert!(service.try_begin_maintenance().is_err());
    drop(read);
    service.recover_catalog(&actor).await.unwrap();
    let guard = service.try_begin_maintenance().unwrap();
    assert!(service.open_workspace(&actor, &other).await.is_err());
    assert!(
        service
            .import_sessions(&actor, &root.path().join("missing"))
            .await
            .is_err()
    );
    assert!(service.read_scope(&actor, None).await.is_err());
    assert!(service.recover_catalog(&actor).await.is_err());
    drop(guard);
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn runtime_and_offline_maintenance_share_a_stable_directory_lock() {
    let root = tempfile::tempdir().unwrap();
    let service = open(root.path()).await;
    assert!(zf_storage::migration::lock(&root.path().join("data")).is_err());
    service.shutdown().await.unwrap();
    let maintenance = zf_storage::migration::lock(&root.path().join("data")).unwrap();
    // A directory replacement cannot change the sibling lock's identity.
    std::fs::rename(root.path().join("data"), root.path().join("retired-data")).unwrap();
    let error = ExecutionService::open(options(root.path()))
        .await
        .err()
        .unwrap();
    assert!(matches!(
        error.downcast_ref::<ExecutionError>(),
        Some(ExecutionError::Busy(_))
    ));
    assert!(!root.path().join("data").exists());
    drop(maintenance);
    std::fs::rename(root.path().join("retired-data"), root.path().join("data")).unwrap();
    let service = ExecutionService::open(options(root.path())).await.unwrap();
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn window_commands_preserve_history_check_owner_and_retry_without_reapplying() {
    let root = tempfile::tempdir().unwrap();
    let service = open(root.path()).await;
    let actor = actor(&service);
    let id = waiting(&service, &actor).await;
    let store = service.content();
    let data = DataRegistry::new(store.pool().clone(), store.clone(), &id)
        .await
        .unwrap();
    let windows = WindowRegistry::new(data);
    let prepared:PreparedWindow=serde_json::from_value(json!({"strategyId":"fixture","strategyRevision":"frozen","programRevision":"program",
        "items":[{"kind":"fragment","id":"prompt","role":"data","format":"text","value":"initial","sources":[]}],"sourceRevisions":{},"capabilities":[]})).unwrap();
    let path = "root/model";
    let alias = "window";
    let scope = window::agent_scope(path);
    let original = windows.create(&scope, alias, &prepared).await.unwrap();
    let owner_key = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&json!([scope, alias])).unwrap())
    );
    store
        .put_record(
            &id,
            "window-owners",
            &owner_key,
            &json!({"nodePath":path,"alias":alias}),
        )
        .await
        .unwrap();
    let query = WindowQuery {
        node_path: path.into(),
        alias: alias.into(),
        revision: None,
    };
    assert_eq!(
        service
            .context_windows(&actor, &id)
            .await
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let other = root.path().join("other");
    std::fs::create_dir(&other).unwrap();
    let other = service.open_workspace(&actor, &other).await.unwrap();
    let foreign = Actor {
        workspace_id: other.id,
        ..actor.clone()
    };
    assert!(
        service
            .context_window(&foreign, &id, query.clone())
            .await
            .is_err()
    );
    assert!(
        service
            .context_window(
                &actor,
                &id,
                WindowQuery {
                    node_path: "root/other".into(),
                    ..query.clone()
                }
            )
            .await
            .is_err()
    );
    let command = WindowEdit {
        id: uuid::Uuid::new_v4().to_string(),
        node_path: path.into(),
        alias: alias.into(),
        expected_revision: original.revision.clone(),
        patches: vec![WindowPatch::Representation {
            id: "prompt".into(),
            format: FragmentFormat::Text,
            value: json!("edited"),
        }],
    };
    sqlx::query("CREATE TRIGGER fail_window_event BEFORE INSERT ON events WHEN json_extract(NEW.document,'$.type')='context_window_edit_requested' BEGIN SELECT RAISE(FAIL,'fixture request failure'); END").execute(store.pool()).await.unwrap();
    assert!(
        service
            .patch_window(&actor, &id, command.clone())
            .await
            .is_err()
    );
    assert_eq!(
        windows.read(&scope, alias).await.unwrap().revision,
        original.revision
    );
    sqlx::query("DROP TRIGGER fail_window_event")
        .execute(store.pool())
        .await
        .unwrap();
    sqlx::query("CREATE TRIGGER fail_window_event BEFORE INSERT ON events WHEN json_extract(NEW.document,'$.type')='context_window_edited' BEGIN SELECT RAISE(FAIL,'fixture terminal failure'); END").execute(store.pool()).await.unwrap();
    assert!(
        service
            .patch_window(&actor, &id, command.clone())
            .await
            .is_err()
    );
    let committed_patch = windows.read(&scope, alias).await.unwrap();
    assert_ne!(committed_patch.revision, original.revision);
    let author:String=sqlx::query_scalar("SELECT json_extract(document,'$.actor.id') FROM events WHERE run=? AND json_extract(document,'$.type')='context_window_edit_requested' ORDER BY seq DESC LIMIT 1").bind(&id).fetch_one(&service.database()).await.unwrap();
    assert_eq!(author, "operator");
    sqlx::query("DROP TRIGGER fail_window_event")
        .execute(store.pool())
        .await
        .unwrap();
    let changed = service
        .patch_window(&actor, &id, command.clone())
        .await
        .unwrap();
    assert_eq!(
        changed["revision"],
        serde_json::to_value(committed_patch.revision).unwrap()
    );
    let stale = WindowEdit {
        id: uuid::Uuid::new_v4().to_string(),
        ..command.clone()
    };
    assert!(service.patch_window(&actor, &id, stale).await.is_err());
    let next = windows
        .patch(
            &scope,
            alias,
            &serde_json::from_value(changed["revision"].clone()).unwrap(),
            &[WindowPatch::Representation {
                id: "prompt".into(),
                format: FragmentFormat::Text,
                value: json!("later"),
            }],
        )
        .await
        .unwrap();
    assert_eq!(
        service
            .patch_window(&actor, &id, command.clone())
            .await
            .unwrap()["revision"],
        changed["revision"]
    );
    assert_eq!(
        windows.read(&scope, alias).await.unwrap().revision,
        next.revision
    );
    let historic = service
        .context_window(
            &actor,
            &id,
            WindowQuery {
                revision: Some(original.revision),
                ..query.clone()
            },
        )
        .await
        .unwrap();
    assert_eq!(historic["window"]["items"][0]["value"], "initial");
    let selection = WindowSelectionCommand {
        id: uuid::Uuid::new_v4().to_string(),
        node_path: path.into(),
        alias: alias.into(),
        revision: next.revision.clone(),
        program_hash: "program".into(),
    };
    sqlx::query("CREATE TRIGGER fail_window_event BEFORE INSERT ON events WHEN json_extract(NEW.document,'$.type')='context_window_selection_requested' BEGIN SELECT RAISE(FAIL,'fixture request failure'); END").execute(store.pool()).await.unwrap();
    assert!(
        service
            .select_window(&actor, &id, selection.clone())
            .await
            .is_err()
    );
    let pending: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM zf_records WHERE scope=? AND kind LIKE 'window-selections:%'",
    )
    .bind(&id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(pending, 0);
    sqlx::query("DROP TRIGGER fail_window_event")
        .execute(store.pool())
        .await
        .unwrap();
    sqlx::query("CREATE TRIGGER fail_window_event BEFORE INSERT ON events WHEN json_extract(NEW.document,'$.type')='context_window_selection' BEGIN SELECT RAISE(FAIL,'fixture terminal failure'); END").execute(store.pool()).await.unwrap();
    assert!(
        service
            .select_window(&actor, &id, selection.clone())
            .await
            .is_err()
    );
    let pending: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM zf_records WHERE scope=? AND kind LIKE 'window-selections:%'",
    )
    .bind(&id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(pending, 1);
    let author:String=sqlx::query_scalar("SELECT json_extract(document,'$.actor.id') FROM events WHERE run=? AND json_extract(document,'$.type')='context_window_selection_requested' ORDER BY seq DESC LIMIT 1").bind(&id).fetch_one(&service.database()).await.unwrap();
    assert_eq!(author, "operator");
    sqlx::query("DROP TRIGGER fail_window_event")
        .execute(store.pool())
        .await
        .unwrap();
    let selected = service
        .select_window(&actor, &id, selection.clone())
        .await
        .unwrap();
    assert_eq!(
        service.select_window(&actor, &id, selection).await.unwrap(),
        selected
    );
    assert!(
        service
            .select_window(
                &actor,
                &id,
                WindowSelectionCommand {
                    id: uuid::Uuid::new_v4().to_string(),
                    node_path: path.into(),
                    alias: alias.into(),
                    revision: next.revision,
                    program_hash: "wrong".into()
                }
            )
            .await
            .is_err()
    );
    let actors:Vec<String>=sqlx::query_scalar("SELECT json_extract(document,'$.actor.id') FROM events WHERE run=? AND json_extract(document,'$.type')='context_window_edited'").bind(&id).fetch_all(&service.database()).await.unwrap();
    assert!(!actors.is_empty());
    assert!(actors.iter().all(|id| id == "operator"));
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn archives_are_targeted_and_import_remains_idle_and_idempotent() {
    let source = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    let service = open(source.path()).await;
    let actor = actor(&service);
    let id = waiting(&service, &actor).await;
    let duplicate = tokio::time::timeout(
        Duration::from_secs(2),
        service.export_sessions(&actor, &[id.clone(), id.clone()]),
    )
    .await
    .expect("duplicate selection must be rejected before taking export barriers");
    assert!(duplicate.is_err());
    let exported = service
        .export_sessions(&actor, std::slice::from_ref(&id))
        .await
        .unwrap();
    let archive_id = exported
        .download_url
        .split('?')
        .next()
        .unwrap()
        .rsplit('/')
        .next()
        .unwrap()
        .trim_end_matches(".zip");
    let archive = service.download_sessions(&actor, archive_id).await.unwrap();
    assert!(!archive.is_empty());
    let other = source.path().join("other");
    std::fs::create_dir(&other).unwrap();
    let other = service.open_workspace(&actor, &other).await.unwrap();
    let foreign = Actor {
        workspace_id: other.id,
        ..actor.clone()
    };
    assert!(
        service
            .export_sessions(&foreign, std::slice::from_ref(&id))
            .await
            .is_err()
    );
    assert!(
        service
            .download_sessions(&foreign, archive_id)
            .await
            .is_err()
    );
    let imported_service = open(target.path()).await;
    let imported_actor = Actor {
        id: "operator".into(),
        workspace_id: imported_service.default_workspace_id().into(),
    };
    let result = imported_service
        .import_sessions(&imported_actor, &exported.exports[0].path)
        .await
        .unwrap();
    assert_eq!(result.imported, 1);
    let repeated = imported_service
        .import_sessions(&imported_actor, &exported.exports[0].path)
        .await
        .unwrap();
    assert_eq!(repeated.unchanged, 1);
    let run = imported_service.read(&imported_actor, &id).await.unwrap();
    assert_eq!(run["status"], "waiting");
    assert_eq!(run["workspaceId"], imported_actor.workspace_id);
    imported_service
        .answer(
            &imported_actor,
            &id,
            Answer {
                wait_id: run["wait"]["id"].as_str().unwrap().into(),
                value: json!("continue"),
                node_path: None,
            },
        )
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(10), imported_service.wait_idle(&id))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        imported_service.read(&imported_actor, &id).await.unwrap()["status"],
        "completed"
    );
    imported_service.shutdown().await.unwrap();
    service.shutdown().await.unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn archive_commands_do_not_interrupt_or_wait_ahead_of_active_work() {
    let root = tempfile::tempdir().unwrap();
    let service = open(root.path()).await;
    let actor = actor(&service);
    let paused = waiting(&service, &actor).await;
    let composition = serde_json::from_value(json!({"id":"active","name":"Active fixture","nodes":[
        {"id":"start","position":{"x":0,"y":0},"data":{"kind":"start","label":"Start","config":{}}},
        {"id":"effect","position":{"x":0,"y":0},"data":{"kind":"tool","label":"Effect","config":{"tool":"exec","arguments":{"command":"printf ready > ready; while [ ! -f release ]; do sleep 0.01; done; printf done > done"}}}},
        {"id":"end","position":{"x":0,"y":0},"data":{"kind":"end","label":"End","config":{}}}
    ],"edges":[{"id":"a","source":"start","target":"effect"},{"id":"b","source":"effect","target":"end"}]})).unwrap();
    let run = service
        .start(
            &actor,
            StartRequest {
                definition: StartDefinition::Inline(composition),
                input: State::new(),
                model_bindings: json!({}),
                node_path: None,
                prepared_context: None,
                preview_metadata: None,
            },
        )
        .await
        .unwrap();
    let id = run["id"].as_str().unwrap().to_owned();
    tokio::time::timeout(Duration::from_secs(10), async {
        while !root.path().join("workspace/ready").exists() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    let error = tokio::time::timeout(
        Duration::from_secs(2),
        service.export_sessions(&actor, std::slice::from_ref(&id)),
    )
    .await
    .unwrap()
    .unwrap_err();
    assert!(matches!(
        error.downcast_ref::<ExecutionError>(),
        Some(ExecutionError::Busy(_))
    ));
    let error = tokio::time::timeout(
        Duration::from_secs(2),
        service.import_sessions(&actor, &root.path().join("missing")),
    )
    .await
    .unwrap()
    .unwrap_err();
    assert!(matches!(
        error.downcast_ref::<ExecutionError>(),
        Some(ExecutionError::Busy(_))
    ));
    assert!(!root.path().join("workspace/done").exists());
    assert_eq!(
        service.read(&actor, &id).await.unwrap()["status"],
        "running"
    );
    // An unrelated active run does not prevent exporting an already paused run.
    service
        .export_sessions(&actor, std::slice::from_ref(&paused))
        .await
        .unwrap();
    std::fs::write(root.path().join("workspace/release"), "continue").unwrap();
    tokio::time::timeout(Duration::from_secs(10), service.wait_idle(&id))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        service.read(&actor, &id).await.unwrap()["status"],
        "completed"
    );
    assert_eq!(
        std::fs::read_to_string(root.path().join("workspace/done")).unwrap(),
        "done"
    );
    service.shutdown().await.unwrap();
}
