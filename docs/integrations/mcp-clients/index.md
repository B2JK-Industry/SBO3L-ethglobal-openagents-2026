# SBO3L MCP — client wiring

The `sbo3l-mcp` binary exposes SBO3L's policy + capsule + audit primitives as JSON-RPC 2.0 tools over **stdio** (default) or **HTTP** (`--http <addr>`).

Same tool catalogue on both transports; the only difference is wire framing.

## Tool catalogue (6 tools)

| Tool | Wraps | Purpose |
|---|---|---|
| `sbo3l.validate_aprp` | `sbo3l_core::schema::validate_aprp` | Validate APRP against `schemas/aprp_v1.json` |
| `sbo3l.decide` | `sbo3l_server::router` | Drive the policy engine without executing anything |
| `sbo3l.run_guarded_execution` | `sbo3l_server` + executor mocks | Full submit + execute path (mock executor) |
| `sbo3l.verify_capsule` | `sbo3l_core::passport::verify_capsule` | Verify a `PassportCapsule` offline |
| `sbo3l.explain_denial` | `verify_capsule` + projection | Return a structured explanation of a deny capsule |
| `sbo3l.audit_lookup` | `Storage::audit_chain_prefix_through` | Walk the hash-chained audit log |

Plus the meta method `tools/list` that returns the catalogue with input/output schemas.

## Per-client config

- [Claude Desktop](claude-desktop.md) — stdio
- [Cursor](cursor.md) — stdio + HTTP
- [Continue](continue.md) — HTTP

## When to use which transport

- **stdio** is the right default for IDE plugins and chat clients (Claude Desktop, Cursor, Continue, Cline) that spawn the server as a subprocess. Lowest overhead, no port management, no auth surface.
- **HTTP** unblocks: docker-compose topologies; CI smoke jobs; cross-process scenarios (e.g. one MCP server, many agent processes); IDE plugins that target HTTP transport explicitly.

## Server-side commands

```bash
# stdio (default — Claude Desktop / Cursor / Continue / Cline use this)
sbo3l-mcp

# HTTP — POST /mcp accepts one JSON-RPC envelope per body, GET /health returns {status, tools, transport}
sbo3l-mcp --http 127.0.0.1:8731

# tracing → stderr; stdout is the protocol channel in stdio mode.
RUST_LOG=debug sbo3l-mcp
```

## Out of scope (deliberate)

- **SSE / Streamable HTTP** — the MCP spec is in flux on this. Build on top of the HTTP transport when the spec settles.
- **Authentication on the HTTP transport** — server is loopback-by-default. If you bind it to a non-loopback interface, front it with your own reverse proxy + auth layer.
- **HTTP/2 / TLS** — same reasoning. Terminate TLS at your reverse proxy.
