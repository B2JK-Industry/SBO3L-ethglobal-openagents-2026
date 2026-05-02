# Claude Desktop — SBO3L MCP

Claude Desktop spawns MCP servers as subprocesses and talks to them over **stdio**. Add this snippet to your `claude_desktop_config.json`:

**macOS:** `~/Library/Application Support/Claude/claude_desktop_config.json`
**Windows:** `%APPDATA%\Claude\claude_desktop_config.json`
**Linux:** `~/.config/Claude/claude_desktop_config.json`

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

If `sbo3l-mcp` is not on `$PATH`, use the absolute path:

```json
{
  "mcpServers": {
    "sbo3l": {
      "command": "/Users/you/.cargo/bin/sbo3l-mcp",
      "args": []
    }
  }
}
```

Restart Claude Desktop. The 6 SBO3L tools (see [index.md](index.md)) appear in the tools popover.

## Verifying

In Claude Desktop, ask:

> Use `sbo3l.validate_aprp` to validate this APRP body: `{...}`

Claude should round-trip the request through the MCP server and surface the daemon's structured ok / aprp_invalid response.

## Logging

`sbo3l-mcp` logs to **stderr** (stdout is the protocol channel). Claude Desktop captures stderr in its log file:
- macOS: `~/Library/Logs/Claude/mcp.log`
- Windows: `%APPDATA%\Claude\Logs\mcp.log`

Set `RUST_LOG=debug` for verbose tracing:

```json
{
  "mcpServers": {
    "sbo3l": {
      "command": "sbo3l-mcp",
      "args": [],
      "env": { "RUST_LOG": "debug" }
    }
  }
}
```
