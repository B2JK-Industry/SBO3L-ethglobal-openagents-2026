//! `sbo3l-mcp` — SBO3L MCP JSON-RPC server.
//!
//! Two transports, one binary:
//!   - **stdio** (default): reads NDJSON requests from stdin, writes
//!     NDJSON responses to stdout. Logs to stderr. Same as P3.1.
//!   - **HTTP** (`--http <addr>`): runs an axum server exposing
//!     `POST /mcp` (one request per body) and `GET /health`. For
//!     Cursor / Continue / Cline / IDE plugins that target HTTP
//!     transport, plus docker-compose / CI scenarios where stdio
//!     plumbing is awkward.
//!
//! See `docs/integrations/mcp-clients/index.md` for client config
//! snippets (Claude Desktop, Cursor, Continue) and the per-transport
//! protocol notes.

use std::io::{BufRead, Write};
use std::net::SocketAddr;
use std::sync::Arc;

use sbo3l_mcp::{dispatch_to_response, http_transport, jsonrpc, ServerContext};

fn main() {
    // tracing → stderr only; stdout is the JSON-RPC channel in stdio mode.
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let ctx = Arc::new(ServerContext::new());

    match parse_args() {
        Mode::Stdio => run_stdio(ctx),
        Mode::Http(addr) => run_http(ctx, addr),
        Mode::Help => print_help(),
    }
}

enum Mode {
    Stdio,
    Http(SocketAddr),
    Help,
}

fn parse_args() -> Mode {
    // Tiny ad-hoc parser — no clap dep for one flag. Only the first arg
    // is examined; any further args after `--http <addr>` would be
    // unexpected and surface as an error from the OS process layer or
    // get silently ignored — fine for a server with a one-shot flag set.
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(first) = args.first() else {
        return Mode::Stdio;
    };
    match first.as_str() {
        "--http" => {
            let raw = args.get(1).cloned().unwrap_or_else(|| {
                eprintln!("error: --http requires an address (e.g. 127.0.0.1:8731)");
                std::process::exit(2);
            });
            let addr: SocketAddr = raw.parse().unwrap_or_else(|e| {
                eprintln!("error: invalid --http address '{raw}': {e}");
                std::process::exit(2);
            });
            Mode::Http(addr)
        }
        "-h" | "--help" => Mode::Help,
        other => {
            eprintln!("error: unknown argument '{other}' (try --help)");
            std::process::exit(2);
        }
    }
}

fn print_help() {
    println!(
        "sbo3l-mcp — SBO3L MCP JSON-RPC server\n\n\
         USAGE:\n  \
         sbo3l-mcp                    # stdio (default; for Claude Desktop, etc.)\n  \
         sbo3l-mcp --http <addr>      # HTTP (e.g. --http 127.0.0.1:8731)\n  \
         sbo3l-mcp --help\n\n\
         See docs/integrations/mcp-clients/index.md for client wiring."
    );
}

fn run_stdio(ctx: Arc<ServerContext>) {
    tracing::info!(
        "sbo3l-mcp: stdio JSON-RPC server ready (protocol: NDJSON, tools: 6, see docs/cli/mcp.md)"
    );
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

    tracing::info!("sbo3l-mcp: stdin closed, exiting");
}

fn run_http(ctx: Arc<ServerContext>, addr: SocketAddr) {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime");

    runtime.block_on(async move {
        let app = http_transport::router(ctx);
        let listener = match tokio::net::TcpListener::bind(addr).await {
            Ok(l) => l,
            Err(e) => {
                tracing::error!("bind {addr}: {e}");
                std::process::exit(1);
            }
        };
        tracing::info!("sbo3l-mcp: HTTP server ready on http://{addr} (POST /mcp, GET /health)");
        if let Err(e) = axum::serve(listener, app).await {
            tracing::error!("axum serve: {e}");
            std::process::exit(1);
        }
    });
}
