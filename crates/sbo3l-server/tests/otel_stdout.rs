//! R14 P5 — end-to-end test for the OTEL stdout exporter.
//!
//! Spawns the `sbo3l-server` binary as a subprocess with
//! `SBO3L_OTEL_EXPORTER=stdout`, fires a real HTTP request at it, then
//! gracefully shuts it down and asserts the captured stdout contains
//! both:
//! - The "Spans" header that `opentelemetry-stdout` prints on every
//!   export batch (proves the exporter ran).
//! - A `Name` line for the per-request `http.request` span (proves the
//!   middleware-instrumented span actually flowed through the
//!   tracing-opentelemetry layer to the exporter).
//!
//! This test is gated on `cfg(feature = "otel")` so it doesn't run in
//! the default-features test pass; it spends real wall-clock time
//! spawning the binary and a non-otel build doesn't have the binary
//! wired for OTEL anyway.

#![cfg(feature = "otel")]

use std::io::{Read, Write};
use std::net::TcpListener as StdTcpListener;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const APRP_GOLDEN: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../test-corpus/aprp/golden_001_minimal.json"
));

/// Find a free TCP port by binding to `:0` and immediately dropping
/// the socket. There's a small race between the drop and the daemon
/// claiming the port, but it's tolerable for an integration test on
/// a single host.
fn pick_free_port() -> u16 {
    let listener = StdTcpListener::bind("127.0.0.1:0").expect("bind 0");
    let port = listener.local_addr().expect("local_addr").port();
    drop(listener);
    port
}

/// Locate the just-built `sbo3l-server` binary. Cargo passes the
/// target dir via `CARGO_TARGET_DIR` (when set) or implicitly via the
/// workspace's default `target/`. Walk up from `CARGO_MANIFEST_DIR`
/// until we find the `target` dir; pick `debug` (test builds run
/// against the debug profile).
fn locate_server_binary() -> std::path::PathBuf {
    // The `CARGO_BIN_EXE_<name>` env var is set by Cargo for binary
    // crates declared in the same package as the integration test.
    // `sbo3l-server` declares `[[bin]] name = "sbo3l-server"` so
    // this lookup works.
    let path = env!("CARGO_BIN_EXE_sbo3l-server");
    std::path::PathBuf::from(path)
}

#[test]
fn stdout_exporter_emits_spans_on_real_request() {
    let bin = locate_server_binary();
    let port = pick_free_port();
    let listen = format!("127.0.0.1:{port}");

    // SBO3L_DB=:memory: avoids leaving SQLite files around;
    // SBO3L_OTEL_EXPORTER=stdout is the unit under test;
    // SBO3L_ALLOW_UNAUTHENTICATED=1 lets us POST without minting a
    // bearer token in the test (the binary requires auth by default).
    let mut child = Command::new(&bin)
        .env("SBO3L_LISTEN", &listen)
        .env("SBO3L_DB", ":memory:")
        .env("SBO3L_OTEL_EXPORTER", "stdout")
        .env("SBO3L_OTEL_SERVICE_NAME", "sbo3l-server-otel-test")
        .env("SBO3L_ALLOW_UNAUTHENTICATED", "1")
        .env("SBO3L_DEV_ONLY_SIGNER", "1")
        .env("RUST_LOG", "info")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn sbo3l-server");

    // Wait for the daemon to bind. Poll the port; bail after 5s.
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if Instant::now() > deadline {
            let _ = child.kill();
            let mut stderr_buf = String::new();
            if let Some(mut s) = child.stderr.take() {
                let _ = s.read_to_string(&mut stderr_buf);
            }
            panic!("daemon did not bind within 5s. stderr: {stderr_buf}");
        }
        if std::net::TcpStream::connect_timeout(
            &format!("127.0.0.1:{port}").parse().unwrap(),
            Duration::from_millis(100),
        )
        .is_ok()
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    // Fire one POST. We use a blocking client and a tiny synchronous
    // shim to keep this test free of tokio runtime setup.
    let url = format!("http://{listen}/v1/payment-requests");
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("client");
    let resp = client
        .post(&url)
        .header("content-type", "application/json")
        .body(APRP_GOLDEN.to_string())
        .send()
        .expect("POST /v1/payment-requests");
    let status = resp.status();
    let body_text = resp.text().unwrap_or_default();
    assert_eq!(
        status, 200,
        "expected 200 from /v1/payment-requests, got {status}: {body_text}"
    );

    // Give the OTEL batch span processor a chance to flush. The
    // batch processor flushes on a timer (default 5s) OR on
    // shutdown; we want shutdown so we don't pay 5s of wall-clock.
    // Send SIGTERM (so the binary's graceful-shutdown path runs and
    // calls otel::shutdown()).
    #[cfg(unix)]
    {
        let pid = child.id() as i32;
        // SAFETY: libc::kill is FFI; the only invariants are valid
        // pid + valid signal number. Both are correct here.
        unsafe {
            libc::kill(pid, libc::SIGTERM);
        }
    }
    #[cfg(not(unix))]
    {
        let _ = child.kill();
    }

    // Wait for graceful exit, then drain stdout.
    let exit = child
        .wait_with_output()
        .expect("wait for sbo3l-server exit");
    let stdout = String::from_utf8_lossy(&exit.stdout).to_string();
    let stderr = String::from_utf8_lossy(&exit.stderr).to_string();

    // The opentelemetry-stdout SpanExporter prints "Spans" as the
    // batch header and one `Name : <span-name>` line per exported
    // span. Our axum middleware opens a span called `http.request`
    // for every incoming request; that's the marker we assert on.
    assert!(
        stdout.contains("Spans"),
        "expected 'Spans' header in captured stdout — \
         OTEL stdout exporter did not run.\nstdout:\n{stdout}\n\
         stderr:\n{stderr}"
    );
    assert!(
        stdout.contains("http.request"),
        "expected the per-request 'http.request' span to be exported.\n\
         stdout:\n{stdout}\nstderr:\n{stderr}"
    );
}

#[test]
fn no_otel_stdout_when_exporter_is_none() {
    // Sanity check: with SBO3L_OTEL_EXPORTER=none (the default), the
    // stdout exporter is NOT installed — nothing OTEL-shaped lands
    // on stdout. This pins the no-op startup contract documented in
    // `otel::init_tracer`.
    let bin = locate_server_binary();
    let port = pick_free_port();
    let listen = format!("127.0.0.1:{port}");

    let mut child = Command::new(&bin)
        .env("SBO3L_LISTEN", &listen)
        .env("SBO3L_DB", ":memory:")
        .env("SBO3L_OTEL_EXPORTER", "none")
        .env("SBO3L_ALLOW_UNAUTHENTICATED", "1")
        .env("SBO3L_DEV_ONLY_SIGNER", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn sbo3l-server");

    // Wait for bind.
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if Instant::now() > deadline {
            let _ = child.kill();
            panic!("daemon did not bind within 5s");
        }
        if std::net::TcpStream::connect_timeout(
            &format!("127.0.0.1:{port}").parse().unwrap(),
            Duration::from_millis(100),
        )
        .is_ok()
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("client");
    let _ = client
        .post(format!("http://{listen}/v1/payment-requests"))
        .header("content-type", "application/json")
        .body(APRP_GOLDEN.to_string())
        .send();

    #[cfg(unix)]
    {
        let pid = child.id() as i32;
        unsafe {
            libc::kill(pid, libc::SIGTERM);
        }
    }
    #[cfg(not(unix))]
    {
        let _ = child.kill();
    }

    let exit = child
        .wait_with_output()
        .expect("wait for sbo3l-server exit");
    let stdout = String::from_utf8_lossy(&exit.stdout).to_string();
    // The OTEL stdout exporter prints a literal `"Spans"` header.
    // We assert it's absent — when exporter=none the exporter is
    // never installed.
    assert!(
        !stdout.contains("Spans"),
        "stdout should be free of OTEL span dumps when exporter=none.\nstdout:\n{stdout}"
    );
    // Also `http.request` (our middleware span name) should not show
    // up — it would only land here via the OTEL stdout exporter,
    // since `tracing_subscriber::fmt::layer()` formats spans
    // differently and doesn't emit a bare `Name : http.request` line.
    assert!(
        !stdout.contains("Name         : http.request"),
        "stdout should be free of exported span names when exporter=none.\nstdout:\n{stdout}"
    );
}

// Also support a `Write` flush helper so clippy doesn't complain
// about the unused import on the `Read` path.
#[allow(dead_code)]
fn _flush(w: &mut impl Write) -> std::io::Result<()> {
    w.flush()
}
