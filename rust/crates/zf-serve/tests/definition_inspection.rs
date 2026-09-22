mod support;
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use serde_json::{Value, json};
use tower::ServiceExt;
use zf_context::context::*;
use zf_core::types::DataType;
use zf_flows::schema::Composition;
use zf_storage::content_store::ContentStore;
use zf_storage::context_store::ContextStore;

async fn request(
    app: &Router,
    method: &str,
    path: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header("content-type", "application/json")
                .body(body.map_or(Body::empty(), |body| Body::from(body.to_string())))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 12 * 1024 * 1024)
        .await
        .unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}
async fn ok(app: &Router, method: &str, path: &str, body: Option<Value>) -> Value {
    let (status, value) = request(app, method, path, body).await;
    assert_eq!(status, StatusCode::OK, "{value}");
    value
}
async fn boundary(app: &Router, id: &str, workspace: &str) -> Value {
    tokio::time::timeout(std::time::Duration::from_secs(15), async {
        loop {
            let run = ok(
                app,
                "GET",
                &format!("/api/runs/{id}?workspaceId={workspace}"),
                None,
            )
            .await;
            if run["status"] != "running" {
                break run;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap()
}
fn node(id: &str, kind: &str, config: Value) -> Value {
    json!({"id":id,"position":{"x":0,"y":0},"data":{"kind":kind,"label":id,"config":config}})
}
fn doc(nodes: Vec<Value>) -> Composition {
    let edges: Vec<_> = nodes
        .windows(2)
        .enumerate()
        .map(
            |(i, pair)| json!({"id":format!("e{i}"),"source":pair[0]["id"],"target":pair[1]["id"]}),
        )
        .collect();
    serde_json::from_value(json!({"formatVersion":3,"id":"inspection","name":"Inspection","nodes":nodes,"edges":edges})).unwrap()
}

#[tokio::test]
async fn preview_uses_a_temporary_workspace_and_captures_catalogue_from_its_source_workspace() {
    let root = tempfile::tempdir().unwrap();
    let workspace = root.path().join("origin");
    std::fs::create_dir(&workspace).unwrap();
    let app =
        zf_serve::server::router_with_home(root.path().join("data"), workspace.clone(), vec![], {
            let home = root.path().join("home");
            std::fs::create_dir_all(&home).unwrap();
            home
        })
        .await
        .unwrap();
    let sources = ContextStore::new(workspace.clone());
    let strategy = ContextStrategy::new("draft", "Draft")
        .require("input", DataType::Text)
        .with_program(vec![ContextBlock::emit(
            "task",
            FragmentRole::Data,
            FragmentFormat::Text,
            ContextExpr::resource("input"),
        )]);
    let saved = sources.save(&strategy, None).await.unwrap();
    let origin = ok(&app, "GET", "/api/workspaces", None).await[0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let composition = doc(vec![
        node("start", "start", json!({})),
        node(
            "effect",
            "tool",
            json!({"tool":"exec","arguments":{"command":"printf preview > result.txt"}}),
        ),
        node(
            "agent",
            "agent",
            json!({"provider":"fixture","fixtureSteps":[{"echoRequest":true}],"contextStrategy":{"key":"draft","hash":saved.hash},"contextBindings":{"input":{"kind":"state","field":"input"}}}),
        ),
        node("end", "end", json!({})),
    ]);
    let started=ok(&app,"POST","/api/runs/preview",Some(json!({"workspaceId":origin,"composition":composition,"input":{"input":"isolated draft"}}))).await;
    let id = started["id"].as_str().unwrap();
    let target = started["workspaceId"].as_str().unwrap();
    assert_ne!(target, origin);
    let run = boundary(&app, id, target).await;
    assert_eq!(run["status"], "completed", "{}", run["error"]);
    assert_eq!(run["preview"]["sourceWorkspaceId"], origin);
    assert!(run["preview"]["sourceCompositionRef"].is_string());
    let original = ok(
        &app,
        "GET",
        &format!("/api/runs/{id}/preview-source?workspaceId={target}"),
        None,
    )
    .await;
    assert_eq!(original["composition"], json!(composition));
    assert_eq!(run["interactive"], false);
    assert_eq!(
        run["composition"]["nodes"][2]["data"]["config"]["contextProgram"]["hash"],
        saved.hash
    );
    assert!(
        run["state"]["output"]
            .as_str()
            .unwrap()
            .contains("isolated draft")
    );
    assert!(!workspace.join("result.txt").exists());
    assert_eq!(
        std::fs::read_to_string(
            std::path::Path::new(run["workspacePath"].as_str().unwrap()).join("result.txt")
        )
        .unwrap(),
        "preview"
    );
    assert_eq!(sources.read("draft").await.unwrap().hash, saved.hash);
    assert_eq!(
        request(
            &app,
            "GET",
            &format!("/api/runs/{id}/definition?workspaceId={origin}"),
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn inspection_resolves_the_actual_old_and_new_passage_definitions() {
    let root = tempfile::tempdir().unwrap();
    let workspace = root.path().join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    let app = zf_serve::server::router_with_home(root.path().join("data"), workspace, vec![], {
        let home = root.path().join("home");
        std::fs::create_dir_all(&home).unwrap();
        home
    })
    .await
    .unwrap();
    let composition = doc(vec![
        node("start", "start", json!({})),
        node(
            "question",
            "input",
            json!({"field":"input","prompt":"Continue?","responseType":"text"}),
        ),
        node("result", "set", json!({"field":"output","value":"old"})),
        node("end", "end", json!({})),
    ]);
    let file = ok(
        &app,
        "POST",
        "/api/flows",
        Some(json!({"composition":composition})),
    )
    .await;
    let started = ok(
        &app,
        "POST",
        "/api/runs",
        Some(json!({"flowKey":file["key"],"flowHash":file["hash"],"input":{}})),
    )
    .await;
    let id = started["id"].as_str().unwrap();
    let workspace = started["workspaceId"].as_str().unwrap();
    let waiting = boundary(&app, id, workspace).await;
    assert_eq!(waiting["status"], "waiting");
    let old = waiting["activities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["path"] == "question")
        .unwrap();
    let mut modified = file["composition"].clone();
    modified["nodes"][2]["data"]["config"]["value"] = json!("new");
    let next = ok(
        &app,
        "POST",
        "/api/flows",
        Some(json!({"composition":modified,"key":file["key"],"expectedHash":file["hash"]})),
    )
    .await;
    let revisions = ok(
        &app,
        "GET",
        &format!("/api/runs/{id}/revisions?workspaceId={workspace}"),
        None,
    )
    .await;
    assert_eq!(
        revisions["instances"][0]["publishedHash"],
        next["sourceHash"]
    );
    assert_eq!(revisions["instances"][0]["scopes"][0]["pending"], true);
    ok(
        &app,
        "POST",
        &format!("/api/runs/{id}/answer?workspaceId={workspace}"),
        Some(json!({"waitId":waiting["wait"]["id"],"value":"yes"})),
    )
    .await;
    let completed = boundary(&app, id, workspace).await;
    assert_eq!(completed["status"], "completed", "{}", completed["error"]);
    assert_eq!(completed["state"]["output"], "new");
    let new = completed["activities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["path"] == "result")
        .unwrap();
    for (activity, expected) in [(old, &file), (new, &next)] {
        let path = format!(
            "/api/runs/{id}/definition?workspaceId={workspace}&nodePath={}&occurrenceId={}",
            activity["path"].as_str().unwrap(),
            activity["occurrenceId"].as_str().unwrap()
        );
        let exact = ok(&app, "GET", &path, None).await;
        assert_eq!(exact["exact"], true);
        assert_eq!(exact["hash"], expected["sourceHash"]);
        assert_eq!(exact["composition"], expected["composition"]);
        assert_eq!(
            zf_storage::flow_store::hash(exact["source"].as_str().unwrap().as_bytes()),
            exact["hash"]
        );
        let exported=ok(&app,"POST","/api/generate",Some(json!({"runId":id,"workspaceId":workspace,"nodePath":activity["path"],"occurrenceId":activity["occurrenceId"]}))).await;
        assert_eq!(exported["executionRevision"]["hash"], exact["hash"]);
        assert_eq!(
            exported["files"]
                .as_array()
                .unwrap()
                .iter()
                .find(|file| file["path"] == "flows/instance-0/flow.rs")
                .unwrap()["content"],
            exact["source"]
        );
    }
    let mismatched = format!(
        "/api/runs/{id}/definition?workspaceId={workspace}&nodePath=result&occurrenceId={}",
        old["occurrenceId"].as_str().unwrap()
    );
    assert_eq!(
        request(&app, "GET", &mismatched, None).await.0,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn legacy_interactive_metadata_backfill_never_hydrates_history_or_rewrites_entities() {
    let db = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::query("CREATE TABLE runs(id TEXT PRIMARY KEY,document TEXT NOT NULL)")
        .execute(&db)
        .await
        .unwrap();
    let store = ContentStore::new(db.clone()).await.unwrap();
    let composition = store
        .intern(&json!(doc(vec![
            node("start", "start", json!({})),
            node("wait", "input", json!({})),
            node("end", "end", json!({}))
        ])))
        .await
        .unwrap();
    let record = json!({"id":"legacy","storageVersion":2,"compositionRef":composition,"stateRef":"intentionally-unavailable-history","messagesRef":"never-read"});
    sqlx::query("INSERT INTO runs(id,document) VALUES('legacy',?)")
        .bind(record.to_string())
        .execute(&db)
        .await
        .unwrap();
    assert_eq!(
        zf_storage::session_store::backfill_interactive(
            &db,
            &zf_runtime::archive_validation::RuntimeArchiveValidation
        )
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        zf_storage::session_store::backfill_interactive(
            &db,
            &zf_runtime::archive_validation::RuntimeArchiveValidation
        )
        .await
        .unwrap(),
        0
    );
    let raw: String = sqlx::query_scalar("SELECT document FROM runs WHERE id='legacy'")
        .fetch_one(&db)
        .await
        .unwrap();
    let mut result: Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(
        result.as_object_mut().unwrap().remove("interactive"),
        Some(json!(true))
    );
    assert_eq!(result, record);
}

#[tokio::test]
async fn saving_a_global_flow_updates_all_its_runs_with_each_workspaces_own_context() {
    let root = tempfile::tempdir().unwrap();
    let a = root.path().join("a");
    let b = root.path().join("b");
    let unused = root.path().join("unused");
    for path in [&a, &b, &unused] {
        std::fs::create_dir(path).unwrap();
    }
    let app = zf_serve::server::router_with_home(root.path().join("data"), a.clone(), vec![], {
        let home = root.path().join("home");
        std::fs::create_dir_all(&home).unwrap();
        home
    })
    .await
    .unwrap();
    let a_id = ok(&app, "GET", "/api/workspaces", None).await[0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let b_id = ok(&app, "POST", "/api/workspaces", Some(json!({"path":b}))).await["id"]
        .as_str()
        .unwrap()
        .to_owned();
    ok(
        &app,
        "POST",
        "/api/workspaces",
        Some(json!({"path":unused})),
    )
    .await;
    for (workspace, label) in [(&a, "source A"), (&b, "source B")] {
        let strategy =
            ContextStrategy::new("local", "Local context").with_program(vec![ContextBlock::emit(
                "context",
                FragmentRole::Data,
                FragmentFormat::Text,
                ContextExpr::literal(DataType::Text, json!(label)),
            )]);
        ContextStore::new(workspace.clone())
            .save(&strategy, None)
            .await
            .unwrap();
    }
    let composition = doc(vec![
        node("start", "start", json!({})),
        node(
            "question",
            "input",
            json!({"field":"input","prompt":"Continue?","responseType":"text"}),
        ),
        node(
            "agent",
            "agent",
            json!({"provider":"fixture","fixtureSteps":[{"echoRequest":true}],"contextStrategy":{"key":"local"},"contextBindings":{}}),
        ),
        node("result", "set", json!({"field":"output","value":"old"})),
        node("end", "end", json!({})),
    ]);
    let file = ok(
        &app,
        "POST",
        "/api/flows",
        Some(json!({"workspaceId":a_id,"scope":"global","composition":composition})),
    )
    .await;
    let mut runs = Vec::new();
    for workspace in [&a_id, &b_id] {
        let started=ok(&app,"POST","/api/runs",Some(json!({"workspaceId":workspace,"flowKey":file["key"],"flowHash":file["hash"],"input":{}}))).await;
        let id = started["id"].as_str().unwrap().to_owned();
        let waiting = boundary(&app, &id, workspace).await;
        assert_eq!(waiting["status"], "waiting");
        runs.push((id, workspace.clone(), waiting["wait"]["id"].clone()));
    }
    let mut modified = file["composition"].clone();
    modified["nodes"][3]["data"]["config"]["value"] = json!("new globally");
    ok(&app,"POST","/api/flows",Some(json!({"workspaceId":a_id,"key":file["key"],"expectedHash":file["hash"],"composition":modified}))).await;
    let mut hashes = Vec::new();
    for (id, workspace, wait) in runs {
        let revision = ok(
            &app,
            "GET",
            &format!("/api/runs/{id}/revisions?workspaceId={workspace}"),
            None,
        )
        .await;
        assert_eq!(revision["instances"][0]["scopes"][0]["pending"], true);
        hashes.push(revision["instances"][0]["publishedHash"].clone());
        ok(
            &app,
            "POST",
            &format!("/api/runs/{id}/answer?workspaceId={workspace}"),
            Some(json!({"waitId":wait,"value":"continue"})),
        )
        .await;
        let completed = boundary(&app, &id, &workspace).await;
        assert_eq!(completed["status"], "completed", "{}", completed["error"]);
        assert_eq!(completed["state"]["output"], "new globally");
        let definition = ok(
            &app,
            "GET",
            &format!("/api/runs/{id}/definition?workspaceId={workspace}&nodePath=agent"),
            None,
        )
        .await;
        let label = if workspace == a_id {
            "source A"
        } else {
            "source B"
        };
        assert!(
            definition["composition"]["nodes"][2]["data"]["config"]["contextProgram"]["source"]
                .as_str()
                .unwrap()
                .contains(label)
        );
    }
    assert_ne!(
        hashes[0], hashes[1],
        "Executable revisions retain their workspace-specific sources"
    );
}

// Two independently versioned instances; no node or model is executed by this fixture.
fn inspection_graph(value: &str) -> zf_compiler::prepared::PreparedRuntime {
    use std::collections::BTreeMap;
    use zf_compiler::prepared::{FrozenFlow, PreparedRuntime};
    use zf_flows::{
        composition::{CompositionCatalog, ResolveRequest},
        flow_contract,
    };
    let flows = ["root", "worker"].map(|instance| {
        let mut composition = doc(vec![
            node("start", "start", json!({"exports":{"contract":{"entries":{"main":{"input":{"kind":"text"}}}},"entries":{"main":{"node":"result","inputField":"input"}},"branches":{},"data":{},"requires":{}}})),
            node("result", "set", json!({"field":"output","value":format!("{instance}-{value}")})),
            node("end", "end", json!({})),
        ]);
        composition.id = instance.into();
        let source = zf_flows::flow_source::render(&composition, &zf_compiler::graph_compiler::GraphValidator::new(&zf_runtime::materialize::RuntimePrimitives)).unwrap();
        (instance.to_string(), FrozenFlow { key: instance.into(), hash: zf_storage::flow_store::hash(source.as_bytes()), source, exports: flow_contract::validate(&composition).unwrap().unwrap(), composition })
    });
    let catalog: CompositionCatalog = serde_json::from_value(json!({"flows":flows.iter().map(|(id,flow)|(id.clone(), flow.exports.contract.clone())).collect::<BTreeMap<_,_>>(),"bridges":{"bridge":{"imports":{"worker":{"flow":"worker"}},"connections":{},"bindings":{}}}})).unwrap();
    let graph = zf_compiler::resolve::resolve(
        &catalog,
        &ResolveRequest {
            flow: "root".into(),
            entry: "main".into(),
            bridges: vec!["bridge".into()],
        },
    )
    .unwrap();
    let [root, worker] = flows;
    let mut runtime = PreparedRuntime {
        graph,
        flows: BTreeMap::from([root, ("bridge/worker".into(), worker.1)]),
        definitions: Default::default(),
    };
    let bridge_source = zf_flows::bridge_source::generate(&catalog.bridges["bridge"]).unwrap();
    runtime.definitions.bridge_hashes.insert(
        "bridge".into(),
        zf_storage::flow_store::hash(bridge_source.as_bytes()),
    );
    runtime
        .definitions
        .bridge_sources
        .insert("bridge".into(), bridge_source);
    for flow in runtime.flows.values() {
        let package = zf_flows::package::PackageSnapshot::capture(
            json!({"formatVersion":1,"id":flow.key,"name":flow.key,"entry":"flow.rs","files":["flow.rs","README.md"]}).to_string(),
            BTreeMap::from([("flow.rs".into(),flow.source.as_bytes().to_vec()),("README.md".into(),value.as_bytes().to_vec())]), BTreeMap::new()).unwrap();
        runtime
            .definitions
            .flow_hashes
            .insert(flow.key.clone(), flow.hash.clone());
        runtime
            .definitions
            .flow_packages
            .insert(flow.key.clone(), package);
    }
    runtime
}

#[tokio::test]
async fn initial_root_and_historical_passages_export_their_own_graph_even_when_last_activity_is_child()
 {
    use zf_runtime::revisions::RevisionDefinition;
    let root = tempfile::tempdir().unwrap();
    let workspace_path = root.path().join("workspace");
    std::fs::create_dir_all(&workspace_path).unwrap();
    let (app, service) = support::open_router(
        root.path().join("data"),
        workspace_path.clone(),
        vec![],
        root.path().join("home"),
    )
    .await
    .unwrap();
    let workspace = ok(&app, "GET", "/api/workspaces", None).await[0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let other = root.path().join("other");
    std::fs::create_dir(&other).unwrap();
    let foreign = ok(&app, "POST", "/api/workspaces", Some(json!({"path":other}))).await["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let store = ContentStore::new(service.database()).await.unwrap();
    let initial = inspection_graph("initial");
    let adopted = inspection_graph("adopted");
    let initial_ref = store.intern(&json!(initial)).await.unwrap();
    let adopted_ref = store.intern(&json!(adopted)).await.unwrap();
    let root_def = RevisionDefinition::from_prepared(&initial, "root").unwrap();
    assert_ne!(root_def.revision(), root_def.hash);
    assert_ne!(root_def.package.as_ref().unwrap().root, root_def.hash);
    assert_ne!(root_def.package.as_ref().unwrap().root, root_def.revision());
    let mut activities = Vec::new();
    for (occurrence, instance, graph, reference) in [
        ("old-root", "root", &initial, &initial_ref),
        ("old-child", "bridge/worker", &initial, &initial_ref),
        ("new-root", "root", &adopted, &adopted_ref),
        ("new-child", "bridge/worker", &adopted, &adopted_ref),
    ] {
        let definition = RevisionDefinition::from_prepared(graph, instance).unwrap();
        let definition_ref = store.intern(&json!(definition)).await.unwrap();
        activities.push(json!({"path":format!("{instance}/result"),"node":"result","occurrenceId":occurrence,"status":"completed","flowRevision":{"instance":instance,"scope":instance,"key":definition.key,"hash":definition.hash,"definitionRevision":definition.revision(),"definitionRef":definition_ref,"graphRef":reference}}));
    }
    // Test both the original graph and persisted captures after graph adoption.
    for count in [2, 4] {
        let id = uuid::Uuid::new_v4().to_string();
        let run = json!({"id":id,"name":"Inspection fixture","workspaceId":workspace,"workspacePath":workspace_path,"status":"completed","interactive":false,"composition":root_def.composition,"flowSource":root_def.source,"runtimeGraphRef":initial_ref,"activities":activities[..count],"state":{},"messages":[],"wait":null});
        zf_storage::session_store::save(&service.database(), &id, &run)
            .await
            .unwrap();
        let endpoint = format!("/api/runs/{id}/definition?workspaceId={workspace}");
        let selected = ok(&app, "GET", &endpoint, None).await;
        assert_eq!(selected["instance"], "root");
        assert_eq!(selected["nodePath"], "");
        assert!(selected["occurrenceId"].is_null());
        assert_eq!(selected["source"], root_def.source);
        assert_eq!(selected["definitionRevision"], root_def.revision());
        assert_eq!(selected["graphRef"], initial_ref);
        assert_eq!(selected["runtime"], initial.summary());
        assert_eq!(
            ok(&app, "GET", &format!("{endpoint}&nodePath="), None).await,
            selected
        );
        assert_eq!(
            ok(&app, "GET", &format!("{endpoint}&nodePath=root"), None).await["instance"],
            "root"
        );
        for (query, definition, graph) in std::iter::once((json!({"hash":selected["definitionRevision"]}),selected.clone(),&initial)).chain(activities[..count].iter().map(|activity| {
            let graph = if activity["occurrenceId"].as_str().unwrap().starts_with("old") {&initial} else {&adopted};
            (json!({"nodePath":activity["path"],"occurrenceId":activity["occurrenceId"],"hash":activity["flowRevision"]["definitionRevision"]}),activity.clone(),graph)
        })) {
            let mut body = query.clone(); body["runId"]=json!(id);body["workspaceId"]=json!(workspace);
            let exported = ok(&app,"POST","/api/generate",Some(body)).await;
            assert_eq!(exported["executionRevision"]["graphRef"],if query.get("occurrenceId").is_some(){definition["flowRevision"]["graphRef"].clone()}else{json!(initial_ref)});
            let sources:Vec<_> = exported["files"].as_array().unwrap().iter().filter(|file|file["path"].as_str().unwrap().starts_with("flows/")&&file["path"].as_str().unwrap().ends_with("/flow.rs")).map(|file|file["content"].as_str().unwrap()).collect();
            for flow in graph.flows.values() {assert!(sources.contains(&flow.source.as_str()));}
            if let Some(occurrence) = query["occurrenceId"].as_str() {
                let inspected = ok(&app,"GET",&format!("{endpoint}&nodePath={}&occurrenceId={occurrence}&hash={}",query["nodePath"].as_str().unwrap(),query["hash"].as_str().unwrap()),None).await;
                assert_eq!(inspected["graphRef"],exported["executionRevision"]["graphRef"]);
                assert_eq!(inspected["runtime"],graph.summary());
                assert!(sources.contains(&inspected["source"].as_str().unwrap()));
            }
        }
        for hash in [
            activities[1]["flowRevision"]["definitionRevision"]
                .as_str()
                .unwrap(),
            activities[2]["flowRevision"]["definitionRevision"]
                .as_str()
                .unwrap(),
        ] {
            assert_eq!(
                request(&app, "GET", &format!("{endpoint}&hash={hash}"), None)
                    .await
                    .0,
                StatusCode::BAD_REQUEST
            );
            assert_eq!(
                request(
                    &app,
                    "POST",
                    "/api/generate",
                    Some(json!({"runId":id,"workspaceId":workspace,"hash":hash}))
                )
                .await
                .0,
                StatusCode::BAD_REQUEST
            );
        }
        assert_eq!(request(&app,"POST","/api/generate",Some(json!({"runId":id,"workspaceId":workspace,"nodePath":"root/result","occurrenceId":"old-root","hash":activities[2]["flowRevision"]["definitionRevision"]}))).await.0,StatusCode::BAD_REQUEST);
        assert_eq!(
            request(
                &app,
                "POST",
                "/api/generate",
                Some(json!({"runId":id,"workspaceId":foreign,"hash":root_def.revision()}))
            )
            .await
            .0,
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            request(
                &app,
                "GET",
                &format!("{endpoint}&nodePath=&occurrenceId=old-child"),
                None
            )
            .await
            .0,
            StatusCode::BAD_REQUEST
        );
        if count == 4 {
            let mixed_id = uuid::Uuid::new_v4().to_string();
            let mut mixed = run.clone();
            mixed["id"] = json!(mixed_id);
            mixed["activities"][3]["flowRevision"]["graphRef"] = json!(initial_ref);
            zf_storage::session_store::save(&service.database(), &mixed_id, &mixed)
                .await
                .unwrap();
            let inspected = ok(&app,"GET",&format!("/api/runs/{mixed_id}/definition?workspaceId={workspace}&nodePath=bridge/worker/result&occurrenceId=new-child"),None).await;
            assert_eq!(inspected["definitionMatchesGraph"], false);
            let (status, error) = request(&app,"POST","/api/generate",Some(json!({"runId":mixed_id,"workspaceId":workspace,"nodePath":"bridge/worker/result","occurrenceId":"new-child","hash":activities[3]["flowRevision"]["definitionRevision"]}))).await;
            assert_eq!(status, StatusCode::BAD_REQUEST);
            assert!(
                error["error"]
                    .as_str()
                    .unwrap()
                    .contains("aucun graphe mixte")
            );
        }
    }
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn composed_run_adopts_a_graph_but_whole_run_inspection_and_export_stay_at_admission() {
    use zf_flows::composition::{
        BridgeDefinition, Connection, Endpoint, InvocationKind, RouteMode,
    };
    let root = tempfile::tempdir().unwrap();
    let workspace_path = root.path().join("workspace");
    std::fs::create_dir_all(&workspace_path).unwrap();
    let (app, service) = support::open_router(
        root.path().join("data"),
        workspace_path.clone(),
        vec![],
        root.path().join("home"),
    )
    .await
    .unwrap();
    let contract = json!({"input":{"kind":"text"},"output":{"kind":"text"}});
    let mut parent = doc(vec![
        node(
            "start",
            "start",
            json!({"exports":{"interactive":true,"contract":{"entries":{"main":contract},"branches":{"work":{"contract":contract,"invocations":["node"]}}},"entries":{"main":{"node":"route","inputField":"input","outputField":"output"}},"branches":{"work":"route"}}}),
        ),
        node("route", "route", json!({"branch":"work"})),
        node(
            "pause",
            "input",
            json!({"field":"output","prompt":"Parent pause"}),
        ),
        node("end", "end", json!({})),
    ]);
    parent.id = "inspection-parent".into();
    let mut child = doc(vec![
        node(
            "start",
            "start",
            json!({"exports":{"interactive":true,"contract":{"entries":{"main":contract}},"entries":{"main":{"node":"gate","inputField":"input","outputField":"output"}}}}),
        ),
        node(
            "gate",
            "input",
            json!({"field":"input","prompt":"Child pause"}),
        ),
        node(
            "result",
            "set",
            json!({"field":"output","value":"old child"}),
        ),
        node("end", "end", json!({})),
    ]);
    child.id = "inspection-child".into();
    let parent = ok(
        &app,
        "POST",
        "/api/flows",
        Some(json!({"composition":parent})),
    )
    .await;
    let child = ok(
        &app,
        "POST",
        "/api/flows",
        Some(json!({"composition":child})),
    )
    .await;
    let bridge = BridgeDefinition::new()
        .import("worker", child["key"].as_str().unwrap())
        .connect(
            "call",
            Connection::new(
                Endpoint::new("root", "work"),
                Endpoint::new("worker", "main"),
                RouteMode::CallAwait,
                InvocationKind::Node,
            ),
        );
    zf_storage::bridge_store::BridgeStore::new(workspace_path)
        .unwrap()
        .save("inspection", &bridge, None)
        .await
        .unwrap();
    let started = ok(&app,"POST","/api/runs",Some(json!({"runtimeSelection":{"flow":parent["key"],"entry":"main","bridges":["inspection"],"flowHashes":{parent["key"].as_str().unwrap():parent["hash"],child["key"].as_str().unwrap():child["hash"]}},"input":{"input":"fixture"}}))).await;
    let id = started["id"].as_str().unwrap();
    let workspace = started["workspaceId"].as_str().unwrap();
    let waiting = boundary(&app, id, workspace).await;
    assert_eq!(waiting["status"], "waiting", "{waiting}");
    let old = waiting["activities"].as_array().unwrap().last().unwrap();
    assert_eq!(old["path"], "inspection/worker/gate");
    let endpoint = format!("/api/runs/{id}/definition?workspaceId={workspace}");
    let initial = ok(&app, "GET", &endpoint, None).await;
    assert_eq!(initial["instance"], "root");
    assert!(initial["graphRef"].is_string());
    let mut changed = child["composition"].clone();
    changed["nodes"][2]["data"]["config"]["value"] = json!("new child");
    let revised=ok(&app,"POST","/api/flows",Some(json!({"workspaceId":workspace,"key":child["key"],"expectedHash":child["hash"],"composition":changed}))).await;
    ok(
        &app,
        "POST",
        &format!("/api/runs/{id}/answer?workspaceId={workspace}"),
        Some(json!({"waitId":waiting["wait"]["id"],"value":"continue"})),
    )
    .await;
    let next = boundary(&app, id, workspace).await;
    assert_eq!(next["status"], "waiting", "{next}");
    assert_eq!(next["wait"]["nodePath"], "root/pause");
    let new = next["activities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["path"] == "inspection/worker/result")
        .unwrap();
    assert_ne!(
        old["flowRevision"]["graphRef"],
        new["flowRevision"]["graphRef"]
    );
    assert_eq!(ok(&app, "GET", &endpoint, None).await, initial);
    for (activity, file) in [(old, &child), (new, &revised)] {
        let exact = ok(
            &app,
            "GET",
            &format!(
                "{endpoint}&nodePath={}&occurrenceId={}",
                activity["path"].as_str().unwrap(),
                activity["occurrenceId"].as_str().unwrap()
            ),
            None,
        )
        .await;
        assert_eq!(exact["source"], file["source"]);
        assert_eq!(exact["hash"], file["sourceHash"]);
        let exported=ok(&app,"POST","/api/generate",Some(json!({"workspaceId":workspace,"runId":id,"nodePath":activity["path"],"occurrenceId":activity["occurrenceId"],"hash":exact["definitionRevision"]}))).await;
        assert_eq!(exported["executionRevision"]["graphRef"], exact["graphRef"]);
        assert!(
            exported["files"]
                .as_array()
                .unwrap()
                .iter()
                .any(|f| f["content"] == exact["source"])
        );
    }
    let exported = ok(
        &app,
        "POST",
        "/api/generate",
        Some(json!({"workspaceId":workspace,"runId":id,"hash":initial["definitionRevision"]})),
    )
    .await;
    assert_eq!(
        exported["executionRevision"]["graphRef"],
        initial["graphRef"]
    );
    assert!(
        exported["files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["content"] == child["source"])
    );
    assert!(
        !exported["files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["content"] == revised["source"])
    );
    service.shutdown().await.unwrap();
}
