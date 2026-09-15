//! Native acquisition adapters bound explicitly to a flow directory.
//! Pure reader contracts and validation stay in zf-context; this host owns
//! cancellation, immutable content references and shared resident values.
use anyhow::{Context, Result, ensure};
use futures::future::BoxFuture;
use serde_json::{Value, json};
use sqlx::Connection;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, Weak},
};
use tokio::io::AsyncReadExt;
use tokio_util::sync::CancellationToken;
use zf_context::resource_readers::{
    ReadResource, ReaderContract, ReaderRegistry, ResourceReader, standard_contracts,
};
use zf_core::types::{DataType, TypeRegistry};
use zf_storage::content_store::ContentStore;

const MAX_BYTES: usize = 16 * 1024 * 1024;

/// A run's acquisition services. Clones share the resident cache; each flow
/// explicitly creates its registry with its own working directory.
#[derive(Clone)]
pub struct ResourceReads {
    content: Option<ContentStore>,
    cancel: CancellationToken,
    values: Arc<Mutex<BTreeMap<String, Weak<Value>>>>,
}
impl ResourceReads {
    pub fn new(content: Option<ContentStore>, cancel: CancellationToken) -> Self {
        Self {
            content,
            cancel,
            values: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    /// Installs native implementations only on explicit host request. Reading
    /// the authoring catalogue alone never opens resources or installs readers.
    pub fn native_readers(&self, cwd: PathBuf) -> Result<ReaderRegistry> {
        let mut registry = ReaderRegistry::new();
        for contract in standard_contracts() {
            let kind = match contract.id.as_str() {
                "file.text" => Builtin::FileText,
                "file.json" => Builtin::FileJson,
                "sqlite.json" => Builtin::SqliteJson,
                "content.text" => Builtin::ContentText,
                "content.json" => Builtin::ContentJson,
                id => anyhow::bail!("Native resource reader is not implemented: {id}"),
            };
            registry.register(Arc::new(NativeReader {
                kind,
                contract,
                cwd: cwd.clone(),
                content: self.content.clone(),
            }))?;
        }
        Ok(registry)
    }

    /// Validates via the pure registry before persisting anything. Extensions
    /// use this same boundary, so cancellation and provenance are not optional.
    pub async fn read(
        &self,
        registry: &ReaderRegistry,
        id: &str,
        input: &Value,
        expected: &DataType,
        types: &TypeRegistry,
    ) -> Result<Option<ReadResource>> {
        let result = tokio::select! { biased;
            _ = self.cancel.cancelled() => anyhow::bail!("Resource read cancelled"),
            result = registry.read(id, input, expected, types) => result?,
        };
        let Some(mut result) = result else {
            return Ok(None);
        };
        let (output_ref, input_ref) = if let Some(store) = &self.content {
            (
                Some(store.intern(&result.value).await?),
                Some(store.intern(input).await?),
            )
        } else {
            (None, None)
        };
        if let Some(reference) = &output_ref {
            let mut cache = self.values.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(value) = cache.get(reference).and_then(Weak::upgrade) {
                result.value = value;
            } else {
                if cache.len() >= 256 {
                    cache.retain(|_, value| value.strong_count() > 0);
                    if cache.len() >= 256 {
                        cache.pop_first();
                    }
                }
                cache.insert(reference.clone(), Arc::downgrade(&result.value));
            }
        }
        // The registry already wrapped native provenance; replace that one
        // envelope instead of nesting an extra reader use around it.
        let contract = result.provenance["reader"].take();
        let source = result.provenance["source"].take();
        result.provenance = json!({"kind":"reader","reader":contract,"inputRef":input_ref,
            "input":if input_ref.is_none(){Some(input)}else{None},"contentRef":output_ref,"source":source});
        Ok(Some(result))
    }
}

#[derive(Clone, Copy)]
enum Builtin {
    FileText,
    FileJson,
    SqliteJson,
    ContentText,
    ContentJson,
}
struct NativeReader {
    kind: Builtin,
    contract: ReaderContract,
    cwd: PathBuf,
    content: Option<ContentStore>,
}
impl ResourceReader for NativeReader {
    fn contract(&self) -> ReaderContract {
        self.contract.clone()
    }
    fn read<'a>(&'a self, input: &'a Value) -> BoxFuture<'a, Result<Option<ReadResource>>> {
        Box::pin(async move {
            let (value, provenance) = match self.kind {
                Builtin::FileText | Builtin::FileJson => {
                    let path = path(
                        &self.cwd,
                        input["path"].as_str().context("Reader path absent")?,
                    );
                    let Some(bytes) = file_bytes(&path).await? else {
                        return Ok(None);
                    };
                    let hash = byte_hash(&bytes);
                    let value = if matches!(self.kind, Builtin::FileText) {
                        Value::String(
                            String::from_utf8(bytes).context("Reader source is not UTF-8")?,
                        )
                    } else {
                        serde_json::from_slice(&bytes).context("Reader source is not JSON")?
                    };
                    (value, json!({"path":path,"hash":hash}))
                }
                Builtin::SqliteJson => {
                    let path = path(
                        &self.cwd,
                        input["path"].as_str().context("SQLite path absent")?,
                    );
                    match tokio::fs::metadata(&path).await {
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                        Err(e) => return Err(e.into()),
                        Ok(m) => ensure!(
                            m.is_file(),
                            "SQLite reader requires a regular database file"
                        ),
                    }
                    let table = input["table"].as_str().context("SQLite table absent")?;
                    ensure!(
                        !table.is_empty()
                            && table.len() <= 128
                            && table
                                .bytes()
                                .all(|c| c.is_ascii_alphanumeric() || c == b'_'),
                        "SQLite table must be an identifier"
                    );
                    let id = input["id"].as_str().context("SQLite row identity absent")?;
                    let mut connection = sqlx::SqliteConnection::connect_with(
                        &sqlx::sqlite::SqliteConnectOptions::new()
                            .filename(&path)
                            .read_only(true)
                            .create_if_missing(false),
                    )
                    .await?;
                    let query = format!(
                        "SELECT CASE WHEN length(CAST(document AS BLOB)) <= {MAX_BYTES} THEN document END FROM \"{table}\" WHERE id=? LIMIT 2"
                    );
                    let mut rows: Vec<Option<String>> = sqlx::query_scalar(&query)
                        .bind(id)
                        .fetch_all(&mut connection)
                        .await?;
                    connection.close().await?;
                    ensure!(rows.len() <= 1, "SQLite row identity is ambiguous");
                    let Some(text) = rows.pop() else {
                        return Ok(None);
                    };
                    let text = text.context("SQLite document is null or exceeds reader limit")?;
                    let value =
                        serde_json::from_str(&text).context("SQLite document is not JSON")?;
                    (
                        value,
                        json!({"path":path,"table":table,"id":id,"hash":byte_hash(text.as_bytes())}),
                    )
                }
                Builtin::ContentText | Builtin::ContentJson => {
                    let reference = input["contentRef"]
                        .as_str()
                        .context("Content reference absent")?;
                    let store = self.content.as_ref().context("Content store unavailable")?;
                    let max = if matches!(self.kind, Builtin::ContentJson) {
                        MAX_BYTES
                    } else {
                        MAX_BYTES * 2
                    };
                    let stored = store.resolve_with_limit(reference, max as u64).await?;
                    let value = if matches!(self.kind, Builtin::ContentJson) || stored.is_string() {
                        stored
                    } else {
                        Value::String(
                            String::from_utf8(zf_storage::content_store::decode_full_output(
                                &stored,
                            )?)
                            .context("Content is not UTF-8")?,
                        )
                    };
                    if let Value::String(text) = &value {
                        ensure!(text.len() <= MAX_BYTES, "Content text exceeds 16 MiB");
                    }
                    (value, json!({"contentRef":reference}))
                }
            };
            Ok(Some(ReadResource {
                value: Arc::new(value),
                provenance,
            }))
        })
    }
}
fn path(cwd: &Path, value: &str) -> PathBuf {
    let path = PathBuf::from(value);
    if path.is_absolute() {
        path
    } else {
        cwd.join(path)
    }
}
fn byte_hash(bytes: &[u8]) -> String {
    use sha2::Digest;
    format!("{:x}", sha2::Sha256::digest(bytes))
}
async fn file_bytes(path: &Path) -> Result<Option<Vec<u8>>> {
    let metadata = match tokio::fs::metadata(path).await {
        Ok(metadata) => metadata,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    ensure!(metadata.is_file(), "Reader source must be a regular file");
    let mut options = tokio::fs::OpenOptions::new();
    options.read(true);
    // Refuse special files before opening and avoid a blocking FIFO open if an
    // external writer swaps the path between metadata and open.
    #[cfg(unix)]
    options.custom_flags(nix::libc::O_NONBLOCK);
    let file = match options.open(path).await {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    let metadata = file.metadata().await?;
    ensure!(
        metadata.is_file() && metadata.len() <= MAX_BYTES as u64,
        "Reader source must be a regular file of at most 16 MiB"
    );
    let mut bytes = Vec::new();
    file.take(MAX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .await?;
    ensure!(bytes.len() <= MAX_BYTES, "Reader source exceeds 16 MiB");
    Ok(Some(bytes))
}

/// Captured state inputs are intentionally left to that invocation. This helper
/// performs metadata checks and never opens a database or reads a source body.
pub fn dependency_diagnostics(
    doc: &zf_flows::schema::Composition,
    cwd: &Path,
) -> Vec<zf_core::diagnostics::Diagnostic> {
    fn visit(
        doc: &zf_flows::schema::Composition,
        cwd: &Path,
        prefix: &str,
        out: &mut Vec<zf_core::diagnostics::Diagnostic>,
    ) {
        for node in &doc.nodes {
            let node_path = if prefix.is_empty() {
                node.id.clone()
            } else {
                format!("{prefix}/{}", node.id)
            };
            if node.data.kind == "subgraph" {
                if let Ok(child) = serde_json::from_value(node.data.config["composition"].clone()) {
                    visit(&child, cwd, &node_path, out);
                }
                continue;
            }
            let bindings = node.data.config["contextProgram"]
                .get("bindings")
                .unwrap_or(&node.data.config["contextBindings"]);
            for (name, binding) in bindings.as_object().into_iter().flatten() {
                if binding["kind"] != "reader"
                    || binding["input"]["kind"] != "literal"
                    || !matches!(
                        binding["reader"].as_str(),
                        Some("file.text" | "file.json" | "sqlite.json")
                    )
                {
                    continue;
                }
                let Some(source) = binding["input"]["value"]["path"].as_str() else {
                    continue;
                };
                let source = path(cwd, source);
                let reason = match std::fs::metadata(&source) {
                    Ok(metadata) if metadata.is_file() => continue,
                    Ok(_) => "source is not a regular file".into(),
                    Err(error) => error.to_string(),
                };
                out.push(zf_core::diagnostics::Diagnostic::new("reader_dependency",format!("{node_path}/bindings/{name}"),format!("{}: {reason}; required only if this resource is selected at the next invocation",source.display())));
            }
        }
    }
    let mut diagnostics = Vec::new();
    visit(doc, cwd, "", &mut diagnostics);
    diagnostics
}
