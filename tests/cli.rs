use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;

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

#[test]
fn local_200_server_prints_report_headings() {
    let server = TestServer::start(vec![(200, "OK")]);
    let mut cmd = Command::cargo_bin("http-probe").unwrap();
    cmd.args([&server.url(), "--count", "1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("HTTP Latency Probe"))
        .stdout(predicate::str::contains(
            "Latency statistics for successful requests",
        ));
}

#[test]
fn local_503_server_exits_with_code_1() {
    let server = TestServer::start(vec![(503, "NO")]);
    let mut cmd = Command::cargo_bin("http-probe").unwrap();
    cmd.args([&server.url(), "--count", "1"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("HTTP 503"));
}

#[test]
fn verbose_attempt_output_goes_to_stderr() {
    let server = TestServer::start(vec![(200, "OK")]);
    let mut cmd = Command::cargo_bin("http-probe").unwrap();
    cmd.args([&server.url(), "--count", "1", "--verbose"])
        .assert()
        .success()
        .stdout(predicate::str::contains("[1/1]").not())
        .stderr(predicate::str::contains("[1/1] success"));
}

struct TestServer {
    url: String,
    handle: Option<thread::JoinHandle<()>>,
}

impl TestServer {
    fn start(responses: Vec<(u16, &'static str)>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = thread::spawn(move || {
            for response in responses {
                let (stream, _) = listener.accept().unwrap();
                handle_connection(stream, response);
            }
        });

        Self {
            url: format!("http://{addr}/"),
            handle: Some(handle),
        }
    }

    fn url(&self) -> String {
        self.url.clone()
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn handle_connection(mut stream: TcpStream, response: (u16, &'static str)) {
    {
        let mut reader = BufReader::new(&mut stream);
        let mut line = String::new();
        loop {
            line.clear();
            let Ok(bytes) = reader.read_line(&mut line) else {
                return;
            };
            if bytes == 0 || line == "\r\n" {
                break;
            }
        }
    }

    let (status, body) = response;
    let reason = if status == 200 {
        "OK"
    } else {
        "Service Unavailable"
    };
    let _ = write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
}
