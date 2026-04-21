use tempfile::TempDir;
use crate::common::run_rstube;

#[test]
fn version_subcommand_prints_build_metadata() {
    let dir = TempDir::new().expect("Failed to create temp dir");
    let output = run_rstube(dir.path(), &["version"]);

    assert!(output.status.success(), "rstube version exited non-zero: {:?}", output.status);

    let stdout = String::from_utf8_lossy(&output.stdout);
    for key in ["GIT_SHA:", "GIT_BRANCH:", "RUSTC_SEMVER:", "RUST_EDITION:", "BUILD_TIMESTAMP:"] {
        assert!(stdout.contains(key), "expected {key} in output, got:\n{stdout}");
    }
    assert!(stdout.starts_with("rstube "), "expected name+version prefix, got:\n{stdout}");
}
