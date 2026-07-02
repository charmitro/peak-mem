//! End-to-end tests for the peak-mem command-line interface.
//!
//! These tests run the compiled binary against real short-lived
//! processes. Monitored commands are chosen to produce no stdout of
//! their own, since the child inherits peak-mem's stdio and would
//! otherwise interleave with the report being asserted on.

use assert_cmd::Command;
use predicates::prelude::*;

fn peak_mem() -> Command {
    Command::cargo_bin("peak-mem").unwrap()
}

#[test]
fn basic_run_reports_peak_memory() {
    peak_mem()
        .args(["--", "sleep", "0.3"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Command: sleep 0.3"))
        .stdout(predicate::str::contains("Peak memory usage:"));
}

#[test]
fn json_output_is_valid_and_complete() {
    let assert = peak_mem()
        .args(["--json", "--", "sleep", "0.3"])
        .assert()
        .success();

    let json: serde_json::Value = serde_json::from_slice(&assert.get_output().stdout)
        .expect("--json should emit valid JSON on stdout");
    assert_eq!(json["command"], "sleep 0.3");
    assert_eq!(json["exit_code"], 0);
    assert!(json["peak_rss_bytes"].as_u64().unwrap() > 0);
    assert!(json["peak_vsz_bytes"].as_u64().unwrap() > 0);
    assert!(json["duration_ms"].as_u64().unwrap() >= 300);
}

#[test]
fn csv_output_has_header_and_row() {
    let assert = peak_mem()
        .args(["--csv", "--", "sleep", "0.3"])
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 2);
    assert!(lines[0].starts_with("command,peak_rss_bytes,peak_vsz_bytes"));
    assert!(lines[1].starts_with("sleep 0.3,"));
}

#[test]
fn quiet_outputs_only_rss_bytes() {
    let assert = peak_mem()
        .args(["--quiet", "--", "sleep", "0.3"])
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let rss: u64 = stdout
        .trim()
        .parse()
        .expect("--quiet should output a single number");
    assert!(rss > 0);
}

#[test]
fn exit_code_is_passed_through() {
    peak_mem()
        .args(["--", "sh", "-c", "exit 7"])
        .assert()
        .code(7);
}

#[test]
fn threshold_exceeded_exits_with_one() {
    peak_mem()
        .args(["--threshold", "1", "--", "sleep", "0.3"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("THRESHOLD EXCEEDED"));
}

#[test]
fn baseline_save_list_delete_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let dir_arg = dir.path().to_str().unwrap();

    peak_mem()
        .args(["--baseline-dir", dir_arg, "--save-baseline", "ci"])
        .args(["--", "sleep", "0.3"])
        .assert()
        .success();

    peak_mem()
        .args(["--baseline-dir", dir_arg, "--list-baselines"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Saved baselines:"))
        .stdout(predicate::str::contains("ci"));

    peak_mem()
        .args(["--baseline-dir", dir_arg, "--delete-baseline", "ci"])
        .assert()
        .success();

    peak_mem()
        .args(["--baseline-dir", dir_arg, "--list-baselines"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No baselines found"));
}

#[test]
fn invalid_baseline_name_is_rejected() {
    let dir = tempfile::tempdir().unwrap();

    peak_mem()
        .args(["--baseline-dir", dir.path().to_str().unwrap()])
        .args(["--save-baseline", "..", "--", "sleep", "0.1"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Invalid baseline name"));
}
