use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn help_prints_usage() {
    let mut cmd = Command::cargo_bin("http-probe").unwrap();
    cmd.arg("--help").assert().success().stdout(
        predicate::str::contains("Usage: http-probe")
            .and(predicate::str::contains("[OPTIONS] <URL>")),
    );
}

#[test]
fn version_prints_package_version() {
    let mut cmd = Command::cargo_bin("http-probe").unwrap();
    cmd.arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("http-probe 0.1.0"));
}

#[test]
fn unsupported_scheme_exits_with_configuration_error() {
    let mut cmd = Command::cargo_bin("http-probe").unwrap();
    cmd.args(["ftp://example.com", "--count", "1"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("http:// or https://"));
}

#[test]
fn invalid_timeout_relationship_exits_with_configuration_error() {
    let mut cmd = Command::cargo_bin("http-probe").unwrap();
    cmd.args([
        "http://example.com",
        "--count",
        "1",
        "--timeout-ms",
        "10",
        "--connect-timeout-ms",
        "11",
    ])
    .assert()
    .code(2)
    .stderr(predicate::str::contains("connect-timeout-ms"));
}
