//! Operations are ADK nodes; ADK owns ordering, state updates and continuation.
use adk_core::{Content, FunctionResponseData, Part, Tool};
use adk_graph::prelude::*;
use adk_tool::{FunctionTool, SimpleToolContext};
use serde_json::Value;
use std::{collections::HashMap, sync::Arc, time::Duration};

/// Pure, local tools that are available identically in the daemon and exported Rust.
pub fn tool_declarations() -> HashMap<String, Value> {
    let mut declarations = HashMap::from([
        (
            "inspect_json".into(),
            json!({"name":"inspect_json","description":"Inspecter la structure d'une valeur JSON","parameters":{"type":"object","properties":{"value":{}},"required":["value"]}}),
        ),
        (
            "format_text".into(),
            json!({"name":"format_text","description":"Transformer la casse d'un texte","parameters":{"type":"object","properties":{"text":{"type":"string"},"mode":{"type":"string","enum":["uppercase","lowercase","trim"]}},"required":["text","mode"]}}),
        ),
        (
            "delay".into(),
            json!({"name":"delay","description":"Attendre un délai en millisecondes (60 secondes maximum)","parameters":{"type":"object","properties":{"milliseconds":{"type":"integer","minimum":0,"maximum":60000}},"required":["milliseconds"]}}),
        ),
    ]);
    declarations.extend(crate::workspace_tools::declarations());
    declarations
}

fn tool_error(message: impl Into<String>) -> adk_core::AdkError {
    adk_core::AdkError::tool(message.into())
}

pub fn tool(name: &str) -> anyhow::Result<FunctionTool> {
    let name_owned = name.to_owned();
    anyhow::ensure!(
        tool_declarations().contains_key(name),
        "Outil non disponible : {name}"
    );
    Ok(
        FunctionTool::new(name, "Outil local Zedflow", move |_ctx, args| {
            let name = name_owned.clone();
            async move {
                match name.as_str() {
                    "inspect_json" => {
                        let value = args
                            .get("value")
                            .ok_or_else(|| tool_error("Argument value requis"))?;
                        let kind = match value {
                            Value::Null => "null",
                            Value::Bool(_) => "boolean",
                            Value::Number(_) => "number",
                            Value::String(_) => "string",
                            Value::Array(_) => "array",
                            Value::Object(_) => "object",
                        };
                        let size = match value {
                            Value::Array(v) => v.len(),
                            Value::Object(v) => v.len(),
                            Value::String(v) => v.chars().count(),
                            _ => 1,
                        };
                        let rows: Vec<Value> = match value {
                            Value::Object(values) => values
                                .iter()
                                .map(|(key, value)| json!({"key":key,"value":value}))
                                .collect(),
                            Value::Array(values) => values
                                .iter()
                                .enumerate()
                                .map(|(key, value)| json!({"key":key,"value":value}))
                                .collect(),
                            _ => vec![json!({"key":"value","value":value})],
                        };
                        Ok(json!({"type":kind,"size":size,"rows":rows}))
                    }
                    "format_text" => {
                        let text = args["text"]
                            .as_str()
                            .ok_or_else(|| tool_error("Argument text requis"))?;
                        let text = match args["mode"].as_str().unwrap_or("trim") {
                            "uppercase" => text.to_uppercase(),
                            "lowercase" => text.to_lowercase(),
                            "trim" => text.trim().to_owned(),
                            _ => {
                                return Err(tool_error(
                                    "Mode attendu : uppercase, lowercase ou trim",
                                ));
                            }
                        };
                        Ok(json!({"text":text}))
                    }
                    "delay" => {
                        let ms = args["milliseconds"]
                            .as_u64()
                            .filter(|v| *v <= 60_000)
                            .ok_or_else(|| {
                                tool_error("milliseconds doit être un entier entre 0 et 60000")
                            })?;
                        tokio::time::sleep(Duration::from_millis(ms)).await;
                        Ok(json!({"waitedMs":ms,"status":"completed"}))
                    }
                    _ => Err(tool_error("Outil inconnu")),
                }
            }
        })
        .with_read_only(true)
        .with_concurrency_safe(true),
    )
}

/// Recreate ADK channel semantics in generated artifacts using the same implementation.
pub fn state_schema(channels: &Value) -> anyhow::Result<adk_graph::StateSchema> {
    let mut schema = adk_graph::StateSchema::simple(&[
        "input",
        "output",
        "response",
        "messages",
        "toolCalls",
        "toolResults",
        "hasToolCalls",
        "modelResponse",
        "hasSteering",
        "hasFollowUp",
        "__zedflow:consumedMessages",
        "__zedflow:context",
    ]);
    for channel in channels.as_array().into_iter().flatten() {
        let name = channel["name"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Nom du canal requis"))?;
        let mut value = match channel["reducer"].as_str().unwrap_or("overwrite") {
            "overwrite" => adk_graph::Channel::new(name),
            "append" => adk_graph::Channel::list(name),
            "sum" => adk_graph::Channel::counter(name),
            other => anyhow::bail!("Reducer inconnu : {other}"),
        };
        if let Some(default) = channel.get("default") {
            value = value.with_default(default.clone());
        }
        schema.channels.insert(name.into(), value);
    }
    Ok(schema)
}

pub fn retry_policy(value: &Value) -> adk_graph::retry::RetryPolicy {
    use adk_graph::retry::{RetryOn, RetryPolicy};
    RetryPolicy::new(
        value["maxAttempts"]
            .as_u64()
            .and_then(|v| u32::try_from(v).ok())
            .unwrap_or(1),
    )
    .with_initial_delay(Duration::from_millis(
        value["initialDelayMs"].as_u64().unwrap_or(1000),
    ))
    .with_max_delay(Duration::from_millis(
        value["maxDelayMs"].as_u64().unwrap_or(60_000),
    ))
    .with_backoff_factor(value["backoffFactor"].as_f64().unwrap_or(2.0))
    .with_jitter(value["jitter"].as_f64().unwrap_or(0.0))
    .with_retry_on(if value["retryOn"] == "timeout" {
        RetryOn::Timeout
    } else {
        RetryOn::Any
    })
}

pub fn configure(
    mut graph: CompiledGraph,
    settings: &Value,
    node_retries: &[(&str, Value)],
) -> CompiledGraph {
    graph = graph.with_recursion_limit(
        settings["recursionLimit"]
            .as_u64()
            .and_then(|v| usize::try_from(v).ok())
            .unwrap_or(100),
    );
    if let Some(limit) = settings["maxConcurrency"]
        .as_u64()
        .and_then(|v| usize::try_from(v).ok())
    {
        graph = graph.with_max_concurrency(limit);
    }
    if settings["strictChannels"] == true {
        graph = graph.with_strict_channels();
    }
    let timeout = adk_graph::timeout::TimeoutPolicy {
        run_timeout: settings["timeoutMs"].as_u64().map(Duration::from_millis),
        idle_timeout: settings["idleTimeoutMs"]
            .as_u64()
            .map(Duration::from_millis),
        on_timeout: adk_graph::timeout::OnTimeout::Fail,
    };
    let mut defaults = adk_graph::graph::NodeDefaults::new().with_timeout(timeout);
    if settings["retry"].is_object() {
        defaults = defaults.with_retry(retry_policy(&settings["retry"]));
    }
    graph = graph.with_node_defaults(defaults);
    for (node, retry) in node_retries {
        graph = graph.with_node_retry(node, retry_policy(retry));
    }
    graph
}

/// Substitute placeholders once; values are never reinterpreted as templates.
pub fn render(template: &str, state: &State) -> String {
    let mut result = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find("{{") {
        result.push_str(&rest[..start]);
        let opening = &rest[start..];
        let after_open = &opening[2..];
        let Some(end) = after_open.find("}}") else {
            result.push_str(opening);
            return result;
        };
        let key = &after_open[..end];
        if let Some(value) = state.get(key) {
            result.push_str(
                &value
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| value.to_string()),
            );
        } else {
            result.push_str(&opening[..end + 4]);
        }
        rest = &after_open[end + 2..];
    }
    result.push_str(rest);
    result
}

pub async fn execute(
    kind: &str,
    config: &Value,
    ctx: NodeContext,
) -> adk_graph::error::Result<NodeOutput> {
    let field = config
        .get("field")
        .and_then(Value::as_str)
        .unwrap_or("output");
    match kind {
        "set" => {
            let value = config.get("value").cloned().unwrap_or(Value::Null);
            let value = if let Some(text) = value.as_str() {
                json!(render(text, &ctx.state))
            } else {
                value
            };
            Ok(NodeOutput::new().with_update(field, value))
        }
        "output" => {
            let template = config
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or("{{output}}");
            Ok(NodeOutput::new().with_update("response", json!(render(template, &ctx.state))))
        }
        "input" => {
            let node = config
                .get("nodeId")
                .and_then(Value::as_str)
                .unwrap_or("input");
            let consumed_key = format!("__zedflow:answerConsumed:{node}");
            let key = format!("answer:{}", node);
            if let Some(answer) = ctx.state.get(&key)
                && !answer.is_null()
                && (answer.get("__zedflowAnswerId").is_none()
                    || ctx.state.get(&consumed_key) != answer.get("__zedflowAnswerId"))
            {
                let value = if answer.get("__zedflowAnswerId").is_some() {
                    answer["value"].clone()
                } else {
                    answer.clone()
                };
                return Ok(NodeOutput::new()
                    .with_update(field, value)
                    .with_update(
                        &consumed_key,
                        answer
                            .get("__zedflowAnswerId")
                            .cloned()
                            .unwrap_or(Value::Null),
                    )
                    .with_update(
                        &format!("__zedflow:input:{field}"),
                        answer
                            .get("__zedflowAnswerId")
                            .cloned()
                            .unwrap_or(json!(ctx.step)),
                    )
                    .with_update(&key, Value::Null));
            }
            Ok(NodeOutput::interrupt_with_data(
                config
                    .get("prompt")
                    .and_then(Value::as_str)
                    .unwrap_or("Votre réponse"),
                config.clone(),
            ))
        }
        "condition" if crate::agent_capabilities::is_v2(config) => {
            let node = config["nodeId"].as_str().unwrap_or("condition");
            let value = zf_flows::node_contracts::parse_predicate(&config["predicate"])
                .and_then(|predicate| crate::predicates::evaluate(&predicate, &ctx.state))
                .map_err(|error| GraphError::NodeExecutionFailed {
                    node: node.into(),
                    message: error.to_string(),
                })?;
            Ok(NodeOutput::new().with_update(&format!("__zedflow:condition:{node}"), json!(value)))
        }
        "condition" => Ok(NodeOutput::new()),
        "tool" => execute_tools(config, &ctx).await,
        _ => Err(GraphError::NodeExecutionFailed {
            node: kind.into(),
            message: "Unsupported operation".into(),
        }),
    }
}

pub fn stopped(path: &str) -> NodeOutput {
    NodeOutput::interrupt_with_data(
        "Exécution arrêtée",
        json!({"kind":"stopped","nodePath":path,"prompt":"Exécution arrêtée"}),
    )
}

fn skipped_route(path: &str, config: &Value, reason: &str) -> NodeOutput {
    let output = NodeOutput::new().with_event(StreamEvent::custom(
        path,
        "route_skipped",
        json!({"reason":reason}),
    ));
    // An explicit fallback prevents a later passage from consuming a result
    // retained from an earlier visit. Existing route definitions keep their state.
    if let Some(value) = config.get("fallback") {
        output.with_update(config["field"].as_str().unwrap_or("output"), value.clone())
    } else {
        output
    }
}

/// One ADK operation. Queue consumption is committed with its graph state; the
/// graph's edges decide when this operation is reached.
pub async fn execute_with_services(
    kind: &str,
    config: &Value,
    ctx: NodeContext,
    path: &str,
    services: Arc<crate::runtime::RunServices>,
) -> adk_graph::error::Result<NodeOutput> {
    match kind {
        "route" => {
            if services.cancel.is_cancelled() {
                return Ok(stopped(path));
            }
            let error = |message: String| GraphError::NodeExecutionFailed {
                node: path.into(),
                message,
            };
            let Some(host) = services.dynamic_capabilities() else {
                // A conditional plug has a local continuation when no bridge
                // is active. An explicitly selected route must still resolve.
                if config["invocation"] == "condition" && config["routeId"].is_null() {
                    return Ok(skipped_route(path, config, "no_active_bridge"));
                }
                return Err(error(
                    "Ce point de branchement requiert un Runtime Graph résolu".into(),
                ));
            };
            let field = config["inputField"].as_str().unwrap_or("input");
            let input = ctx
                .state
                .get(field)
                .cloned()
                .ok_or_else(|| error(format!("Canal absent : {field}")))?;
            let outcome = host
                .invoke_branch(crate::runtime::BranchInvocation {
                    path: path.into(),
                    branch: config["branch"].as_str().unwrap_or_default().into(),
                    invocation: if config["invocation"] == "condition" {
                        zf_flows::composition::InvocationKind::Condition
                    } else {
                        zf_flows::composition::InvocationKind::Node
                    },
                    route_id: config["routeId"].as_str().map(str::to_owned),
                    call_id: format!("{}:{path}:{}", ctx.config.thread_id, ctx.step),
                    input,
                    caller_state: ctx.state,
                })
                .await
                .map_err(|e| error(format!("{e:#}")))?;
            let target = config["field"].as_str().unwrap_or("output");
            match outcome {
                crate::runtime::RouteOutcome::Skipped { reason } => {
                    Ok(skipped_route(path, config, &reason))
                }
                crate::runtime::RouteOutcome::Completed { result, .. } => {
                    Ok(NodeOutput::new().with_update(target, result))
                }
                crate::runtime::RouteOutcome::Handoff { result, .. } => Ok(NodeOutput::new()
                    .with_update(target, result)
                    .with_goto([adk_graph::END])),
                crate::runtime::RouteOutcome::Launched { .. } => Ok(NodeOutput::new().with_update(
                    target,
                    serde_json::to_value(outcome).map_err(|e| error(e.to_string()))?,
                )),
                crate::runtime::RouteOutcome::Waiting {
                    wait,
                    visit_id,
                    thread_id,
                } => Ok(NodeOutput::interrupt_with_data(
                    "Flow appelé en attente",
                    json!({"kind":"route","nodePath":path,"visitId":visit_id,"threadId":thread_id,"childWait":wait}),
                )),
            }
        }
        "await_route" => {
            if services.cancel.is_cancelled() {
                return Ok(stopped(path));
            }
            let error = |message: String| GraphError::NodeExecutionFailed {
                node: path.into(),
                message,
            };
            let field = config["inputField"].as_str().unwrap_or("output");
            let handle = ctx
                .state
                .get(field)
                .ok_or_else(|| error(format!("Handle d’appel absent : {field}")))?;
            let visit = handle["visitId"]
                .as_str()
                .or_else(|| handle["__zedflowRoute"]["visitId"].as_str())
                .ok_or_else(|| error("Handle d’appel invalide".into()))?;
            let host = services
                .dynamic_capabilities()
                .ok_or_else(|| error("Runtime Graph requis".into()))?;
            let outcome = host
                .await_visit(path, visit, &ctx.state)
                .await
                .map_err(|e| error(format!("{e:#}")))?;
            let target = config["field"].as_str().unwrap_or("output");
            match outcome {
                crate::runtime::RouteOutcome::Completed { result, .. } => {
                    Ok(NodeOutput::new().with_update(target, result))
                }
                crate::runtime::RouteOutcome::Handoff { result, .. } => Ok(NodeOutput::new()
                    .with_update(target, result)
                    .with_goto([adk_graph::END])),
                crate::runtime::RouteOutcome::Waiting {
                    wait,
                    visit_id,
                    thread_id,
                } => Ok(NodeOutput::interrupt_with_data(
                    "Flow appelé en attente",
                    json!({"kind":"route","nodePath":path,"visitId":visit_id,"threadId":thread_id,"childWait":wait}),
                )),
                crate::runtime::RouteOutcome::Launched { .. }
                | crate::runtime::RouteOutcome::Skipped { .. } => {
                    Err(error("L’appel attendu n’a pas produit de résultat".into()))
                }
            }
        }
        "tool" => execute_tools_with_services(config, &ctx, Some((&services, path))).await,
        "context" => {
            if services.cancel.is_cancelled() {
                return Ok(stopped(path));
            }
            let snapshot = services.system_prompt();
            services
                .emit(json!({"type":"context_loaded","nodePath":path,"step":ctx.step}))
                .await;
            Ok(NodeOutput::new().with_update(
                "__zedflow:context",
                json!({"loaded":true,"characters":snapshot.chars().count()}),
            ))
        }
        "steering" | "inbox" => {
            if services.cancel.is_cancelled() {
                return Ok(stopped(path));
            }
            let field = config["field"].as_str().unwrap_or("input");
            let consumed: Vec<String> = ctx
                .state
                .get("__zedflow:consumedMessages")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect();
            if kind == "steering" && ctx.state.get("hasToolCalls") == Some(&json!(true)) {
                return Err(GraphError::NodeExecutionFailed {
                    node: path.into(),
                    message: "Le lot d'outils doit terminer avant le contrôle steering".into(),
                });
            }
            let mut message = services.claim_message("steering", &consumed).await;
            if message.is_none() && kind == "inbox" {
                message = services.claim_message("followup", &consumed).await;
            }
            if let Some(message) = message {
                let mut consumed = consumed;
                let id = message["id"]
                    .as_str()
                    .ok_or_else(|| GraphError::NodeExecutionFailed {
                        node: path.into(),
                        message: "Identifiant de message absent".into(),
                    })?;
                consumed.push(id.into());
                return Ok(NodeOutput::new()
                    .with_update(field, message["text"].clone())
                    .with_update(&format!("__zedflow:input:{field}"), json!(id))
                    .with_update("__zedflow:consumedMessages", json!(consumed))
                    .with_update("hasSteering", json!(message["kind"] == "steering"))
                    .with_update("hasFollowUp", json!(message["kind"] == "followup")));
            }
            if kind == "steering" {
                return Ok(NodeOutput::new().with_update("hasSteering", json!(false)));
            }
            let mut input_config = config.clone();
            input_config["field"] = json!(field);
            let mut result = execute("input", &input_config, ctx).await?;
            if result.interrupt.is_some() {
                let prompt = config["prompt"].as_str().unwrap_or("Sur quoi continuer ?");
                result.interrupt = NodeOutput::interrupt_with_data(
                    prompt,
                    json!({
                        "kind":"input", "nodePath":path, "prompt":prompt,
                        "responseType":"text", "field":field,
                    }),
                )
                .interrupt;
            }
            Ok(result)
        }
        _ => {
            if services.cancel.is_cancelled() {
                return Ok(stopped(path));
            }
            let mut result = execute(kind, config, ctx).await?;
            if kind == "input" && result.interrupt.is_some() {
                let mut payload = config.clone();
                payload["kind"] = json!("input");
                payload["nodePath"] = json!(path);
                result.interrupt = NodeOutput::interrupt_with_data(
                    config["prompt"].as_str().unwrap_or("Votre réponse"),
                    payload,
                )
                .interrupt;
            }
            Ok(result)
        }
    }
}

async fn execute_tools(config: &Value, ctx: &NodeContext) -> adk_graph::error::Result<NodeOutput> {
    execute_tools_with_services(config, ctx, None).await
}

async fn execute_tools_with_services(
    config: &Value,
    ctx: &NodeContext,
    runtime: Option<(&Arc<crate::runtime::RunServices>, &str)>,
) -> adk_graph::error::Result<NodeOutput> {
    let node = config["nodeId"].as_str().unwrap_or("tool");
    let field = config["field"].as_str().unwrap_or("output");
    let history_field = config["historyField"].as_str().unwrap_or("messages");
    let calls_field = config["toolCallsField"].as_str().unwrap_or("toolCalls");
    let error = |message: String| GraphError::NodeExecutionFailed {
        node: node.into(),
        message,
    };
    let single = config["tool"] == "execute_next_call";
    let dispatch = config["tool"] == "execute_calls" || single;
    let calls = if dispatch {
        ctx.state
            .get(calls_field)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    } else {
        let args = match config["inputField"].as_str().filter(|s| !s.is_empty()) {
            Some(key) => ctx
                .state
                .get(key)
                .cloned()
                .ok_or_else(|| error(format!("Canal d'entrée absent : {key}")))?,
            None => config
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({})),
        };
        vec![
            json!({"name":config["tool"].as_str().unwrap_or("inspect_json"),"args":args,"id":format!("{}:{node}:{}",ctx.config.thread_id,ctx.step)}),
        ]
    };
    let mut history: Vec<Content> = ctx
        .state
        .get(history_field)
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map_err(|e| error(format!("Historique invalide : {e}")))?
        .unwrap_or_default();
    let mut response = Content::new("user");
    let mut output = NodeOutput::new();
    let mut results = if single {
        ctx.state
            .get("toolResults")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    let take = if single {
        calls.len().min(1)
    } else {
        calls.len()
    };
    let mut processed = 0;
    for (index, call) in calls.iter().take(take).enumerate() {
        let name = call["name"]
            .as_str()
            .ok_or_else(|| error("Nom de l'outil requis".into()))?;
        let call_id = call["id"]
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| format!("{}:{node}:{}:{index}", ctx.config.thread_id, ctx.step));
        let args = call.get("args").cloned().unwrap_or_else(|| json!({}));
        if !args.is_object() {
            return Err(error(format!("Arguments invalides pour {name}")));
        }
        output = output.with_event(StreamEvent::custom(
            node,
            "tool_call",
            json!({"id":call_id,"name":name,"arguments":args,"ui":config["ui"]}),
        ));
        let authorization = if dispatch
            && (crate::agent_capabilities::is_v2(config) || call.get("provenance").is_some())
        {
            match runtime {
                Some((services, _)) => crate::agent_capabilities::authorize_call(services, call)
                    .await
                    .map(Some),
                None => Err(anyhow::anyhow!(
                    "Un appel v2 nécessite une provenance durable"
                )),
            }
        } else {
            Ok(None)
        };
        let result = if let Err(error) = &authorization {
            json!({"error":format!("Appel refusé : {error}"),"denied":true})
        } else if let Some((services, path)) = runtime {
            let (owner, identity) = authorization.ok().flatten().unwrap_or_else(|| {
                (
                    path.to_owned(),
                    format!("{}:{}:{call_id}", ctx.config.thread_id, ctx.step),
                )
            });
            if let Some(dynamic) = services.dynamic_capabilities() {
                dynamic
                    .resume_state(&owner, &identity, &ctx.state)
                    .await
                    .map_err(|e| error(format!("{e:#}")))?;
            }
            if services.cancel.is_cancelled() {
                json!({"error":"Exécution annulée avant cet appel", "cancelled":true})
            } else if matches!(name, "inspect_json" | "format_text" | "delay") {
                let context = SimpleToolContext::new(node)
                    .with_session_id(&services.id)
                    .with_function_call_id(&call_id);
                let fixture = tool(name).map_err(|e| error(e.to_string()))?;
                services.emit(json!({"type":"tool_call","nodePath":path,"callId":identity,"name":name,"arguments":args})).await;
                let result = tokio::select! {
                    biased;
                    _ = services.cancel.cancelled() => json!({"error":"Exécution annulée", "cancelled":true}),
                    result = fixture.execute(Arc::new(context), args) => match result {
                        Ok(result) => result,
                        Err(error) => json!({"error":error.to_string()}),
                    },
                };
                services.emit(json!({"type":"tool_result","nodePath":path,"callId":identity,"name":name,"status":if result.get("error").is_some() {"failed"} else {"completed"},"result":result,"error":result.get("error")})).await;
                result
            } else {
                match services.execute_tool(&owner, &identity, name, args).await {
                    Ok(result) => result,
                    Err(error) => {
                        json!({"error":error.to_string(),"cancelled":services.cancel.is_cancelled()})
                    }
                }
            }
        } else {
            let tool = tool(name).map_err(|e| error(e.to_string()))?;
            let context = SimpleToolContext::new(node)
                .with_session_id(&ctx.config.thread_id)
                .with_function_call_id(&call_id);
            match tool.execute(Arc::new(context), args).await {
                Ok(result) => result,
                Err(error) => json!({"error":error.to_string()}),
            }
        };
        if result["__zedflowRoute"]["status"] == "waiting" {
            // A suspended routed call is still the same pending model request.
            // Prior completed calls can commit, but this one has no response yet.
            if !response.parts.is_empty() {
                history.push(response);
            }
            return Ok(output.with_updates(std::collections::HashMap::from([
                (history_field.into(),json!(history)),(calls_field.into(),json!(&calls[processed..])),
                ("toolResults".into(),json!(results)),("hasToolCalls".into(),json!(true)),
            ])).with_interrupt(adk_graph::interrupt::interrupt_with_data("Flow appelé en attente",json!({"kind":"route","nodePath":runtime.map_or(node,|(_,path)|path),"visitId":result["__zedflowRoute"]["visitId"],"threadId":result["__zedflowRoute"]["threadId"],"childWait":result["__zedflowRoute"]["wait"]}))));
        }
        if result["__zedflowRoute"]["status"] == "handoff" {
            output = output.with_goto([adk_graph::END]);
        }
        output = output.with_event(StreamEvent::custom(
            node,
            "tool_result",
            json!({"id":call_id,"name":name,"result":result,"ui":config["ui"]}),
        ));
        response.parts.push(Part::FunctionResponse {
            function_response: FunctionResponseData::new(name, result.clone()),
            id: call["id"].as_str().map(str::to_owned),
            annotations: None,
        });
        results.push(json!({"id":call_id,"name":name,"result":result}));
        processed += 1;
        if result["__zedflowRoute"]["status"] == "handoff" {
            for remaining in calls.iter().skip(processed) {
                let name = remaining["name"].as_str().unwrap_or("unknown");
                let result =
                    json!({"notExecuted":true,"reason":"Contrôle transféré vers un autre flow"});
                response.parts.push(Part::FunctionResponse {
                    function_response: FunctionResponseData::new(name, result.clone()),
                    id: remaining["id"].as_str().map(str::to_owned),
                    annotations: None,
                });
                results.push(json!({"id":remaining["id"],"name":name,"result":result}));
            }
            processed = calls.len();
            break;
        }
        ctx.report_progress();
        if runtime.is_some_and(|(services, _)| services.cancel.is_cancelled()) {
            break;
        }
    }
    // An abort closes every request in this batch. Commit these responses before
    // the graph reaches its stopped gate, preserving valid model call/result pairs.
    if runtime.is_some_and(|(services, _)| services.cancel.is_cancelled()) {
        for call in calls.iter().skip(processed) {
            let name = call["name"].as_str().unwrap_or("unknown");
            let result = json!({"error":"Appel non exécuté : exécution annulée", "cancelled":true});
            response.parts.push(Part::FunctionResponse {
                function_response: FunctionResponseData::new(name, result.clone()),
                id: call["id"].as_str().map(str::to_owned),
                annotations: None,
            });
            results.push(json!({"id":call["id"],"name":name,"result":result}));
        }
        processed = calls.len();
    }
    let value = if dispatch {
        json!(results)
    } else {
        results
            .first()
            .map(|r| r["result"].clone())
            .unwrap_or(Value::Null)
    };
    output = output
        .with_update(field, value)
        .with_update("toolResults", json!(results));
    if dispatch {
        if !response.parts.is_empty() {
            history.push(response);
        }
        output = output
            .with_update(history_field, json!(history))
            .with_update(calls_field, json!(&calls[processed..]))
            .with_update("hasToolCalls", json!(processed < calls.len()));
    }
    Ok(output)
}
