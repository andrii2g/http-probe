use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

use http_probe::cli::Cli;
use http_probe::execute;
use http_probe::model::{AttemptResult, TransportFailureKind};

#[test]
fn local_200_server_produces_success_summary() {
    let server = TestServer::start(vec![ResponseMode::Status(200, "OK")]);
    let summary = execute(cli(&server.url(), 1, 1000)).unwrap();
    assert_eq!(summary.requested_attempts, 1);
    assert_eq!(summary.successful_attempts, 1);
    assert_eq!(summary.failed_attempts, 0);
    assert!(summary.latency.is_some());
    assert_eq!(summary.status_counts.get(&200), Some(&1));
}

#[test]
fn local_503_server_is_http_failure() {
    let server = TestServer::start(vec![ResponseMode::Status(503, "NO")]);
    let summary = execute(cli(&server.url(), 1, 1000)).unwrap();
    assert_eq!(summary.successful_attempts, 0);
    assert_eq!(summary.failed_attempts, 1);
    assert_eq!(summary.failure_rate, 100.0);
    assert!(summary.latency.is_none());
    assert_eq!(summary.status_counts.get(&503), Some(&1));
}

#[test]
fn mixed_responses_count_all_statuses_but_only_success_latency() {
    let server = TestServer::start(vec![
        ResponseMode::Status(200, "OK"),
        ResponseMode::Status(503, "NO"),
    ]);
    let summary = execute(cli(&server.url(), 2, 1000)).unwrap();
    assert_eq!(summary.successful_attempts, 1);
    assert_eq!(summary.failed_attempts, 1);
    assert_eq!(summary.status_counts.get(&200), Some(&1));
    assert_eq!(summary.status_counts.get(&503), Some(&1));
    assert_eq!(summary.latency.unwrap().total.samples, 1);
}

#[test]
fn delayed_response_records_tolerant_timing() {
    let server = TestServer::start(vec![ResponseMode::DelayThenStatus(
        Duration::from_millis(100),
        200,
        "OK",
    )]);
    let summary = execute(cli(&server.url(), 1, 1000)).unwrap();
    let total = summary.latency.unwrap().total.min_us;
    assert!(total >= 70_000, "total was {total}us");
}

#[test]
fn timeout_is_transport_failure_and_run_continues() {
    let server = TestServer::start(vec![
        ResponseMode::DelayThenStatus(Duration::from_millis(200), 200, "OK"),
        ResponseMode::Status(200, "OK"),
    ]);
    let mut args = cli(&server.url(), 2, 50);
    args.connect_timeout_ms = 50;
    let summary = execute(args).unwrap();
    assert_eq!(summary.requested_attempts, 2);
    assert_eq!(summary.failed_attempts, 1);
    assert_eq!(summary.successful_attempts, 1);
    assert!(matches!(
        &summary.attempts[0],
        AttemptResult::TransportFailure(failure)
            if failure.kind == TransportFailureKind::Timeout || failure.kind == TransportFailureKind::Other
    ));
}

#[test]
fn body_consumption_records_downloaded_bytes() {
    let server = TestServer::start(vec![ResponseMode::Status(200, "hello")]);
    let summary = execute(cli(&server.url(), 1, 1000)).unwrap();
    match &summary.attempts[0] {
        AttemptResult::Success(success) => assert_eq!(success.response.downloaded_bytes, 5),
        other => panic!("unexpected result: {other:?}"),
    }
}

fn cli(url: &str, count: u32, timeout_ms: u64) -> Cli {
    Cli {
        url: url.to_string(),
        count,
        timeout_ms,
        connect_timeout_ms: timeout_ms,
        interval_ms: 0,
        follow_redirects: false,
        verbose: false,
    }
}

struct TestServer {
    url: String,
    handle: Option<thread::JoinHandle<()>>,
}

impl TestServer {
    fn start(responses: Vec<ResponseMode>) -> Self {
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
            handle.join().unwrap();
        }
    }
}

enum ResponseMode {
    Status(u16, &'static str),
    DelayThenStatus(Duration, u16, &'static str),
}

fn handle_connection(mut stream: TcpStream, response: ResponseMode) {
    {
        let mut reader = BufReader::new(&mut stream);
        let mut line = String::new();
        loop {
            line.clear();
            let bytes = reader.read_line(&mut line).unwrap();
            if bytes == 0 || line == "\r\n" {
                break;
            }
        }
    }

    let (status, body) = match response {
        ResponseMode::Status(status, body) => (status, body),
        ResponseMode::DelayThenStatus(delay, status, body) => {
            thread::sleep(delay);
            (status, body)
        }
    };
    let reason = if status == 200 {
        "OK"
    } else {
        "Service Unavailable"
    };
    write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .unwrap();
}
