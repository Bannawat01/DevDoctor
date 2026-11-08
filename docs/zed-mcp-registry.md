# Zed MCP Registry metadata

Status: `READY TO PUBLISH`; `devdoctor-core` and `devdoctor-mcp` are published on crates.io. Draft official metadata is in `docs/mcp-server.json`.

DevDoctor is a local stdio MCP server. A registry entry should launch the
versioned `devdoctor-mcp` release binary for the user's platform and pass no
project content or credentials to a remote service.

## Metadata

| Field | Value |
|---|---|
| Server name | `devdoctor` |
| Repository | `https://github.com/Bannawat01/DevDoctor` |
| Transport | Local stdio |
| Tools | `diagnose_project`, `check_toolchain`, `check_environment`, `check_ports`, `explain_finding` |
| Permissions | Read-only local diagnostics |
| Telemetry | None |
| Supported platform | Windows x86_64 (stable release artifact) |
| Experimental platform | Linux x86_64 (CI-verified, packaging may vary) |

The registry distribution is the `devdoctor-mcp` crates.io package. GitHub ZIPs
remain the native/manual Windows release path.

The server must be distributed separately from a Zed extension when the
extension API requires downloading a binary. Do not embed credentials or a
machine-specific path in registry metadata.

## Manual Zed configuration

For current Zed versions, configure the local server under **Settings → AI →
MCP Servers → Add Local Server**. Use an absolute path to the downloaded
binary:

```json
{
  "context_servers": {
    "devdoctor": {
      "command": "C:/path/to/devdoctor-mcp.exe",
      "args": []
    }
  }
}
```

Verify the release checksum before use. After adding the server, confirm that
all five tools appear in the Agent panel and call `diagnose_project` against a
local fixture. The server must not start containers, install dependencies,
execute repository scripts, or modify environment state.

## Publishing checklist

- [ ] Validate the registry schema against the current Zed registry repository.
- [ ] Verify the release asset and SHA-256 checksum for each advertised platform.
- [ ] Install from Zed and confirm tool discovery.
- [ ] Call all five tools from the Zed Agent UI.
- [ ] Record `ZED_AGENT_VERIFIED` separately from protocol tests.
- [x] Prepare `docs/mcp-server.json` using the current official schema.
- [x] Publish `devdoctor-core` and `devdoctor-mcp` to crates.io.
- [ ] Validate and publish metadata with `mcp-publisher`.
