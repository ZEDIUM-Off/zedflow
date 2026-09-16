//! A single ADK model request over the Codex subscription transport used by Pi.
//!
//! This is a compatibility adapter for the Codex backend, not the public OpenAI
//! API and not a Codex agent turn. The official CLI owns login and token refresh.
//! Credentials never enter a composition, API response, event, or error message.
use adk_core::{
    Content, FinishReason, Llm, LlmRequest, LlmResponse, LlmResponseStream, Part, UsageMetadata,
};
use anyhow::{Context, Result, bail, ensure};
use async_trait::async_trait;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use futures::StreamExt;
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::PathBuf, process::Stdio, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::Command,
    sync::Mutex,
};

const RESPONSES_URL: &str = "https://chatgpt.com/backend-api/codex/responses";
const MAX_FRAME_BYTES: usize = 16 * 1024 * 1024;
static REFRESH_LOCK: Mutex<()> = Mutex::const_new(());

pub fn model(config: &Value) -> Result<Arc<dyn Llm>> {
    validate(config)?;
    let name = config["model"]
        .as_str()
        .filter(|name| !name.trim().is_empty())
        .context("Un identifiant de modèle Codex est requis")?;
    Ok(Arc::new(CodexModel {
        name: name.into(),
        options: config.clone(),
        client: reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(300))
            .redirect(reqwest::redirect::Policy::none())
            .build()?,
    }))
}

/// Validate transport options without reading credentials or contacting a model.
pub fn validate(config: &Value) -> Result<()> {
    ensure!(
        config["model"]
            .as_str()
            .is_some_and(|model| !model.trim().is_empty()),
        "Un identifiant de modèle Codex est requis"
    );
    for (key, values) in [
        (
            "reasoningEffort",
            &[
                "none", "minimal", "low", "medium", "high", "xhigh", "max", "ultra",
            ][..],
        ),
        ("reasoningSummary", &["auto", "concise", "detailed"][..]),
        ("textVerbosity", &["low", "medium", "high"][..]),
    ] {
        if let Some(value) = config.get(key).filter(|value| !value.is_null()) {
            ensure!(
                value.as_str().is_some_and(|value| values.contains(&value)),
                "Option Codex {key} invalide ; valeurs : {}",
                values.join(", ")
            );
        }
    }
    Ok(())
}

struct CodexModel {
    name: String,
    options: Value,
    client: reqwest::Client,
}

// Deliberately neither Debug nor Serialize: these values are backend-only.
struct Credentials {
    access: String,
    account: String,
}

fn codex_home() -> Result<PathBuf> {
    std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".codex")))
        .context("Le répertoire de connexion Codex est introuvable")
}

fn credential_document(document: &Value) -> Result<Credentials> {
    let tokens = &document["tokens"];
    let access = tokens["access_token"]
        .as_str()
        .filter(|token| !token.is_empty())
        .context("Connexion ChatGPT requise : exécutez codex login sur la machine du daemon")?;
    let account = tokens["account_id"]
        .as_str()
        .filter(|account| !account.is_empty())
        .map(str::to_owned)
        .or_else(|| {
            let payload = access.split('.').nth(1)?;
            let payload = URL_SAFE_NO_PAD.decode(payload).ok()?;
            let payload: Value = serde_json::from_slice(&payload).ok()?;
            payload["https://api.openai.com/auth"]["chatgpt_account_id"]
                .as_str()
                .map(str::to_owned)
        })
        .context("La connexion Codex ne contient pas d’identité de compte ChatGPT")?;
    Ok(Credentials {
        access: access.into(),
        account,
    })
}

async fn credentials() -> Result<Credentials> {
    let bytes = tokio::fs::read(codex_home()?.join("auth.json")).await.map_err(|_| {
        anyhow::anyhow!(
            "Connexion Codex locale requise. Si le CLI utilise le trousseau système, connectez un profil dédié avec codex -c cli_auth_credentials_store=\"file\" login"
        )
    })?;
    let document = serde_json::from_slice(&bytes)
        .map_err(|_| anyhow::anyhow!("Le stockage de connexion Codex est invalide"))?;
    credential_document(&document)
}

/// Safe local connection status. Never forwards CLI stdout/stderr or identity.
pub async fn status() -> Result<Value> {
    let mut result = cli_status("codex").await;
    result["credentialFilePresent"] =
        json!(codex_home().is_ok_and(|home| home.join("auth.json").is_file()));
    result["loginCommand"] = json!("codex -c cli_auth_credentials_store=\"file\" login");
    result["transport"] = json!("codex-responses-compatibility");
    Ok(result)
}

async fn cli_status(program: &str) -> Value {
    let output = tokio::time::timeout(
        Duration::from_secs(10),
        Command::new(program)
            .args([
                "-c",
                "cli_auth_credentials_store=\"file\"",
                "login",
                "status",
            ])
            .stdin(Stdio::null())
            .kill_on_drop(true)
            .output(),
    )
    .await;
    match output {
        Ok(Ok(output)) => {
            let text = format!(
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            let chatgpt = output.status.success() && text.contains("ChatGPT");
            json!({
                "installed":true, "authenticated":chatgpt,
                "authMode":if chatgpt {"chatgpt"} else if output.status.success() {"other"} else {"none"},
                "message":if chatgpt {"Connexion ChatGPT disponible sur le daemon"} else {"Connectez Codex avec ChatGPT sur la machine du daemon"}
            })
        }
        Ok(Err(_)) => {
            json!({"installed":false,"authenticated":false,"authMode":"none","message":"Codex CLI est introuvable sur la machine du daemon"})
        }
        Err(_) => {
            json!({"installed":true,"authenticated":false,"authMode":"unknown","message":"Le contrôle de connexion Codex a expiré"})
        }
    }
}

// Let Codex manage OAuth refresh/storage, including file permissions and rotation.
// This sends no model prompt and starts no Codex thread or agent harness.
async fn refresh_credentials(rejected_access: &str) -> Result<()> {
    let _guard = REFRESH_LOCK.lock().await;
    if credentials()
        .await
        .is_ok_and(|auth| auth.access != rejected_access)
    {
        return Ok(());
    }
    tokio::time::timeout(Duration::from_secs(40), refresh_with("codex"))
        .await
        .context("Le rafraîchissement de connexion Codex a expiré")??;
    Ok(())
}

async fn refresh_with(program: &str) -> Result<()> {
    let mut child = Command::new(program)
        .args([
            "-c",
            "cli_auth_credentials_store=\"file\"",
            "app-server",
            "--listen",
            "stdio://",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .context("Impossible de démarrer Codex pour rafraîchir sa connexion")?;
    let mut input = child.stdin.take().context("Entrée Codex indisponible")?;
    let output = child.stdout.take().context("Sortie Codex indisponible")?;
    let mut output = BufReader::new(output);
    input
        .write_all(b"{\"id\":1,\"method\":\"initialize\",\"params\":{\"clientInfo\":{\"name\":\"zedflow\",\"version\":\"0.1.0\"}}}\n")
        .await?;
    rpc_response(&mut output, 1).await?;
    input
        .write_all(b"{\"method\":\"initialized\",\"params\":{}}\n{\"id\":2,\"method\":\"account/read\",\"params\":{\"refreshToken\":true}}\n")
        .await?;
    let account = rpc_response(&mut output, 2).await?;
    let is_chatgpt = account["account"]["type"].as_str() == Some("chatgpt");
    let _ = child.kill().await;
    ensure!(
        is_chatgpt,
        "Connexion ChatGPT requise : exécutez codex login"
    );
    Ok(())
}

async fn rpc_response<R: tokio::io::AsyncBufRead + Unpin>(
    reader: &mut R,
    id: u64,
) -> Result<Value> {
    loop {
        let mut line = Vec::new();
        let count = (&mut *reader)
            .take((MAX_FRAME_BYTES + 1) as u64)
            .read_until(b'\n', &mut line)
            .await?;
        ensure!(count != 0, "Codex a fermé le canal de connexion");
        ensure!(
            count <= MAX_FRAME_BYTES,
            "Réponse de connexion Codex trop volumineuse"
        );
        let value: Value = serde_json::from_slice(&line)
            .map_err(|_| anyhow::anyhow!("Protocole de connexion Codex invalide"))?;
        if value["id"].as_u64() == Some(id) {
            ensure!(
                value.get("error").is_none(),
                "Codex n’a pas pu rafraîchir sa connexion ; relancez codex login"
            );
            return Ok(value["result"].clone());
        }
    }
}

#[async_trait]
impl Llm for CodexModel {
    fn name(&self) -> &str {
        &self.name
    }

    async fn generate_content(
        &self,
        req: LlmRequest,
        _stream: bool,
    ) -> adk_core::Result<LlmResponseStream> {
        let body = request_body(&self.name, &self.options, &req)
            .map_err(|error| adk_core::AdkError::model(error.to_string()))?;
        let parts = crate::inference_raw::segments(&body)
            .map_err(|error| adk_core::AdkError::model(error.to_string()))?;
        crate::inference_raw::capture("codexHttpBody", &parts)
            .await
            .map_err(|error| adk_core::AdkError::model(error.to_string()))?;
        let body = parts.concat().into_bytes();
        let result = async {
            let auth = credentials().await?;
            let mut response = send_request(&self.client, RESPONSES_URL, &auth, &body).await?;
            crate::inference_raw::dispatched().await?;
            if response.status() == reqwest::StatusCode::UNAUTHORIZED {
                refresh_credentials(&auth.access).await?;
                response =
                    send_request(&self.client, RESPONSES_URL, &credentials().await?, &body).await?;
            }
            consume_response(response).await
        }
        .await
        .map_err(|error: anyhow::Error| adk_core::AdkError::model(error.to_string()))?;
        Ok(Box::pin(futures::stream::once(async move { Ok(result) })))
    }
}

async fn send_request(
    client: &reqwest::Client,
    url: &str,
    auth: &Credentials,
    body: &[u8],
) -> Result<reqwest::Response> {
    client
        .post(url)
        .bearer_auth(&auth.access)
        .header("chatgpt-account-id", &auth.account)
        .header("originator", "zedflow")
        .header("user-agent", "zedflow/0.1.0")
        .header("OpenAI-Beta", "responses=experimental")
        .header("accept", "text/event-stream")
        .header("content-type", "application/json")
        .body(body.to_vec())
        .send()
        .await
        .map_err(|_| anyhow::anyhow!("Connexion au transport Codex impossible"))
}

pub(crate) fn request_body(model: &str, options: &Value, req: &LlmRequest) -> Result<Value> {
    let mut instructions = Vec::new();
    let mut input = Vec::new();
    for content in &req.contents {
        for part in &content.parts {
            match part {
                Part::Text { text } if matches!(content.role.as_str(), "system" | "developer") => instructions.push(text.clone()),
                Part::Text { text } => {
                    let assistant = matches!(content.role.as_str(), "model" | "assistant");
                    input.push(json!({"role":if assistant {"assistant"} else {"user"},"content":[{"type":if assistant {"output_text"} else {"input_text"},"text":text}]}));
                }
                Part::FunctionCall { name, args, id, .. } => input.push(json!({"type":"function_call","name":name,"arguments":args.to_string(),"call_id":id.as_deref().context("Un appel outil Codex doit conserver son identifiant")?})),
                Part::FunctionResponse { function_response, id, .. } => input.push(json!({"type":"function_call_output","call_id":id.as_deref().context("Un résultat outil Codex doit conserver son identifiant")?,"output":function_response.response.to_string()})),
                Part::Thinking { signature: Some(signature), .. } => {
                    let item: Value = serde_json::from_str(signature).context("Signature de raisonnement Codex invalide")?;
                    ensure!(item["type"] == "reasoning", "Signature de raisonnement Codex incompatible");
                    input.push(item);
                }
                Part::Thinking { .. } => {},
                Part::InlineData { mime_type, data, .. } if mime_type.starts_with("image/") => {
                    let data = base64::engine::general_purpose::STANDARD.encode(data);
                    input.push(json!({"role":"user","content":[{"type":"input_image","image_url":format!("data:{mime_type};base64,{data}")}]}));
                }
                Part::FileData { mime_type, file_uri, .. } if mime_type.starts_with("image/") => input.push(json!({"role":"user","content":[{"type":"input_image","image_url":file_uri}]})),
                _ => bail!("Ce type de contenu ADK n’est pas encore pris en charge par le transport Codex"),
            }
        }
    }
    let mut declarations: Vec<_> = req.tools.iter().collect();
    declarations.sort_by_key(|(name, _)| *name);
    let tools: Vec<Value> = declarations.into_iter().map(|(name, tool)| json!({
        "type":"function", "name":name,
        "description":tool["description"].as_str().unwrap_or(""),
        "parameters":tool.get("parameters").cloned().unwrap_or_else(|| json!({"type":"object","properties":{}})),
        "strict":false
    })).collect();
    let mut body = json!({
        "model":model, "store":false, "stream":true,
        "instructions":if instructions.is_empty()
            && options["__zedflowVersion"].as_u64().unwrap_or(1)<2
            && options.get("contextProgram").is_none_or(Value::is_null)
            {"You are a helpful assistant.".into()} else {instructions.join("\n\n")},
        "input":input, "tools":tools, "tool_choice":"auto", "parallel_tool_calls":true,
        "include":["reasoning.encrypted_content"],
        "text":{"verbosity":options["textVerbosity"].as_str().unwrap_or("low")}
    });
    if options["reasoningEffort"].is_string() || options["reasoningSummary"].is_string() {
        body["reasoning"] =
            json!({"summary":options["reasoningSummary"].as_str().unwrap_or("auto")});
        if let Some(effort) = options["reasoningEffort"].as_str() {
            body["reasoning"]["effort"] = json!(effort);
        }
    }
    if let Some(config) = &req.config {
        ensure!(
            config.top_k.is_none()
                && config.frequency_penalty.is_none()
                && config.presence_penalty.is_none()
                && config.seed.is_none()
                && config.top_logprobs.is_none()
                && config.stop_sequences.is_empty()
                && config.extensions.is_empty()
                && config.cached_content.is_none(),
            "Un paramètre de génération configuré n’est pas compatible avec Codex"
        );
        ensure!(
            config.max_output_tokens.is_none() && config.top_p.is_none(),
            "Codex ne prend pas en charge max_output_tokens ou top_p sur ce transport"
        );
        if let Some(temperature) = config.temperature {
            body["temperature"] = json!(temperature);
        }
        if let Some(schema) = &config.response_schema {
            body["text"]["format"] =
                json!({"type":"json_schema","name":"zedflow_output","schema":schema,"strict":true});
        }
    }
    Ok(body)
}

async fn consume_response(response: reqwest::Response) -> Result<LlmResponse> {
    ensure!(
        response.status().is_success(),
        "Codex a refusé la requête (HTTP {}). Vérifiez la connexion, le modèle et les limites de votre abonnement",
        response.status().as_u16()
    );
    let mut stream = response.bytes_stream();
    let mut buffer = Vec::new();
    let mut data = Vec::new();
    let mut accumulated = ResponseAccumulator::default();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| anyhow::anyhow!("Le flux Codex a été interrompu"))?;
        ensure!(
            buffer.len() + chunk.len() <= MAX_FRAME_BYTES,
            "Événement Codex trop volumineux"
        );
        buffer.extend_from_slice(&chunk);
        while let Some(end) = buffer.iter().position(|byte| *byte == b'\n') {
            let mut line: Vec<u8> = buffer.drain(..=end).collect();
            line.pop();
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            if line.is_empty() {
                if !data.is_empty() {
                    if let Some(response) = accumulated.parse_event(&data)? {
                        return Ok(response);
                    }
                    data.clear();
                }
            } else if let Some(payload) = line.strip_prefix(b"data:") {
                if !data.is_empty() {
                    data.push(b'\n');
                }
                data.extend_from_slice(payload.strip_prefix(b" ").unwrap_or(payload));
                ensure!(
                    data.len() <= MAX_FRAME_BYTES,
                    "Événement Codex trop volumineux"
                );
            }
        }
    }
    bail!("Le flux Codex s’est terminé sans réponse complète")
}

/// Codex can omit output items from the terminal response. Preserve the items
/// delivered earlier in the stream, as Pi does, while retaining terminal usage.
#[derive(Default)]
struct ResponseAccumulator {
    items: BTreeMap<usize, Value>,
}

impl ResponseAccumulator {
    fn parse_event(&mut self, data: &[u8]) -> Result<Option<LlmResponse>> {
        if data == b"[DONE]" {
            return Ok(None);
        }
        let mut event: Value = serde_json::from_slice(data)
            .map_err(|_| anyhow::anyhow!("Événement Codex invalide"))?;
        let kind = event["type"].as_str().unwrap_or("");
        match kind {
            "response.output_item.added" | "response.output_item.done" => {
                let index = stream_index(&event, "output_index")?;
                ensure!(event["item"].is_object(), "Élément Codex invalide");
                self.items.insert(index, event["item"].take());
            }
            "response.content_part.added"
            | "response.content_part.done"
            | "response.reasoning_summary_part.added"
            | "response.reasoning_summary_part.done" => {
                let summary = kind.starts_with("response.reasoning_summary");
                let index = stream_index(
                    &event,
                    if summary {
                        "summary_index"
                    } else {
                        "content_index"
                    },
                )?;
                let part = event["part"].clone();
                ensure!(part.is_object(), "Contenu Codex invalide");
                let item = self.item(&event, if summary { "reasoning" } else { "message" })?;
                *stream_part(item, if summary { "summary" } else { "content" }, index)? = part;
            }
            "response.output_text.delta"
            | "response.output_text.done"
            | "response.refusal.delta"
            | "response.refusal.done"
            | "response.reasoning_summary_text.delta"
            | "response.reasoning_summary_text.done" => {
                let summary = kind.starts_with("response.reasoning_summary");
                let refusal = kind.starts_with("response.refusal");
                let delta = kind.ends_with(".delta");
                let field = if refusal { "refusal" } else { "text" };
                let text = event[if delta { "delta" } else { field }]
                    .as_str()
                    .context("Texte Codex invalide")?;
                let index = stream_index(
                    &event,
                    if summary {
                        "summary_index"
                    } else {
                        "content_index"
                    },
                )?;
                let item = self.item(&event, if summary { "reasoning" } else { "message" })?;
                let part = stream_part(item, if summary { "summary" } else { "content" }, index)?;
                part["type"] = json!(if summary {
                    "summary_text"
                } else if refusal {
                    "refusal"
                } else {
                    "output_text"
                });
                stream_text(part, field, text, delta)?;
            }
            "response.function_call_arguments.delta" | "response.function_call_arguments.done" => {
                let delta = kind.ends_with(".delta");
                let text = event[if delta { "delta" } else { "arguments" }]
                    .as_str()
                    .context("Arguments Codex invalides")?;
                stream_text(
                    self.item(&event, "function_call")?,
                    "arguments",
                    text,
                    delta,
                )?;
            }
            "response.completed" | "response.incomplete" | "response.done" => {
                let mut response = event["response"].take();
                // A populated final output is authoritative and must not be
                // appended to the accumulated items (which would duplicate it).
                if response["output"].as_array().is_none_or(Vec::is_empty) {
                    response["output"] = json!(
                        std::mem::take(&mut self.items)
                            .into_values()
                            .collect::<Vec<_>>()
                    );
                }
                return completed_response(&response).map(Some);
            }
            "response.failed" | "error" => bail!(
                "Codex a signalé une erreur de génération ; vérifiez le modèle et les limites de l’abonnement"
            ),
            _ => {}
        }
        Ok(None)
    }

    fn item(&mut self, event: &Value, kind: &str) -> Result<&mut Value> {
        let index = stream_index(event, "output_index")?;
        Ok(self
            .items
            .entry(index)
            .or_insert_with(|| json!({"type":kind,"id":event["item_id"]})))
    }
}

fn stream_index(event: &Value, field: &str) -> Result<usize> {
    let index = event[field]
        .as_u64()
        .context("Index de contenu Codex invalide")?;
    ensure!(index < 4096, "Trop d’éléments de contenu Codex");
    Ok(index as usize)
}

fn stream_part<'a>(item: &'a mut Value, field: &str, index: usize) -> Result<&'a mut Value> {
    if item[field].is_null() {
        item[field] = json!([]);
    }
    let parts = item[field]
        .as_array_mut()
        .context("Contenu Codex invalide")?;
    while parts.len() <= index {
        parts.push(json!({}));
    }
    Ok(&mut parts[index])
}

fn stream_text(item: &mut Value, field: &str, text: &str, append: bool) -> Result<()> {
    if !append || item[field].is_null() {
        item[field] = json!(text);
    } else {
        let Value::String(target) = &mut item[field] else {
            bail!("Texte Codex invalide");
        };
        ensure!(
            target.len() + text.len() <= MAX_FRAME_BYTES,
            "Contenu Codex trop volumineux"
        );
        target.push_str(text);
    }
    Ok(())
}

fn completed_response(response: &Value) -> Result<LlmResponse> {
    ensure!(
        matches!(
            response["status"].as_str(),
            Some("completed" | "incomplete")
        ),
        "Statut de réponse Codex invalide"
    );
    let mut content = Content::new("model");
    for item in response["output"]
        .as_array()
        .context("Réponse Codex sans sortie")?
    {
        match item["type"].as_str() {
            Some("message") => {
                for part in item["content"]
                    .as_array()
                    .context("Message Codex invalide")?
                {
                    if let Some(text) = part["text"].as_str().or_else(|| part["refusal"].as_str()) {
                        content.parts.push(Part::Text { text: text.into() });
                    }
                }
            }
            Some("function_call") => content.parts.push(Part::FunctionCall {
                name: item["name"]
                    .as_str()
                    .context("Appel outil Codex sans nom")?
                    .into(),
                args: serde_json::from_str(
                    item["arguments"]
                        .as_str()
                        .context("Appel outil Codex sans arguments")?,
                )
                .map_err(|_| anyhow::anyhow!("Arguments de l’outil Codex invalides"))?,
                id: Some(
                    item["call_id"]
                        .as_str()
                        .context("Appel outil Codex sans identifiant")?
                        .into(),
                ),
                thought_signature: None,
            }),
            Some("reasoning") => {
                let summary = item["summary"]
                    .as_array()
                    .map(|parts| {
                        parts
                            .iter()
                            .filter_map(|part| part["text"].as_str())
                            .collect::<Vec<_>>()
                            .join("\n")
                    })
                    .unwrap_or_default();
                content.parts.push(Part::Thinking {
                    thinking: summary,
                    signature: Some(item.to_string()),
                });
            }
            _ => bail!("Type de sortie Codex non pris en charge"),
        }
    }
    let token = |value: &Value| value.as_i64().and_then(|value| i32::try_from(value).ok());
    ensure!(
        content.parts.iter().any(|part| match part {
            Part::Text { text } => !text.trim().is_empty(),
            Part::FunctionCall { .. } => true,
            _ => false,
        }),
        "Codex a terminé sans contenu exploitable ; aucun texte ni appel outil n’a été reçu"
    );
    let usage = &response["usage"];
    Ok(LlmResponse {
        turn_complete: !content.has_function_calls(),
        content: Some(content),
        finish_reason: Some(if response["status"] == "incomplete" {
            FinishReason::MaxTokens
        } else {
            FinishReason::Stop
        }),
        usage_metadata: usage.is_object().then(|| UsageMetadata {
            prompt_token_count: token(&usage["input_tokens"]).unwrap_or(0),
            candidates_token_count: token(&usage["output_tokens"]).unwrap_or(0),
            total_token_count: token(&usage["total_tokens"]).unwrap_or(0),
            cache_read_input_token_count: token(&usage["input_tokens_details"]["cached_tokens"]),
            thinking_token_count: token(&usage["output_tokens_details"]["reasoning_tokens"]),
            ..Default::default()
        }),
        provider_metadata: Some(
            json!({"provider":"codex","responseId":response["id"],"transport":"codex-responses-compatibility"}),
        ),
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_context_never_acquires_transport_instructions() {
        let request = LlmRequest::new(
            "test",
            vec![Content::new("user").with_text("Only selected data")],
        );
        for options in [
            json!({"__zedflowVersion":2}),
            json!({"__zedflowVersion":3}),
            json!({"contextProgram":{"hash":"captured"}}),
        ] {
            let body = request_body("test", &options, &request).unwrap();
            assert_eq!(body["instructions"], "");
            assert_eq!(body["input"][0]["content"][0]["text"], "Only selected data");
        }
        assert_eq!(
            request_body("test", &json!({}), &request).unwrap()["instructions"],
            "You are a helpful assistant."
        );
        let request = LlmRequest::new(
            "test",
            vec![Content::new("system").with_text("Exact explicit instruction")],
        );
        assert_eq!(
            request_body("test", &json!({"__zedflowVersion":3}), &request).unwrap()["instructions"],
            "Exact explicit instruction"
        );
    }

    #[test]
    fn request_preserves_tools_and_history_without_harness() {
        let mut request = LlmRequest::new(
            "test",
            vec![
                Content::new("system").with_text("Instruction"),
                Content::new("user").with_text("Bonjour"),
            ],
        );
        request.tools.insert("lookup".into(), json!({"description":"Read a source","parameters":{"type":"object","properties":{"q":{"type":"string"}}}}));
        let body = request_body("test", &json!({"reasoningEffort":"low"}), &request).unwrap();
        assert_eq!(body["instructions"], "Instruction");
        assert_eq!(body["tools"][0]["name"], "lookup");
        assert_eq!(body["input"].as_array().unwrap().len(), 1);
        assert_eq!(body["reasoning"]["effort"], "low");
        assert_eq!(body["store"], false);
    }

    #[test]
    fn tool_result_keeps_the_call_identity_for_the_next_graph_iteration() {
        let call = Part::FunctionCall {
            name: "lookup".into(),
            args: json!({"q":"test"}),
            id: Some("call-42".into()),
            thought_signature: None,
        };
        let result = Part::FunctionResponse {
            function_response: adk_core::FunctionResponseData::new("lookup", json!({"found":3})),
            id: Some("call-42".into()),
            annotations: None,
        };
        let mut assistant = Content::new("model");
        assistant.parts.push(call);
        let mut tool = Content::new("user");
        tool.parts.push(result);
        let request = LlmRequest::new("test", vec![assistant, tool]);
        let body = request_body("test", &json!({}), &request).unwrap();
        assert_eq!(body["input"][0]["call_id"], "call-42");
        assert_eq!(body["input"][1]["type"], "function_call_output");
        assert_eq!(body["input"][1]["call_id"], "call-42");
        assert_eq!(body["input"][1]["output"], "{\"found\":3}");
    }

    #[tokio::test]
    async fn captured_segments_are_the_exact_http_body() {
        use axum::{Router, routing::post};
        let app = Router::new().route(
            "/responses",
            post(|body: axum::body::Bytes| async move { body }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let body = json!({"input":[{"role":"user","content":"é🦀\n\"quoted\""},{"type":"function_call","arguments":"{\"x\":18446744073709551615}"}],"tools":[{"name":"lookup","parameters":{"type":"object"}}],"stream":true});
        let parts = crate::inference_raw::segments(&body).unwrap();
        let capture = crate::inference_raw::document("codexHttpBody", &parts);
        let sent = parts.concat().into_bytes();
        let auth = Credentials {
            access: "fixture".into(),
            account: "fixture".into(),
        };
        let response = send_request(
            &reqwest::Client::new(),
            &format!("http://{address}/responses"),
            &auth,
            &sent,
        )
        .await
        .unwrap();
        let received = response.bytes().await.unwrap();
        assert_eq!(
            received.as_ref(),
            crate::inference_raw::bytes(&capture).unwrap()
        );
        server.abort();
    }

    #[tokio::test]
    async fn simulated_sse_maps_tool_calls_and_usage_without_executing_tool() {
        use axum::{Router, body::Body, routing::post};
        let event = json!({"type":"response.completed","response":{"id":"r1","status":"completed","output":[
            {"type":"reasoning","id":"rs1","summary":[{"type":"summary_text","text":"Résumé public"}],"encrypted_content":"opaque"},
            {"type":"message","content":[{"type":"output_text","text":"Je consulte."}]},
            {"type":"function_call","name":"lookup","call_id":"c1","arguments":"{\"q\":\"test\"}"}
        ],"usage":{"input_tokens":7,"output_tokens":3,"total_tokens":10}}});
        let sse = format!(
            "event: response.created\r\ndata: {{\"type\":\"response.created\"}}\r\n\r\ndata: {event}\r\n\r\n"
        );
        let app = Router::new().route(
            "/responses",
            post(
                move |headers: axum::http::HeaderMap, body: axum::body::Bytes| {
                    let sse = sse.clone();
                    async move {
                        assert_eq!(headers["authorization"], "Bearer fixture-only");
                        assert_eq!(body.as_ref(), br#"{"stream":true}"#);
                        // Chunk every byte, including multi-byte UTF-8 and CRLF boundaries.
                        let chunks = sse
                            .into_bytes()
                            .into_iter()
                            .map(|byte| Ok::<_, std::io::Error>(vec![byte]))
                            .collect::<Vec<_>>();
                        Body::from_stream(futures::stream::iter(chunks))
                    }
                },
            ),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let auth = Credentials {
            access: "fixture-only".into(),
            account: "fixture-account".into(),
        };
        let response = send_request(
            &reqwest::Client::new(),
            &format!("http://{address}/responses"),
            &auth,
            br#"{"stream":true}"#,
        )
        .await
        .unwrap();
        let result = consume_response(response).await.unwrap();
        server.abort();
        assert!(!result.turn_complete);
        assert_eq!(result.usage_metadata.unwrap().total_token_count, 10);
        let content = result.content.unwrap();
        assert!(
            matches!(&content.parts[2], Part::FunctionCall {name,id:Some(id),args,..} if name=="lookup" && id=="c1" && args["q"]=="test")
        );
        let followup =
            request_body("test", &json!({}), &LlmRequest::new("test", vec![content])).unwrap();
        assert_eq!(followup["input"][0]["encrypted_content"], "opaque");
    }

    #[test]
    fn provider_error_never_reflects_raw_remote_body() {
        let error = ResponseAccumulator::default()
            .parse_event(br#"{"type":"error","message":"secret-provider-value"}"#)
            .unwrap_err();
        assert!(!error.to_string().contains("secret-provider-value"));
        let error = credential_document(&json!({"OPENAI_API_KEY":"secret-key-value"}))
            .err()
            .unwrap();
        assert!(!error.to_string().contains("secret-key-value"));
    }

    #[tokio::test]
    async fn completed_metadata_without_output_keeps_streamed_items() {
        use axum::{Router, body::Body, routing::post};
        let events = [
            json!({"type":"response.output_item.added","output_index":0,"item":{"type":"message","id":"msg-1","role":"assistant","content":[]}}),
            json!({"type":"response.output_text.delta","output_index":0,"content_index":0,"delta":"Réponse "}),
            json!({"type":"response.output_text.delta","output_index":0,"content_index":0,"delta":"reçue 🟢"}),
            json!({"type":"response.output_item.done","output_index":0,"item":{"type":"message","id":"msg-1","role":"assistant","content":[{"type":"output_text","text":"Réponse reçue 🟢"}]}}),
            json!({"type":"response.completed","response":{"id":"r2","status":"completed","output":[],"usage":{"input_tokens":24,"output_tokens":65,"total_tokens":89}}}),
        ];
        let sse = events
            .iter()
            .map(|event| format!("data: {event}\r\n\r\n"))
            .collect::<String>();
        let app = Router::new().route(
            "/responses",
            post(move || {
                let chunks = sse
                    .bytes()
                    .map(|byte| Ok::<_, std::io::Error>(vec![byte]))
                    .collect::<Vec<_>>();
                async { Body::from_stream(futures::stream::iter(chunks)) }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let response = reqwest::Client::new()
            .post(format!("http://{address}/responses"))
            .send()
            .await
            .unwrap();
        let result = consume_response(response).await.unwrap();
        server.abort();
        assert_eq!(result.usage_metadata.unwrap().total_token_count, 89);
        let content = result.content.unwrap();
        assert_eq!(content.parts.len(), 1);
        assert!(matches!(&content.parts[0], Part::Text { text } if text == "Réponse reçue 🟢"));
    }

    #[test]
    fn deltas_and_done_arguments_are_reconstructed_in_output_order() {
        let mut accumulator = ResponseAccumulator::default();
        for event in [
            json!({"type":"response.output_item.added","output_index":1,"item":{"type":"function_call","name":"lookup","call_id":"call-1","arguments":""}}),
            json!({"type":"response.function_call_arguments.delta","output_index":1,"delta":"{\"q\":"}),
            json!({"type":"response.function_call_arguments.delta","output_index":1,"delta":"\"Paris"}),
            json!({"type":"response.function_call_arguments.done","output_index":1,"arguments":"{\"q\":\"Paris\"}"}),
            json!({"type":"response.output_text.delta","output_index":0,"content_index":0,"delta":"Je "}),
            json!({"type":"response.output_text.delta","output_index":0,"content_index":0,"delta":"consulte."}),
        ] {
            assert!(
                accumulator
                    .parse_event(event.to_string().as_bytes())
                    .unwrap()
                    .is_none()
            );
        }
        let result = accumulator
            .parse_event(
                br#"{"type":"response.done","response":{"status":"completed","output":[]}}"#,
            )
            .unwrap()
            .unwrap();
        assert!(!result.turn_complete);
        let parts = result.content.unwrap().parts;
        assert_eq!(parts.len(), 2);
        assert!(matches!(&parts[0], Part::Text { text } if text == "Je consulte."));
        assert!(
            matches!(&parts[1], Part::FunctionCall { name, args, id: Some(id), .. } if name == "lookup" && args["q"] == "Paris" && id == "call-1")
        );
    }

    #[test]
    fn terminal_output_does_not_duplicate_streamed_text_or_tools() {
        let item = json!({"type":"message","content":[{"type":"output_text","text":"Final"}]});
        let mut accumulator = ResponseAccumulator::default();
        accumulator
            .parse_event(
                json!({"type":"response.output_item.done","output_index":0,"item":item})
                    .to_string()
                    .as_bytes(),
            )
            .unwrap();
        let result = accumulator.parse_event(json!({"type":"response.completed","response":{"status":"completed","output":[item]}}).to_string().as_bytes()).unwrap().unwrap();
        assert_eq!(result.content.unwrap().parts.len(), 1);
    }

    #[test]
    fn empty_or_truncated_generation_does_not_succeed_silently() {
        let error = ResponseAccumulator::default().parse_event(br#"{"type":"response.completed","response":{"status":"completed","output":[],"usage":{"output_tokens":65}}}"#).unwrap_err();
        assert!(error.to_string().contains("sans contenu exploitable"));
        let mut accumulator = ResponseAccumulator::default();
        accumulator.parse_event(br#"{"type":"response.output_item.added","output_index":0,"item":{"type":"function_call","name":"lookup","call_id":"c1","arguments":"{\"q\":"}}"#).unwrap();
        assert!(accumulator.parse_event(br#"{"type":"response.incomplete","response":{"status":"incomplete","output":[]}}"#).is_err());
    }

    #[test]
    fn reasoning_only_is_not_a_successful_visible_response() {
        for summary in [
            json!([]),
            json!([{"type":"summary_text","text":"Résumé public"}]),
        ] {
            let event = json!({"type":"response.completed","response":{
                "status":"completed","output":[{"type":"reasoning","id":"rs1","summary":summary,"encrypted_content":"opaque"}]
            }});
            let error = ResponseAccumulator::default()
                .parse_event(event.to_string().as_bytes())
                .unwrap_err();
            assert!(error.to_string().contains("sans contenu exploitable"));
        }
    }

    #[test]
    fn options_are_validated_and_summary_without_effort_is_sent() {
        for config in [
            json!({"model":""}),
            json!({"model":"test","reasoningEffort":"invalid"}),
            json!({"model":"test","reasoningSummary":9}),
            json!({"model":"test","textVerbosity":"verbose"}),
        ] {
            assert!(validate(&config).is_err());
        }
        let options = json!({"model":"test","reasoningSummary":"concise"});
        validate(&options).unwrap();
        let body = request_body(
            "test",
            &options,
            &LlmRequest::new("test", vec![Content::new("user").with_text("Hello")]),
        )
        .unwrap();
        assert_eq!(body["reasoning"]["summary"], "concise");
        assert!(body["reasoning"].get("effort").is_none());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn status_sanitizes_cli_output_and_refresh_uses_only_account_protocol() {
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::tempdir().unwrap();
        let program = directory.path().join("codex-fixture");
        let script = r#"#!/usr/bin/env python3
import json, sys
if sys.argv[1:] == ['-c', 'cli_auth_credentials_store="file"', 'login', 'status']:
    print('Logged in using ChatGPT credential-fixture-secret', file=sys.stderr)
else:
    assert sys.argv[1:] == ['-c', 'cli_auth_credentials_store="file"', 'app-server', '--listen', 'stdio://']
    init=json.loads(sys.stdin.readline()); assert init['method']=='initialize'
    print(json.dumps({'id':1,'result':{}}), flush=True)
    assert json.loads(sys.stdin.readline())['method']=='initialized'
    request=json.loads(sys.stdin.readline())
    assert request['method']=='account/read' and request['params']['refreshToken'] is True
    print(json.dumps({'id':2,'result':{'account':{'type':'chatgpt'}}}), flush=True)
    sys.stdin.read()
"#;
        std::fs::write(&program, script).unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o700)).unwrap();
        let status = cli_status(program.to_str().unwrap()).await;
        assert_eq!(status["authenticated"], true);
        assert!(!status.to_string().contains("credential-fixture-secret"));
        tokio::time::timeout(
            Duration::from_secs(3),
            refresh_with(program.to_str().unwrap()),
        )
        .await
        .unwrap()
        .unwrap();
    }
}
