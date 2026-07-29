//! Source-only extension installation.

use std::path::{Path, PathBuf};

pub use crate::extensions::{ExtensionSource, ProvenanceReceipt};

/// Installs a trusted extension source, builds it locally, and records its
/// source and artifact digests. `artifact` is relative to Cargo's target dir.
pub fn install_extension(
    source: &ExtensionSource,
    cache: &Path,
    staging: &Path,
    artifact: &Path,
    store: &Path,
    previous: Option<String>,
    trusted: bool,
) -> Result<(PathBuf, ProvenanceReceipt), String> {
    let source_dir = crate::extensions::acquire_source(source, cache, trusted)?;
    crate::extensions::build_and_store(source, &source_dir, staging, artifact, store, previous)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_refuses_untrusted_source_before_building() {
        let root =
            std::env::temp_dir().join(format!("zedflow-package-manager-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("source")).unwrap();
        let source = ExtensionSource::Path(root.join("source"));
        assert!(
            install_extension(
                &source,
                &root.join("cache"),
                &root.join("staging"),
                Path::new("target/release/extension"),
                &root.join("store"),
                None,
                false,
            )
            .is_err()
        );
        let _ = std::fs::remove_dir_all(root);
    }
}
