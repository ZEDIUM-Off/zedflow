//! Immutable packaging support. The executable entrypoint belongs to execution.
//! Source capture happens at build time; exporting never reads the source tree.
use zf_compiler::export::RuntimeSupport;

include!(concat!(env!("OUT_DIR"), "/export-support.rs"));

pub fn support() -> RuntimeSupport {
    let mut support = RuntimeSupport {
        files: SUPPORT
            .iter()
            .map(|(path, source)| ((*path).into(), source.as_bytes().to_vec()))
            .collect(),
    };
    support.files.insert(
        "Cargo.lock".into(),
        include_bytes!("../export/Cargo.lock").to_vec(),
    );
    support
}
