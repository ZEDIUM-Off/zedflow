use serde_json::{Value, json};
use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};
use std::collections::BTreeMap;
use tempfile::TempDir;
use zf_compiler::{
    graph_compiler::GraphValidator,
    programs::{self, SourceKind, SourceOverride},
};
use zf_context::{
    context::{
        ContextBlock, ContextExpr, ContextFunction, ContextLibrary, ContextStrategy,
        FragmentFormat, FragmentRole, LibraryKind,
    },
    context_source,
};
use zf_core::types::DataType;
use zf_execution::{live_files, sources};
use zf_flows::{flow_source, schema::Composition};
use zf_runtime::{
    materialize::RuntimePrimitives,
    revisions::{self, RevisionDefinition, RevisionPublication},
};
use zf_storage::{
    content_store::ContentStore,
    context_store::{self, ContextStore, LibraryStore, SourceStore, TypeStore},
    flow_store::FlowStore,
    source_acceptance::{self, FilePrecondition},
    workspaces::{self, Workspace},
};

async fn freeze(doc: &mut Composition, workspace: &std::path::Path) -> anyhow::Result<()> {
    let captured = sources::program_sources(doc, workspace, &[]).await?;
    programs::freeze(doc, &captured)?;
    Ok(())
}
async fn freeze_with_overrides(
    doc: &mut Composition,
    workspace: &std::path::Path,
    overrides: &[SourceOverride],
) -> anyhow::Result<()> {
    let captured = sources::program_sources(doc, workspace, overrides).await?;
    programs::freeze_with_overrides(doc, &captured, overrides)?;
    Ok(())
}

struct Fixture {
    root: TempDir,
    db: SqlitePool,
    store: ContentStore,
    workspace: Workspace,
    flows: FlowStore,
}
impl Fixture {
    async fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let workspace_path = root.path().join("workspace");
        let home = root.path().join("isolated-home");
        std::fs::create_dir_all(&workspace_path).unwrap();
        std::fs::create_dir_all(&home).unwrap();
        let db = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let store = ContentStore::new(db.clone()).await.unwrap();
        workspaces::initialize(&db).await.unwrap();
        sqlx::query("CREATE TABLE runs(id TEXT PRIMARY KEY,document TEXT NOT NULL)")
            .execute(&db)
            .await
            .unwrap();
        let workspace = Workspace {
            id: workspaces::path_id(&workspace_path),
            name: "Fixture".into(),
            path: workspace_path,
            open: true,
        };
        workspaces::save(&db, &workspace).await.unwrap();
        Self {
            root,
            db,
            store,
            workspace,
            flows: FlowStore::new(
                home,
                std::sync::Arc::new(GraphValidator::new(&RuntimePrimitives)),
            ),
        }
    }
    async fn run(&self, id: &str, doc: &Composition, status: &str) {
        let source = flow_source::render(doc, &GraphValidator::new(&RuntimePrimitives)).unwrap();
        let run = json!({"id":id,"workspaceId":self.workspace.id,"status":status,"composition":doc,"flowSource":source,"flowRef":{"key":"fixture-flow"}});
        sqlx::query("INSERT INTO runs VALUES(?,?)")
            .bind(id)
            .bind(run.to_string())
            .execute(&self.db)
            .await
            .unwrap();
    }
    async fn head(&self, id: &str) -> RevisionDefinition {
        let head = self
            .store
            .record(id, "revision-heads", &context_store::hash(b""))
            .await
            .unwrap()
            .unwrap();
        serde_json::from_value(
            self.store
                .resolve(head["definitionRef"].as_str().unwrap())
                .await
                .unwrap(),
        )
        .unwrap()
    }
    async fn flow(&self, doc: &Composition) {
        let dir = self.workspace.path.join(".zedflow/flows");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(format!("{}.rs", doc.id)),
            flow_source::render(doc, &GraphValidator::new(&RuntimePrimitives)).unwrap(),
        )
        .unwrap();
    }
}
fn text(value: &str) -> ContextStrategy {
    ContextStrategy::new("policy", "Policy").with_program(vec![ContextBlock::emit(
        "policy",
        FragmentRole::Instruction,
        FragmentFormat::Text,
        ContextExpr::literal(DataType::Text, json!(value)),
    )])
}
fn flow(strategy_hash: &str) -> Composition {
    serde_json::from_value(json!({"formatVersion":3,"id":"dependent","name":"Dependent","nodes":[
        {"id":"s","position":{"x":0,"y":0},"data":{"kind":"start","label":"Start","config":{}}},
        {"id":"agent","position":{"x":0,"y":0},"data":{"kind":"agent","label":"Agent","config":{"provider":"fixture","contextStrategy":{"key":"policy","hash":strategy_hash},"contextBindings":{}}}},
        {"id":"e","position":{"x":0,"y":0},"data":{"kind":"end","label":"End","config":{}}}
    ],"edges":[{"id":"a","source":"s","target":"agent"},{"id":"b","source":"agent","target":"e"}]})).unwrap()
}
fn candidate(strategy: &ContextStrategy, hash: Option<&str>) -> SourceOverride {
    SourceOverride {
        kind: SourceKind::Strategy,
        key: strategy.id.clone(),
        source: context_source::generate(strategy).unwrap(),
        expected_hash: hash.map(str::to_owned),
    }
}
fn definition(doc: Composition) -> RevisionDefinition {
    let source = flow_source::render(&doc, &GraphValidator::new(&RuntimePrimitives)).unwrap();
    RevisionDefinition {
        key: "fixture-flow".into(),
        hash: context_store::hash(source.as_bytes()),
        source,
        composition: doc,
    }
}

#[tokio::test]
async fn accepted_lineage_relinks_old_pins_but_unknown_and_external_revisions_are_refused() {
    let f = Fixture::new().await;
    let catalog = ContextStore::new(f.workspace.path.clone());
    let first = catalog.save(&text("first"), None).await.unwrap();
    let authored = flow(&first.hash);
    f.flow(&authored).await;
    let second = live_files::accept_source(
        &f.db,
        &f.flows,
        &f.workspace,
        candidate(&text("second"), Some(&first.hash)),
    )
    .await
    .unwrap();
    let mut linked = authored.clone();
    freeze(&mut linked, &f.workspace.path).await.unwrap();
    assert_eq!(
        linked.nodes[1].data.config["contextProgram"]["hash"],
        second.hash
    );
    assert_eq!(
        linked.nodes[1].data.config["contextStrategy"]["hash"], first.hash,
        "Authored reference is not silently rewritten"
    );
    let mut unknown = authored.clone();
    unknown.nodes[1].data.config["contextStrategy"]["hash"] = json!("f".repeat(64));
    assert!(freeze(&mut unknown, &f.workspace.path).await.is_err());
    let external = context_source::generate(&text("external")).unwrap();
    std::fs::write(&first.path, &external).unwrap();
    assert!(
        freeze(&mut authored.clone(), &f.workspace.path)
            .await
            .is_err()
    );
    let external_hash = context_store::hash(external.as_bytes());
    let accepted = live_files::accept_source(
        &f.db,
        &f.flows,
        &f.workspace,
        candidate(&text("accepted external"), Some(&external_hash)),
    )
    .await
    .unwrap();
    freeze(&mut linked, &f.workspace.path).await.unwrap();
    assert_eq!(
        linked.nodes[1].data.config["contextProgram"]["hash"],
        accepted.hash
    );
    assert_eq!(
        std::fs::read_to_string(first.path).unwrap(),
        accepted.source.unwrap()
    );
}

#[tokio::test]
async fn source_save_publishes_all_resumable_runs_and_rejects_incompatible_dependency_before_writing()
 {
    let f = Fixture::new().await;
    let original = ContextStore::new(f.workspace.path.clone())
        .save(&text("original"), None)
        .await
        .unwrap();
    let authored = flow(&original.hash);
    f.flow(&authored).await;
    let mut frozen = authored.clone();
    freeze(&mut frozen, &f.workspace.path).await.unwrap();
    f.run("waiting", &frozen, "waiting").await;
    f.run("running", &frozen, "running").await;
    f.run("done", &frozen, "completed").await;
    let saved = live_files::accept_source(
        &f.db,
        &f.flows,
        &f.workspace,
        candidate(&text("next"), Some(&original.hash)),
    )
    .await
    .unwrap();
    for id in ["waiting", "running"] {
        assert_eq!(
            f.head(id).await.composition.nodes[1].data.config["contextProgram"]["hash"],
            saved.hash
        );
    }
    assert!(
        f.store
            .record("done", "revision-heads", &context_store::hash(b""))
            .await
            .unwrap()
            .is_none()
    );
    let incompatible = text("missing requirement").require("new-resource", DataType::Text);
    let error = live_files::accept_source(
        &f.db,
        &f.flows,
        &f.workspace,
        candidate(&incompatible, Some(&saved.hash)),
    )
    .await
    .unwrap_err();
    assert!(format!("{error:#}").contains("binding"));
    assert_eq!(
        std::fs::read_to_string(&saved.path).unwrap(),
        saved.source.unwrap()
    );
    assert_eq!(
        f.head("running").await.composition.nodes[1].data.config["contextProgram"]["hash"],
        saved.hash
    );
    assert!(
        !f.workspace
            .path
            .join(".zedflow/.source-acceptance.json")
            .exists()
    );
}

#[tokio::test]
async fn library_contract_changes_preflight_every_dependent_flow() {
    let f = Fixture::new().await;
    let library = ContextLibrary::new().projection(
        "label",
        ContextFunction::new(
            BTreeMap::new(),
            DataType::Text,
            ContextExpr::literal(DataType::Text, json!("old")),
        ),
    );
    let catalog = LibraryStore::new(f.workspace.path.clone());
    let first = catalog.save("display", &library, None).await.unwrap();
    let mut strategy = text("unused");
    strategy.program = vec![ContextBlock::emit(
        "label",
        FragmentRole::Data,
        FragmentFormat::Text,
        ContextExpr::call(LibraryKind::Projection, "label", BTreeMap::new()),
    )];
    let strategy = ContextStore::new(f.workspace.path.clone())
        .save(&strategy, None)
        .await
        .unwrap();
    let mut doc = flow(&strategy.hash);
    doc.nodes[1].data.config["contextLibraryRef"] = json!({"key":"display","hash":first.hash});
    f.flow(&doc).await;
    let invalid = ContextLibrary::new().projection(
        "label",
        ContextFunction::new(
            BTreeMap::new(),
            DataType::Number,
            ContextExpr::literal(DataType::Number, json!(42)),
        ),
    );
    let change = SourceOverride {
        kind: SourceKind::Library,
        key: "display".into(),
        source: context_source::generate_library(&invalid).unwrap(),
        expected_hash: Some(first.hash.clone()),
    };
    assert!(
        live_files::accept_source(&f.db, &f.flows, &f.workspace, change)
            .await
            .is_err()
    );
    assert_eq!(catalog.read("display").await.unwrap().hash, first.hash);
}

#[tokio::test]
async fn linked_types_are_exact_portable_and_acceptance_checks_dependents_before_writing() {
    use zf_context::frozen_context;
    use zf_runtime::inference;
    let f = Fixture::new().await;
    let catalog = TypeStore::new(f.workspace.path.clone());
    let types = BTreeMap::from([(
        "Report".into(),
        DataType::Record {
            fields: BTreeMap::from([("title".into(), DataType::Text)]),
        },
    )]);
    let first = catalog.save("domain", &types, None).await.unwrap();
    let strategy = ContextStrategy::new("policy", "Policy")
        .require(
            "report",
            DataType::Named {
                name: "Report".into(),
            },
        )
        .with_program(vec![ContextBlock::emit(
            "title",
            FragmentRole::Data,
            FragmentFormat::Text,
            ContextExpr::field(ContextExpr::resource("report"), "title"),
        )]);
    let strategy_file = ContextStore::new(f.workspace.path.clone())
        .save(&strategy, None)
        .await
        .unwrap();
    let mut authored = flow(&strategy_file.hash);
    authored.nodes[1].data.config["contextTypesRef"] = json!({"key":"domain","hash":first.hash});
    authored.nodes[1].data.config["contextBindings"] =
        json!({"report":{"kind":"state","field":"report"}});
    f.flow(&authored).await;
    let mut frozen = authored.clone();
    freeze(&mut frozen, &f.workspace.path).await.unwrap();
    let exact = &frozen.nodes[1].data.config["contextProgram"];
    assert_eq!(
        exact["typeSources"][0]["source"],
        first.source.as_deref().unwrap()
    );
    frozen_context::validate_frozen(exact).unwrap();
    let original_revision = inference::program(exact).unwrap().revision().unwrap();
    let mut tampered = exact.clone();
    tampered["types"]["Report"]["fields"]["title"] = json!({"kind":"number"});
    assert!(frozen_context::validate_frozen(&tampered).is_err());

    let invalid = BTreeMap::from([(
        "Report".into(),
        DataType::Record {
            fields: BTreeMap::from([("title".into(), DataType::Number)]),
        },
    )]);
    let candidate = |types: &BTreeMap<String, DataType>| SourceOverride {
        kind: SourceKind::Types,
        key: "domain".into(),
        source: context_source::generate_types(types).unwrap(),
        expected_hash: Some(first.hash.clone()),
    };
    assert!(
        live_files::accept_source(&f.db, &f.flows, &f.workspace, candidate(&invalid))
            .await
            .is_err()
    );
    assert_eq!(catalog.read("domain").await.unwrap().hash, first.hash);
    let mut expanded = types.clone();
    expanded.insert("Extra".into(), DataType::Boolean);
    let accepted = live_files::accept_source(&f.db, &f.flows, &f.workspace, candidate(&expanded))
        .await
        .unwrap();
    let mut latest = authored.clone();
    freeze(&mut latest, &f.workspace.path).await.unwrap();
    let latest_program = &latest.nodes[1].data.config["contextProgram"];
    assert_eq!(latest_program["hash"], exact["hash"]);
    assert_ne!(
        inference::program(latest_program)
            .unwrap()
            .revision()
            .unwrap(),
        original_revision
    );
    assert_eq!(latest_program["typeSources"][0]["hash"], accepted.hash);
}

#[tokio::test]
async fn interrupted_acceptance_recovers_sql_heads_once_without_regressing_a_later_publication() {
    let f = Fixture::new().await;
    let first = ContextStore::new(f.workspace.path.clone())
        .save(&text("first"), None)
        .await
        .unwrap();
    let mut base = flow(&first.hash);
    freeze(&mut base, &f.workspace.path).await.unwrap();
    let mut next = base.clone();
    freeze_with_overrides(
        &mut next,
        &f.workspace.path,
        &[candidate(&text("second"), Some(&first.hash))],
    )
    .await
    .unwrap();
    let publication = RevisionPublication {
        run_id: "crashed".into(),
        instance: "".into(),
        baseline: base.clone(),
        definition: definition(next.clone()),
    };
    let manifest = live_files::stage_publications(&f.store, std::slice::from_ref(&publication))
        .await
        .unwrap();
    let pending = source_acceptance::begin(
        f.workspace.path.clone(),
        vec!["context".into()],
        "policy".into(),
        context_source::generate(&text("second")).unwrap(),
        Some(first.hash),
        Some(manifest),
        vec![],
    )
    .await
    .unwrap();
    let batch_id = pending.id().to_owned();
    drop(pending); // Process died after source/head install, before SQL publication.
    assert!(
        ContextStore::new(f.workspace.path.clone())
            .list()
            .await
            .is_err()
    );
    assert!(
        f.store
            .record("crashed", "revision-heads", &context_store::hash(b""))
            .await
            .unwrap()
            .is_none()
    );
    let pending = source_acceptance::recover(f.workspace.path.clone())
        .await
        .unwrap()
        .unwrap();
    revisions::publish_batch_unique(&f.store, &batch_id, std::slice::from_ref(&publication))
        .await
        .unwrap();
    drop(pending); // Process died after SQL commit, before removing intent.
    let mut later = next;
    later.nodes[1].data.config["temperature"] = json!(0.7);
    let later = definition(later);
    revisions::publish_batch(
        &f.store,
        &[RevisionPublication {
            run_id: "crashed".into(),
            instance: "".into(),
            baseline: base,
            definition: later.clone(),
        }],
    )
    .await
    .unwrap();
    live_files::recover(&f.db, &f.workspace.path).await.unwrap();
    assert_eq!(f.head("crashed").await.hash, later.hash);
    assert!(
        ContextStore::new(f.workspace.path.clone())
            .list()
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn preconditions_and_external_edits_never_get_clobbered_and_flow_limit_is_preserved() {
    let f = Fixture::new().await;
    let store = SourceStore::new(f.workspace.path.clone(), &["context"]).unwrap();
    let first = store.save("source", "original", None).await.unwrap();
    let dependency = f.root.path().join("dependency.rs");
    std::fs::write(&dependency, "changed").unwrap();
    assert!(
        source_acceptance::begin(
            f.workspace.path.clone(),
            vec!["context".into()],
            "source".into(),
            "new".into(),
            Some(first.hash.clone()),
            None,
            vec![FilePrecondition {
                path: dependency,
                hash: context_store::hash(b"old")
            }]
        )
        .await
        .is_err()
    );
    assert_eq!(std::fs::read_to_string(&first.path).unwrap(), "original");
    let pending = source_acceptance::begin(
        f.workspace.path.clone(),
        vec!["context".into()],
        "source".into(),
        "new".into(),
        Some(first.hash),
        None,
        vec![],
    )
    .await
    .unwrap();
    std::fs::write(&first.path, "external after install").unwrap();
    assert!(pending.finish().await.is_err());
    assert_eq!(
        std::fs::read_to_string(first.path).unwrap(),
        "external after install"
    );
    let path = f
        .workspace
        .path
        .join(".agents/flows/Dossier avec espace/Édition.rs");
    let source = "x".repeat(1536 * 1024);
    let pending = source_acceptance::begin_flow(
        path.clone(),
        f.workspace.path.clone(),
        source.clone(),
        None,
        None,
        vec![],
    )
    .await
    .unwrap();
    pending.finish().await.unwrap();
    assert_eq!(std::fs::read_to_string(path).unwrap(), source);
}

#[tokio::test]
async fn absent_global_source_root_has_no_recovery_work_and_is_not_created_by_a_read() {
    let f = Fixture::new().await;
    let absent = f.root.path().join("not-yet-created-home");
    live_files::recover(&f.db, &absent).await.unwrap();
    assert!(
        SourceStore::new(absent.clone(), &["context"])
            .unwrap()
            .list()
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        FlowStore::new(
            absent.clone(),
            std::sync::Arc::new(GraphValidator::new(&RuntimePrimitives))
        )
        .list(&f.workspace)
        .await
        .unwrap()
        .is_empty()
    );
    assert!(!absent.exists());
    let invalid = f.root.path().join("file-instead-of-directory");
    std::fs::write(&invalid, "occupied").unwrap();
    assert!(live_files::recover(&f.db, &invalid).await.is_err());
}

#[tokio::test]
async fn bridge_save_publishes_a_new_plan_and_refuses_a_live_instance_rebuild_before_source_write()
{
    use zf_compiler::{
        prepared::{self, CompilationSnapshot},
        programs::SourceSnapshot,
    };
    use zf_flows::composition::{BridgeDefinition, ResolveRequest};
    use zf_storage::bridge_store::BridgeStore;
    let f = Fixture::new().await;
    let node = |id: &str, kind: &str, config: Value| json!({"id":id,"position":{"x":0,"y":0},"data":{"kind":kind,"label":id,"config":config}});
    let edges = json!([{"id":"first","source":"s","target":"work"},{"id":"last","source":"work","target":"e"}]);
    let contract = json!({"input":{"kind":"text"},"output":{"kind":"text"}});
    let root:Composition=serde_json::from_value(json!({"formatVersion":3,"id":"root-flow","name":"Root","nodes":[
        node("s","start",json!({"exports":{"contract":{"entries":{"main":contract},"branches":{"delegate":{"contract":contract,"invocations":["node"]}}},"entries":{"main":{"node":"work","inputField":"input","outputField":"output"}},"branches":{"delegate":"work"}}})),
        node("work","route",json!({"branch":"delegate"})),node("e","end",json!({}))],"edges":edges})).unwrap();
    let worker:Composition=serde_json::from_value(json!({"formatVersion":3,"id":"worker-flow","name":"Worker","nodes":[
        node("s","start",json!({"exports":{"contract":{"entries":{"main":contract,"alternate":contract}},"entries":{"main":{"node":"work","inputField":"input","outputField":"output"},"alternate":{"node":"work","inputField":"input","outputField":"output"}}}})),
        node("work","set",json!({"field":"output","value":"result"})),node("e","end",json!({}))],"edges":edges})).unwrap();
    f.flow(&root).await;
    f.flow(&worker).await;
    let files = f.flows.list(&f.workspace).await.unwrap();
    let root_key = files.iter().find(|f| f.id == root.id).unwrap().key.clone();
    let worker_key = files
        .iter()
        .find(|f| f.id == worker.id)
        .unwrap()
        .key
        .clone();
    let bridge:BridgeDefinition=serde_json::from_value(json!({"imports":{"worker":{"flow":worker_key}},"connections":{"call":{"from":{"instance":"root","port":"delegate"},"to":{"instance":"worker","port":"main"},"invocation":"node","mode":"callAwait"}}})).unwrap();
    let first = live_files::accept_bridge(&f.db, &f.flows, &f.workspace, "delegate", &bridge, None)
        .await
        .unwrap();
    let mut captured_flows = BTreeMap::new();
    for file in files {
        let file = f.flows.get(&f.workspace, &file.key).await.unwrap();
        captured_flows.insert(file.key, SourceSnapshot::capture(file.source.unwrap()));
    }
    let snapshot = CompilationSnapshot {
        flows: captured_flows,
        bridges: BTreeMap::from([(
            "delegate".into(),
            SourceSnapshot::capture(first.source.clone().unwrap()),
        )]),
        ..Default::default()
    };
    let mut prepared = prepared::prepare(
        &snapshot,
        &ResolveRequest {
            flow: root_key,
            entry: "main".into(),
            bridges: vec!["delegate".into()],
        },
        &BTreeMap::new(),
        &BTreeMap::new(),
        &RuntimePrimitives,
    )
    .unwrap();
    prepared
        .definitions
        .bridge_hashes
        .insert("delegate".into(), first.hash.clone());
    prepared
        .definitions
        .bridge_sources
        .insert("delegate".into(), first.source.unwrap());
    prepared.validate(&RuntimePrimitives).unwrap();
    let run = json!({"id":"composed","workspaceId":f.workspace.id,"status":"waiting","runtimeGraph":prepared});
    sqlx::query("INSERT INTO runs VALUES(?,?)")
        .bind("composed")
        .bind(run.to_string())
        .execute(&f.db)
        .await
        .unwrap();
    let mut next = serde_json::to_value(&bridge).unwrap();
    next["connections"]["call"]["to"]["port"] = json!("alternate");
    let next: BridgeDefinition = serde_json::from_value(next).unwrap();
    let accepted = live_files::accept_bridge(
        &f.db,
        &f.flows,
        &f.workspace,
        "delegate",
        &next,
        Some(first.hash),
    )
    .await
    .unwrap();
    let head = f
        .store
        .record("composed", "runtime-graph-heads", "current")
        .await
        .unwrap()
        .unwrap();
    let graph = f
        .store
        .resolve(head["graphRef"].as_str().unwrap())
        .await
        .unwrap();
    assert_eq!(
        graph["graph"]["routes"]["delegate/call"]["to"]["port"],
        "alternate"
    );
    assert_eq!(
        prepared.graph.routes["delegate/call"].to.port, "main",
        "Frozen original plan remains unchanged"
    );
    let mut rebuild = serde_json::to_value(&next).unwrap();
    rebuild["imports"]["extra"] = json!({"flow":worker_key});
    let rebuild = serde_json::from_value(rebuild).unwrap();
    assert!(
        live_files::accept_bridge(
            &f.db,
            &f.flows,
            &f.workspace,
            "delegate",
            &rebuild,
            Some(accepted.hash.clone())
        )
        .await
        .is_err()
    );
    assert_eq!(
        BridgeStore::new(f.workspace.path.clone())
            .unwrap()
            .read("delegate")
            .await
            .unwrap()
            .hash,
        accepted.hash
    );
    assert_eq!(
        f.store
            .record("composed", "runtime-graph-heads", "current")
            .await
            .unwrap()
            .unwrap(),
        head
    );
}

#[tokio::test]
async fn global_flow_edit_links_each_active_workspace_and_leaves_unused_catalogs_alone() {
    use zf_execution::live_flows;
    let f = Fixture::new().await;
    let second_path = f.root.path().join("second");
    std::fs::create_dir(&second_path).unwrap();
    let second = Workspace {
        id: workspaces::path_id(&second_path),
        name: "Second".into(),
        path: second_path,
        open: true,
    };
    workspaces::save(&f.db, &second).await.unwrap();
    let unused_path = f.root.path().join("unused");
    std::fs::create_dir(&unused_path).unwrap();
    let unused = Workspace {
        id: workspaces::path_id(&unused_path),
        name: "Unused".into(),
        path: unused_path,
        open: false,
    };
    workspaces::save(&f.db, &unused).await.unwrap();
    ContextStore::new(f.workspace.path.clone())
        .save(&text("workspace one"), None)
        .await
        .unwrap();
    let second_policy = ContextStore::new(second.path.clone())
        .save(&text("workspace two"), None)
        .await
        .unwrap();
    let mut authored = flow("");
    authored.nodes[1].data.config["contextStrategy"] = json!("policy");
    let plan = f
        .flows
        .plan(&f.workspace, authored.clone(), "global", None, None)
        .await
        .unwrap();
    let file = live_flows::accept(&f.db, &f.store, &f.flows, &f.workspace, plan, None, true)
        .await
        .unwrap();
    for (id, workspace) in [("one", &f.workspace), ("two", &second)] {
        let mut frozen = authored.clone();
        freeze(&mut frozen, &workspace.path).await.unwrap();
        let source =
            flow_source::render(&frozen, &GraphValidator::new(&RuntimePrimitives)).unwrap();
        let run = json!({"workspaceId":workspace.id,"status":"waiting","composition":frozen,"flowSource":source,"flowRef":{"key":file.key}});
        sqlx::query("INSERT INTO runs VALUES(?,?)")
            .bind(id)
            .bind(run.to_string())
            .execute(&f.db)
            .await
            .unwrap();
    }
    let before: String = sqlx::query_scalar("SELECT document FROM runs WHERE id='two'")
        .fetch_one(&f.db)
        .await
        .unwrap();
    authored.nodes[1].data.config["temperature"] = json!(0.4);
    let plan = f
        .flows
        .plan(
            &f.workspace,
            authored,
            "global",
            Some(&file.key),
            Some(&file.hash),
        )
        .await
        .unwrap();
    live_flows::accept(
        &f.db,
        &f.store,
        &f.flows,
        &f.workspace,
        plan,
        Some(file.hash),
        true,
    )
    .await
    .unwrap();
    let first_head = f.head("one").await;
    let second_head = f.head("two").await;
    assert_eq!(
        second_head.composition.nodes[1].data.config["contextProgram"]["hash"],
        second_policy.hash
    );
    assert_ne!(
        first_head.composition.nodes[1].data.config["contextProgram"]["hash"],
        second_head.composition.nodes[1].data.config["contextProgram"]["hash"]
    );
    assert_eq!(
        second_head.composition.nodes[1].data.config["temperature"],
        0.4
    );
    let after: String = sqlx::query_scalar("SELECT document FROM runs WHERE id='two'")
        .fetch_one(&f.db)
        .await
        .unwrap();
    assert_eq!(
        before, after,
        "Historical frozen run is never overwritten by a live edit"
    );
    assert!(!unused.path.join(".zedflow/context").exists());
}

#[tokio::test]
async fn flow_edit_preserves_selected_context_and_snapshot_projection_stays_frozen() {
    use zf_compiler::{
        prepared::{self, CompilationSnapshot, ContextSelection},
        programs::SourceSnapshot,
    };
    use zf_execution::live_flows;
    use zf_flows::composition::ResolveRequest;
    let f = Fixture::new().await;
    let policy = ContextStore::new(f.workspace.path.clone())
        .save(&text("authored default"), None)
        .await
        .unwrap();
    let mut alternate = text("run overlay");
    alternate.id = "alternate".into();
    let alternate = ContextStore::new(f.workspace.path.clone())
        .save(&alternate, None)
        .await
        .unwrap();
    let mut authored = flow(&policy.hash);
    authored.nodes[0].data.config = json!({"exports":{"contract":{"entries":{"main":{"input":{"kind":"text"},"output":{"kind":"text"}}}},"entries":{"main":{"node":"agent","inputField":"input","outputField":"output"}}}});
    let file = f
        .flows
        .store(&f.workspace, authored.clone(), "workspace", None, None)
        .await
        .unwrap();
    let selected = ContextSelection {
        key: "alternate".into(),
        hash: alternate.hash.clone(),
    };
    let mut selected_doc = authored.clone();
    prepared::apply_context_selections(
        &mut selected_doc,
        &BTreeMap::from([("agent".into(), selected.clone())]),
        None,
    )
    .unwrap();
    let snapshot = CompilationSnapshot {
        flows: BTreeMap::from([(
            file.key.clone(),
            SourceSnapshot::capture(file.source.clone().unwrap()),
        )]),
        programs: sources::program_sources(&selected_doc, &f.workspace.path, &[])
            .await
            .unwrap(),
        ..Default::default()
    };
    let runtime = prepared::prepare(
        &snapshot,
        &ResolveRequest {
            flow: file.key.clone(),
            entry: "main".into(),
            bridges: vec![],
        },
        &BTreeMap::new(),
        &BTreeMap::from([("root/agent".into(), selected)]),
        &RuntimePrimitives,
    )
    .unwrap();
    let run = json!({"workspaceId":f.workspace.id,"status":"waiting","runtimeGraph":runtime});
    sqlx::query("INSERT INTO runs VALUES('overlay',?)")
        .bind(run.to_string())
        .execute(&f.db)
        .await
        .unwrap();
    let captured = zf_storage::live_files::snapshots(&f.db, &f.workspace.id)
        .await
        .unwrap();
    authored.nodes[1].data.config["temperature"] = json!(0.6);
    let plan = f
        .flows
        .plan(
            &f.workspace,
            authored,
            "workspace",
            Some(&file.key),
            Some(&file.hash),
        )
        .await
        .unwrap();
    live_flows::accept(
        &f.db,
        &f.store,
        &f.flows,
        &f.workspace,
        plan,
        Some(file.hash),
        false,
    )
    .await
    .unwrap();
    let current = live_files::run_definitions(&f.db, &f.workspace.id)
        .await
        .unwrap();
    assert_eq!(current.len(), 1);
    assert_eq!(current[0].context_selections["agent"].key, "alternate");
    assert_eq!(
        current[0].definition.composition.nodes[1].data.config["contextProgram"]["hash"],
        alternate.hash
    );
    assert_eq!(
        current[0].definition.composition.nodes[1].data.config["temperature"],
        0.6
    );
    let old = live_files::definitions_from_snapshots(&captured).unwrap();
    assert!(
        old[0].definition.composition.nodes[1]
            .data
            .config
            .get("temperature")
            .is_none()
    );
    let old_graphs = live_files::graphs_from_snapshots(&captured).unwrap();
    assert!(
        old_graphs[0].prepared.flows["root"].composition.nodes[1]
            .data
            .config
            .get("temperature")
            .is_none()
    );
    let new_graphs = live_files::run_graphs(&f.db, &f.workspace.id)
        .await
        .unwrap();
    assert_eq!(
        new_graphs[0].prepared.flows["root"].composition.nodes[1]
            .data
            .config["temperature"],
        0.6
    );
    assert!(
        new_graphs[0].baseline.flows["root"].composition.nodes[1]
            .data
            .config
            .get("temperature")
            .is_none()
    );
    let mut invalid = zf_storage::live_files::snapshots(&f.db, &f.workspace.id)
        .await
        .unwrap();
    invalid[0]
        .revision_heads
        .get_mut(&context_store::hash(b"root"))
        .unwrap()
        .head["instance"] = json!("other");
    assert!(
        live_files::definitions_from_snapshots(&invalid)
            .unwrap_err()
            .to_string()
            .contains("instance mismatch")
    );
}

#[tokio::test]
async fn source_capture_authenticates_each_pin_and_ignores_inactive_library_references() {
    let f = Fixture::new().await;
    let catalog = ContextStore::new(f.workspace.path.clone());
    let first = catalog.save(&text("first"), None).await.unwrap();
    let second = catalog
        .save(&text("second"), Some(&first.hash))
        .await
        .unwrap();
    let mut doc = flow(&first.hash);
    let mut another = doc.nodes[1].clone();
    another.id = "other".into();
    another.data.config["contextStrategy"]["hash"] = json!(second.hash);
    doc.nodes.push(another);
    let sources = sources::program_sources(&doc, &f.workspace.path, &[])
        .await
        .unwrap();
    assert_eq!(sources.strategies["policy"].hash, second.hash);
    assert!(
        sources.strategies["policy"]
            .accepted_references
            .contains(&first.hash)
    );
    doc.nodes[3].data.config["contextStrategy"]["hash"] = json!("f".repeat(64));
    assert!(
        sources::program_sources(&doc, &f.workspace.path, &[])
            .await
            .is_err()
    );
    let mut inactive = flow("");
    inactive.nodes[1]
        .data
        .config
        .as_object_mut()
        .unwrap()
        .remove("contextStrategy");
    inactive.nodes[1].data.config["contextLibraryRef"] =
        json!({"invalid":"inactive malformed reference"});
    inactive.nodes[1].data.config["contextTypesRef"] = json!("nonexistent");
    let empty = sources::program_sources(&inactive, &f.workspace.path, &[])
        .await
        .unwrap();
    assert!(empty.strategies.is_empty() && empty.libraries.is_empty() && empty.types.is_empty());
    programs::freeze(&mut inactive, &empty).unwrap();
}
