use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::PathBuf};
use tempfile::TempDir;
use tower::ServiceExt;
use zf_context::context::ContextBlock;
use zf_context::context::ContextCapability;
use zf_context::context::ContextExpr;
use zf_context::context::ContextPredicate;
use zf_context::context::ContextStrategy;
use zf_context::context::FragmentFormat;
use zf_context::context::FragmentRole;
use zf_core::types::DataType;

struct Fixture {
    _root: TempDir,
    app: Router,
    a: String,
    b: String,
    workspace_a: PathBuf,
    workspace_b: PathBuf,
    initial_context_a: BTreeMap<PathBuf, Vec<u8>>,
    initial_context_b: BTreeMap<PathBuf, Vec<u8>>,
}

impl Fixture {
    async fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let workspace_a = root.path().join("workspace-a");
        let workspace_b = root.path().join("workspace-b");
        let home = root.path().join("home");
        for path in [&workspace_a, &workspace_b, &home] {
            std::fs::create_dir(path).unwrap();
        }
        let app = zf_serve::server::router_with_home(
            root.path().join("data"),
            workspace_a.clone(),
            vec![],
            {
                std::fs::create_dir_all(&home).unwrap();
                home
            },
        )
        .await
        .unwrap();
        let (status, workspaces) = request(&app, "GET", "/api/workspaces", None).await;
        assert_eq!(status, StatusCode::OK, "{workspaces}");
        let a = workspaces[0]["id"].as_str().unwrap().to_owned();
        let (status, opened) = request(
            &app,
            "POST",
            "/api/workspaces",
            Some(json!({"path":workspace_b})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{opened}");
        let b = opened["id"].as_str().unwrap().to_owned();
        let initial_context_a = context_files(&workspace_a);
        let initial_context_b = context_files(&workspace_b);
        let fixture = Self {
            _root: root,
            app,
            a,
            b,
            workspace_a,
            workspace_b,
            initial_context_a,
            initial_context_b,
        };
        fixture.assert_no_runs().await;
        fixture
    }

    async fn assert_no_runs(&self) {
        for workspace in [&self.a, &self.b] {
            let (status, runs) = request(
                &self.app,
                "GET",
                &format!("/api/runs?workspaceId={workspace}"),
                None,
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{runs}");
            assert_eq!(runs, json!([]), "Authoring must never create a run");
        }
    }

    async fn save(&self, strategy: &ContextStrategy) -> Value {
        let (status, file) = request(
            &self.app,
            "POST",
            "/api/context-strategies",
            Some(json!({"workspaceId":self.a,"strategy":strategy})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{file}");
        file
    }

    async fn preview(&self, selection: Value, resources: Value) -> (StatusCode, Value) {
        request(
            &self.app,
            "POST",
            "/api/context-strategies/preview",
            Some(json!({"workspaceId":self.a,"selection":selection,"resources":resources})),
        )
        .await
    }
}

// Workspace onboarding installs example strategies. Preview/validation must
// leave those exact bytes untouched and must never save the draft being viewed.
fn context_files(workspace: &std::path::Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let root = workspace.join(".zedflow/context");
    let mut files = BTreeMap::new();
    let mut pending = vec![root.clone()];
    while let Some(directory) = pending.pop() {
        if !directory.exists() {
            continue;
        }
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else {
                files.insert(
                    path.strip_prefix(&root).unwrap().to_owned(),
                    std::fs::read(path).unwrap(),
                );
            }
        }
    }
    files
}

async fn request(
    app: &Router,
    method: &str,
    path: &str,
    value: Option<Value>,
) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header("content-type", "application/json")
                .body(Body::from(value.map(|v| v.to_string()).unwrap_or_default()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 4_000_000).await.unwrap();
    let body = zf_context::context_json::from_slice(&bytes)
        .unwrap_or_else(|_| json!({"raw":String::from_utf8_lossy(&bytes)}));
    (status, body)
}

fn text(id: &str, expression: ContextExpr) -> ContextBlock {
    ContextBlock::emit(id, FragmentRole::Data, FragmentFormat::Text, expression)
}

fn simple_strategy() -> ContextStrategy {
    ContextStrategy::new("conversation", "Contexte de conversation")
        .require("history", DataType::Text)
        .with_program(vec![text("history", ContextExpr::resource("history"))])
}

fn draft(strategy: &ContextStrategy) -> Value {
    json!({"kind":"draft","strategy":strategy})
}

fn selection(file: &Value) -> Value {
    json!({"kind":"file","key":file["key"],"hash":file["hash"]})
}

fn assert_diagnostic(response: &Value, code: &str, path: &str) {
    assert!(
        response["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == code && d["path"] == path),
        "Missing {code} at {path}: {response}"
    );
}

#[tokio::test]
async fn standalone_request_profiles_are_explicit_pure_and_use_provider_boundaries() {
    let fixture = Fixture::new().await;
    let mut strategy = ContextStrategy::new_v2("request-trial", "Requête d’essai")
        .require("input", DataType::Text)
        .with_program(vec![text("input", ContextExpr::resource("input"))]);
    strategy.capabilities.push(ContextCapability {
        id: "read".into(),
        input: DataType::Text,
        output: DataType::Text,
    });
    let mut body = json!({"workspaceId":fixture.a,"selection":draft(&strategy),
        "resources":{"input":"Été 🦀 avec \"citation\""},
        "profile":{"provider":"fixture","model":"fixture","config":{}}});
    let (status, missing) = request(
        &fixture.app,
        "POST",
        "/api/context-strategies/preview",
        Some(body.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{missing}");
    assert_eq!(missing["evaluation"]["complete"], true);
    assert_eq!(missing["request"]["status"], "trial");
    assert!(
        missing["request"]["diagnostics"][0]["message"]
            .as_str()
            .unwrap()
            .contains("read")
    );
    assert!(missing["request"]["raw"].is_null());
    body["profile"]["tools"] = json!({"read":{"description":"Lire un fichier","parameters":{"type":"object","properties":{"path":{"type":"string"}}}}});
    for (provider, boundary) in [
        ("fixture", "fixtureInput"),
        ("gemini", "adkRequest"),
        ("codex", "codexHttpBody"),
    ] {
        body["profile"]["provider"] = json!(provider);
        let (status, response) = request(
            &fixture.app,
            "POST",
            "/api/context-strategies/preview",
            Some(body.clone()),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{response}");
        let prepared = &response["request"];
        assert_eq!(prepared["status"], "prepared", "{response}");
        assert_eq!(prepared["boundary"], boundary);
        let raw = prepared["raw"].as_str().unwrap();
        assert_eq!(
            prepared["sha256"],
            format!("{:x}", Sha256::digest(raw.as_bytes()))
        );
        assert_eq!(prepared["byteLength"], raw.len());
        let actual: Value = serde_json::from_str(raw).unwrap();
        assert_eq!(actual["model"], "fixture");
        assert!(raw.contains("Été 🦀"));
        assert_eq!(
            response["selection"]["strategy"],
            body["selection"]["strategy"]
        );
        if provider == "codex" {
            assert_eq!(actual["tools"][0]["name"], "read");
        } else {
            assert_eq!(actual["tools"], body["profile"]["tools"]);
        }
    }
    body["profile"]["config"]["unsupportedParameter"] = json!(true);
    let (status, _) = request(
        &fixture.app,
        "POST",
        "/api/context-strategies/preview",
        Some(body),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    fixture.assert_no_runs().await;
    assert_eq!(
        context_files(&fixture.workspace_a),
        fixture.initial_context_a
    );
    assert_eq!(
        context_files(&fixture.workspace_b),
        fixture.initial_context_b
    );
}

#[tokio::test]
async fn request_preview_requires_explicit_media_bytes_without_reading_the_reference() {
    let fixture = Fixture::new().await;
    let strategy = ContextStrategy::new_v2("image-trial", "Image")
        .require(
            "image",
            DataType::Media {
                media_type: "image/png".into(),
            },
        )
        .with_program(vec![ContextBlock::emit(
            "image",
            FragmentRole::Data,
            FragmentFormat::Media,
            ContextExpr::resource("image"),
        )]);
    let mut body = json!({"workspaceId":fixture.a,"selection":draft(&strategy),
        "resources":{"image":{"mediaType":"image/png","contentRef":"explicit-image"}},
        "profile":{"provider":"fixture","model":"fixture","config":{}}});
    let (_, missing) = request(
        &fixture.app,
        "POST",
        "/api/context-strategies/preview",
        Some(body.clone()),
    )
    .await;
    assert_eq!(missing["request"]["status"], "trial");
    assert!(
        missing["request"]["diagnostics"][0]["message"]
            .as_str()
            .unwrap()
            .contains("explicit-image")
    );
    body["profile"]["media"] =
        json!({"explicit-image":{"encoding":"base64","byteLength":3,"chunks":["AQID"]}});
    let (_, prepared) = request(
        &fixture.app,
        "POST",
        "/api/context-strategies/preview",
        Some(body),
    )
    .await;
    assert_eq!(prepared["request"]["status"], "prepared", "{prepared}");
    let raw: Value = serde_json::from_str(prepared["request"]["raw"].as_str().unwrap()).unwrap();
    let contents: Vec<adk_core::Content> = serde_json::from_value(raw["contents"].clone()).unwrap();
    assert!(
        matches!(&contents[0].parts[0],adk_core::Part::InlineData{data,mime_type,..} if data==&[1,2,3] && mime_type=="image/png")
    );
    fixture.assert_no_runs().await;
}

#[tokio::test]
async fn saving_selected_named_types_makes_the_strategy_source_self_contained() {
    let fixture = Fixture::new().await;
    let strategy = ContextStrategy::new_v2("typed-terms", "Termes")
        .require(
            "term",
            DataType::Named {
                name: "Term".into(),
            },
        )
        .with_program(vec![text(
            "definition",
            ContextExpr::field(ContextExpr::resource("term"), "definition"),
        )]);
    let types = json!({"Term":{"kind":"record","fields":{"definition":{"kind":"text"}}}});
    let (status, saved) = request(
        &fixture.app,
        "POST",
        "/api/context-strategies",
        Some(json!({"workspaceId":fixture.a,"strategy":strategy,"types":types})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(saved["strategy"]["types"], types);
    assert!(
        saved["source"]
            .as_str()
            .unwrap()
            .contains(".define_type(\"Term\"")
    );
    let (status, reloaded) = request(
        &fixture.app,
        "GET",
        &format!(
            "/api/context-strategies/typed-terms?workspaceId={}",
            fixture.a
        ),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{reloaded}");
    let (status, preview) = fixture
        .preview(
            selection(&reloaded),
            json!({"term":{"definition":"Un contexte composé explicitement"}}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(preview["evaluation"]["complete"], true);
    assert_eq!(
        preview["evaluation"]["items"][0]["value"],
        "Un contexte composé explicitement"
    );
    let (status, conflict) = request(&fixture.app, "POST", "/api/context-strategies/preview", Some(json!({"workspaceId":fixture.a,"selection":selection(&reloaded),"types":{"Term":{"kind":"number"}},"resources":{"term":42}}))).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_diagnostic(&conflict, "type_conflict", "types.Term");
    fixture.assert_no_runs().await;
}

#[tokio::test]
async fn source_catalog_combines_real_workspace_schemas_and_public_flow_data_without_merging_conflicts()
 {
    let fixture = Fixture::new().await;
    let mut ty = json!({"Address":{"kind":"record","fields":{"city":{"kind":"text"}}},"Person":{"kind":"record","fields":{"name":{"kind":"text"},"address":{"kind":"named","name":"Address"}}},"Unused":{"kind":"number"}});
    for (key, types) in [
        ("people", ty.clone()),
        ("other-person", json!({"Person":{"kind":"number"}})),
    ] {
        let (status, response) = request(
            &fixture.app,
            "POST",
            "/api/context-types",
            Some(json!({"workspaceId":fixture.a,"key":key,"types":types})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{response}");
    }
    ty = json!({"Report":{"kind":"record","fields":{"title":{"kind":"text"}}}});
    let composition = json!({"formatVersion":3,"id":"report-source","name":"Rapport public","nodes":[
        {"id":"start","type":"flow","position":{"x":0,"y":0},"data":{"kind":"start","label":"Départ","config":{"exports":{"contract":{"entries":{"main":{"input":{"kind":"text"}}},"data":{"report":{"dataType":{"kind":"named","name":"Report"},"permissions":{"read":true,"write":false}}}},"types":ty,"entries":{"main":{"node":"start","inputField":"input"}},"data":{"report":"report"}}}}},
        {"id":"end","type":"flow","position":{"x":200,"y":0},"data":{"kind":"end","label":"Fin","config":{}}}
    ],"edges":[{"id":"done","source":"start","target":"end"}],"channels":[{"name":"report","reducer":"overwrite","default":{"title":"Exemple"}}]});
    let (status, flow) = request(
        &fixture.app,
        "POST",
        "/api/flows",
        Some(json!({"workspaceId":fixture.a,"composition":composition})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{flow}");
    let (status, catalog) = request(
        &fixture.app,
        "GET",
        &format!("/api/context-source-types?workspaceId={}", fixture.a),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{catalog}");
    let entries = catalog["entries"].as_array().unwrap();
    let person = entries
        .iter()
        .find(|entry| entry["id"] == "catalog:people:Person")
        .unwrap();
    assert_eq!(
        person["types"]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>(),
        ["Address", "Person"]
    );
    assert_eq!(person["types"]["Person"]["kind"], "record");
    assert!(
        entries
            .iter()
            .any(|entry| entry["id"] == "catalog:other-person:Person"
                && entry["types"]["Person"]["kind"] == "number")
    );
    assert!(
        catalog["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|error| error["code"] == "type_identity_conflict")
    );
    let exposed = entries
        .iter()
        .find(|entry| entry["id"] == format!("flow:{}:data:report", flow["key"].as_str().unwrap()))
        .unwrap();
    assert_eq!(exposed["type"], json!({"kind":"named","name":"Report"}));
    assert_eq!(exposed["types"], ty);
    assert!(entries.iter().any(|entry| entry["id"] == "builtin:document"
        && entry["types"]["Document"]["fields"]["content"]["kind"] == "text"));
    let (status, other) = request(
        &fixture.app,
        "GET",
        &format!("/api/context-source-types?workspaceId={}", fixture.b),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{other}");
    assert!(
        other["entries"]
            .as_array()
            .unwrap()
            .iter()
            .all(|entry| entry["id"] != "catalog:people:Person"
                && !entry["id"].as_str().unwrap().contains("report-source"))
    );
    fixture.assert_no_runs().await;
}

#[tokio::test]
async fn rust_strategies_are_workspace_scoped_and_concurrent_saves_compare_loaded_hash() {
    let fixture = Fixture::new().await;
    let original = simple_strategy();
    let saved = fixture.save(&original).await;
    let path = fixture.workspace_a.join(".zedflow/context/conversation.rs");
    assert_eq!(PathBuf::from(saved["path"].as_str().unwrap()), path);
    let source = std::fs::read_to_string(&path).unwrap();
    assert!(source.starts_with("// @zedflow-context 1\n"));
    assert!(source.contains("ContextStrategy::new("));
    assert_eq!(saved["source"], source);
    assert_eq!(saved["strategy"], json!(original));
    assert_eq!(
        saved["hash"],
        format!("{:x}", Sha256::digest(source.as_bytes()))
    );
    let (_, listed) = request(
        &fixture.app,
        "GET",
        &format!("/api/context-strategies?workspaceId={}", fixture.a),
        None,
    )
    .await;
    let listed = listed.as_array().unwrap();
    let expected_initial = fixture
        .initial_context_a
        .keys()
        .filter(|path| {
            path.components().count() == 1
                && path.extension().is_some_and(|extension| extension == "rs")
        })
        .count();
    assert_eq!(listed.len(), expected_initial + 1);
    assert_eq!(
        listed
            .iter()
            .filter(|file| file["key"] == "conversation")
            .count(),
        1
    );
    assert_eq!(
        listed
            .iter()
            .find(|file| file["key"] == "conversation")
            .unwrap()["strategy"],
        json!(original)
    );
    let (status, read) = request(
        &fixture.app,
        "GET",
        &format!(
            "/api/context-strategies/conversation?workspaceId={}",
            fixture.a
        ),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{read}");
    assert_eq!(read, saved);
    let (_, other) = request(
        &fixture.app,
        "GET",
        &format!("/api/context-strategies?workspaceId={}", fixture.b),
        None,
    )
    .await;
    assert!(
        other
            .as_array()
            .unwrap()
            .iter()
            .all(|file| file["key"] != "conversation")
    );
    assert_eq!(
        context_files(&fixture.workspace_b),
        fixture.initial_context_b
    );
    let (status, _) = request(
        &fixture.app,
        "GET",
        &format!(
            "/api/context-strategies/conversation?workspaceId={}",
            fixture.b
        ),
        None,
    )
    .await;
    assert!(
        !status.is_success(),
        "A's source must not be readable through B"
    );
    assert_eq!(
        context_files(&fixture.workspace_b),
        fixture.initial_context_b
    );

    let mut writer_a = original.clone();
    writer_a.name = "Writer A".into();
    let mut writer_b = original;
    writer_b.name = "Writer B".into();
    let (first, second) = tokio::join!(
        request(
            &fixture.app,
            "POST",
            "/api/context-strategies",
            Some(json!({"workspaceId":fixture.a,"strategy":writer_a,"expectedHash":saved["hash"]}))
        ),
        request(
            &fixture.app,
            "POST",
            "/api/context-strategies",
            Some(json!({"workspaceId":fixture.a,"strategy":writer_b,"expectedHash":saved["hash"]}))
        )
    );
    assert!([first.0, second.0].contains(&StatusCode::OK));
    assert!([first.0, second.0].contains(&StatusCode::CONFLICT));
    let winner = if first.0 == StatusCode::OK {
        first.1
    } else {
        second.1
    };
    assert_eq!(std::fs::read_to_string(path).unwrap(), winner["source"]);
    let (status, _) = fixture
        .preview(selection(&saved), json!({"history":"old selection"}))
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    fixture.assert_no_runs().await;
}

#[tokio::test]
async fn previews_capture_exact_draft_or_file_source_without_saving_or_executing_it() {
    let fixture = Fixture::new().await;
    let strategy = simple_strategy();
    let (status, preview) = fixture
        .preview(draft(&strategy), json!({"history":"Bonjour"}))
        .await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(preview["evaluation"]["complete"], true);
    assert_eq!(preview["evaluation"]["items"][0]["value"], "Bonjour");
    assert_eq!(preview["selection"]["strategy"], json!(strategy));
    let source = preview["selection"]["source"].as_str().unwrap();
    assert_eq!(
        preview["selection"]["hash"],
        format!("{:x}", Sha256::digest(source.as_bytes()))
    );
    assert_eq!(
        context_files(&fixture.workspace_a),
        fixture.initial_context_a
    );

    let file = fixture.save(&strategy).await;
    let path = PathBuf::from(file["path"].as_str().unwrap());
    let exact_source = format!(
        "{}\n// commentaire conservé, UTF-8 : été\n",
        file["source"].as_str().unwrap()
    );
    std::fs::write(&path, &exact_source).unwrap();
    let (_, reread) = request(
        &fixture.app,
        "GET",
        &format!(
            "/api/context-strategies/conversation?workspaceId={}",
            fixture.a
        ),
        None,
    )
    .await;
    let (status, preview) = fixture
        .preview(selection(&reread), json!({"history":"depuis fichier"}))
        .await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(preview["selection"]["source"], exact_source);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), exact_source);

    let marker = fixture.workspace_a.join("must-not-exist");
    let injected = exact_source.replace(
        "pub fn strategy() -> ContextStrategy {",
        &format!(
            "pub fn strategy() -> ContextStrategy {{\nstd::fs::write({:?}, \"executed\").unwrap();",
            marker.to_string_lossy()
        ),
    );
    std::fs::write(&path, injected).unwrap();
    let (_, rejected) = request(
        &fixture.app,
        "GET",
        &format!(
            "/api/context-strategies/conversation?workspaceId={}",
            fixture.a
        ),
        None,
    )
    .await;
    assert!(!rejected["diagnostics"].as_array().unwrap().is_empty());
    let (status, _) = fixture
        .preview(selection(&rejected), json!({"history":"unused"}))
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(!marker.exists());
    fixture.assert_no_runs().await;
}

#[tokio::test]
async fn preview_requests_only_resources_used_by_the_selected_branch() {
    let fixture = Fixture::new().await;
    let strategy = ContextStrategy::new("conditional", "Conditionnel")
        .require("enabled", DataType::Boolean)
        .require("history", DataType::Text)
        .with_program(vec![ContextBlock::branch(
            "choose",
            ContextPredicate::equal(
                ContextExpr::resource("enabled"),
                ContextExpr::literal(DataType::Boolean, json!(true)),
            ),
            vec![text("full", ContextExpr::resource("history"))],
            vec![text(
                "minimal",
                ContextExpr::literal(DataType::Text, json!("minimal")),
            )],
        )]);
    let (status, inactive) = fixture
        .preview(draft(&strategy), json!({"enabled":false}))
        .await;
    assert_eq!(status, StatusCode::OK, "{inactive}");
    assert_eq!(inactive["evaluation"]["complete"], true);
    assert_eq!(inactive["evaluation"]["needs"], json!([]));
    assert_eq!(inactive["evaluation"]["items"][0]["value"], "minimal");
    let (status, active) = fixture
        .preview(draft(&strategy), json!({"enabled":true}))
        .await;
    assert_eq!(status, StatusCode::OK, "{active}");
    assert_eq!(active["evaluation"]["complete"], false);
    assert_eq!(active["evaluation"]["needs"].as_array().unwrap().len(), 1);
    assert_eq!(active["evaluation"]["needs"][0]["resource"], "history");
    assert_eq!(active["evaluation"]["diagnostics"], json!([]));
    let (_, condition) = fixture.preview(draft(&strategy), json!({})).await;
    assert_eq!(
        condition["evaluation"]["needs"].as_array().unwrap().len(),
        1
    );
    assert_eq!(condition["evaluation"]["needs"][0]["resource"], "enabled");
    let (_, invalid) = fixture
        .preview(draft(&strategy), json!({"enabled":true,"history":42}))
        .await;
    assert_eq!(invalid["evaluation"]["complete"], false);
    assert_eq!(
        invalid["evaluation"]["diagnostics"][0]["code"],
        "value_type"
    );
    assert_eq!(
        context_files(&fixture.workspace_a),
        fixture.initial_context_a
    );
    fixture.assert_no_runs().await;
}

#[tokio::test]
async fn binding_checks_resource_types_and_capabilities_without_granting_or_invoking_them() {
    let fixture = Fixture::new().await;
    let strategy = ContextStrategy::new("typed", "Typé")
        .require(
            "record",
            DataType::Record {
                fields: BTreeMap::from([("text".into(), DataType::Text)]),
            },
        )
        .capability(ContextCapability::new(
            "search",
            DataType::Text,
            DataType::Text,
        ))
        .with_program(vec![text(
            "record",
            ContextExpr::field(ContextExpr::resource("record"), "text"),
        )]);
    let mut payload = json!({"workspaceId":fixture.a,"selection":draft(&strategy),"resources":{"record":{"kind":"record","fields":{"text":{"kind":"text"},"extra":{"kind":"number"}}}}});
    let (status, denied) = request(
        &fixture.app,
        "POST",
        "/api/context-strategies/validate",
        Some(payload.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_diagnostic(&denied, "capability_not_granted", "capabilities.search");
    let (status, preview) = fixture
        .preview(draft(&strategy), json!({"record":{"text":"ok"}}))
        .await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(preview["evaluation"]["complete"], true);
    assert_eq!(preview["evaluation"]["capabilities"][0]["id"], "search");
    fixture.assert_no_runs().await;
    payload["grantedCapabilities"] = json!(["search"]);
    let (status, valid) = request(
        &fixture.app,
        "POST",
        "/api/context-strategies/validate",
        Some(payload.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{valid}");
    assert_eq!(valid["valid"], true);
    payload["resources"]["record"] = json!({"kind":"number"});
    let (status, incompatible) = request(
        &fixture.app,
        "POST",
        "/api/context-strategies/validate",
        Some(payload.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_diagnostic(&incompatible, "resource_type", "requirements.record");
    payload["resources"] = json!({});
    let (status, absent) = request(
        &fixture.app,
        "POST",
        "/api/context-strategies/validate",
        Some(payload),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_diagnostic(&absent, "resource_binding", "requirements.record");
    let (status, preview) = request(&fixture.app, "POST", "/api/context-strategies/preview", Some(json!({"workspaceId":fixture.a,"selection":draft(&strategy),"grantedCapabilities":["search"],"resources":{"record":{"text":"supplied"}}}))).await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(preview["evaluation"]["complete"], true);
    assert_eq!(preview["evaluation"]["capabilities"][0]["id"], "search");
    assert_eq!(preview["evaluation"]["items"][0]["value"], "supplied");
    assert_eq!(
        context_files(&fixture.workspace_a),
        fixture.initial_context_a
    );
    fixture.assert_no_runs().await;
}

#[tokio::test]
async fn resolution_binds_exact_strategy_source_and_reports_node_local_incompatibility() {
    let fixture = Fixture::new().await;
    let strategy = simple_strategy();
    let saved = fixture.save(&strategy).await;
    let mut payload = json!({
        "workspaceId":fixture.a,
        "catalog":{"flows":{"worker":{
            "entries":{"start":{"input":{"kind":"text"}}},
            "data":{"history":{"dataType":{"kind":"text"},"permissions":{"read":true}}},
            "inferenceNodes":{"agent":{"model":{"kind":"runtime"},"resources":["history"],"contextStrategy":"conversation"}}
        }}},
        "request":{"flow":"worker","entry":"start"}
    });
    let (status, resolved) = request(
        &fixture.app,
        "POST",
        "/api/runtime-graphs/resolve",
        Some(payload.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{resolved}");
    assert_eq!(resolved["stage"], "resolved");
    assert_eq!(
        resolved["contexts"]["root/agent"]["source"],
        saved["source"]
    );
    assert_eq!(resolved["contexts"]["root/agent"]["hash"], saved["hash"]);
    payload["catalog"]["flows"]["worker"]["data"]["history"]["dataType"] = json!({"kind":"number"});
    let (status, incompatible) = request(
        &fixture.app,
        "POST",
        "/api/runtime-graphs/resolve",
        Some(payload.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_diagnostic(
        &incompatible,
        "resource_type",
        "contexts.root/agent.requirements.history",
    );
    payload["catalog"]["flows"]["worker"]["data"]["history"]["dataType"] = json!({"kind":"text"});
    let mut changed = strategy;
    changed.name = "Brouillon pour cette résolution".into();
    payload["contexts"] = json!({"root/agent":draft(&changed)});
    let (status, draft_resolution) = request(
        &fixture.app,
        "POST",
        "/api/runtime-graphs/resolve",
        Some(payload.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{draft_resolution}");
    assert_eq!(
        draft_resolution["contexts"]["root/agent"]["strategy"]["name"],
        changed.name
    );
    assert_eq!(
        std::fs::read_to_string(saved["path"].as_str().unwrap()).unwrap(),
        saved["source"]
    );
    payload["contexts"] = json!({"root/missing":draft(&changed)});
    let (status, missing) = request(
        &fixture.app,
        "POST",
        "/api/runtime-graphs/resolve",
        Some(payload),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_diagnostic(&missing, "inference_missing", "contexts.root/missing");
    fixture.assert_no_runs().await;
}

#[tokio::test]
async fn unknown_workspace_never_falls_back_to_default_catalog_or_preview_scope() {
    let fixture = Fixture::new().await;
    let strategy = simple_strategy();
    fixture.save(&strategy).await;
    for path in [
        "/api/context-strategies",
        "/api/context-strategies/conversation",
    ] {
        let (status, _) = request(
            &fixture.app,
            "GET",
            &format!("{path}?workspaceId=unknown"),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
    for (path, body) in [
        (
            "/api/context-strategies",
            json!({"strategy":strategy,"workspaceId":"unknown"}),
        ),
        (
            "/api/context-strategies/preview",
            json!({"selection":draft(&strategy),"workspaceId":"unknown"}),
        ),
        (
            "/api/context-strategies/validate",
            json!({"selection":draft(&strategy),"workspaceId":"unknown"}),
        ),
    ] {
        assert_eq!(
            request(&fixture.app, "POST", path, Some(body)).await.0,
            StatusCode::NOT_FOUND
        );
    }
    fixture.assert_no_runs().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn deep_schemas_save_reopen_preview_and_reject_excess_without_aborting() {
    fn nested(depth: usize) -> (DataType, Value) {
        let mut data_type = DataType::Text;
        let mut value = json!("Valeur profonde");
        for index in (1..=depth).rev() {
            let key = format!("level_{index}");
            data_type = DataType::Record {
                fields: BTreeMap::from([(key.clone(), data_type)]),
            };
            value = json!({key: value});
        }
        (data_type, value)
    }
    let fixture = Fixture::new().await;
    for depth in [32, 64] {
        let (data_type, value) = nested(depth);
        let strategy = ContextStrategy::new_v2(&format!("deep-{depth}"), "Schéma profond")
            .require("deep", data_type)
            .with_program(vec![ContextBlock::emit(
                "deep-value",
                FragmentRole::Data,
                FragmentFormat::Json,
                ContextExpr::resource("deep"),
            )]);
        let file = fixture.save(&strategy).await;
        assert_eq!(file["strategy"], json!(strategy));
        let (status, reopened) = request(
            &fixture.app,
            "GET",
            &format!(
                "/api/context-strategies/{}?workspaceId={}",
                strategy.id, fixture.a
            ),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{reopened}");
        assert_eq!(reopened["source"], file["source"]);
        assert_eq!(reopened["strategy"], file["strategy"]);
        let (status, preview) = fixture
            .preview(
                json!({"kind":"file","key":file["key"],"hash":file["hash"]}),
                json!({"deep":value}),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{preview}");
        assert_eq!(preview["evaluation"]["complete"], true, "{preview}");
        assert_eq!(preview["evaluation"]["items"][0]["value"], value);
    }
    let strategy = ContextStrategy::new_v2("too-deep", "Too deep").require("deep", nested(65).0);
    let (status, rejected) = request(
        &fixture.app,
        "POST",
        "/api/context-strategies",
        Some(json!({"workspaceId":fixture.a,"strategy":strategy})),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{rejected}");
    assert!(
        rejected["diagnostics"]
            .as_array()
            .is_some_and(|items| !items.is_empty())
    );
    assert!(
        !fixture
            .workspace_a
            .join(".zedflow/context/too-deep.rs")
            .exists()
    );
    fixture.assert_no_runs().await;
    let (status, catalog) = request(
        &fixture.app,
        "GET",
        &format!("/api/context-source-types?workspaceId={}", fixture.a),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{catalog}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn deep_frozen_strategy_remains_usable_through_flow_authoring_transport() {
    let fixture = Fixture::new().await;
    let data_type = (0..64).fold(DataType::Text, |item, _| DataType::Record {
        fields: BTreeMap::from([("child".into(), item)]),
    });
    let strategy = ContextStrategy::new_v2("deep-flow", "Deep flow").require("deep", data_type);
    let stored = fixture.save(&strategy).await;
    let node = |id: &str, kind: &str, config: Value| json!({"id":id,"type":"custom","position":{"x":0,"y":0},"data":{"label":id,"kind":kind,"config":config}});
    let composition = json!({"formatVersion":3,"id":"deep-flow","name":"Deep flow","revision":0,
        "nodes":[node("start","start",json!({})),
            node("context","context",json!({"modelNode":"model","fanIn":"any","contextProgram":{
                "strategy":strategy,"source":stored["source"],"hash":stored["hash"],
                "types":{},"bindings":{"deep":{"kind":"state","field":"input"}}
            }})),
            node("model","model",json!({"contextNode":"context","provider":"fixture","fixtureSteps":[{"text":"deep fixture"}]})),
            node("end","end",json!({}))],
        "edges":[{"id":"a","source":"start","target":"context"},{"id":"b","source":"context","target":"model"},{"id":"c","source":"model","target":"end"}]
    });
    let (status, validated) = request(
        &fixture.app,
        "POST",
        "/api/validate",
        Some(composition.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{validated}");
    let (status, saved) = request(
        &fixture.app,
        "POST",
        "/api/flows",
        Some(json!({"workspaceId":fixture.a,"composition":composition})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    let (status, reopened) = request(
        &fixture.app,
        "GET",
        &format!(
            "/api/flows/{}?workspaceId={}",
            saved["key"].as_str().unwrap(),
            fixture.a
        ),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{reopened}");
    assert_eq!(reopened["source"], saved["source"]);
    assert_eq!(
        reopened["composition"]["nodes"][1]["data"]["config"]["contextProgram"]["strategy"],
        serde_json::to_value(&strategy).unwrap()
    );
    let (status, exported) = request(
        &fixture.app,
        "POST",
        "/api/generate",
        Some(json!({"workspaceId":fixture.a,"composition":composition})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{exported}");
    let source = exported["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|file| file["path"] == "flows/instance-0/flow.rs")
        .unwrap();
    let restored = zf_flows::flow_source::parse(
        source["content"].as_str().unwrap(),
        &zf_compiler::graph_compiler::GraphValidator::new(
            &zf_runtime::materialize::RuntimePrimitives,
        ),
    )
    .unwrap();
    assert_eq!(
        restored.nodes[1].data.config["contextProgram"]["strategy"],
        serde_json::to_value(&strategy).unwrap()
    );
    fixture.assert_no_runs().await;
}
