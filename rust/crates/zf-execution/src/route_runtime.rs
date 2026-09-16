//! Durable routing between explicitly resolved flow instances. Each child is a
//! native ADK invocation with its own checkpoint frontier, not a second executor.
use adk_graph::{ExecutionConfig, State, checkpoint::Checkpointer, error::GraphError};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{
    collections::{BTreeMap, HashMap},
    sync::{Arc, Mutex, RwLock, Weak},
};
use tokio::{sync::Mutex as AsyncMutex, task::JoinHandle};
use zf_compiler::prepared_model::PreparedRuntime;
use zf_core::{
    identity::{Permission, Revision, Scope},
    types::{DataType, TypeRegistry, validate_value},
};
use zf_flows::{
    composition::{InvocationKind, RouteMode},
    flow_contract,
};
use zf_runtime::{
    event_sink::EventSink,
    materialize::RuntimePrimitives,
    runtime::{BranchInvocation, DynamicCapabilities, RouteOutcome, RunServices},
};
use zf_storage::data::DataError;

tokio::task_local! {
    static CURRENT_VISIT: (String, usize);
}

/// A compiled source module retained by a portable Cargo artifact.
pub type NativeFactory = Arc<
    dyn Fn(Arc<RunServices>, Arc<dyn Checkpointer>, &str) -> Result<adk_graph::CompiledGraph>
        + Send
        + Sync,
>;

type VisitTasks = BTreeMap<String, JoinHandle<Result<RouteOutcome>>>;

pub struct RouteRuntime {
    prepared: Arc<PreparedRuntime>,
    services: Weak<RunServices>,
    this: Weak<Self>,
    checkpoints: Arc<dyn Checkpointer>,
    native_factories: RwLock<BTreeMap<String, NativeFactory>>,
    sender: Option<EventSink>,
    tools: BTreeMap<String, Vec<Value>>,
    resume_channels: Vec<String>,
    locks: Mutex<HashMap<String, Arc<AsyncMutex<()>>>>,
    tasks: AsyncMutex<VisitTasks>,
    active_launches: AtomicUsize,
    launch_changes: tokio::sync::watch::Sender<()>,
}

struct ActiveLaunch(Arc<RouteRuntime>);
impl Drop for ActiveLaunch {
    fn drop(&mut self) {
        self.0.active_launches.fetch_sub(1, Ordering::AcqRel);
        self.0.launch_changes.send_replace(());
    }
}

fn hash(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

impl RouteRuntime {
    pub fn new(
        prepared: PreparedRuntime,
        services: &Arc<RunServices>,
        checkpoints: Arc<dyn Checkpointer>,
        sender: Option<EventSink>,
    ) -> Result<Arc<Self>> {
        prepared.validate(&RuntimePrimitives)?;
        ensure!(
            services.content_store().is_some(),
            "Composed routing requires durable content storage"
        );
        let mut tools: BTreeMap<String, Vec<Value>> = BTreeMap::new();
        let mut resume_channels = Vec::new();
        for (instance, flow) in &prepared.flows {
            resume_channels.extend(
                zf_flows::schema::answer_paths(&flow.composition)?
                    .into_iter()
                    .map(|path| format!("answer:{instance}/{path}")),
            );
        }
        for (route_id, route) in prepared
            .graph
            .routes
            .iter()
            .filter(|(_, r)| r.invocation == InvocationKind::Tool)
        {
            let source = &prepared.flows[&route.from.instance];
            let name = route
                .tool_name
                .as_ref()
                .context("Tool route has no public name")?;
            ensure!(
                !zf_runtime::operations::tool_declarations().contains_key(name),
                "Route cannot shadow a built-in tool: {name}"
            );
            let input = schema(&route.input, &prepared.graph.types, 0)?;
            for node in flow_contract::requesters(&source.exports, &route.from.port) {
                if !source.composition.nodes.iter().any(|item| {
                    item.id == node && matches!(item.data.kind.as_str(), "model" | "agent")
                }) {
                    continue;
                }
                let path = format!("{}/{node}", route.from.instance);
                tools.entry(path).or_default().push(json!({"name":name,"description":format!("Route {route_id}"),"parameters":{"type":"object","properties":{"input":input},"required":["input"],"additionalProperties":false}}));
            }
        }
        Ok(Arc::new_cyclic(|this| Self {
            prepared: Arc::new(prepared),
            services: Arc::downgrade(services),
            this: this.clone(),
            checkpoints,
            native_factories: RwLock::new(BTreeMap::new()),
            sender,
            tools,
            resume_channels,
            locks: Mutex::new(HashMap::new()),
            tasks: AsyncMutex::new(BTreeMap::new()),
            active_launches: AtomicUsize::new(0),
            launch_changes: tokio::sync::watch::channel(()).0,
        }))
    }

    /// Install all compiled modules before any routed visit starts.
    pub fn set_native_factories(&self, factories: BTreeMap<String, NativeFactory>) -> Result<()> {
        ensure!(
            factories.keys().eq(self.prepared.flows.keys()),
            "native factories must cover exactly the resolved instances"
        );
        let mut current = self
            .native_factories
            .write()
            .unwrap_or_else(|poison| poison.into_inner());
        ensure!(current.is_empty(), "native factories are already installed");
        *current = factories;
        Ok(())
    }
    pub fn build_instance_with_revisions(
        &self,
        instance: &str,
        projection: &zf_flows::schema::Composition,
        controller: Option<Arc<zf_runtime::revisions::RevisionRuntime>>,
        definition_revision: Option<&str>,
    ) -> Result<adk_graph::CompiledGraph> {
        let services = self.services()?;
        let scope = format!("{instance}/");
        let factory = self
            .native_factories
            .read()
            .unwrap_or_else(|poison| poison.into_inner())
            .get(instance)
            .cloned();
        // A revision boundary may rebuild this entry from a different source.
        // The initial module is valid only for projections of its own source.
        let source = self
            .prepared
            .flows
            .get(instance)
            .context("Unknown native instance")?;
        let mut exact_projection = false;
        if factory.is_some() {
            let expected = serde_json::to_value(projection)?;
            for entry in source.exports.entries.keys() {
                if serde_json::to_value(flow_contract::at_entry(&source.composition, entry)?)?
                    == expected
                {
                    exact_projection = true;
                    break;
                }
            }
        }
        let initial_revision =
            zf_runtime::revisions::RevisionDefinition::from_prepared(&self.prepared, instance)?
                .revision();
        let native = factory
            .filter(|_| {
                exact_projection
                    && definition_revision.is_none_or(|revision| revision == initial_revision)
            })
            .map(|factory| factory(services.clone(), self.checkpoints.clone(), &scope))
            .transpose()?;
        let controller = controller.or_else(|| services.revisions());
        zf_runtime::materialize::build_scope_with_native_and_revisions(
            projection,
            self.sender.clone(),
            &scope,
            Some(services),
            Some(self.checkpoints.clone()),
            native.as_ref(),
            controller,
        )
    }

    fn services(&self) -> Result<Arc<RunServices>> {
        self.services
            .upgrade()
            .context("Runtime owner has been released")
    }
    fn instance<'a>(&'a self, path: &'a str) -> Result<&'a str> {
        let instance = path
            .rsplit_once('/')
            .map_or("root", |(instance, _)| instance);
        ensure!(
            self.prepared.flows.contains_key(instance),
            "Unknown route caller instance: {instance}"
        );
        Ok(instance)
    }
    fn normalized_path(&self, path: &str) -> String {
        if path.contains('/') {
            path.into()
        } else {
            format!("root/{path}")
        }
    }
    async fn status(&self, key: &str, value: Value) -> Result<()> {
        let services = self.services()?;
        services
            .content_store()
            .context("Content storage unavailable")?
            .put_record(&services.id, "route-status", key, &value)
            .await?;
        services
            .emit(json!({"type":"route_status","visitId":key,"status":value}))
            .await;
        Ok(())
    }

    async fn store_plan(&self, plan: &PreparedRuntime) -> Result<String> {
        let services = self.services()?;
        let store = services
            .content_store()
            .context("Content storage unavailable")?;
        let value = json!(plan);
        let reference = store.intern(&value).await?;
        store
            .claim_record(
                &services.id,
                "runtime-graph-definitions",
                &reference,
                &value,
            )
            .await?;
        Ok(reference)
    }
    async fn plan_at(&self, reference: Option<&str>) -> Result<Arc<PreparedRuntime>> {
        if let Some(reference) = reference {
            let store = self
                .services()?
                .content_store()
                .context("Content storage unavailable")?;
            Ok(Arc::new(serde_json::from_value(
                store.resolve(reference).await?,
            )?))
        } else {
            // Legacy visits predate plan capture and retain their initial graph.
            Ok(self.prepared.clone())
        }
    }
    async fn latest_plan(&self) -> Result<PreparedRuntime> {
        let services = self.services()?;
        let store = services
            .content_store()
            .context("Content storage unavailable")?;
        Ok(
            zf_runtime::revisions::latest_runtime_graph(&store, &services.id)
                .await?
                .unwrap_or_else(|| self.prepared.as_ref().clone()),
        )
    }
    async fn pinned_plan(&self, fallback: Option<&str>) -> Result<(String, Arc<PreparedRuntime>)> {
        let services = self.services()?;
        let store = services
            .content_store()
            .context("Content storage unavailable")?;
        let boundary = zf_runtime::revisions::current_revision();
        let pin_key = boundary
            .as_ref()
            .map(|pin| {
                hash(&format!(
                    "{}\0{}",
                    pin["threadId"].as_str().unwrap_or_default(),
                    pin["step"]
                ))
            })
            .or_else(|| fallback.map(|identity| hash(&format!("invocation\0{identity}"))));
        if let Some(key) = &pin_key
            && let Some(pin) = store
                .record(&services.id, "runtime-graph-steps", key)
                .await?
        {
            let reference = pin["graphRef"]
                .as_str()
                .context("Graph step reference absent")?
                .to_owned();
            return Ok((reference.clone(), self.plan_at(Some(&reference)).await?));
        }
        let plan = self.latest_plan().await?;
        let reference = self.store_plan(&plan).await?;
        if let Some(key) = pin_key {
            store.claim_record(&services.id, "runtime-graph-steps", &key,
                &json!({"graphRef":reference,"threadId":boundary.as_ref().map(|v| &v["threadId"]),"step":boundary.as_ref().map(|v| &v["step"])})).await?;
            let pin = store
                .record(&services.id, "runtime-graph-steps", &key)
                .await?
                .context("Graph step claim absent")?;
            let reference = pin["graphRef"]
                .as_str()
                .context("Graph step reference absent")?
                .to_owned();
            return Ok((reference.clone(), self.plan_at(Some(&reference)).await?));
        }
        Ok((reference, Arc::new(plan)))
    }

    /// Seed only declared datasets. Restarts retain the original seed instead of
    /// applying new defaults or a resume answer to an existing entity head.
    pub async fn initialize(&self, root_input: &State) -> Result<()> {
        let services = self.services()?;
        let store = services
            .content_store()
            .context("Content storage unavailable")?;
        let reference = self.store_plan(&self.prepared).await?;
        store
            .claim_record(
                &services.id,
                "runtime-graph-heads",
                "current",
                &json!({"graphRef":reference}),
            )
            .await?;
        let seed = if let Some(seed) = store.record(&services.id, "route-runtime", "seed").await? {
            seed
        } else {
            let mut values = BTreeMap::new();
            for (instance, flow) in &self.prepared.flows {
                let mut state: State = flow
                    .composition
                    .channels
                    .iter()
                    .filter_map(|channel| {
                        channel
                            .default
                            .clone()
                            .map(|value| (channel.name.clone(), value))
                    })
                    .collect();
                if *instance == self.prepared.graph.entry.instance {
                    state.extend(root_input.clone());
                }
                values.insert(instance.clone(), self.dataset_values(instance, &state)?);
            }
            let seed = json!(values);
            store
                .claim_record(&services.id, "route-runtime", "seed", &seed)
                .await?;
            store
                .record(&services.id, "route-runtime", "seed")
                .await?
                .context("Seed claim disappeared")?
        };
        for (instance, values) in seed.as_object().context("Invalid runtime seed")? {
            self.publish_values(instance, values.clone(), &format!("seed:{instance}"), None)
                .await?;
        }
        self.share().await?;
        // A launched child is durable even if the parent already checkpointed
        // past the launch node when the daemon stopped.
        for record in store
            .records(&services.id)
            .await?
            .into_iter()
            .filter(|r| r.kind == "route-visits")
        {
            let visit = store.resolve(&record.value_ref).await?;
            if visit["mode"] == "launch" && !self.terminal(&record.key).await? {
                self.spawn_visit(record.key, visit).await?;
            }
        }
        Ok(())
    }

    fn dataset_values(&self, instance: &str, state: &State) -> Result<Value> {
        let flow = self
            .prepared
            .flows
            .get(instance)
            .context("Unknown dataset owner")?;
        let mut values = serde_json::Map::new();
        for (name, channel) in &flow.exports.data {
            if let Some(value) = state.get(channel) {
                validate_value(
                    &flow.exports.contract.data[name].data_type,
                    value,
                    &self.prepared.graph.types,
                )
                .map_err(|error| anyhow::anyhow!("Dataset {instance}/{name}: {error:?}"))?;
                values.insert(name.clone(), value.clone());
            }
        }
        Ok(Value::Object(values))
    }

    async fn publish_values(
        &self,
        instance: &str,
        values: Value,
        publication_id: &str,
        captured_heads: Option<&Value>,
    ) -> Result<()> {
        let services = self.services()?;
        let registry = services
            .data_registry()
            .context("Data registry unavailable")?;
        let publication_lock = {
            let mut locks = self.locks.lock().unwrap_or_else(|p| p.into_inner());
            locks
                .entry(format!("data:{instance}"))
                .or_insert_with(|| Arc::new(AsyncMutex::new(())))
                .clone()
        };
        let _publication_guard = publication_lock.lock().await;
        let scope = Scope::Flow(instance.into());
        let key = hash(publication_id);
        let intent = if let Some(intent) = services.read_record("route-publications", &key).await? {
            ensure!(
                intent["instance"] == instance && intent["values"] == values,
                "Dataset publication identity reused"
            );
            intent
        } else {
            let mut expected = BTreeMap::new();
            let mut unchanged = Vec::new();
            for (name, value) in values.as_object().context("Invalid dataset map")? {
                let channel_key = hash(&format!("{instance}\0{name}"));
                if let Some(view) = services
                    .read_record("route-channel-views", &channel_key)
                    .await?
                {
                    if view["value"] == *value {
                        unchanged.push(name.clone());
                    }
                    expected.insert(
                        name.clone(),
                        Some(serde_json::from_value(view["revision"].clone())?),
                    );
                    continue;
                }
                match registry.snapshot(&scope, name).await {
                    Ok(snapshot) => {
                        if snapshot.value.as_ref() == value {
                            unchanged.push(name.clone());
                        }
                        expected.insert(name.clone(), Some(snapshot.revision));
                    }
                    Err(DataError::NotFound) => {
                        expected.insert(name.clone(), None);
                    }
                    Err(error) => return Err(error.into()),
                }
            }
            if let Some(heads) = captured_heads {
                for name in values.as_object().context("Invalid dataset map")?.keys() {
                    expected.insert(name.clone(), serde_json::from_value(heads[name].clone())?);
                }
                // A concurrent publication must conflict even if its bytes happen
                // to match this output. Its revision is a distinct causal fact.
                unchanged.clear();
            }
            let intent = json!({"instance":instance,"values":values,"expected":expected,"unchanged":unchanged});
            services
                .persist_record("route-publications", &key, &intent)
                .await?;
            intent
        };
        for (name, value) in values.as_object().context("Invalid dataset map")? {
            let expected: Option<Revision> =
                serde_json::from_value(intent["expected"][name].clone())?;
            let unchanged = intent["unchanged"]
                .as_array()
                .is_some_and(|names| names.contains(&json!(name)));
            let revision = if unchanged {
                expected
                    .clone()
                    .context("Unchanged dataset has no revision")?
            } else {
                registry
                    .publish_unique(
                        &scope,
                        name,
                        expected.as_ref(),
                        value,
                        &format!("{key}:{name}"),
                    )
                    .await?
                    .revision
            };
            let channel_key = hash(&format!("{instance}\0{name}"));
            let prior = services
                .read_record("route-channel-views", &channel_key)
                .await?;
            // Replaying an old publication must not move this channel's view
            // backwards after a later publication. Alias edits do not change it:
            // an unchanged ADK channel therefore preserves those edits.
            if prior.is_none()
                || prior
                    .as_ref()
                    .is_some_and(|v| v["revision"] == json!(expected))
            {
                services.content_store().context("Content storage unavailable")?.put_record(&services.id,"route-channel-views",&channel_key,&json!({"instance":instance,"dataset":name,"revision":revision,"value":value})).await?;
            }
        }
        self.share().await
    }

    async fn publish_state(
        &self,
        instance: &str,
        state: &State,
        publication_id: &str,
    ) -> Result<()> {
        self.publish_values(
            instance,
            self.dataset_values(instance, state)?,
            publication_id,
            None,
        )
        .await
    }

    async fn share(&self) -> Result<()> {
        let services = self.services()?;
        let registry = services
            .data_registry()
            .context("Data registry unavailable")?;
        for binding in self.prepared.graph.data_bindings.values() {
            let source_scope = Scope::Flow(binding.from.instance.clone());
            let source = match registry.snapshot(&source_scope, &binding.from.port).await {
                Ok(value) => value,
                Err(DataError::NotFound) => continue,
                Err(error) => return Err(error.into()),
            };
            let permission = if binding.permissions.write {
                Permission::Write
            } else {
                Permission::Read
            };
            for (scope, alias) in [
                (
                    Scope::Bridge(binding.bridge.clone()),
                    format!("{}/{}", binding.from.instance, binding.from.port),
                ),
                (
                    Scope::Flow(binding.to.instance.clone()),
                    binding.to.port.clone(),
                ),
            ] {
                match registry.snapshot(&scope, &alias).await {
                    Ok(target) => ensure!(
                        target.entity_id == source.entity_id,
                        "A dataset binding cannot replace another entity"
                    ),
                    Err(DataError::NotFound) => {
                        registry
                            .grant(
                                &source_scope,
                                &binding.from.port,
                                &scope,
                                &alias,
                                permission,
                            )
                            .await?
                    }
                    Err(error) => return Err(error.into()),
                }
            }
        }
        Ok(())
    }

    async fn terminal(&self, visit_id: &str) -> Result<bool> {
        let status = self
            .services()?
            .read_record("route-status", visit_id)
            .await?;
        Ok(status.is_some_and(|v| matches!(v["status"].as_str(), Some("completed" | "handoff"))))
    }

    async fn invoke_branch_impl(&self, invocation: BranchInvocation) -> Result<RouteOutcome> {
        let services = self.services()?;
        let instance = self.instance(&invocation.path)?;
        let node = invocation
            .path
            .rsplit('/')
            .next()
            .context("Missing caller node")?;
        ensure!(
            flow_contract::requester_accepts(
                &self.prepared.flows[instance].composition,
                node,
                invocation.invocation
            ),
            "Node cannot issue this invocation kind"
        );
        ensure!(
            flow_contract::can_request(
                &self.prepared.flows[instance].exports,
                &invocation.branch,
                node
            ),
            "Branch does not belong to this node"
        );
        // Exported datasets advance at the caller's real passage, including a
        // condition which ultimately selects no route.
        self.publish_state(
            instance,
            &invocation.caller_state,
            &format!("branch:{}:{}", invocation.path, invocation.call_id),
        )
        .await?;
        let store = services
            .content_store()
            .context("Content storage unavailable")?;
        let dispatch_key = hash(&format!(
            "{}\0{}\0{:?}\0{}",
            invocation.path, invocation.branch, invocation.invocation, invocation.call_id
        ));
        let request = json!({"path":invocation.path,"branch":invocation.branch,"invocation":invocation.invocation,"routeId":invocation.route_id,"callId":invocation.call_id,"input":invocation.input});
        let dispatch = if let Some(dispatch) = services
            .read_record("route-dispatches", &dispatch_key)
            .await?
        {
            dispatch
        } else {
            let mut existing = None;
            for id in self.prepared.graph.routes.keys() {
                let key = hash(&format!(
                    "{}\0{id}\0{}",
                    invocation.path, invocation.call_id
                ));
                if let Some(visit) = services.read_record("route-visits", &key).await? {
                    ensure!(existing.is_none(), "Ambiguous legacy route visit");
                    ensure!(
                        visit["branch"] == invocation.branch
                            && visit["invocation"] == json!(invocation.invocation)
                            && visit["input"] == invocation.input
                            && invocation
                                .route_id
                                .as_ref()
                                .is_none_or(|wanted| wanted == id),
                        "Route visit identity reused with different arguments"
                    );
                    existing = Some((id.clone(), visit));
                }
            }
            let captured = if invocation.invocation == InvocationKind::Tool {
                if let Some((id, _)) = invocation.call_id.rsplit_once(':') {
                    services.read_record("route-inputs", id).await?
                } else {
                    None
                }
            } else {
                None
            };
            let (reference, plan) = if let Some(value) = existing
                .as_ref()
                .map(|(_, visit)| visit)
                .or(captured.as_ref())
            {
                let plan = self.plan_at(value["graphRef"].as_str()).await?;
                (self.store_plan(&plan).await?, plan)
            } else {
                self.pinned_plan(Some(&dispatch_key)).await?
            };
            let node = invocation
                .path
                .rsplit('/')
                .next()
                .context("Missing caller node")?;
            ensure!(
                flow_contract::can_request(&plan.flows[instance].exports, &invocation.branch, node),
                "Branch does not belong to this node"
            );
            if let Some(id) = &invocation.route_id {
                ensure!(
                    plan.graph
                        .routes
                        .get(id)
                        .is_some_and(|route| route.from.instance == instance
                            && route.from.port == invocation.branch
                            && route.invocation == invocation.invocation),
                    "Selected route {id} does not belong to this branch point"
                );
            }
            let mut eligible = Vec::new();
            for (id, route) in &plan.graph.routes {
                if let Some((existing_id, _)) = &existing {
                    if existing_id == id {
                        eligible.push(id);
                    }
                    continue;
                }
                if route.from.instance != instance
                    || route.from.port != invocation.branch
                    || route.invocation != invocation.invocation
                    || invocation
                        .route_id
                        .as_ref()
                        .is_some_and(|wanted| wanted != id)
                {
                    continue;
                }
                if let Some(condition) = &route.condition
                    && !zf_runtime::predicates::evaluate(
                        &zf_flows::node_contracts::parse_predicate(condition)?,
                        &invocation.caller_state,
                    )?
                {
                    continue;
                }
                eligible.push(id);
            }
            ensure!(
                eligible.len() == 1
                    || (eligible.is_empty() && invocation.invocation == InvocationKind::Condition),
                "Expected one eligible route, found {}; select an exact routeId",
                eligible.len()
            );
            let dispatch =
                json!({"request":request,"graphRef":reference,"routeId":eligible.first()});
            store
                .claim_record(&services.id, "route-dispatches", &dispatch_key, &dispatch)
                .await?;
            services
                .read_record("route-dispatches", &dispatch_key)
                .await?
                .context("Route dispatch claim absent")?
        };
        ensure!(
            dispatch["request"] == request,
            "Route dispatch identity reused with different arguments"
        );
        let graph_ref = dispatch["graphRef"]
            .as_str()
            .context("Route dispatch graph absent")?;
        let plan = self.plan_at(Some(graph_ref)).await?;
        let Some(route_id) = dispatch["routeId"].as_str() else {
            services.emit(json!({"type":"route_skipped","nodePath":invocation.path,"branch":invocation.branch,"callId":invocation.call_id,"graphRef":graph_ref,"reason":"no_eligible_route"})).await;
            return Ok(RouteOutcome::Skipped {
                reason: "no_eligible_route".into(),
            });
        };
        let route = plan
            .graph
            .routes
            .get(route_id)
            .context("Captured route absent")?;
        validate_value(&route.input, &invocation.input, &plan.graph.types)
            .map_err(|d| anyhow::anyhow!("Route input: {d:?}"))?;
        let visit_id = hash(&format!(
            "{}\0{}\0{}",
            invocation.path, route_id, invocation.call_id
        ));
        let thread_id = format!("{}/route/{visit_id}", services.id);
        let target = &plan.flows[&route.to.instance];
        let prior_visit = services.read_record("route-visits", &visit_id).await?;
        if let Some(prior) = &prior_visit {
            ensure!(
                prior["path"] == invocation.path
                    && prior["branch"] == invocation.branch
                    && prior["input"] == invocation.input,
                "Route visit identity reused with different arguments"
            );
            self.save_resume(
                &visit_id,
                prior["instance"].as_str().context("Visit target absent")?,
                &invocation.caller_state,
            )
            .await?;
            if prior["mode"] == "launch" {
                self.spawn_visit(visit_id.clone(), prior.clone()).await?;
                return Ok(RouteOutcome::Launched {
                    visit_id,
                    thread_id,
                });
            }
            return self.run_visit(&visit_id, prior).await;
        }
        let (parent, depth) = CURRENT_VISIT
            .try_with(|(id, depth)| (Some(id.clone()), depth.saturating_add(1)))
            .unwrap_or((None, 1));
        let depth_limit = plan.flows[&plan.graph.entry.instance]
            .composition
            .settings
            .recursion_limit;
        ensure!(
            depth <= depth_limit,
            "Route depth {depth} exceeds runtime recursion limit {depth_limit}"
        );
        let visit = json!({"parentVisitId":parent,"depth":depth,"version":1,"graphRef":graph_ref,"routeId":route_id,"path":invocation.path,"branch":invocation.branch,"invocation":invocation.invocation,"callId":invocation.call_id,"input":invocation.input,"threadId":thread_id,"instance":route.to.instance,"entry":route.to.port,"targetHash":target.hash,"targetRevision":zf_runtime::revisions::RevisionDefinition::from_prepared(&plan, &route.to.instance)?.revision(),"mode":route.mode});
        services
            .persist_record("route-visits", &visit_id, &visit)
            .await?;
        self.save_resume(&visit_id, &route.to.instance, &invocation.caller_state)
            .await?;
        if route.mode == RouteMode::Launch {
            self.spawn_visit(visit_id.clone(), visit).await?;
            Ok(RouteOutcome::Launched {
                visit_id,
                thread_id,
            })
        } else {
            self.run_visit(&visit_id, &visit).await
        }
    }

    async fn spawn_visit(&self, key: String, visit: Value) -> Result<()> {
        let mut tasks = self.tasks.lock().await;
        if tasks.contains_key(&key) || self.terminal(&key).await? {
            return Ok(());
        }
        let runtime = self
            .this
            .upgrade()
            .context("Runtime released before launch")?;
        let owned_key = key.clone();
        runtime.active_launches.fetch_add(1, Ordering::AcqRel);
        let active = ActiveLaunch(runtime.clone());
        tasks.insert(
            key,
            tokio::spawn(async move {
                let _active = active;
                runtime.run_visit(&owned_key, &visit).await
            }),
        );
        Ok(())
    }

    async fn save_resume(&self, visit_id: &str, instance: &str, state: &State) -> Result<()> {
        let mut input = State::new();
        let prefix = format!("answer:{instance}/");
        for (name, value) in state {
            if let Some(local) = name.strip_prefix(&prefix) {
                input.insert(format!("answer:{local}"), value.clone());
            }
            if name == "__zedflow:consumedMessages" {
                input.insert(name.clone(), value.clone());
            }
        }
        if !input.is_empty() {
            let services = self.services()?;
            services
                .content_store()
                .context("Content store missing")?
                .put_record(&services.id, "route-resume", visit_id, &json!(input))
                .await?;
        }
        Ok(())
    }

    async fn run_visit(&self, visit_id: &str, visit: &Value) -> Result<RouteOutcome> {
        let lock = {
            let mut locks = self.locks.lock().unwrap_or_else(|p| p.into_inner());
            locks
                .entry(visit_id.into())
                .or_insert_with(|| Arc::new(AsyncMutex::new(())))
                .clone()
        };
        let _guard = lock.lock().await;
        let result = self.run_visit_inner(visit_id, visit).await;
        if let Err(error) = &result {
            self.status(visit_id,json!({"status":"failed","visitId":visit_id,"threadId":visit["threadId"],"instance":visit["instance"],"entry":visit["entry"],"error":format!("{error:#}")})).await?;
        }
        result
    }

    async fn run_visit_inner(&self, visit_id: &str, visit: &Value) -> Result<RouteOutcome> {
        let services = self.services()?;
        if let Some(status) = services.read_record("route-status", visit_id).await?
            && matches!(status["status"].as_str(), Some("completed" | "handoff"))
        {
            return Ok(serde_json::from_value(status)?);
        }
        let instance = visit["instance"]
            .as_str()
            .context("Visit instance absent")?;
        let entry = visit["entry"].as_str().context("Visit entry absent")?;
        let thread_id = visit["threadId"].as_str().context("Visit thread absent")?;
        let plan = self.plan_at(visit["graphRef"].as_str()).await?;
        let flow = plan
            .flows
            .get(instance)
            .context("Visit target absent from frozen runtime")?;
        ensure!(
            visit["targetHash"] == flow.hash,
            "Cannot resume a routed visit with a different flow revision"
        );
        let checkpoint = self.checkpoints.load(thread_id).await?;
        let mut definition =
            zf_runtime::revisions::RevisionDefinition::from_prepared(&plan, instance)?;
        ensure!(
            visit.get("targetRevision").map_or(
                definition.package.is_none() && definition.context_selections.is_empty(),
                |revision| revision == &definition.revision()
            ),
            "Cannot resume a routed visit with a different package definition"
        );
        if let Some(checkpoint) = &checkpoint
            && let Some(selected) = zf_runtime::revisions::checkpoint_definition(
                &services.content_store().context("Content storage absent")?,
                &services.id,
                thread_id,
                checkpoint.step,
                instance,
            )
            .await?
        {
            ensure!(
                selected.key == definition.key,
                "Checkpoint belongs to another flow definition"
            );
            definition = selected;
        }
        let projection = flow_contract::at_entry(&definition.composition, entry)?;
        let mut controller = if let Some(revisions) = services.revisions() {
            Some(revisions.rebased(instance, definition.clone()).await?)
        } else {
            None
        };
        let mut graph = self.build_instance_with_revisions(
            instance,
            &projection,
            controller.clone(),
            Some(&definition.revision()),
        )?;
        let mut input = State::new();
        if checkpoint.is_none() {
            if let Some(captured) = services.read_record("route-bindings", visit_id).await? {
                input = serde_json::from_value(captured["input"].clone())?;
            } else {
                input = flow_contract::entry_input(&flow.exports, entry, visit["input"].clone())?;
                let registry = services
                    .data_registry()
                    .context("Data registry unavailable")?;
                let mut bindings = BTreeMap::new();
                // Required resources enter only their declared channels. A stable
                // capture precedes execution, including a crash before checkpoint 1.
                for (name, channel) in &flow.exports.requires {
                    match registry.snapshot(&Scope::Flow(instance.into()), name).await {
                        Ok(snapshot) => {
                            input.insert(channel.clone(), snapshot.value.as_ref().clone());
                            bindings.insert(name, json!({"entityId":snapshot.entity_id,"revision":snapshot.revision,"contentRef":snapshot.content_ref}));
                        }
                        Err(DataError::NotFound)
                            if flow.exports.contract.requires[name].optional => {}
                        Err(DataError::NotFound) => {
                            let outcome = RouteOutcome::Waiting {
                                visit_id: visit_id.into(),
                                thread_id: thread_id.into(),
                                wait: json!({"kind":"context_resources","nodePath":format!("{instance}/{}",flow.exports.entries[entry].node),"needs":[{"resource":name,"dataType":flow.exports.contract.requires[name].data_type}]}),
                            };
                            self.status(visit_id, serde_json::to_value(&outcome)?)
                                .await?;
                            return Ok(outcome);
                        }
                        Err(error) => return Err(error.into()),
                    }
                }
                services
                    .persist_record(
                        "route-bindings",
                        visit_id,
                        &json!({"input":input,"bindings":bindings}),
                    )
                    .await?;
            }
        }
        let output_heads =
            if let Some(heads) = services.read_record("route-output-heads", visit_id).await? {
                heads
            } else {
                let registry = services
                    .data_registry()
                    .context("Data registry unavailable")?;
                let mut heads = BTreeMap::new();
                for name in flow.exports.data.keys() {
                    let head = match registry.snapshot(&Scope::Flow(instance.into()), name).await {
                        Ok(snapshot) => Some(snapshot.revision),
                        Err(DataError::NotFound) => None,
                        Err(error) => return Err(error.into()),
                    };
                    heads.insert(name, head);
                }
                let heads = json!(heads);
                services
                    .persist_record("route-output-heads", visit_id, &heads)
                    .await?;
                heads
            };
        if let Some(resume) = services.read_record("route-resume", visit_id).await? {
            input.extend(serde_json::from_value::<State>(resume)?);
        }
        self.status(visit_id,json!({"status":"running","visitId":visit_id,"threadId":thread_id,"instance":instance,"entry":entry})).await?;
        let mut config = ExecutionConfig::new(thread_id);
        config = config.with_recursion_limit(flow.composition.settings.recursion_limit);
        let depth = visit["depth"].as_u64().context("Visit depth absent")? as usize;
        let execution = loop {
            let execution = CURRENT_VISIT
                .scope(
                    (visit_id.into(), depth),
                    graph.invoke_detailed(std::mem::take(&mut input), config.clone()),
                )
                .await;
            if let Err(GraphError::Interrupted(interrupted)) = &execution
                && let adk_graph::Interrupt::Dynamic {
                    data: Some(request),
                    ..
                } = &interrupted.interrupt
                && request["kind"] == "revision_boundary"
            {
                ensure!(
                    request["scope"] == instance && request["threadId"] == thread_id,
                    "Revision boundary belongs to another route frontier"
                );
                let checkpoint = self
                    .checkpoints
                    .load(thread_id)
                    .await?
                    .context("Revision boundary checkpoint absent")?;
                ensure!(
                    checkpoint.thread_id == thread_id
                        && checkpoint.step
                            == request["step"].as_u64().context("Boundary step missing")? as usize
                        && checkpoint.pending_nodes.len() == 1
                        && checkpoint.pending_nodes[0]
                            == request["node"].as_str().context("Boundary node missing")?,
                    "Revision boundary does not match the durable sequential frontier"
                );
                let definition: zf_runtime::revisions::RevisionDefinition = serde_json::from_value(
                    services
                        .content_store()
                        .context("Content storage absent")?
                        .resolve(
                            request["definitionRef"]
                                .as_str()
                                .context("Boundary definition missing")?,
                        )
                        .await?,
                )?;
                ensure!(
                    definition.hash == request["toHash"]
                        && request.get("toRevision").map_or(
                            definition.package.is_none()
                                && definition.context_selections.is_empty(),
                            |revision| revision == &definition.revision()
                        ),
                    "Revision boundary source identity mismatch"
                );
                let revisions = controller.as_ref().context("Revision controller absent")?;
                controller = Some(revisions.rebased(instance, definition.clone()).await?);
                let projection = flow_contract::at_entry(&definition.composition, entry)?;
                graph = self.build_instance_with_revisions(
                    instance,
                    &projection,
                    controller.clone(),
                    Some(&definition.revision()),
                )?;
                services.emit(json!({"type":"revision_adopted","scope":instance,"threadId":thread_id,"step":checkpoint.step,"hash":definition.hash,"definitionRevision":definition.revision(),"packageRevision":definition.package.as_ref().map(|p|&p.root),"definitionRef":request["definitionRef"],"visitId":visit_id})).await;
                continue;
            }
            break execution;
        };
        let outcome = match execution {
            Ok(completed) => {
                self.publish_values(
                    instance,
                    self.dataset_values(instance, &completed.state)?,
                    &format!("result:{visit_id}"),
                    Some(&output_heads),
                )
                .await?;
                let value = flow.exports.entries[entry]
                    .output_field
                    .as_ref()
                    .and_then(|name| completed.state.get(name))
                    .cloned()
                    .unwrap_or(Value::Null);
                if let Some(output) = &flow.exports.contract.entries[entry].output {
                    validate_value(output, &value, &plan.graph.types)
                        .map_err(|d| anyhow::anyhow!("Route output: {d:?}"))?;
                }
                if visit["mode"] == "handoff" {
                    RouteOutcome::Handoff {
                        visit_id: visit_id.into(),
                        thread_id: thread_id.into(),
                        result: value,
                    }
                } else {
                    RouteOutcome::Completed {
                        visit_id: visit_id.into(),
                        thread_id: thread_id.into(),
                        result: value,
                    }
                }
            }
            Err(GraphError::Interrupted(interrupted)) => {
                let wait = match &interrupted.interrupt {
                    adk_graph::Interrupt::Dynamic {
                        data: Some(data), ..
                    } => data.clone(),
                    _ => json!({"kind":"graph_interrupt","interrupt":interrupted.interrupt}),
                };
                RouteOutcome::Waiting {
                    visit_id: visit_id.into(),
                    thread_id: thread_id.into(),
                    wait: json!({"checkpointId":interrupted.checkpoint_id,"child":wait}),
                }
            }
            Err(error) => return Err(error.into()),
        };
        self.status(visit_id, serde_json::to_value(&outcome)?)
            .await?;
        Ok(outcome)
    }

    /// Whether launched work is still executing, including handles currently
    /// owned by a drain rather than the task map.
    pub fn has_active_launches(&self) -> bool {
        self.active_launches.load(Ordering::Acquire) != 0
    }

    /// Error cleanup waits for tasks transferred into a cancelled drain as well
    /// as those still registered here. The parent joins its producer first, so
    /// no new top-level launch can escape this boundary.
    pub async fn cancel_and_drain(&self) -> Result<()> {
        self.services()?.cancel.cancel();
        let mut changes = self.launch_changes.subscribe();
        let result = self.drain().await;
        while self.has_active_launches() {
            changes
                .changed()
                .await
                .context("Launch completion channel closed")?;
        }
        result.map(|_| ())
    }
    pub async fn drain(&self) -> Result<Vec<RouteOutcome>> {
        let mut outcomes = Vec::new();
        let mut errors = Vec::new();
        loop {
            let tasks = std::mem::take(&mut *self.tasks.lock().await);
            if tasks.is_empty() {
                break;
            }
            // The drain can outlive a parent passage, but its cancellation must
            // never detach children after removing their handles from the map.
            let tasks: Vec<_> = tasks
                .into_values()
                .map(tokio_util::task::AbortOnDropHandle::new)
                .collect();
            for task in tasks {
                match task.await {
                    Ok(Ok(outcome)) => outcomes.push(outcome),
                    Ok(Err(error)) => errors.push(error.to_string()),
                    Err(error) => errors.push(error.to_string()),
                }
            }
        }
        ensure!(
            errors.is_empty(),
            "Routed launches failed: {}",
            errors.join("; ")
        );
        Ok(outcomes)
    }
    pub async fn result_snapshot(&self) -> Result<Vec<Value>> {
        let services = self.services()?;
        let store = services
            .content_store()
            .context("Content storage unavailable")?;
        let mut status = Vec::new();
        for record in store
            .records(&services.id)
            .await?
            .into_iter()
            .filter(|r| r.kind == "route-status")
        {
            status.push(store.resolve(&record.value_ref).await?);
        }
        Ok(status)
    }

    async fn tool_invocation(
        &self,
        path: &str,
        call_id: &str,
        name: &str,
        arguments: &Value,
    ) -> Result<BranchInvocation> {
        let instance = self.instance(path)?;
        let node = path.rsplit('/').next().context("Missing tool caller")?;
        let invocation_id = call_id
            .rsplit_once(':')
            .map(|(invocation, _)| invocation)
            .context("Routed tool requires model invocation provenance")?;
        let capture = self
            .services()?
            .read_record("route-inputs", invocation_id)
            .await?
            .context("Route caller state was not captured before the model")?;
        let plan = self.plan_at(capture["graphRef"].as_str()).await?;
        let routes: Vec<_> = plan
            .graph
            .routes
            .iter()
            .filter(|(_, route)| {
                route.invocation == InvocationKind::Tool
                    && route.from.instance == instance
                    && route.tool_name.as_deref() == Some(name)
                    && flow_contract::can_request(
                        &plan.flows[instance].exports,
                        &route.from.port,
                        node,
                    )
            })
            .collect();
        ensure!(
            routes.len() == 1,
            "Tool route is unavailable or ambiguous: {name}"
        );
        let (route_id, route) = routes[0];
        ensure!(
            capture["agentPath"] == path,
            "Route invocation belongs to another agent"
        );
        ensure!(
            arguments
                .as_object()
                .is_some_and(|args| args.len() == 1 && args.contains_key("input")),
            "Routed tool arguments must contain only the typed input"
        );
        Ok(BranchInvocation {
            path: path.into(),
            branch: route.from.port.clone(),
            invocation: InvocationKind::Tool,
            route_id: Some(route_id.clone()),
            call_id: call_id.into(),
            input: arguments["input"].clone(),
            caller_state: serde_json::from_value(capture["state"].clone())?,
        })
    }
}

#[async_trait::async_trait]
impl DynamicCapabilities for RouteRuntime {
    fn tools(&self, path: &str) -> Vec<Value> {
        self.tools
            .get(&self.normalized_path(path))
            .cloned()
            .unwrap_or_default()
    }
    fn resume_channels(&self) -> Vec<String> {
        self.resume_channels.clone()
    }
    async fn route_contract(
        &self,
        path: &str,
        branch: &str,
        route_id: Option<&str>,
    ) -> Result<Value> {
        let (_, plan) = self.pinned_plan(None).await?;
        let instance = self.instance(path)?;
        let node = path.rsplit('/').next().context("Missing route owner")?;
        ensure!(
            flow_contract::can_request(&plan.flows[instance].exports, branch, node),
            "Branch does not belong to this node"
        );
        let routes: Vec<_> = plan
            .graph
            .routes
            .iter()
            .filter(|(id, route)| {
                route.from.instance == instance
                    && route.from.port == branch
                    && route_id.is_none_or(|wanted| wanted == id.as_str())
            })
            .collect();
        ensure!(
            routes.len() == 1,
            "Expected one declared route, found {}; select an exact routeId",
            routes.len()
        );
        let (id, route) = routes[0];
        Ok(
            json!({"routeId":id,"mode":route.mode,"invocation":route.invocation,"targetHash":plan.flows[&route.to.instance].hash,"targetRevision":zf_runtime::revisions::RevisionDefinition::from_prepared(&plan, &route.to.instance)?.revision(),"input":route.input,"output":route.output,"condition":route.condition}),
        )
    }
    async fn await_visit(&self, path: &str, visit_id: &str, state: &State) -> Result<RouteOutcome> {
        let visit = self
            .services()?
            .read_record("route-visits", visit_id)
            .await?
            .context("Unknown routed visit")?;
        let original = visit["path"].as_str().context("Visit caller absent")?;
        ensure!(
            self.instance(path)? == self.instance(original)?,
            "Routed visit belongs to another flow instance"
        );
        self.save_resume(
            visit_id,
            visit["instance"].as_str().context("Visit target absent")?,
            state,
        )
        .await?;
        self.run_visit(visit_id, &visit).await
    }
    async fn capture(&self, path: &str, invocation_id: &str, state: &State) -> Result<()> {
        let services = self.services()?;
        if let Some(captured) = services.read_record("route-inputs", invocation_id).await? {
            ensure!(
                captured["agentPath"] == path && captured["state"] == json!(state),
                "Route capture identity reused"
            );
            return Ok(());
        }
        let (reference, _) = self.pinned_plan(Some(invocation_id)).await?;
        let instance = self.instance(path)?;
        self.publish_state(
            instance,
            state,
            &format!("inference:{path}:{invocation_id}"),
        )
        .await?;
        services
            .persist_record(
                "route-inputs",
                invocation_id,
                &json!({"agentPath":path,"state":state,"graphRef":reference}),
            )
            .await?;
        Ok(())
    }
    async fn invoke_branch(&self, invocation: BranchInvocation) -> Result<RouteOutcome> {
        self.invoke_branch_impl(invocation).await
    }
    async fn invoke(
        &self,
        path: &str,
        call_id: &str,
        name: &str,
        arguments: Value,
    ) -> Result<Value> {
        Ok(self
            .invoke_branch_impl(
                self.tool_invocation(path, call_id, name, &arguments)
                    .await?,
            )
            .await?
            .marker())
    }
    async fn recover_started(
        &self,
        path: &str,
        call_id: &str,
        name: &str,
        arguments: &Value,
    ) -> Result<Option<Value>> {
        if !self.tools(path).iter().any(|tool| tool["name"] == name) {
            return Ok(None);
        }
        // Reconstruct the immutable visit from the captured model invocation;
        // persist_record rejects any route/source/input identity disagreement.
        Ok(Some(
            self.invoke(path, call_id, name, arguments.clone()).await?,
        ))
    }
    async fn resume_state(&self, path: &str, call_id: &str, state: &State) -> Result<()> {
        let services = self.services()?;
        let store = services
            .content_store()
            .context("Content storage unavailable")?;
        for record in store
            .records(&services.id)
            .await?
            .into_iter()
            .filter(|r| r.kind == "route-visits")
        {
            let visit = store.resolve(&record.value_ref).await?;
            if visit["path"] == path && visit["callId"] == call_id {
                self.save_resume(
                    &record.key,
                    visit["instance"]
                        .as_str()
                        .context("Visit instance missing")?,
                    state,
                )
                .await?;
            }
        }
        Ok(())
    }
}

fn schema(data_type: &DataType, types: &TypeRegistry, depth: usize) -> Result<Value> {
    ensure!(depth <= 64, "Tool contract nesting exceeds limit");
    Ok(match data_type {
        DataType::Boolean => json!({"type":"boolean"}),
        DataType::Number => json!({"type":"number"}),
        DataType::Text => json!({"type":"string"}),
        DataType::List { item } => json!({"type":"array","items":schema(item,types,depth+1)?}),
        DataType::Record { fields } => {
            let mut properties = BTreeMap::new();
            for (name, data_type) in fields {
                properties.insert(name, schema(data_type, types, depth + 1)?);
            }
            json!({"type":"object","properties":properties,"required":fields.keys().collect::<Vec<_>>()})
        }
        DataType::Media { media_type } => {
            json!({"type":"object","properties":{"contentRef":{"type":"string"},"mediaType":{"type":"string","const":media_type}},"required":["contentRef","mediaType"]})
        }
        DataType::Named { name } => schema(
            types
                .get(name)
                .with_context(|| format!("Unknown tool contract type: {name}"))?,
            types,
            depth + 1,
        )?,
    })
}
