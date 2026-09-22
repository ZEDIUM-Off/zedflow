//! Admitted capture for portable exports. No graph is executed or resumed here.
use crate::{
    commands::{Actor, CommandKind},
    preparation,
    service::ExecutionService,
};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use zf_compiler::{
    compiler,
    export::{CargoExport, export_runtime, export_single, export_single_with_context},
    graph_compiler::GraphValidator,
    programs,
};
use zf_flows::{flow_format, schema::Composition};
use zf_runtime::{
    inspection::{self, DefinitionQuery},
    materialize::RuntimePrimitives,
    runtime_export::support,
};
use zf_storage::workspaces;

pub enum ExportRequest {
    Draft(Composition),
    Stored {
        key: String,
        expected_hash: String,
    },
    Runtime(preparation::RuntimeSelection),
    Passage {
        run_id: String,
        query: DefinitionQuery,
    },
}
pub struct ExportCapture {
    pub project: CargoExport,
    pub execution_revision: Option<Value>,
}
impl ExecutionService {
    /// Capture through the same workspace admission and source serialization as
    /// execution. Historical exports use only persisted definitions and packages.
    pub async fn cargo_export(
        &self,
        actor: &Actor,
        request: ExportRequest,
    ) -> Result<ExportCapture> {
        let id = match &request {
            ExportRequest::Passage { run_id, .. } => Some(run_id.as_str()),
            _ => None,
        };
        let admitted = self.admit(actor, CommandKind::Read, None).await?;
        let b = if let Some(id) = id {
            self.require_run_scope(actor, id).await?;
            self.admit(actor, CommandKind::Read, Some(id)).await?
        } else {
            admitted
        };
        let _authoring = b.authoring_writer.lock().await;
        let workspace = workspaces::get(&b.db, &actor.workspace_id).await?;
        let support = support();
        if let ExportRequest::Passage { run_id, query } = request {
            let run = crate::commands::definition_run(&b, &run_id).await?;
            let selected = inspection::definition(&b.content, &run, query).await?;
            ensure!(
                selected["exact"] == true,
                "La version exacte de ce passage historique n’est pas disponible"
            );
            let runtime = if let Some(reference) = selected["graphRef"]
                .as_str()
                .or(run["runtimeGraphRef"].as_str())
            {
                b.content.resolve(reference).await?
            } else {
                run["runtimeGraph"].clone()
            };
            let project = if runtime.is_null() {
                let doc = serde_json::from_value(selected["composition"].clone())?;
                let package = selected
                    .get("package")
                    .filter(|v| !v.is_null())
                    .map(|value| serde_json::from_value(value.clone()))
                    .transpose()?;
                export_single_with_context(
                    &doc,
                    selected["source"]
                        .as_str()
                        .context("Source historique absente")?,
                    package.as_ref(),
                    &serde_json::from_value(selected["contextSelections"].clone())?,
                    &RuntimePrimitives,
                    &support,
                )?
            } else {
                let prepared: zf_compiler::prepared_model::PreparedRuntime =
                    serde_json::from_value(runtime)?;
                let instance = selected["instance"]
                    .as_str()
                    .context("Instance historique absente")?;
                let captured =
                    zf_runtime::revisions::RevisionDefinition::from_prepared(&prepared, instance)?;
                ensure!(
                    captured.revision() == selected["definitionRevision"],
                    "La définition de ce passage et le graphe capturé ont des révisions différentes ; aucun graphe mixte n’est exporté"
                );
                let plan = compiler::compile_prepared(prepared, &RuntimePrimitives)
                    .map_err(|errors| anyhow::anyhow!("{errors:?}"))?;
                export_runtime(&plan, &support)?
            };
            return Ok(ExportCapture {
                project,
                execution_revision: Some(
                    json!({"runId":run_id,"nodePath":selected["nodePath"],"occurrenceId":selected["occurrenceId"],"hash":selected["hash"],"graphRef":selected["graphRef"]}),
                ),
            });
        }
        crate::live_files::recover(&b.db, &b.home).await?;
        if workspace.path != b.home {
            crate::live_files::recover(&b.db, &workspace.path).await?;
        }
        if let ExportRequest::Runtime(selection) = request {
            let prepared = preparation::prepare(&b.flows, &workspace, &selection).await?;
            let plan = compiler::compile_prepared(prepared, &RuntimePrimitives)
                .map_err(|errors| anyhow::anyhow!("{errors:?}"))?;
            return Ok(ExportCapture {
                project: export_runtime(&plan, &support)?,
                execution_revision: None,
            });
        }
        let (mut doc, mut source, package) = match request {
            ExportRequest::Draft(doc) => {
                let source = flow_format::render(&doc, &GraphValidator::new(&RuntimePrimitives))?;
                (doc, source, None)
            }
            ExportRequest::Stored { key, expected_hash } => {
                let file = b.flows.get(&workspace, &key).await?;
                ensure!(
                    file.hash == expected_hash,
                    zf_storage::flow_store::Conflict(
                        "Le flow a changé ; actualisez avant de l’exporter"
                    )
                );
                ensure!(
                    file.diagnostics.is_empty(),
                    "Flow non exportable : {:?}",
                    file.diagnostics
                );
                (
                    file.composition.context("Définition absente")?,
                    file.source.context("Source absente")?,
                    file.package,
                )
            }
            _ => unreachable!("runtime and passage handled above"),
        };
        let sources = crate::sources::program_sources(&doc, &workspace.path, &[]).await?;
        if !programs::freeze(&mut doc, &sources)?.is_empty() {
            source = flow_format::render(&doc, &GraphValidator::new(&RuntimePrimitives))?;
        }
        let project = export_single(
            &doc,
            &source,
            package.as_ref(),
            &RuntimePrimitives,
            &support,
        )?;
        Ok(ExportCapture {
            project,
            execution_revision: None,
        })
    }
}
