//! Authoring schemas, not ambient resources or runtime grants. Every selected
//! entry still needs an explicit value in preview or a binding in its flow.
use crate::context_store::{ContextStore, TypeStore};
use crate::{flow_store::FlowStore, workspaces::Workspace};
use anyhow::Result;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use zf_context::resource_readers::{ReaderOutput, standard_contracts};
use zf_core::types::{DataType, Diagnostic, TypeRegistry, validate_type};
use zf_flows::flow_contract;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceTypeEntry {
    pub id: String,
    pub label: String,
    pub category: String,
    #[serde(rename = "type")]
    pub data_type: DataType,
    pub types: TypeRegistry,
    pub origin: String,
    pub providers: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct SourceCatalog {
    pub entries: Vec<SourceTypeEntry>,
    pub diagnostics: Vec<Diagnostic>,
}

fn record(fields: &[(&str, DataType)]) -> DataType {
    DataType::Record {
        fields: fields
            .iter()
            .map(|(name, ty)| ((*name).into(), ty.clone()))
            .collect(),
    }
}
fn named(name: &str) -> DataType {
    DataType::Named { name: name.into() }
}

fn builtins() -> Vec<SourceTypeEntry> {
    let text = DataType::Text;
    let object = record(&[]);
    let definitions = [
        (
            "route-result",
            "Résultat de routage",
            "RouteResult",
            "execution",
            text.clone(),
        ),
        (
            "instructions",
            "Instructions",
            "Instructions",
            "documents",
            record(&[("content", text.clone()), ("origin", text.clone())]),
        ),
        (
            "user-message",
            "Message utilisateur",
            "UserMessage",
            "messages",
            record(&[("content", text.clone())]),
        ),
        (
            "model-output",
            "Sortie du modèle",
            "ModelOutput",
            "messages",
            record(&[
                ("content", text.clone()),
                ("model", text.clone()),
                ("usage", object.clone()),
            ]),
        ),
        (
            "tool-definition",
            "Définition d’outil",
            "ToolDefinition",
            "tools",
            record(&[
                ("name", text.clone()),
                ("description", text.clone()),
                ("parameters", object.clone()),
            ]),
        ),
        (
            "tool-call",
            "Appel d’outil",
            "ToolCall",
            "tools",
            record(&[
                ("id", text.clone()),
                ("name", text.clone()),
                ("arguments", object.clone()),
            ]),
        ),
        (
            "tool-result",
            "Résultat d’outil",
            "ToolResult",
            "tools",
            record(&[
                ("callId", text.clone()),
                ("status", text.clone()),
                ("content", text.clone()),
            ]),
        ),
        (
            "document",
            "Document",
            "Document",
            "documents",
            record(&[
                ("title", text.clone()),
                ("content", text.clone()),
                ("path", text.clone()),
            ]),
        ),
        (
            "skill",
            "Skill",
            "Skill",
            "documents",
            record(&[
                ("name", text.clone()),
                ("description", text.clone()),
                ("content", text.clone()),
            ]),
        ),
        (
            "passage",
            "Passage de nœud",
            "Passage",
            "execution",
            record(&[
                ("nodePath", text.clone()),
                ("status", text.clone()),
                ("output", object.clone()),
            ]),
        ),
        (
            "run",
            "Exécution",
            "Run",
            "execution",
            record(&[("id", text.clone()), ("status", text.clone())]),
        ),
        ("state", "État", "State", "execution", object.clone()),
    ];
    let mut entries: Vec<_> = definitions
        .into_iter()
        .map(|(id, label, name, category, ty)| SourceTypeEntry {
            id: format!("builtin:{id}"),
            label: label.into(),
            category: category.into(),
            data_type: named(name),
            types: BTreeMap::from([(name.into(), ty)]),
            origin: "Type constructible · liaison explicite dans le flow".into(),
            providers: vec!["Valeur ou projection explicitement liée".into()],
        })
        .collect();
    for (id, label, category, ty) in [
        ("text", "Texte", "data", DataType::Text),
        ("number", "Nombre", "data", DataType::Number),
        ("boolean", "Booléen", "data", DataType::Boolean),
        ("record", "Objet structuré", "data", object),
        (
            "list",
            "Liste de textes",
            "data",
            DataType::List {
                item: Box::new(DataType::Text),
            },
        ),
        (
            "image",
            "Image",
            "media",
            DataType::Media {
                media_type: "image".into(),
            },
        ),
        (
            "audio",
            "Audio",
            "media",
            DataType::Media {
                media_type: "audio".into(),
            },
        ),
        (
            "video",
            "Vidéo",
            "media",
            DataType::Media {
                media_type: "video".into(),
            },
        ),
    ] {
        entries.push(SourceTypeEntry {
            id: format!("builtin:{id}"),
            label: label.into(),
            category: category.into(),
            data_type: ty,
            types: TypeRegistry::new(),
            origin: "Type de donnée".into(),
            providers: vec!["Liaison explicite dans le flow".into()],
        });
    }
    entries
}

/// Retain only the selected type and its transitive named dependencies.
pub(super) fn dependencies(data_type: &DataType, registry: &TypeRegistry) -> TypeRegistry {
    let mut result = TypeRegistry::new();
    let mut pending = vec![data_type];
    while let Some(data_type) = pending.pop() {
        match data_type {
            DataType::Named { name } => {
                if !result.contains_key(name)
                    && let Some(value) = registry.get(name)
                {
                    result.insert(name.clone(), value.clone());
                    pending.push(value);
                }
            }
            DataType::Record { fields } => pending.extend(fields.values()),
            DataType::List { item } => pending.push(item),
            _ => {}
        }
    }
    result
}

pub async fn collect(store: &FlowStore, workspace: &Workspace) -> Result<SourceCatalog> {
    let mut catalog = SourceCatalog {
        entries: builtins(),
        diagnostics: vec![],
    };
    let mut registries: Vec<(String, TypeRegistry)> = catalog
        .entries
        .iter()
        .map(|entry| (entry.id.clone(), entry.types.clone()))
        .collect();
    for file in TypeStore::new(workspace.path.clone()).list().await? {
        catalog.diagnostics.extend(file.diagnostics);
        if let Some(types) = file.types {
            for name in types.keys() {
                catalog.entries.push(SourceTypeEntry {
                    id: format!("catalog:{}:{name}", file.key),
                    label: name.clone(),
                    category: "custom".into(),
                    data_type: named(name),
                    types: types.clone(),
                    origin: file.path.display().to_string(),
                    providers: vec![format!("Catalogue {}", file.key)],
                });
            }
            registries.push((format!("catalog:{}", file.key), types));
        }
    }
    for file in ContextStore::new(workspace.path.clone()).list().await? {
        if let Some(strategy) = file.strategy {
            for name in strategy.types.keys() {
                catalog.entries.push(SourceTypeEntry {
                    id: format!("strategy:{}:{name}", file.key),
                    label: name.clone(),
                    category: "custom".into(),
                    data_type: named(name),
                    types: strategy.types.clone(),
                    origin: file.path.display().to_string(),
                    providers: vec![format!("Stratégie {}", strategy.name)],
                });
            }
            registries.push((format!("strategy:{}", file.key), strategy.types));
        }
    }
    for file in store.list(workspace).await? {
        let Some(doc) = file.composition else {
            continue;
        };
        let exports = match flow_contract::read(&doc) {
            Ok(Some(exports)) => exports,
            Ok(None) => continue,
            Err(error) => {
                catalog.diagnostics.push(Diagnostic::new(
                    "flow_source_types",
                    file.key,
                    error.to_string(),
                ));
                continue;
            }
        };
        for (name, data) in exports.contract.data {
            if !data.permissions.read {
                continue;
            }
            catalog.entries.push(SourceTypeEntry {
                id: format!("flow:{}:data:{name}", file.key),
                label: format!("{} · {name}", file.name),
                category: "execution".into(),
                data_type: data.data_type,
                types: exports.types.clone(),
                origin: file.path.display().to_string(),
                providers: vec![format!("{} · donnée publique {name}", file.name)],
            });
        }
        for name in exports.types.keys() {
            catalog.entries.push(SourceTypeEntry {
                id: format!("flow:{}:type:{name}", file.key),
                label: name.clone(),
                category: "custom".into(),
                data_type: named(name),
                types: exports.types.clone(),
                origin: file.path.display().to_string(),
                providers: vec![format!("Flow {}", file.name)],
            });
        }
        registries.push((format!("flow:{}", file.key), exports.types));
    }
    for reader in standard_contracts() {
        if let ReaderOutput::Fixed { data_type } = reader.output {
            catalog.entries.push(SourceTypeEntry {
                id: format!("reader:{}", reader.id),
                label: reader.id.clone(),
                category: "data".into(),
                data_type,
                types: TypeRegistry::new(),
                origin: format!("Lecteur natif {} · {}", reader.id, reader.version),
                providers: vec![reader.id],
            });
        }
    }
    let mut common = TypeRegistry::new();
    let mut origins: BTreeMap<String, String> = BTreeMap::new();
    let mut conflicting = BTreeSet::new();
    for (origin, types) in registries {
        for (name, ty) in types {
            if common.get(&name).is_some_and(|existing| existing != &ty) {
                conflicting.insert(name.clone());
                catalog.diagnostics.push(Diagnostic::new("type_identity_conflict", format!("types.{name}"), format!("Le type {name} diffère entre {} et {origin}. Choisissez une définition avant de les composer.", origins[&name])));
            } else {
                origins
                    .entry(name.clone())
                    .or_insert_with(|| origin.clone());
                common.insert(name, ty);
            }
        }
    }
    for name in &conflicting {
        common.remove(name);
    }
    catalog.entries.retain_mut(|entry| {
        let mut registry = common.clone();
        // A catalog's own declaration wins only inside that individual entry.
        // Conflicting entries remain separate and importing both is rejected.
        registry.extend(entry.types.clone());
        if let Err(errors) = validate_type(&entry.data_type, &registry) {
            catalog
                .diagnostics
                .extend(errors.into_iter().map(|mut error| {
                    error.path = format!("entries.{}:{}", entry.id, error.path);
                    error
                }));
            return false;
        }
        entry.types = dependencies(&entry.data_type, &registry);
        true
    });
    Ok(catalog)
}

/// Workspace examples keyed by type identity and its complete schema.
pub mod examples {
    use crate::context_store;
    use anyhow::{Context, Result, ensure};
    use serde_json::Value;
    use std::{
        fs::{self, File, OpenOptions},
        io::{Read, Write},
        path::PathBuf,
    };
    use zf_context::type_examples::{TypeExample, builtin, identity, parse};
    use zf_core::types::{DataType, TypeRegistry, validate_value};
    pub async fn catalog(workspace: PathBuf) -> Result<Vec<context_store::SourceFile>> {
        tokio::task::spawn_blocking(move || {
            let Some(_guard) = context_store::workspace_lock(&workspace, false, false)? else {
                return Ok(vec![]);
            };
            let Some(directory) =
                context_store::directory(&workspace, &["examples".into()], false)?
            else {
                return Ok(vec![]);
            };
            let mut files = vec![];
            for entry in fs::read_dir(directory)? {
                let path = entry?.path();
                if path.extension().is_none_or(|extension| extension != "json") {
                    continue;
                }
                let key = path
                    .file_stem()
                    .and_then(|name| name.to_str())
                    .context("Invalid example filename")?
                    .to_owned();
                let mut options = OpenOptions::new();
                options.read(true);
                context_store::no_follow(&mut options);
                let mut bytes = vec![];
                options
                    .open(&path)
                    .with_context(|| format!("Open example {}", path.display()))?
                    .take(1024 * 1024 + 1)
                    .read_to_end(&mut bytes)
                    .with_context(|| format!("Read example {}", path.display()))?;
                ensure!(bytes.len() <= 1024 * 1024, "Example exceeds 1 MiB");
                let source = String::from_utf8(bytes)?;
                let validation = parse(&source).and_then(|example| {
                    ensure!(
                        example.id == key,
                        "Example identity does not match its filename"
                    );
                    Ok(())
                });
                let diagnostics = validation
                    .err()
                    .map(|error| {
                        vec![zf_core::diagnostics::Diagnostic::new(
                            "example",
                            &key,
                            format!("{error:#}"),
                        )]
                    })
                    .unwrap_or_default();
                files.push(context_store::SourceFile {
                    key,
                    path,
                    hash: context_store::hash(source.as_bytes()),
                    source: Some(source),
                    diagnostics,
                });
            }
            files.sort_by(|a, b| a.key.cmp(&b.key));
            Ok(files)
        })
        .await?
    }
    pub async fn list(
        workspace: PathBuf,
        data_type: DataType,
        types: TypeRegistry,
    ) -> Result<Vec<TypeExample>> {
        let (hash, types) = identity(&data_type, &types)?;
        let mut examples = builtin(&data_type, &types)?.into_iter().collect::<Vec<_>>();
        for file in catalog(workspace).await? {
            ensure!(
                file.diagnostics.is_empty(),
                "Invalid example {}: {:?}",
                file.path.display(),
                file.diagnostics
            );
            let example = parse(file.source.as_deref().context("Example source absent")?)?;
            if example.schema_hash == hash {
                examples.push(example);
            }
        }
        examples.sort_by(|a, b| a.label.cmp(&b.label).then(a.id.cmp(&b.id)));
        Ok(examples)
    }

    pub async fn save(
        workspace: PathBuf,
        data_type: DataType,
        types: TypeRegistry,
        label: String,
        value: Value,
    ) -> Result<TypeExample> {
        ensure!(
            !label.trim().is_empty() && label.len() <= 200,
            "Example label required (200 bytes maximum)"
        );
        validate_value(&data_type, &value, &types).map_err(context_store::diagnostics_error)?;
        let (schema_hash, types) = identity(&data_type, &types)?;
        let example = TypeExample {
            version: 1,
            id: uuid::Uuid::new_v4().to_string(),
            label,
            schema_hash,
            data_type,
            types,
            value,
        };
        tokio::task::spawn_blocking(move || {
            let _lock = context_store::workspace_lock(&workspace, true, true)?
                .context("Workspace absent")?;
            let directory = context_store::directory(&workspace, &["examples".into()], true)?
                .context("Examples directory absent")?;
            let bytes = serde_json::to_vec_pretty(&example)?;
            ensure!(bytes.len() <= 1024 * 1024, "Example exceeds 1 MiB");
            let temporary = directory.join(format!(".{}.pending", example.id));
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            context_store::no_follow(&mut options);
            let mut file = options
                .open(&temporary)
                .with_context(|| format!("Create example {}", temporary.display()))?;
            file.write_all(&bytes)
                .with_context(|| format!("Write example {}", temporary.display()))?;
            file.sync_all()
                .with_context(|| format!("Synchronize example {}", temporary.display()))?;
            fs::rename(&temporary, directory.join(format!("{}.json", example.id)))
                .with_context(|| format!("Publish example {}", example.id))?;
            File::open(&directory)
                .and_then(|file| file.sync_all())
                .with_context(|| {
                    format!("Synchronize examples directory {}", directory.display())
                })?;
            Ok(example)
        })
        .await?
    }
}
