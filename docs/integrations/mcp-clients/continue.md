# Continue — SBO3L MCP

Continue (>= 0.10) targets MCP over **HTTP**. Stdio is on the roadmap but not yet stable across all builds; use HTTP for reproducible wiring.

## 1. Start the server

```bash
sbo3l-mcp --http 127.0.0.1:8731
```

`GET http://127.0.0.1:8731/health` should return `{"status":"ok","tools":6,"transport":"http"}`.

## 2. Wire into Continue

Edit `~/.continue/config.json` (or `config.yaml` in newer builds):

```json
{
  "experimental": {
    "modelContextProtocolServers": [
      {
        "transport": {
          "type": "http",
          "url": "http://127.0.0.1:8731/mcp"
        }
      }
    ]
  }
}
```

Restart Continue. The 6 SBO3L tools (see [index.md](index.md)) appear under the MCP tools section.

## 3. Verifying

In a Continue chat:

> Look up the audit chain prefix through audit_event_id `evt-...` using the SBO3L MCP server.

Continue invokes `sbo3l.audit_lookup` and renders the response inline.

## 4. Auth + multi-tenant

The HTTP transport is **loopback-by-default and unauthenticated**. If you bind it to a non-loopback interface (e.g. `--http 0.0.0.0:8731` for a shared dev box), front it with your own reverse proxy + auth layer. The MCP wire format itself does not include authentication — that is a transport concern.

## Tracing

`RUST_LOG=debug sbo3l-mcp --http …` logs every JSON-RPC request + response shape to stderr. Pipe to a file:

```bash
RUST_LOG=debug sbo3l-mcp --http 127.0.0.1:8731 2> /tmp/sbo3l-mcp.log
```
