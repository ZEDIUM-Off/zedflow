use serde_json::{Value, json};
use std::{collections::BTreeMap, path::Path, process::Command};
use zf_compiler::{
    compiler::{CompileRequest, CompiledPlan, compile},
    export::{CargoExport, export_runtime, export_single},
    graph_compiler::GraphValidator,
    prepared::CompilationSnapshot,
    programs::SourceSnapshot,
};
use zf_flows::{
    composition::{
        BridgeDefinition, Connection, Endpoint, InvocationKind, ResolveRequest, RouteMode,
    },
    flow_format,
    package::PackageSnapshot,
};
use zf_runtime::{materialize::RuntimePrimitives, runtime_export::support};

fn fixture() -> CompiledPlan {
    let doc = serde_json::from_value(json!({"formatVersion":3,"id":"portable","name":"Portable","nodes":[
        {"id":"start","position":{"x":0,"y":0},"data":{"kind":"start","label":"Start","config":{"exports":{"contract":{"entries":{"main":{"input":{"kind":"text"},"output":{"kind":"text"}}}},"entries":{"main":{"node":"start","inputField":"input","outputField":"response"}},"interactive":true}}}},
        {"id":"effect","position":{"x":0,"y":1},"data":{"kind":"tool","label":"Effect","config":{"tool":"exec","arguments":{"command":"printf x >> visits"}}}},
        {"id":"pause","position":{"x":0,"y":2},"data":{"kind":"input","label":"Pause","config":{"field":"output","prompt":"Continue","responseType":"text"}}},
        {"id":"output","position":{"x":0,"y":3},"data":{"kind":"output","label":"Output","config":{"inputField":"output"}}},
        {"id":"end","position":{"x":0,"y":4},"data":{"kind":"end","label":"End","config":{}}}
    ],"edges":[{"id":"a","source":"start","target":"effect"},{"id":"b","source":"effect","target":"pause"},{"id":"c","source":"pause","target":"output"},{"id":"d","source":"output","target":"end"}]})).unwrap();
    let source = format!(
        "{}\n// exact exported source: é λ\n",
        flow_format::render(&doc, &GraphValidator::new(&RuntimePrimitives)).unwrap()
    );
    let dependency = PackageSnapshot::capture(
        json!({"formatVersion":1,"id":"helper","name":"Helper","entry":"flow.rs","files":["flow.rs","asset.bin"],"dependencies":{}}).to_string(),
        BTreeMap::from([("flow.rs".into(), b"pub const BYTES: &[u8] = include_bytes!(\"asset.bin\");\n".to_vec()), ("asset.bin".into(), vec![0, 255, 128, 13, 10])]),
        BTreeMap::new(),
    ).unwrap();
    let package = PackageSnapshot::capture(
        json!({"formatVersion":1,"id":"portable","name":"Portable","entry":"flow.rs","files":["flow.rs","README.md"],"dependencies":{"helper":{"path":"../helper"}}}).to_string(),
        BTreeMap::from([("flow.rs".into(), source.as_bytes().to_vec()), ("README.md".into(), b"Exact package readme\r\n".to_vec())]),
        BTreeMap::from([("helper".into(), dependency)]),
    ).unwrap();
    let mut parent = doc.clone();
    parent.id = "parent".into();
    parent.name = "Parent".into();
    parent.nodes.retain(|node| node.id != "pause");
    parent.edges.retain(|edge| edge.id != "c");
    parent
        .edges
        .iter_mut()
        .find(|edge| edge.id == "b")
        .unwrap()
        .target = "output".into();
    parent.nodes[0].data.config["exports"]["contract"]["branches"] = json!({"work":{"contract":{"input":{"kind":"text"},"output":{"kind":"text"}},"invocations":["node"]}});
    parent.nodes[0].data.config["exports"]["branches"] = json!({"work":"effect"});
    parent.nodes[1].data.kind = "route".into();
    parent.nodes[1].data.config = json!({"branch":"work","inputField":"input","field":"output"});
    let parent_source =
        flow_format::render(&parent, &GraphValidator::new(&RuntimePrimitives)).unwrap();
    let bridge = BridgeDefinition::new()
        .import("worker", "portable")
        .connect(
            "work",
            Connection::new(
                Endpoint::new("root", "work"),
                Endpoint::new("worker", "main"),
                RouteMode::CallAwait,
                InvocationKind::Node,
            ),
        );
    let bridge_source = zf_flows::bridge_source::generate(&bridge).unwrap();
    compile(
        &CompilationSnapshot {
            flows: BTreeMap::from([
                ("portable".into(), SourceSnapshot::capture(source)),
                ("parent".into(), SourceSnapshot::capture(parent_source)),
            ]),
            packages: BTreeMap::from([("portable".into(), package)]),
            bridges: BTreeMap::from([("routing".into(), SourceSnapshot::capture(bridge_source))]),
            ..Default::default()
        },
        &CompileRequest::new(ResolveRequest {
            flow: "parent".into(),
            entry: "main".into(),
            bridges: vec!["routing".into()],
        }),
        &RuntimePrimitives,
    )
    .unwrap()
}

#[test]
fn preserves_exact_sources_binary_assets_and_local_dependency_closure() {
    let plan = fixture();
    let bundle = export_runtime(&plan, &support()).unwrap();
    assert_eq!(
        bundle.files["flows/instance-0/flow.rs"],
        plan.prepared().root().unwrap().source.as_bytes()
    );
    for package in plan.prepared().definitions.flow_packages.values() {
        for (revision, node) in &package.packages {
            assert_eq!(
                bundle.files[&format!("packages/{revision}/flow.json")],
                node.manifest_source.as_bytes()
            );
            for (path, bytes) in &node.files {
                assert_eq!(&bundle.files[&format!("packages/{revision}/{path}")], bytes);
            }
        }
    }
    assert_eq!(bundle.revision, plan.revision());
    assert!(!bundle.files.contains_key("crates/zf-serve/Cargo.toml"));
    let lock = std::str::from_utf8(&bundle.files["Cargo.lock"]).unwrap();
    assert!(lock.contains("name = \"adk-graph\"\nversion = \"2.2.0\""));
}

#[test]
fn support_lock_contains_only_existing_product_versions_and_checksums() {
    fn identities(lock: &str) -> std::collections::BTreeSet<Vec<String>> {
        lock.split("[[package]]")
            .skip(1)
            .filter(|block| block.lines().any(|line| line.starts_with("source = ")))
            .map(|block| {
                ["name = ", "version = ", "source = ", "checksum = "]
                    .iter()
                    .map(|prefix| {
                        block
                            .lines()
                            .find(|line| line.starts_with(prefix))
                            .unwrap()
                            .into()
                    })
                    .collect()
            })
            .collect()
    }
    let product = identities(include_str!("../../../Cargo.lock"));
    let portable = identities(include_str!("../export/Cargo.lock"));
    assert!(!portable.is_empty());
    assert!(
        portable.is_subset(&product),
        "support lock changed an external version, source or checksum"
    );
}

#[test]
fn relocated_export_builds_locked_and_resumes_without_checkout_access() {
    if std::env::var("ZEDFLOW_TEST_CODEGEN").ok().as_deref() != Some("1") {
        return;
    }
    let plan = fixture();
    let bundle = export_runtime(&plan, &support()).unwrap();
    exercise_relocated(bundle, r#"{"answer:routing/worker/pause":"resumed"}"#);
}

#[test]
fn relocated_single_package_preserves_legacy_rust_and_resumes_without_ports() {
    if std::env::var("ZEDFLOW_TEST_CODEGEN").ok().as_deref() != Some("1") {
        return;
    }
    let plan = fixture();
    let mut doc = plan.prepared().flows["routing/worker"].composition.clone();
    doc.nodes[0].data.config = json!({});
    let source = flow_format::render(&doc, &GraphValidator::new(&RuntimePrimitives))
        .unwrap()
        .replace(
            "use zf_runtime::{models, operations, runtime, subgraphs};",
            "use crate::{models, operations, runtime, subgraphs};",
        );
    let package = PackageSnapshot::capture(
        json!({"formatVersion":1,"id":"portable","name":"Single","entry":"flow.rs","files":["flow.rs","asset.bin"],"dependencies":{}}).to_string(),
        BTreeMap::from([("flow.rs".into(), source.as_bytes().to_vec()),("asset.bin".into(),vec![0,255,128])]),
        BTreeMap::new(),
    ).unwrap();
    let bundle = export_single(
        &doc,
        &source,
        Some(&package),
        &RuntimePrimitives,
        &support(),
    )
    .unwrap();
    assert_eq!(bundle.files["flows/instance-0/flow.rs"], source.as_bytes());
    assert_eq!(
        bundle.files[&format!("packages/{}/asset.bin", package.root)],
        vec![0, 255, 128]
    );
    exercise_relocated(bundle, r#"{"answer:pause":"resumed"}"#);
}

fn exercise_relocated(bundle: CargoExport, answer: &str) {
    let binary_asset = bundle
        .files
        .keys()
        .find(|path| path.starts_with("packages/") && path.ends_with("/asset.bin"))
        .unwrap()
        .clone();
    let temp = tempfile::tempdir().unwrap();
    let original = temp.path().join("original");
    for (path, content) in bundle.files {
        let target = original.join(path);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(target, content).unwrap();
    }
    let project = temp.path().join("relocated");

    std::fs::rename(&original, &project).unwrap();
    let workspace = temp.path().join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    let data = temp.path().join("data");
    let lock_before = std::fs::read(project.join("Cargo.lock")).unwrap();
    let first = isolated_run(&project, &workspace, &data, r#"{"input":"begin"}"#);
    assert_eq!(first["status"], "waiting", "{first}");
    assert_eq!(
        std::fs::read_to_string(workspace.join("visits")).unwrap(),
        "x"
    );
    let second = isolated_run(&project, &workspace, &data, answer);
    assert_eq!(second["status"], "completed", "{second}");
    assert_eq!(second["state"]["response"], "resumed", "{second}");
    assert_eq!(
        std::fs::read_to_string(workspace.join("visits")).unwrap(),
        "x"
    );
    assert_eq!(
        std::fs::read(project.join("Cargo.lock")).unwrap(),
        lock_before
    );
    let events = std::fs::read_to_string(data.join("portable/events.jsonl")).unwrap();
    assert!(events.contains("receiptRef"));
    assert!(events.contains("tool_result"));
    std::fs::write(project.join(binary_asset), [127, 1]).unwrap();
    let tampered = isolated_output(&project, &workspace, &data, answer);
    assert!(!tampered.status.success());
    assert!(
        String::from_utf8_lossy(&tampered.stderr)
            .contains("compiled package file differs from frozen revision"),
        "{}",
        String::from_utf8_lossy(&tampered.stderr)
    );
    assert_eq!(
        std::fs::read_to_string(workspace.join("visits")).unwrap(),
        "x"
    );
}

fn isolated_run(project: &Path, workspace: &Path, data: &Path, input: &str) -> Value {
    let output = isolated_output(project, workspace, data, input);
    assert!(
        output.status.success(),
        "isolated export failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("{error}: {}", String::from_utf8_lossy(&output.stdout)))
}

fn isolated_output(
    project: &Path,
    workspace: &Path,
    data: &Path,
    input: &str,
) -> std::process::Output {
    let checkout = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .unwrap();
    Command::new("timeout")
        .args(["300", "python3", "-c", LANDLOCK])
        .arg(checkout)
        .arg("cargo")
        .args(["run", "--offline", "--locked", "--quiet", "--manifest-path"])
        .arg(project.join("Cargo.toml"))
        .args(["--", "--workspace"])
        .arg(workspace)
        .arg("--home")
        .arg(workspace.join(".fixture-home"))
        .arg("--data")
        .arg(data)
        .args(["--run-id", "portable", "--input", input])
        .current_dir(project)
        .env("CARGO_TARGET_DIR", "/tmp/zedflow-adk-target")
        .output()
        .unwrap()
}

// Landlock is inherited by Cargo, rustc, build scripts and the executed binary.
// Allow ordinary system files, toolchain caches and /tmp; deny the actual checkout.
// Verify its canaries are accessible first, then require read and write denial.
const LANDLOCK: &str = r#"
import ctypes, os, sys
canaries=[os.path.join(sys.argv[1],'rust',name) for name in ['Cargo.toml','Cargo.lock']]
for path in canaries:
    for mode in [os.O_RDONLY, os.O_WRONLY]:
        probe=os.open(path,mode)
        os.close(probe)
libc=ctypes.CDLL(None, use_errno=True)
def checked(value):
    if value < 0: raise OSError(ctypes.get_errno(), os.strerror(ctypes.get_errno()))
    return value
class Ruleset(ctypes.Structure): _fields_=[('access',ctypes.c_uint64)]
class Beneath(ctypes.Structure):
    _pack_=1
    _fields_=[('access',ctypes.c_uint64),('parent',ctypes.c_int32)]
# Restrict all filesystem operations through TRUNCATE (ABI 3), including REFER.
# Cache and fixture writes remain permitted; checkout reads and writes are denied.
access=(1<<15)-1
rules=Ruleset(access)
fd=checked(libc.syscall(444,ctypes.byref(rules),ctypes.sizeof(rules),0))
allowed=[os.path.join('/',p) for p in os.listdir('/') if p != 'home']
home=os.path.expanduser('~')
allowed += [os.path.join(home,p) for p in ['.cargo','.rustup','.cache'] if os.path.exists(os.path.join(home,p))]
for path in allowed:
    pfd=os.open(path,os.O_PATH|os.O_CLOEXEC)
    rights=access if os.path.isdir(path) else (1<<0)|(1<<1)|(1<<2)|(1<<14)
    rule=Beneath(rights,pfd)
    checked(libc.syscall(445,fd,1,ctypes.byref(rule),0)); os.close(pfd)
checked(libc.prctl(38,1,0,0,0))
checked(libc.syscall(446,fd,0));os.close(fd)
for path in canaries:
    for mode in [os.O_RDONLY, os.O_WRONLY]:
        try:
            probe=os.open(path,mode)
        except PermissionError: pass
        else:
            os.close(probe)
            raise RuntimeError('checkout remained accessible: '+path)
os.execvp(sys.argv[2],sys.argv[2:])
"#;
