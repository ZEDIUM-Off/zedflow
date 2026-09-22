//! Pure Cargo workspace assembly from a validated plan and captured support.
use crate::compiler::CompiledPlan;
use anyhow::{Context, Result, ensure};
use serde::Serialize;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;
use zf_flows::package::PackageNode;

const INTERNAL: &[&str] = &[
    "zf-core",
    "zf-context",
    "zf-flows",
    "zf-compiler",
    "zf-storage",
    "zf-runtime",
    "zf-execution",
];
const FLOW_DEPENDENCIES: &[&str] = &[
    "adk-graph",
    "anyhow",
    "serde_json",
    "zf-runtime",
    "zf-flows",
];
const FLOW_WRAPPER: &str = r#"#![recursion_limit = "1024"]
#[path = "flow.rs"]
mod definition;
pub use zf_runtime::{models, operations, runtime, subgraphs};
pub fn build_scope(
    services: std::sync::Arc<zf_runtime::runtime::RunServices>,
    checkpointer: std::sync::Arc<dyn adk_graph::checkpoint::Checkpointer>,
    scope: &str,
) -> anyhow::Result<adk_graph::CompiledGraph> {
    definition::build_scope(services, checkpointer, scope)
}
"#;

/// The runtime layer captures these sources when the product is built. No
/// compiler dependency on runtime, storage, execution or the filesystem exists.
#[derive(Clone, Debug)]
pub struct RuntimeSupport {
    pub files: BTreeMap<String, Vec<u8>>,
}

/// All file names are portable relative paths, and all contents are exact bytes.
#[derive(Clone, Debug, Serialize)]
pub struct CargoExport {
    pub revision: String,
    pub files: BTreeMap<String, Vec<u8>>,
}

/// Assemble a standalone workspace without running Cargo, reading a catalogue,
/// regenerating frozen Rust, or mutating any persistent state.
pub fn export_runtime(plan: &CompiledPlan, support: &RuntimeSupport) -> Result<CargoExport> {
    let prepared = plan.prepared();
    for flow in prepared.flows.values() {
        validate_portable_context(&flow.composition)?;
    }
    ensure!(
        prepared
            .graph
            .bridges
            .keys()
            .eq(prepared.definitions.bridge_sources.keys()),
        "every resolved bridge requires its frozen Rust source for export"
    );
    let mut files = support.files.clone();
    for path in files.keys() {
        validate_path(path)?;
    }
    for name in INTERNAL {
        ensure!(
            files.contains_key(&format!("crates/{name}/Cargo.toml"))
                && files.contains_key(&format!("crates/{name}/src/lib.rs")),
            "missing internal support crate: {name}"
        );
    }
    let workspace = text(&files, "Cargo.toml")?.to_owned();
    let lock = text(&files, "Cargo.lock")?.to_owned();
    let mut locks = LockedPackages::parse(&lock)?;
    let mut members: Vec<String> = INTERNAL
        .iter()
        .map(|name| format!("crates/{name}"))
        .collect();
    let mut packages = BTreeMap::new();
    let mut package_checks = String::new();
    for package in prepared.definitions.flow_packages.values() {
        crate::package_sources::validate_package_sources(package)?;
        for (revision, node) in &package.packages {
            if let Some(previous) = packages.insert(revision.clone(), node) {
                ensure!(previous == node, "conflicting package revision: {revision}");
            }
        }
    }
    for (revision, node) in &packages {
        let directory = format!("packages/{revision}");
        add_package_files(&mut files, &directory, node)?;
        package_checks.push_str(&package_verification_entries(&directory, node, false)?);
        let dependencies = package_dependencies(node, "../");
        add_flow_crate(
            &mut files,
            &mut locks,
            &directory,
            &format!("zedflow-package-{revision}"),
            &dependencies,
        )?;
        files.insert(
            format!("{directory}/zedflow_export.rs"),
            b"#![recursion_limit = \"1024\"]\n#[path = \"flow.rs\"]\nmod definition;\npub use definition::*;\npub use zf_runtime::{models, operations, runtime, subgraphs};\n".to_vec(),
        );
        members.push(directory);
    }
    let mut main = String::from(
        "#![recursion_limit = \"1024\"]\nuse std::{collections::BTreeMap, sync::Arc};\n",
    );
    let mut flow_inputs = String::new();
    let mut registrations = String::new();
    let mut runner_dependencies: BTreeMap<String, (String, String)> = BTreeMap::new();
    for (index, (instance, flow)) in prepared.flows.iter().enumerate() {
        let directory = format!("flows/instance-{index}");
        let dependencies = if let Some(package) = prepared.definitions.flow_packages.get(&flow.key)
        {
            add_package_files(&mut files, &directory, package.root_node()?)?;
            package_checks.push_str(&package_verification_entries(
                &directory,
                package.root_node()?,
                true,
            )?);
            package_dependencies(package.root_node()?, "../../packages/")
        } else {
            BTreeMap::new()
        };
        // Authored package bytes remain independently under packages/<revision>.
        // This instance uses the exact executed source, which may pin context.
        files.insert(
            format!("{directory}/flow.rs"),
            flow.source.as_bytes().to_vec(),
        );
        let name = format!("zedflow-instance-{index}");
        add_flow_crate(&mut files, &mut locks, &directory, &name, &dependencies)?;
        members.push(directory.clone());
        runner_dependencies.insert(format!("flow_{index}"), (name, format!("../{directory}")));
        writeln!(
            flow_inputs,
            "({instance:?}, {:?}, include_str!(\"../../{directory}/flow.rs\")),",
            flow.key
        )?;
        writeln!(
            registrations,
            "factories.insert({instance:?}.into(), Arc::new(flow_{index}::build_scope));"
        )?;
    }
    let mut bridge_inputs = String::new();
    for (index, (key, source)) in prepared.definitions.bridge_sources.iter().enumerate() {
        ensure!(
            !source.contains("zedflow_daemon"),
            "legacy bridge imports require explicit conversion: {key}"
        );
        let path = format!("bridges/bridge_{index}.rs");
        files.insert(format!("runner/src/{path}"), source.as_bytes().to_vec());
        writeln!(main, "#[path={path:?}] mod bridge_{index};")?;
        writeln!(
            bridge_inputs,
            "({key:?}, include_str!({path:?}), bridge_{index}::bridge()),"
        )?;
    }
    let manifest = json!({
        "graph": prepared.graph,
        "contextSelections": prepared.definitions.context_selections,
        "executedHashes": prepared.flows.iter().map(|(id, flow)| (id, &flow.hash)).collect::<BTreeMap<_,_>>(),
        "flowHashes": prepared.definitions.flow_hashes,
        "flowPackages": prepared.definitions.flow_packages,
        "bridgeHashes": prepared.definitions.bridge_hashes,
    });
    files.insert("runtime.json".into(), serde_json::to_vec_pretty(&manifest)?);
    let package_checks = package_verification_code(
        &package_checks,
        "prepared.definitions.flow_packages.values()",
    );
    writeln!(
        main,
        r#"
#[tokio::main]
async fn main() -> anyhow::Result<()> {{
    let manifest = zf_context::context_json::from_str(include_str!("../../runtime.json"))?;
    let prepared = zf_execution::runtime_export::assemble(manifest, vec![{flow_inputs}], vec![{bridge_inputs}])?;
    {package_checks}
    let mut factories: BTreeMap<String, zf_execution::route_runtime::NativeFactory> = BTreeMap::new();
    {registrations}
    let options = zf_execution::runtime_export::RunOptions::from_args().await?;
    let result = zf_execution::runtime_export::run(prepared, factories, options).await?;
    println!("{{}}", serde_json::to_string_pretty(&result)?);
    Ok(())
}}
"#
    )?;
    files.insert("runner/src/main.rs".into(), main.into_bytes());
    let runner_base = [
        "anyhow",
        "tokio",
        "serde_json",
        "zf-context",
        "zf-flows",
        "zf-execution",
    ];
    add_manifest(
        &mut files,
        &mut locks,
        "runner",
        "zedflow-export",
        &runner_base,
        &runner_dependencies,
        "../crates/",
    )?;
    members.push("runner".into());
    finish_export(files, locks, &workspace, &members, plan.revision().into())
}

/// Export a graph without composition ports. Validate its exact Rust projection
/// and optional package capture, without inventing a public composition contract.
pub fn export_single(
    doc: &zf_flows::schema::Composition,
    source: &str,
    package: Option<&zf_flows::package::PackageSnapshot>,
    primitives: &dyn crate::graph_compiler::PrimitiveContracts,
    support: &RuntimeSupport,
) -> Result<CargoExport> {
    export_single_with_context(doc, source, package, &BTreeMap::new(), primitives, support)
}

/// Preserve relative context selections when exporting a historical standalone
/// definition whose executed Rust differs from its authored package entry.
pub fn export_single_with_context(
    doc: &zf_flows::schema::Composition,
    source: &str,
    package: Option<&zf_flows::package::PackageSnapshot>,
    context_selections: &BTreeMap<String, crate::prepared_model::ContextSelection>,
    primitives: &dyn crate::graph_compiler::PrimitiveContracts,
    support: &RuntimeSupport,
) -> Result<CargoExport> {
    let parsed = zf_flows::flow_format::parse(
        source,
        &crate::graph_compiler::GraphValidator::new(primitives),
    )?;
    ensure!(
        serde_json::to_value(&parsed)? == serde_json::to_value(doc)?,
        "export source and composition disagree"
    );
    crate::plan::lower(doc, primitives)?;
    validate_portable_context(doc)?;
    let mut files = support.files.clone();
    let workspace = text(&files, "Cargo.toml")?.to_owned();
    let mut locks = LockedPackages::parse(text(&files, "Cargo.lock")?)?;
    let mut members: Vec<String> = INTERNAL
        .iter()
        .map(|name| format!("crates/{name}"))
        .collect();
    let directory = "flows/instance-0";
    let mut dependencies = BTreeMap::new();
    let mut package_checks = String::new();
    if let Some(package) = package {
        crate::prepared_model::validate_package_definition(
            package,
            source,
            doc,
            context_selections,
            primitives,
        )?;
        for (revision, node) in &package.packages {
            let directory = format!("packages/{revision}");
            add_package_files(&mut files, &directory, node)?;
            package_checks.push_str(&package_verification_entries(&directory, node, false)?);
            add_flow_crate(
                &mut files,
                &mut locks,
                &directory,
                &format!("zedflow-package-{revision}"),
                &package_dependencies(node, "../"),
            )?;
            files.insert(format!("{directory}/zedflow_export.rs"), b"#![recursion_limit = \"1024\"]\n#[path = \"flow.rs\"]\nmod definition;\npub use definition::*;\npub use zf_runtime::{models, operations, runtime, subgraphs};\n".to_vec());
            members.push(directory);
        }
        add_package_files(&mut files, directory, package.root_node()?)?;
        package_checks.push_str(&package_verification_entries(
            directory,
            package.root_node()?,
            true,
        )?);
        dependencies = package_dependencies(package.root_node()?, "../../packages/");
    }
    files.insert(format!("{directory}/flow.rs"), source.as_bytes().to_vec());
    add_flow_crate(
        &mut files,
        &mut locks,
        directory,
        "zedflow-instance-0",
        &dependencies,
    )?;
    members.push(directory.into());
    let hash = crate::programs::hash(source.as_bytes());
    let definition = json!({"key":doc.id,"hash":hash,"source":source,"composition":doc,"package":package,"contextSelections":context_selections});
    let revision = crate::programs::hash(&serde_json::to_vec(&definition)?);
    files.insert(
        "definition.json".into(),
        serde_json::to_vec_pretty(&definition)?,
    );
    let main = r#"#![recursion_limit = "1024"]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let definition: zf_runtime::revisions::RevisionDefinition = zf_context::context_json::from_str(include_str!("../../definition.json"))?;
    anyhow::ensure!(definition.source == include_str!("../../flows/instance-0/flow.rs"), "compiled source differs from frozen single-flow definition");
    PACKAGE_VERIFICATION
    let options = zf_execution::runtime_export::RunOptions::from_args().await?;
    let result = zf_execution::runtime_export::run_single(definition, std::sync::Arc::new(flow_0::build_scope), options).await?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
"#.replace("PACKAGE_VERIFICATION", &package_verification_code(&package_checks, "definition.package.iter()"));
    files.insert("runner/src/main.rs".into(), main.into_bytes());
    add_manifest(
        &mut files,
        &mut locks,
        "runner",
        "zedflow-export",
        &[
            "anyhow",
            "tokio",
            "serde_json",
            "zf-context",
            "zf-execution",
            "zf-runtime",
        ],
        &BTreeMap::from([(
            "flow_0".into(),
            ("zedflow-instance-0".into(), "../flows/instance-0".into()),
        )]),
        "../crates/",
    )?;
    members.push("runner".into());
    finish_export(files, locks, &workspace, &members, revision)
}

fn finish_export(
    mut files: BTreeMap<String, Vec<u8>>,
    locks: LockedPackages,
    workspace: &str,
    members: &[String],
    revision: String,
) -> Result<CargoExport> {
    let mut workspace_out = String::new();
    let mut replaced = false;
    for line in workspace.lines() {
        if line.starts_with("members = ") {
            writeln!(
                workspace_out,
                "members = {}",
                serde_json::to_string(&members)?
            )?;
            writeln!(workspace_out, "default-members = [\"runner\"]")?;
            replaced = true;
        } else if !line.starts_with("default-members = ") {
            writeln!(workspace_out, "{line}")?;
        }
    }
    ensure!(replaced, "support workspace members declaration missing");
    files.insert("Cargo.toml".into(), workspace_out.into_bytes());
    files.insert("Cargo.lock".into(), locks.project()?.into_bytes());
    files.insert("README.md".into(), README.as_bytes().to_vec());
    for path in files.keys() {
        validate_path(path)?;
    }
    Ok(CargoExport { revision, files })
}

fn text<'a>(files: &'a BTreeMap<String, Vec<u8>>, path: &str) -> Result<&'a str> {
    std::str::from_utf8(
        files
            .get(path)
            .with_context(|| format!("missing support file: {path}"))?,
    )
    .with_context(|| format!("non-UTF8 support file: {path}"))
}

fn validate_portable_context(doc: &zf_flows::schema::Composition) -> Result<()> {
    let contracts = zf_context::resource_readers::standard_contracts();
    for node in &doc.nodes {
        if node.data.kind == "context" {
            let model_id = node.data.config["modelNode"]
                .as_str()
                .context("context model identity absent")?;
            let model = doc
                .nodes
                .iter()
                .find(|model| model.id == model_id)
                .context("context model absent")?;
            let effective =
                crate::graph_compiler::combined_config(&model.data.config, &node.data.config);
            ensure!(
                !effective["contextProgram"].is_null(),
                "unresolved context program cannot be exported: {}",
                node.id
            );
        } else if matches!(node.data.kind.as_str(), "agent" | "llm")
            && !node.data.config["contextStrategy"].is_null()
        {
            ensure!(
                !node.data.config["contextProgram"].is_null(),
                "unresolved context strategy cannot be exported: {}",
                node.id
            );
        }
        if node.data.kind == "subgraph" {
            validate_portable_context(&serde_json::from_value(
                node.data.config["composition"].clone(),
            )?)?;
        }
        for bindings in [
            &node.data.config["contextBindings"],
            &node.data.config["contextProgram"]["bindings"],
        ] {
            if let Some(bindings) = bindings.as_object() {
                for binding in bindings.values() {
                    if binding["kind"] == "reader" {
                        let id = binding["reader"]
                            .as_str()
                            .context("reader identity absent")?;
                        ensure!(
                            contracts.iter().any(|contract| contract.id == id),
                            "native reader implementation absent from Cargo export: {id}"
                        );
                    }
                }
            }
        }
    }
    Ok(())
}

fn validate_path(path: &str) -> Result<()> {
    ensure!(
        !path.is_empty()
            && !path.contains(['\\', ':', '\0'])
            && path
                .split('/')
                .all(|part| !part.is_empty() && part != "." && part != ".."),
        "invalid export file path: {path:?}"
    );
    Ok(())
}

fn add_package_files(
    files: &mut BTreeMap<String, Vec<u8>>,
    directory: &str,
    node: &PackageNode,
) -> Result<()> {
    files.insert(
        format!("{directory}/flow.json"),
        node.manifest_source.as_bytes().to_vec(),
    );
    for (path, content) in &node.files {
        files.insert(format!("{directory}/{path}"), content.clone());
    }
    Ok(())
}

fn package_verification_entries(
    directory: &str,
    node: &PackageNode,
    skip_entry: bool,
) -> Result<String> {
    let revision = node.revision();
    let mut entries = String::new();
    for path in std::iter::once("flow.json").chain(node.files.keys().map(String::as_str)) {
        if skip_entry && path == "flow.rs" {
            continue;
        }
        writeln!(
            entries,
            "({revision:?}, {path:?}, include_bytes!({:?}).as_slice()),",
            format!("../../{directory}/{path}")
        )?;
    }
    Ok(entries)
}

fn package_verification_code(entries: &str, snapshots: &str) -> String {
    format!(
        r#"
    let package_files: &[(&str, &str, &[u8])] = &[{entries}];
    for (revision, path, compiled_bytes) in package_files {{
        let node = {snapshots}.find_map(|package| package.packages.get(*revision))
            .ok_or_else(|| anyhow::anyhow!("compiled package revision absent: {{revision}}"))?;
        let frozen_bytes = if *path == "flow.json" {{ node.manifest_source.as_bytes() }}
            else {{ node.files.get(*path).ok_or_else(|| anyhow::anyhow!("compiled package file absent: {{path}}"))?.as_slice() }};
        anyhow::ensure!(*compiled_bytes == frozen_bytes, "compiled package file differs from frozen revision: {{revision}}/{{path}}");
    }}
"#
    )
}

fn package_dependencies(node: &PackageNode, prefix: &str) -> BTreeMap<String, (String, String)> {
    node.dependencies
        .iter()
        .map(|(alias, revision)| {
            (
                alias.clone(),
                (
                    format!("zedflow-package-{revision}"),
                    format!("{prefix}{revision}"),
                ),
            )
        })
        .collect()
}

fn add_flow_crate(
    files: &mut BTreeMap<String, Vec<u8>>,
    locks: &mut LockedPackages,
    directory: &str,
    name: &str,
    dependencies: &BTreeMap<String, (String, String)>,
) -> Result<()> {
    ensure!(
        !files.contains_key(&format!("{directory}/zedflow_export.rs")),
        "package reserves generated zedflow_export.rs"
    );
    files.insert(
        format!("{directory}/zedflow_export.rs"),
        FLOW_WRAPPER.as_bytes().to_vec(),
    );
    add_manifest(
        files,
        locks,
        directory,
        name,
        FLOW_DEPENDENCIES,
        dependencies,
        "../../crates/",
    )?;
    let path = format!("{directory}/Cargo.toml");
    files
        .get_mut(&path)
        .context("generated manifest missing")?
        .extend_from_slice(b"\n[lib]\npath = \"zedflow_export.rs\"\n");
    Ok(())
}

fn add_manifest(
    files: &mut BTreeMap<String, Vec<u8>>,
    locks: &mut LockedPackages,
    directory: &str,
    name: &str,
    base: &[&str],
    dependencies: &BTreeMap<String, (String, String)>,
    internal_prefix: &str,
) -> Result<()> {
    let mut manifest = format!(
        "[package]\nname = {name:?}\nversion = \"0.0.0\"\nedition.workspace = true\nrust-version.workspace = true\npublish = false\n\n[dependencies]\n"
    );
    let mut lock_dependencies = Vec::new();
    for dependency in base {
        ensure!(
            !dependencies
                .keys()
                .any(|alias| alias.replace('-', "_") == dependency.replace('-', "_")),
            "package dependency shadows support dependency: {dependency}"
        );
        if INTERNAL.contains(dependency) {
            writeln!(
                manifest,
                "{dependency} = {{ path = \"{internal_prefix}{dependency}\" }}"
            )?;
        } else {
            writeln!(manifest, "{dependency}.workspace = true")?;
        }
        lock_dependencies.push(locks.reference(dependency)?);
    }
    for (alias, (package, path)) in dependencies {
        syn::parse_str::<syn::Ident>(&alias.replace('-', "_")).with_context(|| {
            format!("package dependency alias is not a Rust identifier: {alias}")
        })?;
        writeln!(
            manifest,
            "{alias:?} = {{ package = {package:?}, path = {path:?} }}"
        )?;
        lock_dependencies.push(package.clone());
    }
    locks.add(name, lock_dependencies)?;
    files.insert(format!("{directory}/Cargo.toml"), manifest.into_bytes());
    Ok(())
}

// Cargo's checked-in v4 lock format is a small set of package tables. Preserve
// every retained table byte-for-byte (including registry checksums), following
// dependency references; only generated local packages receive new tables.
struct LockedPackages {
    entries: BTreeMap<String, LockEntry>,
}
struct LockEntry {
    name: String,
    version: String,
    source: String,
    dependencies: Vec<String>,
}
impl LockedPackages {
    fn parse(lock: &str) -> Result<Self> {
        ensure!(
            lock.lines().any(|line| line == "version = 4"),
            "unsupported support lockfile version"
        );
        let mut entries = BTreeMap::new();
        for block in lock.split("[[package]]\n").skip(1) {
            let field = |key: &str| -> Result<String> {
                let prefix = format!("{key} = ");
                let value = block
                    .lines()
                    .find_map(|line| line.strip_prefix(&prefix))
                    .context("invalid lock package")?;
                Ok(serde_json::from_str(value)?)
            };
            let name = field("name")?;
            let version = field("version")?;
            let dependencies = if let Some((_, tail)) = block.split_once("dependencies = [\n") {
                tail.split_once("\n]")
                    .context("invalid lock dependencies")?
                    .0
                    .lines()
                    .map(|line| Ok(serde_json::from_str(line.trim().trim_end_matches(','))?))
                    .collect::<Result<Vec<String>>>()?
            } else {
                Vec::new()
            };
            ensure!(
                entries
                    .insert(
                        format!("{name} {version}"),
                        LockEntry {
                            name,
                            version,
                            source: format!("[[package]]\n{}\n\n", block.trim_end()),
                            dependencies
                        }
                    )
                    .is_none(),
                "ambiguous support lock package identity"
            );
        }
        Ok(Self { entries })
    }
    fn reference(&self, name: &str) -> Result<String> {
        let mut matches = self.entries.values().filter(|entry| entry.name == name);
        let entry = matches
            .next()
            .with_context(|| format!("missing locked dependency: {name}"))?;
        ensure!(
            matches.next().is_none(),
            "ambiguous generated dependency: {name}"
        );
        Ok(entry.name.clone())
    }
    fn key(&self, reference: &str) -> Result<String> {
        if self.entries.contains_key(reference) {
            return Ok(reference.into());
        }
        let mut matches = self
            .entries
            .iter()
            .filter(|(_, entry)| entry.name == reference);
        let (key, _) = matches
            .next()
            .with_context(|| format!("unresolved lock dependency: {reference}"))?;
        ensure!(
            matches.next().is_none(),
            "ambiguous lock dependency: {reference}"
        );
        Ok(key.clone())
    }
    fn add(&mut self, name: &str, mut dependencies: Vec<String>) -> Result<()> {
        dependencies.sort();
        dependencies.dedup();
        let mut source =
            format!("[[package]]\nname = {name:?}\nversion = \"0.0.0\"\ndependencies = [\n");
        for dependency in &dependencies {
            writeln!(source, " {dependency:?},")?;
        }
        source.push_str("]\n\n");
        ensure!(
            self.entries
                .insert(
                    format!("{name} 0.0.0"),
                    LockEntry {
                        name: name.into(),
                        version: "0.0.0".into(),
                        source,
                        dependencies
                    }
                )
                .is_none(),
            "duplicate generated crate: {name}"
        );
        Ok(())
    }
    fn project(&self) -> Result<String> {
        let mut pending: Vec<_> = INTERNAL
            .iter()
            .map(|name| self.key(name))
            .collect::<Result<_>>()?;
        pending.extend(
            self.entries
                .iter()
                .filter(|(_, entry)| entry.version == "0.0.0")
                .map(|(key, _)| key.clone()),
        );
        let mut retained = BTreeSet::new();
        while let Some(key) = pending.pop() {
            if retained.insert(key.clone()) {
                for dependency in &self.entries[&key].dependencies {
                    pending.push(self.key(dependency)?);
                }
            }
        }
        let mut lock = String::from(
            "# This file is automatically @generated by Cargo.\n# It is not intended for manual editing.\nversion = 4\n\n",
        );
        for key in retained {
            lock.push_str(&self.entries[&key].source);
        }
        Ok(lock)
    }
}

const README: &str = "# Frozen Zedflow Cargo export\n\nBuild with Rust 1.96.1: `cargo build --locked`. With cached registry dependencies use `--offline --locked`.\n\nRun `cargo run --locked -- --workspace /path/to/workspace --home /path/to/source-home --data /path/to/data --run-id example --input '{\"input\":\"hello\"}'`. Reuse the data directory and run ID to resume a waiting checkpoint; provide answer state through --input. Models and capabilities require explicit bindings. --home selects instruction/skill sources without changing process HOME; omit it for normal home discovery. A resumed run retains its captured context.\n\nThe workspace contains the seven internal library crates, exact executed Rust sources, authored packages and their frozen dependency closure. Registry sources are governed by Cargo.lock; they are not vendored. Package format, runtime metadata, archive format and storage epoch remain independent.\n";
