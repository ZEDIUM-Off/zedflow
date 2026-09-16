//! Explicit agent attachments, captured context and durable model-call grants.
use crate::{
    runtime::RunServices,
    workspace_context::{ContextSnapshot, Skill, read_skill},
};
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

fn enabled() -> bool {
    true
}
fn max_chars() -> usize {
    32_000
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Activation {
    #[default]
    Always,
    Explicit,
}
fn explicit() -> Activation {
    Activation::Explicit
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum InstructionMode {
    #[default]
    Literal,
    Template,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum InstructionSource {
    Text { text: String },
    File { path: PathBuf },
    Workspace,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum SkillSource {
    File { path: PathBuf },
    Workspace,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InstructionItem {
    pub id: String,
    #[serde(default = "enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub activation: Activation,
    pub source: InstructionSource,
    #[serde(default)]
    pub mode: InstructionMode,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SkillItem {
    pub id: String,
    #[serde(default = "enabled")]
    pub enabled: bool,
    #[serde(default = "explicit")]
    pub activation: Activation,
    pub source: SkillSource,
    pub name: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FileItem {
    pub id: String,
    #[serde(default = "enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub activation: Activation,
    pub path: PathBuf,
    pub start_line: Option<usize>,
    pub end_line: Option<usize>,
    #[serde(default = "max_chars")]
    pub max_chars: usize,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolItem {
    pub id: String,
    #[serde(default = "enabled")]
    pub enabled: bool,
    pub name: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Piece<T> {
    pub items: Vec<T>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Attachments {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<Piece<InstructionItem>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skills: Option<Piece<SkillItem>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files: Option<Piece<FileItem>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Piece<ToolItem>>,
}

pub fn is_v2(config: &Value) -> bool {
    config["__zedflowVersion"]
        .as_u64()
        .is_some_and(|version| version >= 2)
}

pub fn attachments(config: &Value) -> Result<Attachments> {
    let attachments: Attachments = match config.get("attachments") {
        None => Attachments::default(),
        Some(value) => {
            serde_json::from_value(value.clone()).context("Pièces de l’agent invalides")?
        }
    };
    let mut ids = HashSet::new();
    let mut check = |id: &str| -> Result<()> {
        ensure!(
            !id.is_empty() && id.len() <= 160 && !id.contains("::"),
            "Identifiant de ressource invalide"
        );
        ensure!(
            ids.insert(id.to_owned()),
            "Identifiant de ressource dupliqué : {id}"
        );
        ensure!(ids.len() <= 256, "256 ressources maximum par agent");
        Ok(())
    };
    for item in attachments
        .instructions
        .iter()
        .flat_map(|piece| &piece.items)
    {
        check(&item.id)?;
    }
    for item in attachments.skills.iter().flat_map(|piece| &piece.items) {
        check(&item.id)?;
    }
    for item in attachments.files.iter().flat_map(|piece| &piece.items) {
        check(&item.id)?;
        ensure!(
            (1..=1_048_576).contains(&item.max_chars),
            "maxChars doit être compris entre 1 et 1048576"
        );
        ensure!(
            item.start_line.is_none_or(|line| line > 0)
                && item
                    .end_line
                    .is_none_or(|line| line >= item.start_line.unwrap_or(1)),
            "Plage de lignes invalide"
        );
    }
    let declarations = crate::operations::tool_declarations();
    for item in attachments.tools.iter().flat_map(|piece| &piece.items) {
        check(&item.id)?;
        ensure!(
            declarations.contains_key(&item.name),
            "Outil non disponible : {}",
            item.name
        );
    }
    Ok(attachments)
}

pub fn tools(config: &Value) -> Result<Vec<String>> {
    let attachments = attachments(config)?;
    let mut tools: Vec<_> = attachments
        .tools
        .iter()
        .flat_map(|piece| &piece.items)
        .filter(|item| item.enabled)
        .map(|item| item.name.clone())
        .collect();
    tools.sort();
    tools.dedup();
    Ok(tools)
}

fn resolved_path(context: &ContextSnapshot, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_owned()
    } else {
        context.cwd.join(path)
    }
}

fn item_skills(item: &SkillItem, context: &ContextSnapshot) -> Result<Vec<Skill>> {
    let skills = match &item.source {
        SkillSource::Workspace => context.skills.clone(),
        SkillSource::File { path } => vec![read_skill(&resolved_path(context, path))?],
    };
    Ok(skills
        .into_iter()
        .filter(|skill| item.name.as_ref().is_none_or(|name| *name == skill.name))
        .collect())
}

pub fn activation_key(
    config: &Value,
    context: &ContextSnapshot,
    item_id: &str,
    skill_name: Option<&str>,
) -> Result<String> {
    let attachments = attachments(config)?;
    if attachments
        .instructions
        .iter()
        .flat_map(|piece| &piece.items)
        .any(|item| item.id == item_id && item.enabled)
        || attachments
            .files
            .iter()
            .flat_map(|piece| &piece.items)
            .any(|item| item.id == item_id && item.enabled)
    {
        ensure!(skill_name.is_none(), "Cette ressource n’est pas un skill");
        return Ok(item_id.into());
    }
    if let Some(item) = attachments
        .skills
        .iter()
        .flat_map(|piece| &piece.items)
        .find(|item| item.id == item_id && item.enabled)
    {
        let skills = item_skills(item, context)?;
        let matches: Vec<_> = skills
            .iter()
            .filter(|skill| skill_name.is_none_or(|name| name == skill.name))
            .collect();
        ensure!(
            matches.len() == 1,
            "Choisissez un skill autorisé de cette pièce"
        );
        return Ok(format!("{item_id}::{}", matches[0].name));
    }
    bail!("Ressource absente ou désactivée pour cet agent : {item_id}")
}

pub fn skill_activation_key(
    config: &Value,
    context: &ContextSnapshot,
    name: &str,
) -> Result<String> {
    let attachments = attachments(config)?;
    let mut matches = Vec::new();
    for item in attachments
        .skills
        .iter()
        .flat_map(|piece| &piece.items)
        .filter(|item| item.enabled)
    {
        for skill in item_skills(item, context)? {
            if skill.name == name {
                matches.push(format!("{}::{name}", item.id));
            }
        }
    }
    ensure!(
        matches.len() == 1,
        "Skill absent ou ambigu parmi les pièces de cet agent : {name}"
    );
    Ok(matches.remove(0))
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectiveContext {
    pub invocation_id: String,
    pub agent_path: String,
    pub origin: Value,
    pub tools: Vec<String>,
    pub system: String,
    pub files: String,
    pub resources: Vec<Value>,
    pub skill_catalog: Vec<Value>,
    /// The exact versioned context program and its evaluated window, when used.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prepared: Option<Value>,
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn read_text(path: &Path) -> Result<String> {
    let metadata = std::fs::metadata(path)?;
    ensure!(
        metadata.is_file(),
        "La source de contexte doit être un fichier ordinaire : {}",
        path.display()
    );
    ensure!(
        metadata.len() <= 1024 * 1024,
        "Fichier de contexte limité à 1 Mio : {}",
        path.display()
    );
    use std::io::Read;
    let mut text = String::new();
    std::fs::File::open(path)?
        .take(1024 * 1024 + 1)
        .read_to_string(&mut text)
        .with_context(|| format!("Lecture UTF-8 impossible : {}", path.display()))?;
    ensure!(
        text.len() <= 1024 * 1024,
        "Fichier de contexte limité à 1 Mio : {}",
        path.display()
    );
    Ok(text)
}

pub async fn capture(
    config: &Value,
    services: &RunServices,
    agent_path: &str,
    state: &HashMap<String, Value>,
) -> Result<EffectiveContext> {
    let result = collect(config, services, agent_path, state)?;
    persist_snapshot(services, &result).await?;
    Ok(result)
}

fn collect(
    config: &Value,
    services: &RunServices,
    agent_path: &str,
    state: &HashMap<String, Value>,
) -> Result<EffectiveContext> {
    let attachments = attachments(config)?;
    let active = services.active_capabilities(agent_path);
    let invocation_id = uuid::Uuid::new_v4().to_string();
    let origin = crate::runtime::current_origin()
        .unwrap_or_else(|| json!({"nodePath":agent_path,"occurrenceId":invocation_id}));
    let mut result = EffectiveContext {
        invocation_id,
        agent_path: agent_path.into(),
        origin,
        tools: tools(config)?,
        system: String::new(),
        files: String::new(),
        resources: vec![],
        skill_catalog: vec![],
        prepared: None,
    };
    let enabled_now = |id: &str, activation: Activation| {
        activation == Activation::Always || active.iter().any(|key| key == id)
    };
    for item in attachments
        .instructions
        .iter()
        .flat_map(|piece| &piece.items)
        .filter(|item| item.enabled && enabled_now(&item.id, item.activation))
    {
        let sources: Vec<(Option<PathBuf>, String)> = match &item.source {
            InstructionSource::Text { text } => vec![(None, text.clone())],
            InstructionSource::File { path } => {
                let path = resolved_path(&services.context, path);
                vec![(Some(path.clone()), read_text(&path)?)]
            }
            InstructionSource::Workspace => services
                .context
                .instructions
                .iter()
                .map(|source| (Some(source.path.clone()), source.content.clone()))
                .collect(),
        };
        for (path, content) in sources {
            let text = if matches!(item.mode, InstructionMode::Template) {
                crate::operations::render(&content, state)
            } else {
                content
            };
            if !result.system.is_empty() {
                result.system.push('\n');
            }
            result.system.push_str(&text);
            result.resources.push(json!({"id":item.id,"kind":"instructions","path":path,"content":text,"hash":digest(text.as_bytes()),"activation":item.activation,"truncated":false}));
        }
    }
    for item in attachments
        .skills
        .iter()
        .flat_map(|piece| &piece.items)
        .filter(|item| item.enabled)
    {
        for skill in item_skills(item, &services.context)? {
            let key = format!("{}::{}", item.id, skill.name);
            result.skill_catalog.push(json!({"itemId":item.id,"name":skill.name,"description":skill.description,"path":skill.path,"manualOnly":skill.manual_only,"activationKey":key}));
            if enabled_now(&key, item.activation) {
                let loaded = read_skill(&skill.path)?;
                ensure!(
                    loaded.name == skill.name,
                    "Le nom du skill a changé : {}",
                    skill.path.display()
                );
                let body = loaded.body.unwrap_or_default();
                let text = format!(
                    "<skill name={:?} path={:?}>\n{}\n</skill>",
                    loaded.name,
                    loaded.path.display().to_string(),
                    body
                );
                if !result.system.is_empty() {
                    result.system.push('\n');
                }
                result.system.push_str(&text);
                result.resources.push(json!({"id":item.id,"kind":"skills","name":loaded.name,"path":loaded.path,"content":body,"hash":digest(body.as_bytes()),"sourceHash":loaded.hash,"activation":item.activation,"truncated":false}));
            }
        }
    }
    if !result.skill_catalog.is_empty() {
        if !result.system.is_empty() {
            result.system.push('\n');
        }
        result.system.push_str("<available_skills>\n");
        for skill in &result.skill_catalog {
            result
                .system
                .push_str(&format!("{}\n", serde_json::to_string(skill)?));
        }
        result.system.push_str("</available_skills>");
    }
    for item in attachments
        .files
        .iter()
        .flat_map(|piece| &piece.items)
        .filter(|item| item.enabled && enabled_now(&item.id, item.activation))
    {
        let path = resolved_path(&services.context, &item.path);
        let raw = read_text(&path)?;
        let start = item.start_line.unwrap_or(1);
        let end = item.end_line.unwrap_or(usize::MAX);
        let selected = raw
            .split_inclusive('\n')
            .enumerate()
            .filter(|(index, _)| *index + 1 >= start && *index < end)
            .map(|(_, line)| line)
            .collect::<String>();
        let text: String = selected.chars().take(item.max_chars).collect();
        let truncated = text.len() != selected.len();
        result.files.push_str(&format!(
            "<file path={:?} startLine={start}>\n{text}\n</file>\n",
            path.display().to_string()
        ));
        result.resources.push(json!({"id":item.id,"kind":"files","path":path,"content":text,"hash":digest(text.as_bytes()),"sourceHash":digest(raw.as_bytes()),"startLine":start,"endLine":item.end_line,"activation":item.activation,"truncated":truncated}));
    }
    ensure!(
        result.system.len() + result.files.len() <= 2 * 1024 * 1024,
        "Contexte effectif limité à 2 Mio par invocation"
    );
    Ok(result)
}

pub async fn persist_snapshot(services: &RunServices, result: &EffectiveContext) -> Result<()> {
    let agent_path = &result.agent_path;
    let snapshot_ref = services
        .persist_record(
            "capability-snapshots",
            &result.invocation_id,
            &serde_json::to_value(result)?,
        )
        .await?;
    let snapshot = match snapshot_ref {
        Some(reference) => {
            json!({"invocationId":result.invocation_id,"nodePath":agent_path,"agentPath":agent_path,"origin":result.origin,"contentRef":reference})
        }
        None => serde_json::to_value(result)?,
    };
    services.emit(json!({"type":"context_snapshot","nodePath":agent_path,"origin":result.origin,"snapshot":snapshot})).await;
    Ok(())
}

/// Acquire one explicitly bound attachment, without adding another attachment,
/// a skill catalogue or workspace instructions to the model input.
pub fn acquire_attachment(
    config: &Value,
    services: &RunServices,
    agent_path: &str,
    state: &HashMap<String, Value>,
    item_id: &str,
    skill_name: Option<&str>,
) -> Result<Option<Value>> {
    let mut selected = attachments(config)?;
    if let Some(piece) = &mut selected.instructions {
        piece.items.retain(|item| item.id == item_id);
    }
    if let Some(piece) = &mut selected.files {
        piece.items.retain(|item| item.id == item_id);
    }
    if let Some(piece) = &mut selected.skills {
        piece.items.retain(|item| item.id == item_id);
        for item in &mut piece.items {
            if let Some(name) = skill_name {
                item.name = Some(name.into());
            }
        }
    }
    selected.tools = None;
    ensure!(
        selected
            .instructions
            .iter()
            .map(|p| p.items.len())
            .sum::<usize>()
            + selected.files.iter().map(|p| p.items.len()).sum::<usize>()
            + selected.skills.iter().map(|p| p.items.len()).sum::<usize>()
            == 1,
        "Attachment is not granted to this agent: {item_id}"
    );
    let config = json!({"attachments":selected});
    let captured = collect(&config, services, agent_path, state)?;
    if captured.resources.is_empty() {
        return Ok(None);
    }
    let text = captured
        .resources
        .iter()
        .filter_map(|value| value["content"].as_str())
        .collect::<Vec<_>>()
        .join("\n");
    Ok(Some(json!({"value":text,"sources":captured.resources})))
}

/// Acquire precisely one declared category, including a skill catalogue and only
/// explicitly activated skill bodies. No tools are granted by this resource.
pub fn acquire_attachments(
    config: &Value,
    services: &RunServices,
    agent_path: &str,
    state: &HashMap<String, Value>,
    slot: &str,
) -> Result<Value> {
    let mut selected = attachments(config)?;
    ensure!(
        ["instructions", "skills", "files"].contains(&slot),
        "Unknown attachment category: {slot}"
    );
    if slot != "instructions" {
        selected.instructions = None;
    }
    if slot != "skills" {
        selected.skills = None;
    }
    if slot != "files" {
        selected.files = None;
    }
    selected.tools = None;
    let captured = collect(
        &json!({"attachments":selected}),
        services,
        agent_path,
        state,
    )?;
    Ok(
        json!({"value":if slot=="files" { &captured.files } else { &captured.system },"sources":captured.resources,"skillCatalog":captured.skill_catalog}),
    )
}

pub async fn seal_calls(
    services: &RunServices,
    snapshot: &EffectiveContext,
    calls: &mut [Value],
) -> Result<()> {
    for (index, call) in calls.iter_mut().enumerate() {
        call["provenance"] = json!({"invocationId":snapshot.invocation_id,"agentPath":snapshot.agent_path,"callIndex":index});
    }
    let record = json!({"invocationId":snapshot.invocation_id,"agentPath":snapshot.agent_path,"origin":snapshot.origin,"tools":snapshot.tools,"calls":calls});
    let reference = services
        .persist_record("model-calls", &snapshot.invocation_id, &record)
        .await?;
    if let Some(reference) = reference {
        services.emit(json!({"type":"invocation_recorded","nodePath":snapshot.agent_path,"origin":snapshot.origin,"invocationId":snapshot.invocation_id,"recordRef":reference})).await;
    } else {
        services.emit(json!({"type":"invocation_recorded","nodePath":snapshot.agent_path,"origin":snapshot.origin,"record":record})).await;
    }
    Ok(())
}

/// Return a stable owner/receipt identity only after checking the trusted record.
pub async fn authorize_call(services: &RunServices, call: &Value) -> Result<(String, String)> {
    let provenance = &call["provenance"];
    let invocation = provenance["invocationId"]
        .as_str()
        .context("Appel sans provenance modèle")?;
    uuid::Uuid::parse_str(invocation).context("Identifiant d’invocation invalide")?;
    let index = provenance["callIndex"]
        .as_u64()
        .and_then(|index| usize::try_from(index).ok())
        .context("Index d’appel invalide")?;
    let record = services
        .read_record("model-calls", invocation)
        .await?
        .context("Invocation modèle inconnue")?;
    let owner = record["agentPath"]
        .as_str()
        .context("Agent demandeur absent")?;
    ensure!(
        provenance["agentPath"] == owner && record["invocationId"] == invocation,
        "Provenance d’appel incohérente"
    );
    ensure!(
        record["calls"]
            .as_array()
            .and_then(|calls| calls.get(index))
            == Some(call),
        "L’appel ne correspond pas à la demande modèle enregistrée"
    );
    ensure!(
        record["tools"]
            .as_array()
            .is_some_and(|tools| tools.contains(&call["name"])),
        "Outil non accordé à l’agent demandeur"
    );
    Ok((owner.into(), format!("{invocation}:{index}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use adk_graph::prelude::*;

    async fn services(root: &Path) -> std::sync::Arc<RunServices> {
        RunServices::new(
            "attachments-test".into(),
            root.into(),
            root.join("data"),
            ContextSnapshot {
                cwd: root.into(),
                ..Default::default()
            },
            json!({}),
            vec![],
        )
        .unwrap()
    }

    #[tokio::test]
    async fn activation_is_agent_local_and_literal_files_are_captured_before_change() {
        let root = tempfile::tempdir().unwrap();
        let skill_path = root.path().join("SKILL.md");
        std::fs::write(
            &skill_path,
            "---\nname: focused\ndescription: focused work\n---\nSkill {{secret}}.\n",
        )
        .unwrap();
        std::fs::write(root.path().join("notes.txt"), "first\n{{secret}}\nlast\n").unwrap();
        let services = services(root.path()).await;
        let config = json!({"attachments":{
            "instructions":{"items":[{"id":"literal","source":{"kind":"text","text":"Keep {{secret}}"}},{"id":"template","source":{"kind":"text","text":"Use {{secret}}"},"mode":"template"}]},
            "skills":{"items":[{"id":"skill","source":{"kind":"file","path":"SKILL.md"}}]},
            "files":{"items":[{"id":"file","path":"notes.txt","startLine":2,"endLine":2}]}
        }});
        assert_eq!(
            skill_activation_key(&config, &services.context, "focused").unwrap(),
            "skill::focused"
        );
        services.set_active_capabilities("a".into(), vec!["skill::focused".into()]);
        let state = HashMap::from([("secret".into(), json!("VALUE"))]);
        let a = capture(&config, &services, "a", &state).await.unwrap();
        let b = capture(&config, &services, "b", &state).await.unwrap();
        assert!(a.system.contains("Keep {{secret}}"));
        assert!(a.system.contains("Use VALUE"));
        assert!(a.system.contains("Skill {{secret}}."));
        assert!(!b.system.contains("Skill {{secret}}."));
        assert!(b.system.contains("focused work"));
        assert_eq!(a.resources.last().unwrap()["content"], "{{secret}}\n");
        assert!(a.tools.is_empty(), "skill metadata must never grant tools");
        std::fs::write(root.path().join("notes.txt"), "changed").unwrap();
        let captured: EffectiveContext = serde_json::from_slice(
            &std::fs::read(
                services
                    .data
                    .join("capability-snapshots")
                    .join(format!("{}.json", a.invocation_id)),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(
            captured.resources.last().unwrap()["content"],
            "{{secret}}\n"
        );
        let bare = capture(&json!({}), &services, "a", &state).await.unwrap();
        assert!(bare.system.is_empty() && bare.files.is_empty() && bare.tools.is_empty());
    }

    #[tokio::test]
    async fn provenance_rejects_forgery_and_reuses_effect_receipts_across_dispatch_nodes() {
        let root = tempfile::tempdir().unwrap();
        let services = services(root.path()).await;
        let config = json!({"attachments":{"tools":{"items":[{"id":"exec","name":"exec"}]}}});
        let snapshot = capture(&config, &services, "agent-a", &HashMap::new())
            .await
            .unwrap();
        let mut calls = vec![
            json!({"name":"exec","id":"provider-1","args":{"command":"printf x >> effect.txt"}}),
        ];
        seal_calls(&services, &snapshot, &mut calls).await.unwrap();
        let mut fake = calls[0].clone();
        fake["provenance"]["agentPath"] = json!("agent-b");
        assert!(authorize_call(&services, &fake).await.is_err());
        fake = calls[0].clone();
        fake["args"]["command"] = json!("touch forged");
        assert!(authorize_call(&services, &fake).await.is_err());
        let dispatch = |call: Value, id: &str| {
            NodeContext::new(
                State::from([("toolCalls".into(), json!([call]))]),
                ExecutionConfig::new(id),
                1,
            )
        };
        let cfg = json!({"__zedflowVersion":2,"nodeId":"broker","tool":"execute_next_call"});
        let denied = crate::operations::execute_with_services(
            "tool",
            &cfg,
            dispatch(fake, "denied"),
            "broker",
            services.clone(),
        )
        .await
        .unwrap();
        assert_eq!(denied.updates["toolResults"][0]["result"]["denied"], true);
        assert!(!root.path().join("forged").exists());
        for broker in ["broker-a", "broker-b"] {
            let result = crate::operations::execute_with_services(
                "tool",
                &cfg,
                dispatch(calls[0].clone(), broker),
                broker,
                services.clone(),
            )
            .await
            .unwrap();
            assert!(
                result.updates["toolResults"][0]["result"]
                    .get("error")
                    .is_none(),
                "{:?}",
                result.updates
            );
        }
        assert_eq!(
            std::fs::read_to_string(root.path().join("effect.txt")).unwrap(),
            "x"
        );
        let direct = json!({"__zedflowVersion":2,"nodeId":"programmed","tool":"exec","arguments":{"command":"printf direct > direct.txt"}});
        crate::operations::execute_with_services(
            "tool",
            &direct,
            NodeContext::new(State::new(), ExecutionConfig::new("direct"), 1),
            "programmed",
            services,
        )
        .await
        .unwrap();
        assert_eq!(
            std::fs::read_to_string(root.path().join("direct.txt")).unwrap(),
            "direct"
        );
    }
}
