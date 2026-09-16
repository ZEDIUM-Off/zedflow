use sha2::{Digest, Sha256};
use std::{fs, path::Path, process::Command};
fn visit(root: &Path, path: &Path, hash: &mut Sha256) {
    if path.is_dir() {
        let mut files: Vec<_> = fs::read_dir(path)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        files.sort();
        for file in files {
            visit(root, &file, hash);
        }
    } else {
        hash.update(
            path.strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/"),
        );
        hash.update([0]);
        hash.update(fs::read(path).unwrap());
        hash.update([0]);
    }
}
fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .unwrap();
    let mut hash = Sha256::new();
    let mut environment: Vec<_> = std::env::vars()
        .filter(|(name, _)| {
            name.starts_with("CARGO_FEATURE_")
                || [
                    "TARGET",
                    "PROFILE",
                    "OPT_LEVEL",
                    "DEBUG",
                    "CARGO_ENCODED_RUSTFLAGS",
                ]
                .contains(&name.as_str())
        })
        .collect();
    environment.sort();
    for (name, value) in environment {
        hash.update(name);
        hash.update([0]);
        hash.update(value);
        hash.update([0]);
    }
    if let Ok(output) = Command::new(std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into()))
        .arg("--version")
        .output()
    {
        hash.update(output.stdout);
    }

    for name in [
        "version.json",
        "rust/crates",
        "rust/Cargo.toml",
        "rust/Cargo.lock",
    ] {
        let path = root.join(name);
        println!("cargo:rerun-if-changed={}", path.display());
        visit(&root, &path, &mut hash);
    }
    let mut value: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("version.json")).unwrap()).unwrap();
    let revision = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(&root)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned());
    value["component"] = "daemon".into();
    value["buildId"] = format!("{:x}", hash.finalize()).into();
    value["revision"] = serde_json::json!(revision);
    value["target"] = std::env::var("TARGET").unwrap().into();
    fs::write(
        Path::new(&std::env::var("OUT_DIR").unwrap()).join("build-info.json"),
        serde_json::to_vec(&value).unwrap(),
    )
    .unwrap();
}
