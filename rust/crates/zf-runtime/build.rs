//! Capture the seven internal library crates, never a checkout at export time.
use std::{env, fs, path::Path};

const CRATES: &[&str] = &[
    "zf-core",
    "zf-context",
    "zf-flows",
    "zf-compiler",
    "zf-storage",
    "zf-runtime",
    "zf-execution",
];

fn capture(root: &Path, path: &Path, generated: &mut String) {
    println!("cargo:rerun-if-changed={}", path.display());
    assert!(!fs::symlink_metadata(path).unwrap().file_type().is_symlink());
    if path.is_dir() {
        let mut entries: Vec<_> = fs::read_dir(path)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        entries.sort();
        for entry in entries {
            capture(root, &entry, generated);
        }
    } else {
        let relative = path
            .strip_prefix(root)
            .unwrap()
            .to_str()
            .unwrap()
            .replace('\\', "/");
        // Literals contain source text, not recursive expansion of include!.
        let source = fs::read_to_string(path).unwrap();
        generated.push_str(&format!("({relative:?}, {source:?}),\n"));
    }
}

fn main() {
    let manifest = env::var_os("CARGO_MANIFEST_DIR").unwrap();
    let root = Path::new(&manifest).parent().unwrap().parent().unwrap();
    let mut generated = String::from("pub(crate) const SUPPORT: &[(&str, &str)] = &[\n");
    for name in ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml"] {
        capture(root, &root.join(name), &mut generated);
    }
    for name in CRATES {
        let directory = root.join("crates").join(name);
        capture(root, &directory.join("Cargo.toml"), &mut generated);
        capture(root, &directory.join("src"), &mut generated);
    }
    capture(
        root,
        &root.join("crates/zf-runtime/build.rs"),
        &mut generated,
    );
    capture(
        root,
        &root.join("crates/zf-runtime/export/Cargo.lock"),
        &mut generated,
    );
    generated.push_str("];\n");
    fs::write(
        Path::new(&env::var_os("OUT_DIR").unwrap()).join("export-support.rs"),
        generated,
    )
    .unwrap();
}
