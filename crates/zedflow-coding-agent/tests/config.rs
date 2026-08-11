use std::path::Path;
use zedflow_coding_agent::config::{InstallMethod, detect_install_method_from_paths};

#[test]
fn install_method_detection_handles_pnpm_paths() {
    assert_eq!(
        detect_install_method_from_paths(
            Path::new("/home/me/.pnpm/pi"),
            Path::new("/bin/pi"),
            false,
            false
        ),
        InstallMethod::Pnpm
    );
}
