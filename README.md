# http-probe

`http-probe` is a Rust CLI that repeatedly probes one HTTP or HTTPS endpoint and reports latency statistics.

Each attempt is a cold network probe: the tool resolves DNS, opens a new TCP connection, performs TLS for HTTPS, sends a GET request, reads the first response byte, consumes the full body, and closes the connection. It is not a load tester and does not measure keep-alive reuse.

## Features

- HTTP and HTTPS GET requests
- Sequential execution with configurable count, timeout, connect timeout, and interval
- Optional redirect following, up to 10 redirects
- DNS, TCP, TLS, TTFB, body transfer, and total timing rows
- Min, max, average, median, P95, and P99 over successful requests
- Failure counts for HTTP statuses and transport errors
- Verbose per-attempt output on stderr

## Install

```bash
cargo install --path .
```

The `curl` crate binds to libcurl. Platform packages may be needed depending on your Rust target and OS image; CI documents the supported setup.

## Usage

```bash
cargo run -- https://example.com/api/health --count 100
```

```text
Usage: http-probe [OPTIONS] <URL>
```

Options:

```text
-c, --count <COUNT>                    Number of requests [default: 10]
    --timeout-ms <MILLISECONDS>        Total timeout per request [default: 5000]
    --connect-timeout-ms <MILLISECONDS> Connection timeout per request [default: 3000]
    --interval-ms <MILLISECONDS>       Delay between requests [default: 0]
    --follow-redirects                 Follow HTTP redirects, up to 10
-v, --verbose                          Print every attempt to stderr
-h, --help                             Print help
-V, --version                          Print version
```

## Sample Report

```text
HTTP Latency Probe
Target:           https://example.com/api/health
Measurement:      fresh DNS and connection per attempt
Attempts:         100
Successful:       98
Failed:           2
Failure rate:     2.00%
HTTP redirects:   disabled

Latency statistics for successful requests (milliseconds)

Phase                  Min       Avg    Median       P95       P99       Max
DNS                  1.203     1.508     1.441     2.107     2.804     2.804
TCP connection       8.604     9.211     9.020    11.335    13.102    13.102
TLS handshake       12.730    14.118    13.904    17.441    20.870    20.870
TTFB                 4.122     5.981     5.617     8.492    12.341    12.341
Body transfer        0.041     0.084     0.073     0.151     0.302     0.302
Total               27.004    30.902    30.110    38.046    46.227    46.227

Failures
HTTP 503:                1
Timeout:                 1
```

## Timing Definitions

Libcurl reports cumulative timings from the start of each transfer. `http-probe` converts them into phases with saturating subtraction:

- DNS: `namelookup_time`
- TCP connection: `connect_time - namelookup_time`
- TLS handshake for HTTPS: `appconnect_time - connect_time`
- TTFB phase: `starttransfer_time - pretransfer_time`
- Body transfer: `total_time - starttransfer_time`
- Total: `total_time`

HTTP targets display TLS as unavailable. All measurements are stored as integer microseconds and displayed as milliseconds with three decimals.

## Statistics

Statistics are calculated only from successful 2xx attempts. Percentiles use the nearest-rank method, so P99 can equal the maximum for small sample counts.

## Success, Failure, and Exit Codes

A successful attempt is a completed transfer with final HTTP status `200..=299`. Any other completed HTTP response is an HTTP failure. DNS, connection, timeout, TLS, redirect, and receive errors are transport failures. Failed attempts are counted in the failure rate but excluded from latency statistics.

Exit codes:

- `0`: all requested attempts succeeded
- `1`: the run completed with one or more failed attempts
- `2`: invalid command-line input or probe configuration
- `3`: unexpected internal error prevented completion

## Development

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo build --release
```

Manual smoke test:

```powershell
.\target\release\http-probe.exe https://example.com --count 5 --verbose
```

## Limitations

Version 0.1.0 intentionally excludes async execution, concurrency, custom methods, request bodies, custom headers, authentication, insecure TLS mode, JSON/CSV output, retries, warm-up requests, and load-test scheduling.

## License

MIT
