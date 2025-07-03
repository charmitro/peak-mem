use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

#[test]
fn test_save_and_compare_baseline() {
    let temp_dir = TempDir::new().unwrap();
    let baseline_dir = temp_dir.path().to_str().unwrap();

    // Save a baseline
    let mut cmd = Command::cargo_bin("peak-mem").unwrap();
    cmd.arg("--baseline-dir")
        .arg(baseline_dir)
        .arg("--save-baseline")
        .arg("test_baseline")
        .arg("--")
        .arg("sleep")
        .arg("0.01");

    cmd.assert()
        .success()
        .stderr(predicate::str::contains("Baseline 'test_baseline' saved"));

    // Compare against baseline
    let mut cmd = Command::cargo_bin("peak-mem").unwrap();
    cmd.arg("--baseline-dir")
        .arg(baseline_dir)
        .arg("--compare-baseline")
        .arg("test_baseline")
        .arg("--")
        .arg("sleep")
        .arg("0.01");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("No regression detected"));
}

#[test]
fn test_list_baselines() {
    let temp_dir = TempDir::new().unwrap();
    let baseline_dir = temp_dir.path().to_str().unwrap();

    // List empty baselines
    let mut cmd = Command::cargo_bin("peak-mem").unwrap();
    cmd.arg("--baseline-dir")
        .arg(baseline_dir)
        .arg("--list-baselines");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("No baselines found"));

    // Save a baseline
    let mut cmd = Command::cargo_bin("peak-mem").unwrap();
    cmd.arg("--baseline-dir")
        .arg(baseline_dir)
        .arg("--save-baseline")
        .arg("test1")
        .arg("--")
        .arg("true");
    cmd.assert().success();

    // List baselines
    let mut cmd = Command::cargo_bin("peak-mem").unwrap();
    cmd.arg("--baseline-dir")
        .arg(baseline_dir)
        .arg("--list-baselines");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("test1"));
}

#[test]
fn test_delete_baseline() {
    let temp_dir = TempDir::new().unwrap();
    let baseline_dir = temp_dir.path().to_str().unwrap();

    // Save a baseline
    let mut cmd = Command::cargo_bin("peak-mem").unwrap();
    cmd.arg("--baseline-dir")
        .arg(baseline_dir)
        .arg("--save-baseline")
        .arg("to_delete")
        .arg("--")
        .arg("true");
    cmd.assert().success();

    // Delete baseline
    let mut cmd = Command::cargo_bin("peak-mem").unwrap();
    cmd.arg("--baseline-dir")
        .arg(baseline_dir)
        .arg("--delete-baseline")
        .arg("to_delete");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Baseline 'to_delete' deleted"));

    // Verify it's gone
    let mut cmd = Command::cargo_bin("peak-mem").unwrap();
    cmd.arg("--baseline-dir")
        .arg(baseline_dir)
        .arg("--list-baselines");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("No baselines found"));
}
