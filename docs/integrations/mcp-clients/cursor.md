# Cursor — SBO3L MCP

Cursor (>= 0.42) supports MCP over both **stdio** (subprocess) and **HTTP** (remote URL). Edit `~/.cursor/mcp.json` (or use Cursor → Settings → MCP).

## Stdio (recommended for local dev)

```json
{
  "mcpServers": {
    "sbo3l": {
      "command": "sbo3l-mcp",
      "args": []
    }
  }
}
```

## HTTP (remote / cross-process)

Start the server in HTTP mode:

```bash
sbo3l-mcp --http 127.0.0.1:8731
```

…then point Cursor at it:

```json
{
  "mcpServers": {
    "sbo3l": {
      "url": "http://127.0.0.1:8731/mcp"
    }
  }
}
```

## Verifying

Open Cursor's MCP tools panel. The 6 SBO3L tools (see [index.md](index.md)) should be listed. Try:

> @sbo3l decide on this APRP: `{...}`

Cursor invokes `sbo3l.decide` and the response surfaces inline.

## Tracing

Stdio mode: stderr is captured by Cursor's MCP log pane.
HTTP mode: tracing goes to the controlling terminal — `RUST_LOG=debug sbo3l-mcp --http …` shows every request.
