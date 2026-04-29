//! `mandate-mcp` — Mandate MCP stdio JSON-RPC server (Passport P3.1).
//!
//! Reads newline-delimited JSON-RPC 2.0 requests from stdin, dispatches
//! them through the in-process tool catalogue (`mandate_mcp::dispatch`),
//! and writes one newline-delimited JSON-RPC 2.0 response per request
//! to stdout. Logs go to stderr so MCP clients consuming stdout aren't
//! confused by interleaved tracing output.
//!
//! The protocol is documented in `docs/cli/mcp.md`. Tests drive the
//! binary by spawning a child process and writing/reading through the
//! pipes; see `crates/mandate-mcp/tests/stdio_jsonrpc.rs`.

use std::io::{BufRead, Write};
use std::sync::Arc;

use mandate_mcp::{dispatch_to_response, jsonrpc, ServerContext};

fn main() {
    // tracing → stderr only; stdout is the JSON-RPC channel.
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tracing::info!(
        "mandate-mcp: stdio JSON-RPC server ready (protocol: NDJSON, tools: 6, see docs/cli/mcp.md)"
    );

    let ctx = Arc::new(ServerContext::new());
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout().lock();
    let reader = stdin.lock();

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                tracing::error!("read stdin: {e}");
                break;
            }
        };
        if line.trim().is_empty() {
            continue;
        }
        let response = match jsonrpc::parse_request(&line) {
            Ok(req) => dispatch_to_response(&req, &ctx),
            Err(resp) => resp,
        };
        let serialised = match serde_json::to_string(&response) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("serialise response: {e}");
                continue;
            }
        };
        if let Err(e) = writeln!(stdout, "{serialised}") {
            tracing::error!("write stdout: {e}");
            break;
        }
        if let Err(e) = stdout.flush() {
            tracing::error!("flush stdout: {e}");
            break;
        }
    }

    tracing::info!("mandate-mcp: stdin closed, exiting");
}
