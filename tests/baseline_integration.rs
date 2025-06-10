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
        .arg("echo")
        .arg("test");

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
        .arg("echo")
        .arg("test");

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

#[test]
fn test_regression_detection() {
    let temp_dir = TempDir::new().unwrap();
    let baseline_dir = temp_dir.path().to_str().unwrap();

    // Save a baseline with a small memory footprint
    let mut cmd = Command::cargo_bin("peak-mem").unwrap();
    cmd.arg("--baseline-dir")
        .arg(baseline_dir)
        .arg("--save-baseline")
        .arg("small_memory")
        .arg("--")
        .arg("true");
    cmd.assert().success();

    // Compare with same command - should not detect regression
    let mut cmd = Command::cargo_bin("peak-mem").unwrap();
    cmd.arg("--baseline-dir")
        .arg(baseline_dir)
        .arg("--compare-baseline")
        .arg("small_memory")
        .arg("--regression-threshold")
        .arg("10")
        .arg("--")
        .arg("true");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("No regression detected"));
}

#[test]
fn test_comparison_output_formats() {
    let temp_dir = TempDir::new().unwrap();
    let baseline_dir = temp_dir.path().to_str().unwrap();

    // Save a baseline
    let mut cmd = Command::cargo_bin("peak-mem").unwrap();
    cmd.arg("--baseline-dir")
        .arg(baseline_dir)
        .arg("--save-baseline")
        .arg("format_test")
        .arg("--")
        .arg("true");
    cmd.assert().success();

    // Test JSON format
    let mut cmd = Command::cargo_bin("peak-mem").unwrap();
    cmd.arg("--baseline-dir")
        .arg(baseline_dir)
        .arg("--compare-baseline")
        .arg("format_test")
        .arg("--json")
        .arg("--")
        .arg("true");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("\"regression_detected\""));

    // Test CSV format
    let mut cmd = Command::cargo_bin("peak-mem").unwrap();
    cmd.arg("--baseline-dir")
        .arg(baseline_dir)
        .arg("--compare-baseline")
        .arg("format_test")
        .arg("--csv")
        .arg("--")
        .arg("true");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("baseline_command"));

    // Test quiet format
    let mut cmd = Command::cargo_bin("peak-mem").unwrap();
    cmd.arg("--baseline-dir")
        .arg(baseline_dir)
        .arg("--compare-baseline")
        .arg("format_test")
        .arg("--quiet")
        .arg("--")
        .arg("true");

    cmd.assert()
        .success()
        .stdout(predicate::str::is_match("^ok\n$").unwrap());
}
