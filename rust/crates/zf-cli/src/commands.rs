//! Thin authoring, compiler and execution adapters for the `zf` binary.
use crate::templates::{self, Kind};
use anyhow::{Context, Result, bail, ensure};
use clap::{Args, Parser, Subcommand};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use zf_compiler::{
    compiler::{self, CompileRequest},
    graph_compiler::GraphValidator,
    prepared::CompilationSnapshot,
    programs::SourceSnapshot,
};
use zf_execution::{
    commands::{Actor, CommandAuthorizer, CommandKind},
    service::{ExecutionOptions, ExecutionService},
    start::{StartDefinition, StartRequest},
};
use zf_flows::{composition::ResolveRequest, flow_format, schema::Composition};
use zf_runtime::materialize::RuntimePrimitives;

#[derive(Parser)]
#[command(
    name = "zf",
    version,
    about = "Créer, valider et exécuter des flows Zedflow"
)]
pub(crate) struct Cli {
    /// Workspace des commandes locales ; utiliser --workspace-id pour le daemon.
    #[arg(long, global = true, default_value = ".")]
    workspace: PathBuf,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Importer explicitement les compositions SQLite, daemon arrêté, sans exécution.
    MigrateCompositions {
        #[arg(long)]
        data: PathBuf,
        #[arg(long)]
        flow_home: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
    /// Créer un template sans écraser, activer ou exécuter.
    Init { kind: Kind, id: String },
    /// Valider une source Rust, un document JSON ou un dossier package sans exécution.
    Validate { path: PathBuf },
    /// Compiler un package/source ou un CompilationSnapshot JSON, sans exécution.
    Compile(CompileArgs),
    /// Lancer via le daemon, ou ouvrir explicitement un service autonome exclusif.
    Run(RunArgs),
    /// Consulter les sessions interactives ; fournir un identifiant pour le détail.
    Sessions {
        id: Option<String>,
        #[command(flatten)]
        connection: Connection,
    },
    /// Exporter une source/package ou un plan compilé vers un projet Cargo autonome.
    Export {
        path: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    /// Démarrer le daemon local.
    Serve(ServeArgs),
}

#[derive(Args)]
struct CompileArgs {
    path: PathBuf,
    /// Le fichier contient un CompilationSnapshot figé, avec sources et packages.
    #[arg(long)]
    snapshot: bool,
    /// Clé du flow racine dans le snapshot.
    #[arg(long, requires = "snapshot")]
    flow: Option<String>,
    #[arg(long, default_value = "main")]
    entry: String,
    #[arg(long, requires = "snapshot")]
    bridge: Vec<String>,
}

#[derive(Args)]
struct Connection {
    #[arg(
        long,
        default_value = "http://127.0.0.1:3142",
        conflicts_with = "standalone"
    )]
    daemon: String,
    #[arg(long)]
    standalone: bool,
    /// Données exclusivement possédées par ce processus ; aucun repli automatique.
    #[arg(long, requires = "standalone")]
    data: Option<PathBuf>,
    #[arg(long, requires = "standalone")]
    flow_home: Option<PathBuf>,
    #[arg(long, requires = "standalone")]
    context_home: Option<PathBuf>,
    #[arg(long = "skill-dir", requires = "standalone")]
    skill_dirs: Vec<PathBuf>,
    /// Workspace déjà enregistré par le daemon ; sinon son workspace par défaut.
    #[arg(long, conflicts_with = "standalone")]
    workspace_id: Option<String>,
}

#[derive(Args)]
struct RunArgs {
    #[arg(long, requires = "flow_hash", conflicts_with = "source")]
    flow_key: Option<String>,
    #[arg(long, requires = "flow_key")]
    flow_hash: Option<String>,
    /// Source isolée JSON/Rust ; les packages s’exécutent par leur clé et hash.
    #[arg(long, required_unless_present = "flow_key")]
    source: Option<PathBuf>,
    #[arg(long, default_value = "{}")]
    input: String,
    #[arg(long, default_value = "{}")]
    model_bindings: String,
    #[command(flatten)]
    connection: Connection,
}

#[derive(Args)]
struct ServeArgs {
    #[arg(long, default_value = "127.0.0.1:3142")]
    listen: std::net::SocketAddr,
    #[arg(long, default_value = ".zedflow")]
    data: PathBuf,
    #[arg(long, default_value = "web/apps/client/dist")]
    web: PathBuf,
    #[arg(long = "skill-dir")]
    skill_dirs: Vec<PathBuf>,
    #[arg(long)]
    flow_home: Option<PathBuf>,
    #[arg(long)]
    context_home: Option<PathBuf>,
}

pub(crate) async fn execute(cli: Cli) -> Result<()> {
    match cli.command {
        Command::MigrateCompositions {
            data,
            flow_home,
            dry_run,
        } => {
            let flows = zf_storage::flow_store::FlowStore::new(
                flow_home.clone(),
                Arc::new(GraphValidator::new(&RuntimePrimitives)),
            );
            let report = zf_storage::legacy_compositions::import(
                &data,
                &cli.workspace,
                &flow_home,
                &flows,
                dry_run,
            )
            .await?;
            print_json(&serde_json::to_value(report)?)
        }
        Command::Init { kind, id } => {
            let path =
                tokio::task::spawn_blocking(move || templates::create(&cli.workspace, kind, &id))
                    .await??;
            print_json(&json!({"created":path,"activated":false,"executed":false}))
        }
        Command::Validate { path } => print_json(&validate(&path).await?),
        Command::Compile(args) => {
            let (snapshot, flow) = if args.snapshot {
                (
                    serde_json::from_slice::<CompilationSnapshot>(
                        &tokio::fs::read(&args.path).await?,
                    )?,
                    args.flow.context("--flow requis avec --snapshot")?,
                )
            } else {
                let (doc, source, package) = read_flow(&args.path).await?;
                let key = doc.id.clone();
                let mut snapshot = CompilationSnapshot::default();
                snapshot
                    .flows
                    .insert(key.clone(), SourceSnapshot::capture(source));
                if let Some(package) = package {
                    snapshot.packages.insert(key.clone(), package);
                }
                (snapshot, key)
            };
            let plan = compiler::compile(
                &snapshot,
                &CompileRequest::new(ResolveRequest {
                    flow,
                    entry: args.entry,
                    bridges: args.bridge,
                }),
                &RuntimePrimitives,
            )
            .map_err(diagnostics)
            .with_context(|| format!("compilation de {}", args.path.display()))?;
            print_json(&serde_json::to_value(plan)?)
        }
        Command::Run(args) => run(&cli.workspace, args).await,
        Command::Sessions { id, connection } => {
            sessions(&cli.workspace, &connection, id.as_deref()).await
        }
        Command::Export { path, output } => export_project(&cli.workspace, &path, &output).await,
        Command::Serve(args) => {
            zf_serve::server::serve(zf_serve::server::ServerOptions {
                listen: args.listen,
                data: args.data,
                workspace: cli.workspace,
                web: args.web,
                skill_dirs: args.skill_dirs,
                flow_home: args.flow_home,
                context_home: args.context_home,
            })
            .await
        }
    }
}

fn print_json(value: &Value) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}
fn diagnostics(values: Vec<zf_core::diagnostics::Diagnostic>) -> anyhow::Error {
    anyhow::anyhow!(
        "{}",
        serde_json::to_string(&values).unwrap_or_else(|_| format!("{values:?}"))
    )
}

async fn export_project(workspace: &Path, path: &Path, output: &Path) -> Result<()> {
    ensure!(
        !tokio::fs::try_exists(output).await?,
        "la destination existe déjà : {}",
        output.display()
    );
    let support = zf_runtime::runtime_export::support();
    let raw = if path.is_file() {
        Some(tokio::fs::read(path).await?)
    } else {
        None
    };
    let json = raw
        .as_deref()
        .and_then(|bytes| serde_json::from_slice::<Value>(bytes).ok());
    let project = if let Some(prepared) = json.as_ref().and_then(|value| value.get("prepared")) {
        let plan = compiler::compile_prepared(
            serde_json::from_value(prepared.clone())?,
            &RuntimePrimitives,
        )
        .map_err(diagnostics)?;
        zf_compiler::export::export_runtime(&plan, &support)?
    } else {
        let (mut doc, mut source, package) = read_flow(path).await?;
        let sources = zf_execution::sources::program_sources(&doc, workspace, &[]).await?;
        if !zf_compiler::programs::freeze(&mut doc, &sources)?.is_empty() {
            source = flow_format::render(&doc, &GraphValidator::new(&RuntimePrimitives))?;
        }
        zf_compiler::export::export_single(
            &doc,
            &source,
            package.as_ref(),
            &RuntimePrimitives,
            &support,
        )?
    };
    // Reserve a new destination after all validation; never overwrite user files.
    // A failed write removes only the directory exclusively created here.
    tokio::fs::create_dir(output).await?;
    let written: Result<()> = async {
        for (name, bytes) in &project.files {
            let destination = output.join(name);
            if let Some(parent) = destination.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            tokio::fs::write(destination, bytes).await?;
        }
        Ok(())
    }
    .await;
    if let Err(error) = written {
        tokio::fs::remove_dir_all(output)
            .await
            .context("nettoyage de l’export incomplet")?;
        return Err(error);
    }
    print_json(
        &json!({"path":output,"revision":project.revision,"files":project.files.len(),"executed":false}),
    )
}

async fn read_flow(
    path: &Path,
) -> Result<(
    Composition,
    String,
    Option<zf_flows::package::PackageSnapshot>,
)> {
    let package_path = if tokio::fs::metadata(path).await?.is_dir() {
        Some(path.to_owned())
    } else if path.file_name().is_some_and(|name| name == "flow.rs") {
        let parent = path.parent().context("parent du flow absent")?;
        match tokio::fs::try_exists(parent.join("flow.json")).await? {
            true => Some(parent.to_owned()),
            false => None,
        }
    } else {
        None
    };
    let package = if let Some(package_path) = package_path {
        Some(
            zf_storage::flow_packages::capture(&package_path)
                .await
                .with_context(|| format!("capture du package {}", path.display()))?,
        )
    } else {
        None
    };
    let source = match &package {
        Some(package) => package.root_node()?.entry_source()?.to_owned(),
        None => tokio::fs::read_to_string(path)
            .await
            .with_context(|| format!("lecture de {}", path.display()))?,
    };
    let validator = GraphValidator::new(&RuntimePrimitives);
    let (doc, source) = if source.trim_start().starts_with('{') {
        ensure!(
            package.is_none(),
            "l’entrée d’un package doit être une source Rust"
        );
        let doc: Composition =
            serde_json::from_str(&source).context("document flow JSON invalide")?;
        let source = flow_format::render(&doc, &validator)?;
        (doc, source)
    } else {
        let doc = flow_format::parse(&source, &validator)
            .with_context(|| format!("source flow {}", path.display()))?;
        (doc, source)
    };
    if let Some(package) = &package {
        ensure!(
            package.root_manifest()?.id.as_str() == doc.id,
            "identité du package différente de celle du flow : {}",
            path.display()
        );
        zf_compiler::package_sources::validate_package_sources(package)?;
    }
    Ok((doc, source, package))
}

async fn validate(path: &Path) -> Result<Value> {
    if !tokio::fs::metadata(path).await?.is_dir() {
        let source = tokio::fs::read_to_string(path).await?;
        if source
            .lines()
            .any(|line| line.starts_with("// @zedflow-context "))
        {
            let strategy = zf_context::context_source::parse(&source)
                .map_err(diagnostics)
                .with_context(|| format!("contexte {}", path.display()))?;
            return Ok(json!({"valid":true,"kind":"context","path":path,"id":strategy.id}));
        }
        if source
            .lines()
            .any(|line| line.starts_with("// @zedflow-bridge "))
        {
            zf_flows::bridge_source::parse(&source)
                .map_err(diagnostics)
                .with_context(|| format!("bridge {}", path.display()))?;
            return Ok(
                json!({"valid":true,"kind":"bridge","path":path,"scope":"source; dépendances résolues par compile --snapshot"}),
            );
        }
    }
    let (doc, _, package) = read_flow(path).await?;
    Ok(
        json!({"valid":true,"kind":"flow","path":path,"id":doc.id,"packageRevision":package.map(|p| p.root)}),
    )
}

struct LocalOperator;
#[async_trait::async_trait]
impl CommandAuthorizer for LocalOperator {
    async fn authorize(
        &self,
        actor: &Actor,
        _: CommandKind,
        _: &zf_storage::workspaces::Workspace,
        _: Option<&Value>,
    ) -> Result<()> {
        ensure!(actor.id == "zf-cli-local", "acteur CLI local requis");
        Ok(())
    }
}

async fn open_service(workspace: &Path, connection: &Connection) -> Result<ExecutionService> {
    let data = connection
        .data
        .clone()
        .context("--data requis avec --standalone pour choisir un propriétaire exclusif")?;
    let home = connection
        .flow_home
        .clone()
        .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
        .context("--flow-home requis : HOME absent")?;
    let home = tokio::fs::canonicalize(&home)
        .await
        .with_context(|| format!("home des flows inaccessible : {}", home.display()))?;
    let context_home = match &connection.context_home {
        Some(path) => Some(
            tokio::fs::canonicalize(path)
                .await
                .with_context(|| format!("home du contexte inaccessible : {}", path.display()))?,
        ),
        None => None,
    };
    ExecutionService::open(ExecutionOptions {
        data,
        workspace: workspace.to_owned(),
        flow_home: home,
        context_home,
        skill_dirs: connection.skill_dirs.clone(),
        authorizer: Arc::new(LocalOperator),
    })
    .await
}
fn actor(service: &ExecutionService) -> Actor {
    Actor {
        id: "zf-cli-local".into(),
        workspace_id: service.default_workspace_id().into(),
    }
}

async fn run(workspace: &Path, args: RunArgs) -> Result<()> {
    let input = serde_json::from_str(&args.input).context("--input doit être un objet JSON")?;
    let bindings: Value =
        serde_json::from_str(&args.model_bindings).context("--model-bindings JSON invalide")?;
    ensure!(
        bindings.is_object(),
        "--model-bindings doit être un objet JSON"
    );
    let (definition, mut body) = if let Some(key) = args.flow_key {
        let hash = args.flow_hash.context("--flow-hash requis")?;
        (
            StartDefinition::Stored {
                key: key.clone(),
                expected_hash: hash.clone(),
            },
            json!({"flowKey":key,"flowHash":hash}),
        )
    } else {
        let path = args.source.context("--source ou --flow-key requis")?;
        let (doc, _, package) = read_flow(&path).await?;
        ensure!(
            package.is_none(),
            "exécuter un package par --flow-key et --flow-hash pour conserver sa fermeture exacte"
        );
        let body = json!({"composition":doc});
        (StartDefinition::Inline(doc), body)
    };
    if args.connection.standalone {
        let service = open_service(workspace, &args.connection).await?;
        let result = async {
            let identity = actor(&service);
            let accepted = service
                .start(
                    &identity,
                    StartRequest {
                        definition,
                        input,
                        model_bindings: bindings,
                        node_path: None,
                        prepared_context: None,
                        preview_metadata: None,
                    },
                )
                .await?;
            let id = accepted["id"]
                .as_str()
                .context("réponse du service sans identifiant")?;
            tokio::select! {
                result = service.wait_idle(id) => result?,
                signal = tokio::signal::ctrl_c() => { signal?; bail!("exécution interrompue par l’opérateur"); }
            }
            let run = service.read(&identity, id).await?;
            ensure!(run["status"] != "failed" && run["status"] != "error" && run["status"] != "interrupted" && run["status"] != "cancelled", "exécution {} : {}", run["status"], run["error"]);
            Ok::<_, anyhow::Error>(run)
        }
        .await;
        // Close ownership after completion, rejected admission or interruption.
        service.shutdown().await?;
        print_json(&result?)
    } else {
        body["input"] = serde_json::to_value(input)?;
        body["modelBindings"] = bindings;
        if let Some(id) = &args.connection.workspace_id {
            body["workspaceId"] = json!(id);
        }
        print_json(
            &remote(
                &args.connection,
                reqwest::Method::POST,
                &["api", "runs"],
                Some(body),
            )
            .await?,
        )
    }
}

async fn sessions(workspace: &Path, connection: &Connection, id: Option<&str>) -> Result<()> {
    let value = if connection.standalone {
        let service = open_service(workspace, connection).await?;
        let result = async {
            let _read=service.read_scope(&actor(&service),id).await?;
            if let Some(id) = id {
                service.read(&actor(&service), id).await
            } else {
                let documents: Vec<String> = sqlx::query_scalar("SELECT document FROM runs WHERE json_extract(document,'$.workspaceId')=? AND json_extract(document,'$.interactive')=1 ORDER BY json_extract(document,'$.updatedAt') DESC")
                    .bind(service.default_workspace_id()).fetch_all(&service.database()).await?;
                let summaries = documents.iter().map(|raw| serde_json::from_str(raw).map(|run| zf_storage::session_store::summary(&run))).collect::<std::result::Result<Vec<_>,_>>()?;
                Ok(json!(summaries))
            }
        }.await;
        service.shutdown().await?;
        result?
    } else {
        let mut segments = vec!["api", "runs"];
        if let Some(id) = id {
            segments.push(id);
        }
        remote(connection, reqwest::Method::GET, &segments, None).await?
    };
    if id.is_some() {
        ensure!(
            value["interactive"] == true,
            "ce run autonome n’est pas une session interactive"
        );
        print_json(&value)
    } else {
        let runs = value.as_array().context("liste des sessions invalide")?;
        print_json(&json!(
            runs.iter()
                .filter(|run| run["interactive"] == true)
                .collect::<Vec<_>>()
        ))
    }
}

async fn remote(
    connection: &Connection,
    method: reqwest::Method,
    segments: &[&str],
    body: Option<Value>,
) -> Result<Value> {
    let mut url = reqwest::Url::parse(&connection.daemon).context("URL du daemon invalide")?;
    ensure!(
        matches!(url.scheme(), "http" | "https"),
        "URL HTTP(S) requise"
    );
    url.path_segments_mut()
        .map_err(|_| anyhow::anyhow!("URL du daemon sans chemin possible"))?
        .clear()
        .extend(segments);
    url.set_query(None);
    if let Some(id) = &connection.workspace_id {
        url.query_pairs_mut().append_pair("workspaceId", id);
    }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()?;
    let mut request = client.request(method, url);
    if let Some(body) = body {
        request = request.json(&body);
    }
    let response = request
        .send()
        .await
        .context("daemon inaccessible ; aucun service autonome n’a été ouvert")?;
    let status = response.status();
    let bytes = response.bytes().await?;
    ensure!(
        status.is_success(),
        "daemon {status} : {}",
        String::from_utf8_lossy(&bytes)
    );
    serde_json::from_slice(&bytes).context("réponse JSON du daemon invalide")
}
