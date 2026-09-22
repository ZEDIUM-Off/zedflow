use serde_json::{Value, json};
use sqlx::{Connection, Executor};
use std::{collections::BTreeMap, sync::Arc};
use tokio_util::sync::CancellationToken;
use zf_context::resource_readers::ReaderInput;
use zf_context::resource_readers::ReaderRegistry;
use zf_core::types::DataType;
use zf_core::types::TypeRegistry;
use zf_runtime::resources::ResourceReads;
use zf_storage::content_store::ContentStore;

#[tokio::test]
async fn host_extensions_are_explicit_versioned_and_checked_against_the_declared_type() {
    use zf_context::resource_readers::ReadResource;
    use zf_context::resource_readers::ReaderContract;
    use zf_context::resource_readers::ReaderOutput;
    use zf_context::resource_readers::ResourceReader;
    struct HostReader;
    impl ResourceReader for HostReader {
        fn contract(&self) -> ReaderContract {
            ReaderContract {
                id: "host.convert".into(),
                version: "2026-09".into(),
                input: DataType::Text,
                output: ReaderOutput::Fixed {
                    data_type: DataType::Text,
                },
            }
        }
        fn read<'a>(
            &'a self,
            input: &'a Value,
        ) -> futures::future::BoxFuture<'a, anyhow::Result<Option<ReadResource>>> {
            Box::pin(async move {
                Ok(Some(ReadResource {
                    value: Arc::new(input.clone()),
                    provenance: json!({"native":"explicitly registered"}),
                }))
            })
        }
    }
    let mut registry = ReaderRegistry::new();
    assert!(!registry.contains("file.text"));
    registry.register(Arc::new(HostReader)).unwrap();
    assert!(registry.register(Arc::new(HostReader)).is_err());
    let cancel = CancellationToken::new();
    let reads = ResourceReads::new(None, cancel.clone());
    let result = reads
        .read(
            &registry,
            "host.convert",
            &json!("{{exact literal}}"),
            &DataType::Text,
            &BTreeMap::new(),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(*result.value, json!("{{exact literal}}"));
    assert_eq!(result.provenance["reader"]["version"], "2026-09");
    assert!(
        reads
            .read(
                &registry,
                "host.convert",
                &json!(42),
                &DataType::Text,
                &BTreeMap::new()
            )
            .await
            .is_err()
    );
    assert!(
        reads
            .read(
                &registry,
                "host.convert",
                &json!("42"),
                &DataType::Number,
                &BTreeMap::new()
            )
            .await
            .is_err()
    );
}

#[tokio::test]
async fn explicit_markdown_json_and_database_sources_share_values_with_distinct_provenance() {
    let root = tempfile::tempdir().unwrap();
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    let store = ContentStore::new(pool).await.unwrap();
    let cancel = CancellationToken::new();
    let reads = ResourceReads::new(Some(store.clone()), cancel.clone());
    let registry = reads.native_readers(root.path().into()).unwrap();
    let markdown = "# Canonical source\n\n{{literal}} → UTF-8\n";
    std::fs::write(root.path().join("guide.md"), markdown).unwrap();
    let read = reads
        .read(
            &registry,
            "file.text",
            &json!({"path":"guide.md"}),
            &DataType::Text,
            &BTreeMap::new(),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(*read.value, json!(markdown));
    let value = json!({"title":"same data","count":3});
    std::fs::write(root.path().join("report.json"), value.to_string()).unwrap();
    let types = TypeRegistry::from([(
        "Report".into(),
        DataType::Record {
            fields: BTreeMap::from([
                ("title".into(), DataType::Text),
                ("count".into(), DataType::Number),
            ]),
        },
    )]);
    let expected = DataType::Named {
        name: "Report".into(),
    };
    let file = reads
        .read(
            &registry,
            "file.json",
            &json!({"path":"report.json"}),
            &expected,
            &types,
        )
        .await
        .unwrap()
        .unwrap();
    let mut connection = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(root.path().join("resources.db"))
            .create_if_missing(true),
    )
    .await
    .unwrap();
    connection
        .execute("CREATE TABLE documents(id TEXT PRIMARY KEY, document TEXT)")
        .await
        .unwrap();
    sqlx::query("INSERT INTO documents VALUES(?,?)")
        .bind("report")
        .bind(value.to_string())
        .execute(&mut connection)
        .await
        .unwrap();
    sqlx::query("INSERT INTO documents VALUES('null',NULL)")
        .execute(&mut connection)
        .await
        .unwrap();
    connection
        .execute("CREATE TABLE ambiguous(id TEXT, document TEXT)")
        .await
        .unwrap();
    connection
        .execute("INSERT INTO ambiguous VALUES('same','{}'),('same','{}')")
        .await
        .unwrap();
    connection.close().await.unwrap();
    let args = json!({"path":"resources.db","table":"documents","id":"report"});
    let db = reads
        .read(&registry, "sqlite.json", &args, &expected, &types)
        .await
        .unwrap()
        .unwrap();
    assert!(Arc::ptr_eq(&file.value, &db.value));
    assert_ne!(file.provenance["source"], db.provenance["source"]);
    assert_eq!(file.provenance["contentRef"], db.provenance["contentRef"]);
    let mut missing = args.clone();
    missing["id"] = json!("missing");
    assert!(
        reads
            .read(&registry, "sqlite.json", &missing, &expected, &types)
            .await
            .unwrap()
            .is_none()
    );
    missing["id"] = json!("null");
    assert!(
        reads
            .read(&registry, "sqlite.json", &missing, &expected, &types)
            .await
            .is_err()
    );
    missing["path"] = json!("must-not-create.db");
    assert!(
        reads
            .read(&registry, "sqlite.json", &missing, &expected, &types)
            .await
            .unwrap()
            .is_none()
    );
    assert!(!root.path().join("must-not-create.db").exists());
    let mut invalid = args;
    invalid["table"] = json!("ambiguous");
    invalid["id"] = json!("same");
    assert!(
        reads
            .read(
                &registry,
                "sqlite.json",
                &invalid,
                &DataType::Record {
                    fields: BTreeMap::new()
                },
                &types
            )
            .await
            .unwrap_err()
            .to_string()
            .contains("ambiguous")
    );
    invalid["table"] = json!("documents; DROP TABLE documents");
    assert!(
        reads
            .read(&registry, "sqlite.json", &invalid, &expected, &types)
            .await
            .is_err()
    );
    std::fs::write(
        root.path().join("report.json"),
        r#"{"title":"bad","count":"3"}"#,
    )
    .unwrap();
    assert!(
        reads
            .read(
                &registry,
                "file.json",
                &json!({"path":"report.json"}),
                &expected,
                &types
            )
            .await
            .is_err()
    );
}

#[tokio::test]
async fn content_reader_resolves_full_output_and_state_input_is_exact() {
    let root = tempfile::tempdir().unwrap();
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    let store = ContentStore::new(pool).await.unwrap();
    let bytes = "long tool result\n".repeat(10_000);
    use base64::Engine;
    let reference = store.intern(&json!({"encoding":"base64","byteLength":bytes.len(),"chunks":[base64::engine::general_purpose::STANDARD.encode(&bytes)]})).await.unwrap();
    let input = ReaderInput::State {
        field: "lastTool".into(),
        pointer: Some("/source".into()),
    };
    let args = input
        .capture(&std::collections::HashMap::from([(
            "lastTool".into(),
            json!({"preview":"short","source":{"contentRef":reference}}),
        )]))
        .unwrap()
        .unwrap();
    let cancel = CancellationToken::new();
    let reads = ResourceReads::new(Some(store.clone()), cancel.clone());
    let registry = reads.native_readers(root.path().into()).unwrap();
    let read = reads
        .read(
            &registry,
            "content.text",
            &args,
            &DataType::Text,
            &BTreeMap::new(),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(*read.value, json!(bytes));
    assert_eq!(read.provenance["source"]["contentRef"], reference);
    assert!(input.capture(&Default::default()).unwrap().is_none());
    assert!(
        ReaderInput::State {
            field: "x".into(),
            pointer: Some("not-a-pointer".into())
        }
        .validate()
        .is_err()
    );
    assert!(
        reads
            .read(
                &registry,
                "plugin.missing",
                &Value::Null,
                &DataType::Text,
                &BTreeMap::new()
            )
            .await
            .is_err()
    );
}

#[cfg(unix)]
#[tokio::test]
async fn special_sources_are_rejected_without_opening_a_fifo() {
    let root = tempfile::tempdir().unwrap();
    let fifo = root.path().join("pipe");
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap()
            .success()
    );
    let cancel = CancellationToken::new();
    let reads = ResourceReads::new(None, cancel.clone());
    let registry = reads.native_readers(root.path().into()).unwrap();
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        reads.read(
            &registry,
            "file.text",
            &json!({"path":"pipe"}),
            &DataType::Text,
            &BTreeMap::new(),
        ),
    )
    .await
    .expect("Opening FIFO must not block");
    assert!(result.unwrap_err().to_string().contains("regular file"));
}
